use super::client::SlackClient;
use crate::cache;
use crate::models::channel::{Channel, ChannelInfoResponse, ChannelsListResponse};
use anyhow::Result;

/// Resolves a channel identifier to a channel ID.
/// Accepts channel IDs (C123, D123, G123), names (general), or names with # prefix (#general).
/// Returns the channel ID.
pub async fn resolve_channel_id(client: &SlackClient, identifier: &str) -> Result<String> {
    // Remove # prefix if present
    let clean_identifier = identifier.strip_prefix('#').unwrap_or(identifier);

    // Check if it looks like a conversation ID (channels, DMs, groups start with C, D, or G)
    let looks_like_id = clean_identifier.len() > 1 &&
        (clean_identifier.starts_with('C') ||
         clean_identifier.starts_with('D') ||
         clean_identifier.starts_with('G'));

    if looks_like_id {
        // Try conversations.info directly - it's faster than listing all channels
        match get_channel(client, clean_identifier).await {
            Ok(channel) => {
                return Ok(channel.id);
            }
            Err(e) => {
                if client.verbose() {
                    eprintln!("[API] conversations.info failed for '{}': {}", clean_identifier, e);
                    eprintln!("[API] Falling back to search by name");
                }
                // Fall through to name search - maybe it's actually a channel name that starts with C/D/G
            }
        }
    }

    // Either doesn't look like an ID, or conversations.info failed
    // Treat as a channel name and search for it
    list_channels_and_find(client, clean_identifier).await
}

async fn list_channels_and_find(client: &SlackClient, name: &str) -> Result<String> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first to avoid API calls (unless refresh requested)
    if !client.refresh_cache() {
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = cache::get_connection(pool).await {
                if let Ok(Some(cached_channels)) = cache::operations::get_conversations(&mut conn, workspace_id, client.verbose()) {
                    // Search cached channels first
                    if let Some(channel) = cached_channels.iter().find(|ch| ch.name == name) {
                        if client.verbose() {
                            eprintln!("[CACHE] Channel '{}' resolved from cache to {}", name, channel.id);
                        }
                        return Ok(channel.id.clone());
                    }
                }
            }
        }
    }

    // Not in cache - search with pagination, stopping when found
    if client.verbose() {
        eprintln!("[API] Searching for channel '{}' via conversations.list", name);
    }

    let mut cursor: Option<String> = None;
    let mut total_checked = 0;

    loop {
        let mut query = vec![
            ("limit", "200".to_string()),
            ("types", "public_channel,private_channel".to_string()),
            ("exclude_archived", "true".to_string()),
        ];

        if let Some(ref c) = cursor {
            query.push(("cursor", c.clone()));
        }

        let response: ChannelsListResponse = client.get("conversations.list", &query).await?;

        if !response.ok {
            anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
        }

        let channels = response.channels;
        total_checked += channels.len();

        // Cache this batch immediately
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = cache::get_connection(pool).await {
                let _ = cache::operations::upsert_conversations(&mut conn, workspace_id, &channels, client.verbose());
            }
        }

        // Check if we found the channel in this batch
        if let Some(channel) = channels.iter().find(|ch| ch.name == name) {
            if client.verbose() {
                eprintln!("[API] Channel '{}' found with ID {}", name, channel.id);
            }
            return Ok(channel.id.clone());
        }

        // Check if there are more pages
        match response.response_metadata {
            Some(metadata) if metadata.next_cursor.is_some() && !metadata.next_cursor.as_ref().unwrap().is_empty() => {
                cursor = metadata.next_cursor;
            }
            _ => break, // No more pages, channel not found
        }
    }

    // Channel not found after checking all pages
    anyhow::bail!(
        "Channel '{}' not found.\n\n\
        Possible reasons:\n\
        1. The channel is private and the bot is not a member\n\
        2. The bot token lacks required scopes (channels:read, groups:read)\n\
        3. The channel name is misspelled\n\n\
        Searched through {} channels. Try 'clack channels' to see the full list.",
        name,
        total_checked
    )
}

async fn fetch_all_channels(
    client: &SlackClient,
    workspace_id: &str,
    include_archived: bool,
    limit: u32,
) -> Result<Vec<Channel>> {
    let exclude_archived = if include_archived { "false" } else { "true" };
    let mut all_channels = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut query = vec![
            ("limit", limit.to_string()),
            ("types", "public_channel,private_channel".to_string()),
            ("exclude_archived", exclude_archived.to_string()),
        ];

        if let Some(ref c) = cursor {
            query.push(("cursor", c.clone()));
        }

        let response: ChannelsListResponse = client.get("conversations.list", &query).await?;

        if !response.ok {
            anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
        }

        let channels = response.channels;

        // Cache this batch immediately before fetching next batch
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = cache::get_connection(pool).await {
                let _ = cache::operations::upsert_conversations(
                    &mut conn,
                    workspace_id,
                    &channels,
                    client.verbose(),
                );
            }
        }

        all_channels.extend(channels);

        // Check if there are more pages
        match response.response_metadata {
            Some(metadata) if metadata.next_cursor.is_some() && !metadata.next_cursor.as_ref().unwrap().is_empty() => {
                cursor = metadata.next_cursor;
            }
            _ => break, // No more pages
        }
    }

    Ok(all_channels)
}

pub async fn list_channels(client: &SlackClient, include_archived: bool, limit: u32) -> Result<Vec<Channel>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Always fetch from API for list operations
    // Caching happens incrementally during pagination in fetch_all_channels
    let channels = fetch_all_channels(client, workspace_id, include_archived, limit).await?;

    Ok(channels)
}

pub async fn get_channel(client: &SlackClient, channel_id: &str) -> Result<Channel> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first (unless refresh requested)
    if !client.refresh_cache() {
        if let Some(pool) = client.cache_pool() {
            match cache::get_connection(pool).await {
                Ok(mut conn) => {
                    match cache::operations::get_conversation(&mut conn, workspace_id, channel_id, client.verbose()) {
                        Ok(Some(cached_channel)) => {
                            return Ok(cached_channel);
                        }
                        Ok(None) => {
                            // Cache miss, continue to API
                        }
                        Err(e) => {
                            if client.verbose() {
                                eprintln!("[CACHE] Error reading cache: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    if client.verbose() {
                        eprintln!("[CACHE] Failed to get connection: {}", e);
                    }
                }
            }
        }
    } else if client.verbose() {
        eprintln!("[CACHE] Conversation {} - SKIP (refresh requested)", channel_id);
    }

    // Fetch from API
    let query = vec![("channel", channel_id.to_string())];
    let response: ChannelInfoResponse = client.get("conversations.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let channel = response.channel;

    // Write through to cache
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = cache::get_connection(pool).await {
            let _ = cache::operations::upsert_conversation(&mut conn, workspace_id, &channel, client.verbose());
        }
    }

    Ok(channel)
}

/// Search for channels by name substring (case-insensitive)
pub async fn search_channels(
    client: &SlackClient,
    query: &str,
    include_archived: bool,
) -> Result<Vec<Channel>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Use default limit of 200 for search operations
    let all_channels = fetch_all_channels(client, workspace_id, include_archived, 200).await?;
    let query_lower = query.to_lowercase();

    // Filter channels that contain the query string (case-insensitive)
    let matching_channels: Vec<Channel> = all_channels
        .into_iter()
        .filter(|channel| channel.name.to_lowercase().contains(&query_lower))
        .collect();

    Ok(matching_channels)
}

pub async fn get_members(client: &SlackClient, channel: &str, limit: u32) -> Result<Vec<String>> {
    let mut query = vec![
        ("channel", channel.to_string()),
        ("limit", limit.to_string()),
    ];

    let mut all_members = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        if let Some(ref c) = cursor {
            query.push(("cursor", c.clone()));
        }

        #[derive(serde::Deserialize)]
        struct MembersResponse {
            ok: bool,
            members: Vec<String>,
            response_metadata: Option<ResponseMetadata>,
            error: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct ResponseMetadata {
            next_cursor: Option<String>,
        }

        let response: MembersResponse = client.get("conversations.members", &query).await?;

        if !response.ok {
            anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
        }

        all_members.extend(response.members);

        // Check if there are more pages
        match response.response_metadata {
            Some(metadata) if metadata.next_cursor.is_some() && !metadata.next_cursor.as_ref().unwrap().is_empty() => {
                cursor = metadata.next_cursor;
                // Remove the old cursor from query before adding new one
                query.retain(|(k, _)| k != &"cursor");
            }
            _ => break,
        }
    }

    Ok(all_members)
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
        let mut client = SlackClient::with_base_url(&server.url(), false, false, false).await.unwrap();

        // Mock auth.test for workspace initialization with unique workspace ID
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
    async fn test_get_channel_success() {
        let (mut server, client) = setup().await;

        // Clear any potential cache pollution for this workspace
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = cache::get_connection(pool).await {
                let workspace_id = client.workspace_id().unwrap();
                let _ = cache::operations::clear_workspace_cache(&mut conn, workspace_id, false);
            }
        }

        let _mock = server
            .mock("GET", "/conversations.info")
            .match_query(mockito::Matcher::UrlEncoded("channel".into(), "C123".into()))
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

    #[tokio::test]
    async fn test_resolve_channel_id_with_id() {
        let (mut server, client) = setup().await;

        // Mock conversations.info for channel ID lookups
        let _mock1 = server
            .mock("GET", "/conversations.info")
            .match_query(mockito::Matcher::UrlEncoded("channel".into(), "C123456".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "channel": {"id": "C123456", "name": "general", "is_channel": true, "is_private": false}}"#)
            .create_async()
            .await;

        let _mock2 = server
            .mock("GET", "/conversations.info")
            .match_query(mockito::Matcher::UrlEncoded("channel".into(), "C999ABC".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok": true, "channel": {"id": "C999ABC", "name": "random", "is_channel": true, "is_private": false}}"#)
            .create_async()
            .await;

        // Should call conversations.info and return the ID
        let result = resolve_channel_id(&client, "C123456").await.unwrap();
        assert_eq!(result, "C123456");

        let result = resolve_channel_id(&client, "C999ABC").await.unwrap();
        assert_eq!(result, "C999ABC");
    }

    #[tokio::test]
    async fn test_resolve_channel_id_with_name() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("limit".into(), "200".into()),
                mockito::Matcher::UrlEncoded("types".into(), "public_channel,private_channel".into()),
                mockito::Matcher::UrlEncoded("exclude_archived".into(), "true".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channels": [
                    {
                        "id": "C123",
                        "name": "general",
                        "is_channel": true
                    },
                    {
                        "id": "C456",
                        "name": "random",
                        "is_channel": true
                    }
                ],
                "response_metadata": {
                    "next_cursor": ""
                }
            }"#,
            )
            .create_async()
            .await;

        let result = resolve_channel_id(&client, "general").await.unwrap();
        assert_eq!(result, "C123");

        let result2 = resolve_channel_id(&client, "random").await.unwrap();
        assert_eq!(result2, "C456");
    }

    #[tokio::test]
    async fn test_resolve_channel_id_with_hash_prefix() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("limit".into(), "200".into()),
                mockito::Matcher::UrlEncoded("types".into(), "public_channel,private_channel".into()),
                mockito::Matcher::UrlEncoded("exclude_archived".into(), "true".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channels": [
                    {
                        "id": "C123",
                        "name": "general",
                        "is_channel": true
                    }
                ],
                "response_metadata": {
                    "next_cursor": ""
                }
            }"#,
            )
            .create_async()
            .await;

        // Should strip the # and look up the name
        let result = resolve_channel_id(&client, "#general").await.unwrap();
        assert_eq!(result, "C123");
    }

    #[tokio::test]
    async fn test_resolve_channel_id_not_found() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("limit".into(), "200".into()),
                mockito::Matcher::UrlEncoded("types".into(), "public_channel,private_channel".into()),
                mockito::Matcher::UrlEncoded("exclude_archived".into(), "true".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channels": [],
                "response_metadata": {
                    "next_cursor": ""
                }
            }"#,
            )
            .create_async()
            .await;

        let result = resolve_channel_id(&client, "nonexistent").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Channel 'nonexistent' not found"));
    }

    #[tokio::test]
    async fn test_pagination() {
        let (mut server, client) = setup().await;

        // Clear cache to ensure clean test state
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let _ = crate::cache::operations::clear_workspace_cache(&mut conn, "T123", false);
            }
        }

        // Mock first page with next_cursor
        let _mock1 = server
            .mock("GET", "/conversations.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("limit".into(), "200".into()),
                mockito::Matcher::UrlEncoded("types".into(), "public_channel,private_channel".into()),
                mockito::Matcher::UrlEncoded("exclude_archived".into(), "true".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channels": [
                    {"id": "C1", "name": "channel1", "is_channel": true},
                    {"id": "C2", "name": "channel2", "is_channel": true}
                ],
                "response_metadata": {
                    "next_cursor": "next_page_cursor"
                }
            }"#,
            )
            .create_async()
            .await;

        // Mock second page without next_cursor
        let _mock2 = server
            .mock("GET", "/conversations.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("limit".into(), "200".into()),
                mockito::Matcher::UrlEncoded("types".into(), "public_channel,private_channel".into()),
                mockito::Matcher::UrlEncoded("exclude_archived".into(), "true".into()),
                mockito::Matcher::UrlEncoded("cursor".into(), "next_page_cursor".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channels": [
                    {"id": "C3", "name": "channel3", "is_channel": true}
                ],
                "response_metadata": {
                    "next_cursor": ""
                }
            }"#,
            )
            .create_async()
            .await;

        let channels = list_channels(&client, false, 200).await.unwrap();
        assert_eq!(channels.len(), 3);
        assert_eq!(channels[0].id, "C1");
        assert_eq!(channels[1].id, "C2");
        assert_eq!(channels[2].id, "C3");
    }

    #[tokio::test]
    async fn test_search_channels() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("limit".into(), "200".into()),
                mockito::Matcher::UrlEncoded("types".into(), "public_channel,private_channel".into()),
                mockito::Matcher::UrlEncoded("exclude_archived".into(), "true".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channels": [
                    {"id": "C1", "name": "engineering", "is_channel": true},
                    {"id": "C2", "name": "engineering-ops", "is_channel": true},
                    {"id": "C3", "name": "marketing", "is_channel": true},
                    {"id": "C4", "name": "sales", "is_channel": true}
                ],
                "response_metadata": {
                    "next_cursor": ""
                }
            }"#,
            )
            .create_async()
            .await;

        let results = search_channels(&client, "eng", false).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "engineering");
        assert_eq!(results[1].name, "engineering-ops");

        let results2 = search_channels(&client, "market", false).await.unwrap();
        assert_eq!(results2.len(), 1);
        assert_eq!(results2[0].name, "marketing");

        let results3 = search_channels(&client, "xyz", false).await.unwrap();
        assert_eq!(results3.len(), 0);
    }

    #[tokio::test]
    async fn test_search_channels_case_insensitive() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/conversations.list")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("limit".into(), "200".into()),
                mockito::Matcher::UrlEncoded("types".into(), "public_channel,private_channel".into()),
                mockito::Matcher::UrlEncoded("exclude_archived".into(), "true".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "channels": [
                    {"id": "C1", "name": "Engineering", "is_channel": true},
                    {"id": "C2", "name": "MARKETING", "is_channel": true}
                ],
                "response_metadata": {
                    "next_cursor": ""
                }
            }"#,
            )
            .create_async()
            .await;

        // Search should be case-insensitive
        let results = search_channels(&client, "eng", false).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Engineering");

        let results2 = search_channels(&client, "MARK", false).await.unwrap();
        assert_eq!(results2.len(), 1);
        assert_eq!(results2[0].name, "MARKETING");
    }

    #[tokio::test]
    async fn test_get_channel_with_refresh_cache() {
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let workspace_id = format!("T{}", test_id);

        let mut server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");

        // Create client with refresh_cache=true
        let mut client = SlackClient::with_base_url(&server.url(), false, false, true).await.unwrap();

        // Mock auth.test
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

        // Pre-populate cache with stale data
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = crate::cache::get_connection(pool).await {
                let stale_channel = Channel {
                    id: "CREFRESH".to_string(),
                    name: "stale-channel".to_string(),
                    is_channel: Some(true),
                    is_group: None,
                    is_im: None,
                    is_mpim: None,
                    is_private: Some(false),
                    is_archived: Some(false),
                    topic: None,
                    purpose: None,
                    num_members: None,
                };
                let _ = crate::cache::operations::upsert_conversation(&mut conn, &workspace_id, &stale_channel, false);
            }
        }

        // Mock API response with fresh data
        let _mock = server
            .mock("GET", "/conversations.info?channel=CREFRESH")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
            "ok": true,
            "channel": {
                "id": "CREFRESH",
                "name": "fresh-channel",
                "is_channel": true,
                "is_private": false,
                "is_archived": false
            }
        }"#,
            )
            .create_async()
            .await;

        // Call get_channel - should skip cache and get fresh data from API
        let channel = get_channel(&client, "CREFRESH").await.unwrap();
        assert_eq!(channel.name, "fresh-channel", "Should get fresh data from API, not stale cache");
    }
}
