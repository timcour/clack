use super::client::SlackClient;
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ReactionResponse {
    ok: bool,
    error: Option<String>,
}

pub async fn add_reaction(
    client: &SlackClient,
    channel: &str,
    timestamp: &str,
    name: &str,
) -> Result<()> {
    let query = vec![
        ("channel", channel.to_string()),
        ("timestamp", timestamp.to_string()),
        ("name", name.to_string()),
    ];
    let response: ReactionResponse = client.get("reactions.add", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(())
}

pub async fn remove_reaction(
    client: &SlackClient,
    channel: &str,
    timestamp: &str,
    name: &str,
) -> Result<()> {
    let query = vec![
        ("channel", channel.to_string()),
        ("timestamp", timestamp.to_string()),
        ("name", name.to_string()),
    ];
    let response: ReactionResponse = client.get("reactions.remove", &query).await?;

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
        let mut client = SlackClient::with_base_url(&server.url(), false).await.unwrap();

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
    async fn test_add_reaction_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/reactions.add")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("channel".into(), "C123".into()),
                mockito::Matcher::UrlEncoded("timestamp".into(), "1234567890.123456".into()),
                mockito::Matcher::UrlEncoded("name".into(), "thumbsup".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true}"#)
            .create_async()
            .await;

        add_reaction(&client, "C123", "1234567890.123456", "thumbsup")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_remove_reaction_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/reactions.remove")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("channel".into(), "C123".into()),
                mockito::Matcher::UrlEncoded("timestamp".into(), "1234567890.123456".into()),
                mockito::Matcher::UrlEncoded("name".into(), "thumbsup".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true}"#)
            .create_async()
            .await;

        remove_reaction(&client, "C123", "1234567890.123456", "thumbsup")
            .await
            .unwrap();
    }
}
