use super::client::SlackClient;
use crate::models::search::{SearchAllResponse, SearchFilesResponse, SearchMessagesResponse};
use anyhow::Result;

pub async fn search_messages(
    client: &SlackClient,
    query: &str,
    count: Option<u32>,
) -> Result<SearchMessagesResponse> {
    let mut params = vec![("query", query.to_string())];

    if let Some(c) = count {
        params.push(("count", c.to_string()));
    }

    let response: SearchMessagesResponse = client.get("search.messages", &params).await?;

    if !response.ok {
        anyhow::bail!(
            "Slack API error: {}",
            response.error.unwrap_or_default()
        );
    }

    Ok(response)
}

pub async fn search_files(
    client: &SlackClient,
    query: &str,
    count: Option<u32>,
) -> Result<SearchFilesResponse> {
    let mut params = vec![("query", query.to_string())];

    if let Some(c) = count {
        params.push(("count", c.to_string()));
    }

    let response: SearchFilesResponse = client.get("search.files", &params).await?;

    if !response.ok {
        anyhow::bail!(
            "Slack API error: {}",
            response.error.unwrap_or_default()
        );
    }

    Ok(response)
}

pub async fn search_all(
    client: &SlackClient,
    query: &str,
    count: Option<u32>,
) -> Result<SearchAllResponse> {
    let mut params = vec![("query", query.to_string())];

    if let Some(c) = count {
        params.push(("count", c.to_string()));
    }

    let response: SearchAllResponse = client.get("search.all", &params).await?;

    if !response.ok {
        anyhow::bail!(
            "Slack API error: {}",
            response.error.unwrap_or_default()
        );
    }

    Ok(response)
}

/// Builds a Slack search query with filters
pub fn build_search_query(
    text: &str,
    from_user: Option<&str>,
    in_channel: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
) -> String {
    let mut query = text.to_string();

    if let Some(user) = from_user {
        query.push_str(&format!(" from:{}", user));
    }

    if let Some(channel) = in_channel {
        query.push_str(&format!(" in:{}", channel));
    }

    if let Some(after_date) = after {
        query.push_str(&format!(" after:{}", after_date));
    }

    if let Some(before_date) = before {
        query.push_str(&format!(" before:{}", before_date));
    }

    query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_query_simple() {
        let query = build_search_query("hello world", None, None, None, None);
        assert_eq!(query, "hello world");
    }

    #[test]
    fn test_build_search_query_with_from() {
        let query = build_search_query("hello", Some("alice"), None, None, None);
        assert_eq!(query, "hello from:alice");
    }

    #[test]
    fn test_build_search_query_with_channel() {
        let query = build_search_query("hello", None, Some("general"), None, None);
        assert_eq!(query, "hello in:general");
    }

    #[test]
    fn test_build_search_query_with_dates() {
        let query = build_search_query("hello", None, None, Some("2024-01-01"), Some("2024-12-31"));
        assert_eq!(query, "hello after:2024-01-01 before:2024-12-31");
    }

    #[test]
    fn test_build_search_query_all_filters() {
        let query = build_search_query(
            "deploy",
            Some("bob"),
            Some("engineering"),
            Some("2024-01-01"),
            Some("2024-12-31"),
        );
        assert_eq!(
            query,
            "deploy from:bob in:engineering after:2024-01-01 before:2024-12-31"
        );
    }

    async fn setup() -> (mockito::ServerGuard, SlackClient) {
        let server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let client = SlackClient::with_base_url(&server.url(), false, false).await.unwrap();
        (server, client)
    }

    #[tokio::test]
    async fn test_search_messages_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/search.messages")
            .match_query(mockito::Matcher::UrlEncoded("query".into(), "hello".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "query": "hello",
                "messages": {
                    "total": 1,
                    "matches": [{
                        "type": "message",
                        "text": "hello world",
                        "ts": "1234567890.123456",
                        "user": "U123",
                        "channel": {
                            "id": "C123",
                            "name": "general"
                        }
                    }]
                }
            }"#,
            )
            .create_async()
            .await;

        let result = search_messages(&client, "hello", None).await.unwrap();
        assert_eq!(result.query, "hello");
        assert_eq!(result.messages.total, 1);
        assert_eq!(result.messages.matches.len(), 1);
    }

    #[tokio::test]
    async fn test_search_messages_with_count() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/search.messages")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "hello".into()),
                mockito::Matcher::UrlEncoded("count".into(), "50".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "query": "hello",
                "messages": {
                    "total": 0,
                    "matches": []
                }
            }"#,
            )
            .create_async()
            .await;

        let _result = search_messages(&client, "hello", Some(50)).await.unwrap();
    }

    #[tokio::test]
    async fn test_search_files_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/search.files")
            .match_query(mockito::Matcher::UrlEncoded("query".into(), "*.pdf".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "query": "*.pdf",
                "files": {
                    "total": 1,
                    "matches": [{
                        "id": "F123",
                        "created": 1234567890,
                        "timestamp": 1234567890,
                        "name": "document.pdf",
                        "title": "Document",
                        "mimetype": "application/pdf",
                        "filetype": "pdf",
                        "pretty_type": "PDF",
                        "user": "U123",
                        "size": 1024
                    }]
                }
            }"#,
            )
            .create_async()
            .await;

        let result = search_files(&client, "*.pdf", None).await.unwrap();
        assert_eq!(result.query, "*.pdf");
        assert_eq!(result.files.total, 1);
    }

    #[tokio::test]
    async fn test_search_all_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/search.all")
            .match_query(mockito::Matcher::UrlEncoded("query".into(), "test".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "query": "test",
                "messages": {
                    "total": 0,
                    "matches": []
                },
                "files": {
                    "total": 0,
                    "matches": []
                }
            }"#,
            )
            .create_async()
            .await;

        let result = search_all(&client, "test", None).await.unwrap();
        assert_eq!(result.query, "test");
    }

    #[tokio::test]
    async fn test_search_error_response() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/search.messages")
            .match_query(mockito::Matcher::UrlEncoded("query".into(), "test".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": false,
                "error": "invalid_auth",
                "query": "test",
                "messages": {
                    "total": 0,
                    "matches": []
                }
            }"#,
            )
            .create_async()
            .await;

        let result = search_messages(&client, "test", None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The client enhances "invalid_auth" to a helpful error message
        assert!(err.to_string().contains("Invalid authentication token"));
    }
}
