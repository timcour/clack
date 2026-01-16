use super::client::SlackClient;
use crate::models::message::{Message, MessagesResponse};
use anyhow::Result;

pub async fn list_messages(
    client: &SlackClient,
    channel: &str,
    limit: u32,
    latest: Option<String>,
    oldest: Option<String>,
) -> Result<Vec<Message>> {
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

    Ok(response.messages)
}

pub async fn get_thread(
    client: &SlackClient,
    channel: &str,
    thread_ts: &str,
) -> Result<Vec<Message>> {
    let query = vec![
        ("channel", channel.to_string()),
        ("ts", thread_ts.to_string()),
    ];

    let response: MessagesResponse = client.get("conversations.replies", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (mockito::ServerGuard, SlackClient) {
        let server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let client = SlackClient::with_base_url(&server.url(), false).await.unwrap();
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

        let messages = list_messages(&client, "C123", 10, None, None)
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

        let result = list_messages(&client, "C999", 10, None, None).await;
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

        let messages = get_thread(&client, "C123", "1234567890.123456")
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

        let result = get_thread(&client, "C123", "9999999999.999999").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("message_not_found"));
    }
}
