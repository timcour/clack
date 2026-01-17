use super::client::SlackClient;
use crate::models::file::{File, FileInfoResponse, FilesListResponse};
use anyhow::Result;

pub async fn list_files(
    client: &SlackClient,
    limit: u32,
    user: Option<&str>,
    channel: Option<&str>,
) -> Result<Vec<File>> {
    let mut query = vec![("count", limit.to_string())];

    if let Some(u) = user {
        query.push(("user", u.to_string()));
    }

    if let Some(ch) = channel {
        query.push(("channel", ch.to_string()));
    }

    let response: FilesListResponse = client.get("files.list", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.files)
}

pub async fn get_file(client: &SlackClient, file_id: &str) -> Result<File> {
    let query = vec![("file", file_id.to_string())];
    let response: FileInfoResponse = client.get("files.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.file)
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
    async fn test_list_files_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/files.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("count".into(), "10".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "files": [{"id": "F123", "name": "test.txt", "title": "Test", "mimetype": "text/plain", "filetype": "txt", "pretty_type": "Text", "user": "U123", "size": 1024, "created": 1234567890, "timestamp": 1234567890}]}"#)
            .create_async()
            .await;

        let files = list_files(&client, 10, None, None).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "F123");
    }

    #[tokio::test]
    async fn test_get_file_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/files.info")
            .match_query(mockito::Matcher::UrlEncoded("file".into(), "F123".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "file": {"id": "F123", "name": "test.txt", "title": "Test", "mimetype": "text/plain", "filetype": "txt", "pretty_type": "Text", "user": "U123", "size": 1024, "created": 1234567890, "timestamp": 1234567890}}"#)
            .create_async()
            .await;

        let file = get_file(&client, "F123").await.unwrap();
        assert_eq!(file.id, "F123");
        assert_eq!(file.name, "test.txt");
    }
}
