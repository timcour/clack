use super::client::SlackClient;
use crate::models::pin::{PinItem, PinResponse, PinsListResponse};
use anyhow::Result;

pub async fn list_pins(client: &SlackClient, channel: &str) -> Result<Vec<PinItem>> {
    let query = vec![("channel", channel.to_string())];
    let response: PinsListResponse = client.get("pins.list", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.items)
}

pub async fn add_pin(client: &SlackClient, channel: &str, timestamp: &str) -> Result<()> {
    let query = vec![
        ("channel", channel.to_string()),
        ("timestamp", timestamp.to_string()),
    ];
    let response: PinResponse = client.get("pins.add", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(())
}

pub async fn remove_pin(client: &SlackClient, channel: &str, timestamp: &str) -> Result<()> {
    let query = vec![
        ("channel", channel.to_string()),
        ("timestamp", timestamp.to_string()),
    ];
    let response: PinResponse = client.get("pins.remove", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(())
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
        let mut client = SlackClient::with_base_url(&server.url(), false, false).await.unwrap();

        // Mock auth.test for workspace initialization
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
    async fn test_list_pins_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/pins.list")
            .match_query(mockito::Matcher::UrlEncoded("channel".into(), "C123".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "items": [{"channel": "C123", "created": 1234567890, "created_by": "U123", "type": "message"}]}"#)
            .create_async()
            .await;

        let pins = list_pins(&client, "C123").await.unwrap();
        assert_eq!(pins.len(), 1);
    }

    #[tokio::test]
    async fn test_add_pin_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/pins.add")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("channel".into(), "C123".into()),
                mockito::Matcher::UrlEncoded("timestamp".into(), "1234567890.123456".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true}"#)
            .create_async()
            .await;

        add_pin(&client, "C123", "1234567890.123456").await.unwrap();
    }

    #[tokio::test]
    async fn test_remove_pin_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/pins.remove")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("channel".into(), "C123".into()),
                mockito::Matcher::UrlEncoded("timestamp".into(), "1234567890.123456".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true}"#)
            .create_async()
            .await;

        remove_pin(&client, "C123", "1234567890.123456").await.unwrap();
    }
}
