use super::client::SlackClient;
use crate::cache;
use crate::models::user::{User, UserInfoResponse, UserProfile, UserProfileResponse, UsersListResponse};
use anyhow::Result;

pub async fn list_users(
    client: &SlackClient,
    limit: u32,
    include_deleted: bool,
) -> Result<Vec<User>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Always fetch from API for list operations
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

    // Try cache first (unless refresh requested)
    if !client.refresh_cache() {
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
    } else if client.verbose() {
        eprintln!("[CACHE] User {} - SKIP (refresh requested)", user_id);
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

pub async fn get_profile(client: &SlackClient, user_id: Option<&str>) -> Result<UserProfile> {
    // Build query - if user_id is None, Slack API will return the authenticated user's profile
    let query = if let Some(uid) = user_id {
        vec![("user", uid.to_string())]
    } else {
        vec![]
    };

    let response: UserProfileResponse = client.get("users.profile.get", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    async fn setup() -> (mockito::ServerGuard, SlackClient) {
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let workspace_id = format!("T{}", test_id);

        let mut server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let mut client = SlackClient::with_base_url(&server.url(), false, false, false).await.unwrap();

        // Mock auth.test for workspace initialization with unique workspace ID
        let auth_body = format!(
            r#"{{"ok": true, "url": "https://test.slack.com/", "team_id": "{}", "team": "Test Team", "user": "testuser", "user_id": "U123"}}"#,
            workspace_id
        );
        let _auth_mock = server
            .mock("GET", "/auth.test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(auth_body)
            .create();

        client.init_workspace().await.unwrap();

        (server, client)
    }

    #[tokio::test]
    async fn test_list_users_success() {
        let (mut server, client) = setup().await;

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

        // Clear any potential cache pollution for this workspace
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = cache::get_connection(pool).await {
                let workspace_id = client.workspace_id().unwrap();
                let _ = cache::operations::clear_workspace_cache(&mut conn, workspace_id, false);
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

    #[tokio::test]
    async fn test_get_user_with_refresh_cache() {
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let workspace_id = format!("T{}", test_id);

        let mut server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");

        // Create client with refresh_cache=true
        let mut client = SlackClient::with_base_url(&server.url(), false, false, true).await.unwrap();

        // Mock auth.test
        let auth_body = format!(
            r#"{{"ok": true, "url": "https://test.slack.com/", "team_id": "{}", "team": "Test Team", "user": "testuser", "user_id": "U123"}}"#,
            workspace_id
        );
        let _auth_mock = server
            .mock("GET", "/auth.test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(auth_body)
            .create();

        client.init_workspace().await.unwrap();

        // Pre-populate cache with stale data
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = cache::get_connection(pool).await {
                let stale_user = User {
                    id: "UREFRESH".to_string(),
                    name: "staleuser".to_string(),
                    real_name: Some("Stale User".to_string()),
                    deleted: false,
                    is_bot: false,
                    is_admin: None,
                    is_owner: None,
                    tz: None,
                    profile: crate::models::user::UserProfile {
                        email: Some("stale@example.com".to_string()),
                        display_name: Some("staleuser".to_string()),
                        status_emoji: None,
                        status_text: None,
                        image_72: None,
                    },
                };
                let _ = cache::operations::upsert_user(&mut conn, &workspace_id, &stale_user, false);
            }
        }

        // Mock API response with fresh data
        let _mock = server
            .mock("GET", "/users.info?user=UREFRESH")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
            "ok": true,
            "user": {
                "id": "UREFRESH",
                "name": "freshuser",
                "real_name": "Fresh User",
                "deleted": false,
                "is_bot": false,
                "profile": {
                    "email": "fresh@example.com",
                    "display_name": "freshuser"
                }
            }
        }"#,
            )
            .create_async()
            .await;

        // Call get_user - should skip cache and get fresh data from API
        let user = get_user(&client, "UREFRESH").await.unwrap();
        assert_eq!(user.name, "freshuser", "Should get fresh data from API, not stale cache");
        assert_eq!(user.profile.email, Some("fresh@example.com".to_string()));
    }
}
