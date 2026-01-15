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

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (mockito::ServerGuard, SlackClient) {
        let server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let client = SlackClient::with_base_url(&server.url()).unwrap();
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
}
