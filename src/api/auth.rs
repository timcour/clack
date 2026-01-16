use super::client::SlackClient;
use crate::models::workspace::AuthTestResponse;
use anyhow::Result;

pub async fn test_auth(client: &SlackClient) -> Result<AuthTestResponse> {
    let query = vec![];
    let response: AuthTestResponse = client.get("auth.test", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response)
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
    async fn test_auth_test_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/auth.test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "url": "https://test-workspace.slack.com/",
                "team": "Test Workspace",
                "user": "testuser",
                "team_id": "T12345678",
                "user_id": "U12345678"
            }"#,
            )
            .create_async()
            .await;

        let response = test_auth(&client).await.unwrap();
        assert_eq!(response.team_id, "T12345678");
        assert_eq!(response.team, "Test Workspace");
    }

    #[tokio::test]
    async fn test_auth_test_error_response() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/auth.test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": false,
                "error": "invalid_auth"
            }"#,
            )
            .create_async()
            .await;

        let result = test_auth(&client).await;
        assert!(result.is_err());
        // The error message is enriched by the client with helpful context
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Slack API error") || error_msg.contains("Invalid authentication"));
    }
}
