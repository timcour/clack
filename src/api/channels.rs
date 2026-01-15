use super::client::SlackClient;
use crate::models::channel::{Channel, ChannelInfoResponse};
use anyhow::Result;

pub async fn get_channel(client: &SlackClient, channel_id: &str) -> Result<Channel> {
    let query = vec![("channel", channel_id.to_string())];
    let response: ChannelInfoResponse = client.get("conversations.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.channel)
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
    async fn test_get_channel_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.info?channel=C123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channel": {
                    "id": "C123",
                    "name": "general",
                    "is_channel": true,
                    "is_private": false,
                    "topic": {
                        "value": "Company-wide announcements"
                    },
                    "purpose": {
                        "value": "This channel is for team-wide communication"
                    },
                    "num_members": 42
                }
            }"#,
            )
            .create_async()
            .await;

        let channel = get_channel(&client, "C123").await.unwrap();
        assert_eq!(channel.id, "C123");
        assert_eq!(channel.name, "general");
        assert_eq!(channel.num_members, Some(42));
    }

    #[tokio::test]
    async fn test_get_channel_error_response() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.info?channel=C999")
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

        let result = get_channel(&client, "C999").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("channel_not_found"));
    }
}
