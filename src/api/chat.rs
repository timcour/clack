use super::client::SlackClient;
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ChatPostResponse {
    ok: bool,
    channel: Option<String>,
    ts: Option<String>,
    message: Option<PostedMessage>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostedMessage {
    text: String,
    user: String,
    ts: String,
}

pub async fn post_message(
    client: &SlackClient,
    channel: &str,
    text: &str,
    thread_ts: Option<&str>,
) -> Result<String> {
    let mut query = vec![
        ("channel", channel.to_string()),
        ("text", text.to_string()),
    ];

    if let Some(ts) = thread_ts {
        query.push(("thread_ts", ts.to_string()));
    }

    let response: ChatPostResponse = client.get("chat.postMessage", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    // Return the timestamp of the posted message
    Ok(response.ts.unwrap_or_default())
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
    async fn test_post_message_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/chat.postMessage")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("channel".into(), "C123".into()),
                mockito::Matcher::UrlEncoded("text".into(), "Hello".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "channel": "C123", "ts": "1234567890.123456", "message": {"text": "Hello", "user": "U123", "ts": "1234567890.123456"}}"#)
            .create_async()
            .await;

        let ts = post_message(&client, "C123", "Hello", None).await.unwrap();
        assert_eq!(ts, "1234567890.123456");
    }

    #[tokio::test]
    async fn test_post_message_with_thread_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/chat.postMessage")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("channel".into(), "C123".into()),
                mockito::Matcher::UrlEncoded("text".into(), "Reply".into()),
                mockito::Matcher::UrlEncoded("thread_ts".into(), "1234567890.123456".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "channel": "C123", "ts": "1234567891.123456", "message": {"text": "Reply", "user": "U123", "ts": "1234567891.123456"}}"#)
            .create_async()
            .await;

        let ts = post_message(&client, "C123", "Reply", Some("1234567890.123456"))
            .await
            .unwrap();
        assert_eq!(ts, "1234567891.123456");
    }
}
