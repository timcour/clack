use super::client::SlackClient;
use crate::cache;
use crate::models::message::Message;
use crate::models::search::{SearchAllResponse, SearchFilesResponse, SearchMessagesResponse};
use anyhow::Result;
use std::collections::HashMap;

pub async fn search_messages(
    client: &SlackClient,
    query: &str,
    count: Option<u32>,
    page: Option<u32>,
) -> Result<SearchMessagesResponse> {
    let mut params = vec![("query", query.to_string())];

    if let Some(c) = count {
        params.push(("count", c.to_string()));
    }

    if let Some(p) = page {
        params.push(("page", p.to_string()));
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
    page: Option<u32>,
) -> Result<SearchFilesResponse> {
    let mut params = vec![("query", query.to_string())];

    if let Some(c) = count {
        params.push(("count", c.to_string()));
    }

    if let Some(p) = page {
        params.push(("page", p.to_string()));
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
    page: Option<u32>,
) -> Result<SearchAllResponse> {
    let mut params = vec![("query", query.to_string())];

    if let Some(c) = count {
        params.push(("count", c.to_string()));
    }

    if let Some(p) = page {
        params.push(("page", p.to_string()));
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

/// Valid values for the --during option
const VALID_DURING_VALUES: &[&str] = &["today", "yesterday", "week", "month", "year"];

/// Validate the --during option value
pub fn validate_during(value: &str) -> Result<()> {
    let value_lower = value.to_lowercase();
    if VALID_DURING_VALUES.contains(&value_lower.as_str()) {
        Ok(())
    } else {
        anyhow::bail!(
            "Invalid --during value: '{}'\n\nValid values are: {}",
            value,
            VALID_DURING_VALUES.join(", ")
        )
    }
}

/// Builds a Slack search query with filters (simple version for backward compatibility)
pub fn build_search_query(
    text: &str,
    from_user: Option<&str>,
    in_channel: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
) -> String {
    build_search_query_full(text, from_user, None, in_channel, None, after, before, None)
}

/// Builds a Slack search query with all filter options
pub fn build_search_query_full(
    text: &str,
    from_user: Option<&str>,
    to_user: Option<&str>,
    in_channel: Option<&str>,
    has: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    during: Option<&str>,
) -> String {
    let mut query = text.to_string();

    if let Some(user) = from_user {
        query.push_str(&format!(" from:{}", user));
    }

    if let Some(user) = to_user {
        query.push_str(&format!(" to:{}", user));
    }

    if let Some(channel) = in_channel {
        query.push_str(&format!(" in:{}", channel));
    }

    if let Some(has_type) = has {
        query.push_str(&format!(" has:{}", has_type));
    }

    if let Some(after_date) = after {
        query.push_str(&format!(" after:{}", after_date));
    }

    if let Some(before_date) = before {
        query.push_str(&format!(" before:{}", before_date));
    }

    if let Some(during_period) = during {
        query.push_str(&format!(" during:{}", during_period));
    }

    query
}

/// Cache messages from search results.
///
/// Search result messages include channel info, allowing us to cache them
/// for offline access. Messages are grouped by channel for efficient caching.
pub async fn cache_search_messages(client: &SlackClient, messages: &[Message]) {
    let workspace_id = match client.workspace_id() {
        Some(id) => id,
        None => return,
    };

    let pool = match client.cache_pool() {
        Some(p) => p,
        None => return,
    };

    let mut conn = match cache::get_connection(pool).await {
        Ok(c) => c,
        Err(_) => return,
    };

    // Group messages by channel ID for efficient caching
    let mut by_channel: HashMap<String, Vec<Message>> = HashMap::new();

    for msg in messages {
        if let Some(ref channel) = msg.channel {
            let channel_id = channel.id().to_string();
            by_channel.entry(channel_id).or_default().push(msg.clone());
        }
    }

    let channel_count = by_channel.len();

    // Cache each group
    for (channel_id, channel_messages) in by_channel {
        let _ = cache::operations::upsert_messages(
            &mut conn,
            workspace_id,
            &channel_id,
            &channel_messages,
            client.verbose(),
        );
    }

    if client.verbose() {
        eprintln!("[CACHE] Search results - cached {} messages from {} channels",
            messages.len(),
            channel_count
        );
    }
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
        let query = build_search_query("hello", None, None, Some("2026-01-01"), Some("2024-12-31"));
        assert_eq!(query, "hello after:2026-01-01 before:2024-12-31");
    }

    #[test]
    fn test_build_search_query_all_filters() {
        let query = build_search_query(
            "deploy",
            Some("bob"),
            Some("engineering"),
            Some("2026-01-01"),
            Some("2024-12-31"),
        );
        assert_eq!(
            query,
            "deploy from:bob in:engineering after:2026-01-01 before:2024-12-31"
        );
    }

    async fn setup() -> (mockito::ServerGuard, SlackClient) {
        let server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let client = SlackClient::with_base_url(&server.url(), false, false, false).await.unwrap();
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

        let result = search_messages(&client, "hello", None, None).await.unwrap();
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

        let _result = search_messages(&client, "hello", Some(50), None).await.unwrap();
    }

    #[tokio::test]
    async fn test_search_messages_with_page() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/search.messages")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "hello".into()),
                mockito::Matcher::UrlEncoded("page".into(), "2".into()),
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

        let _result = search_messages(&client, "hello", None, Some(2)).await.unwrap();
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

        let result = search_files(&client, "*.pdf", None, None).await.unwrap();
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

        let result = search_all(&client, "test", None, None).await.unwrap();
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

        let result = search_messages(&client, "test", None, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The client enhances "invalid_auth" to a helpful error message
        assert!(err.to_string().contains("Invalid authentication token"));
    }

    #[test]
    fn test_validate_during_valid() {
        assert!(validate_during("today").is_ok());
        assert!(validate_during("yesterday").is_ok());
        assert!(validate_during("week").is_ok());
        assert!(validate_during("month").is_ok());
        assert!(validate_during("year").is_ok());
        // Case insensitive
        assert!(validate_during("TODAY").is_ok());
        assert!(validate_during("Week").is_ok());
    }

    #[test]
    fn test_validate_during_invalid() {
        let result = validate_during("invalid");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid --during value"));
        assert!(err.contains("today, yesterday, week, month, year"));
    }

    #[test]
    fn test_build_search_query_full() {
        let query = build_search_query_full(
            "deploy",
            Some("alice"),
            Some("bob"),
            Some("general"),
            Some("link"),
            Some("2026-01-01"),
            Some("2026-12-31"),
            Some("week"),
        );
        assert_eq!(
            query,
            "deploy from:alice to:bob in:general has:link after:2026-01-01 before:2026-12-31 during:week"
        );
    }
}
