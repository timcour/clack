use super::client::SlackClient;
use crate::models::message::{Message, MessagesResponse};
use anyhow::Result;

pub async fn list_messages(
    client: &SlackClient,
    channel: &str,
    limit: u32,
    latest: Option<String>,
    oldest: Option<String>,
    use_cache: bool,
) -> Result<Vec<Message>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first if use_cache is true
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            match crate::cache::get_connection(pool).await {
                Ok(mut conn) => {
                    match crate::cache::operations::get_messages(
                        &mut conn,
                        workspace_id,
                        channel,
                        client.verbose(),
                    ) {
                        Ok(Some(cached_messages)) => {
                            let mut messages = cached_messages;
                            // Apply limit
                            messages.truncate(limit as usize);
                            return Ok(messages);
                        }
                        Ok(None) => {
                            // Cache miss or stale, continue to API
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
    }

    // Fetch from API
    let mut query = vec![
        ("channel", channel.to_string()),
        ("limit", limit.to_string()),
    ];

    if let Some(latest) = latest {
        query.push(("latest", latest));
    }
    if let Some(oldest) = oldest {
        query.push(("oldest", oldest));
    }

    let response: MessagesResponse = client.get("conversations.history", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let messages = response.messages;

    // Write through to cache if use_cache is true (best effort, don't fail on cache errors)
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::upsert_messages(
                    &mut conn,
                    workspace_id,
                    channel,
                    &messages,
                    client.verbose(),
                );
            }
        }
    }

    Ok(messages)
}

pub async fn get_thread(
    client: &SlackClient,
    channel: &str,
    thread_ts: &str,
    use_cache: bool,
) -> Result<Vec<Message>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first if use_cache is true
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            match crate::cache::get_connection(pool).await {
                Ok(mut conn) => {
                    // For threads, we get all messages from this channel and filter by thread_ts
                    match crate::cache::operations::get_messages(
                        &mut conn,
                        workspace_id,
                        channel,
                        client.verbose(),
                    ) {
                        Ok(Some(cached_messages)) => {
                            // Filter for messages in this thread (where thread_ts matches or ts matches for root)
                            let thread_messages: Vec<Message> = cached_messages
                                .into_iter()
                                .filter(|m| {
                                    m.ts == thread_ts
                                        || m.thread_ts.as_ref().map(|t| t == thread_ts).unwrap_or(false)
                                })
                                .collect();

                            if !thread_messages.is_empty() {
                                return Ok(thread_messages);
                            }
                            // If no messages found in cache, continue to API
                        }
                        Ok(None) => {
                            // Cache miss or stale, continue to API
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
    }

    // Fetch from API
    let query = vec![
        ("channel", channel.to_string()),
        ("ts", thread_ts.to_string()),
    ];

    let response: MessagesResponse = client.get("conversations.replies", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let messages = response.messages;

    // Write through to cache if use_cache is true (best effort, don't fail on cache errors)
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::upsert_messages(
                    &mut conn,
                    workspace_id,
                    channel,
                    &messages,
                    client.verbose(),
                );
            }
        }
    }

    Ok(messages)
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
    async fn test_list_messages_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.history?channel=C123&limit=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "messages": [{
                    "ts": "1234567890.123456",
                    "user": "U123",
                    "text": "Hello world"
                }]
            }"#,
            )
            .create_async()
            .await;

        let messages = list_messages(&client, "C123", 10, None, None, false)
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].ts, "1234567890.123456");
        assert_eq!(messages[0].text, "Hello world");
    }

    #[tokio::test]
    async fn test_list_messages_with_timestamps() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock(
                "GET",
                "/conversations.history?channel=C123&limit=10&latest=1234567900&oldest=1234567800",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "messages": []
            }"#,
            )
            .create_async()
            .await;

        let _messages = list_messages(
            &client,
            "C123",
            10,
            Some("1234567900".to_string()),
            Some("1234567800".to_string()),
            false,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_list_messages_error_response() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.history?channel=C999&limit=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": false,
                "error": "channel_not_found"
            }"#,
            )
            .create_async()
            .await;

        let result = list_messages(&client, "C999", 10, None, None, false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("channel_not_found"));
    }

    #[tokio::test]
    async fn test_get_thread_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.replies?channel=C123&ts=1234567890.123456")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "messages": [
                    {
                        "ts": "1234567890.123456",
                        "user": "U123",
                        "text": "Root message",
                        "thread_ts": "1234567890.123456"
                    },
                    {
                        "ts": "1234567891.123456",
                        "user": "U456",
                        "text": "Reply 1",
                        "thread_ts": "1234567890.123456"
                    },
                    {
                        "ts": "1234567892.123456",
                        "user": "U789",
                        "text": "Reply 2",
                        "thread_ts": "1234567890.123456"
                    }
                ]
            }"#,
            )
            .create_async()
            .await;

        let messages = get_thread(&client, "C123", "1234567890.123456", false)
            .await
            .unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text, "Root message");
        assert_eq!(messages[1].text, "Reply 1");
        assert_eq!(messages[2].text, "Reply 2");
    }

    #[tokio::test]
    async fn test_get_thread_not_found() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.replies?channel=C123&ts=9999999999.999999")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": false,
                "error": "message_not_found"
            }"#,
            )
            .create_async()
            .await;

        let result = get_thread(&client, "C123", "9999999999.999999", false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("message_not_found"));
    }
}
