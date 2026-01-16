use super::client::SlackClient;
use crate::cache;
use crate::models::user::{User, UserInfoResponse, UsersListResponse};
use anyhow::Result;

pub async fn list_users(
    client: &SlackClient,
    limit: u32,
    include_deleted: bool,
) -> Result<Vec<User>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first (if pool available)
    if let Some(pool) = client.cache_pool() {
        match cache::get_connection(pool).await {
            Ok(mut conn) => {
                match cache::operations::get_users(&mut conn, workspace_id, client.verbose()) {
                    Ok(Some(cached_users)) => {
                        let mut users = cached_users;
                        if !include_deleted {
                            users.retain(|u| !u.deleted);
                        }
                        return Ok(users);
                    }
                    Ok(None) => {
                        // Cache miss or stale, continue to API
                    }
                    Err(e) => {
                        if client.verbose() {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                        // Fall through to API
                    }
                }
            }
            Err(e) => {
                if client.verbose() {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }

    // Cache miss or error - fetch from API
    let query = vec![("limit", limit.to_string())];
    let response: UsersListResponse = client.get("users.list", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let users = response.members;

    // Write through to cache (best effort, don't fail on cache errors)
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = cache::get_connection(pool).await {
            let _ = cache::operations::upsert_users(&mut conn, workspace_id, &users, client.verbose());
        }
    }

    let mut result = users;
    if !include_deleted {
        result.retain(|u| !u.deleted);
    }

    Ok(result)
}

pub async fn get_user(client: &SlackClient, user_id: &str) -> Result<User> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first
    if let Some(pool) = client.cache_pool() {
        match cache::get_connection(pool).await {
            Ok(mut conn) => {
                match cache::operations::get_user(&mut conn, workspace_id, user_id, client.verbose()) {
                    Ok(Some(cached_user)) => {
                        return Ok(cached_user);
                    }
                    Ok(None) => {
                        // Cache miss, continue to API
                    }
                    Err(e) => {
                        if client.verbose() {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                if client.verbose() {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }

    // Fetch from API
    let query = vec![("user", user_id.to_string())];
    let response: UserInfoResponse = client.get("users.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let user = response.user;

    // Write through to cache
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = cache::get_connection(pool).await {
            let _ = cache::operations::upsert_user(&mut conn, workspace_id, &user, client.verbose());
        }
    }

    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (mockito::ServerGuard, SlackClient) {
        let mut server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let mut client = SlackClient::with_base_url(&server.url(), false).await.unwrap();

        // Mock auth.test for workspace initialization
        let _auth_mock = server
            .mock("GET", "/auth.test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "url": "https://test.slack.com/", "team_id": "T123", "team": "Test Team", "user": "testuser", "user_id": "U123"}"#)
            .create();

        client.init_workspace().await.unwrap();

        (server, client)
    }

    #[tokio::test]
    async fn test_list_users_success() {
        let (mut server, client) = setup().await;

        // Clear cache to ensure clean test state
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::clear_workspace_cache(&mut conn, "T123", false);
            }
        }

        let _mock = server
            .mock("GET", "/users.list?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "members": [{
                    "id": "U123",
                    "name": "testuser",
                    "real_name": "Test User",
                    "deleted": false,
                    "is_bot": false,
                    "profile": {
                        "email": "test@example.com",
                        "display_name": "testuser"
                    }
                }]
            }"#,
            )
            .create_async()
            .await;

        let users = list_users(&client, 200, false).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].id, "U123");
        assert_eq!(users[0].name, "testuser");
    }

    #[tokio::test]
    async fn test_list_users_filters_deleted() {
        let (mut server, client) = setup().await;

        // Clear cache to ensure clean test state
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::clear_workspace_cache(&mut conn, "T123", false);
            }
        }

        let _mock = server
            .mock("GET", "/users.list?limit=200")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "members": [
                    {
                        "id": "U123",
                        "name": "activeuser",
                        "real_name": "Active User",
                        "deleted": false,
                        "is_bot": false,
                        "profile": {}
                    },
                    {
                        "id": "U456",
                        "name": "deleteduser",
                        "real_name": "Deleted User",
                        "deleted": true,
                        "is_bot": false,
                        "profile": {}
                    }
                ]
            }"#,
            )
            .create_async()
            .await;

        // Without include_deleted, should only get active user
        let users = list_users(&client, 200, false).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].id, "U123");

        // With include_deleted, should get both
        let users = list_users(&client, 200, true).await.unwrap();
        assert_eq!(users.len(), 2);
    }

    #[tokio::test]
    async fn test_list_users_with_limit() {
        let (mut server, client) = setup().await;

        // Clear cache to ensure clean test state
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::clear_workspace_cache(&mut conn, "T123", false);
            }
        }

        let _mock = server
            .mock("GET", "/users.list?limit=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "members": []
            }"#,
            )
            .create_async()
            .await;

        let _users = list_users(&client, 10, false).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_user_success() {
        let (mut server, client) = setup().await;

        // Clear cache to ensure clean test state
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::clear_workspace_cache(&mut conn, "T123", false);
            }
        }

        let _mock = server
            .mock("GET", "/users.info?user=U123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "user": {
                    "id": "U123",
                    "name": "testuser",
                    "real_name": "Test User",
                    "deleted": false,
                    "is_bot": false,
                    "profile": {
                        "email": "test@example.com"
                    }
                }
            }"#,
            )
            .create_async()
            .await;

        let user = get_user(&client, "U123").await.unwrap();
        assert_eq!(user.id, "U123");
        assert_eq!(user.name, "testuser");
    }

    #[tokio::test]
    async fn test_get_user_error_response() {
        let (mut server, client) = setup().await;

        // Clear cache to ensure clean test state
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::clear_workspace_cache(&mut conn, "T123", false);
            }
        }

        let _mock = server
            .mock("GET", "/users.info?user=U999")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": false,
                "error": "user_not_found"
            }"#,
            )
            .create_async()
            .await;

        let result = get_user(&client, "U999").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("user_not_found"));
    }
}
