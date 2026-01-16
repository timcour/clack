use super::client::SlackClient;
use crate::models::channel::{Channel, ChannelInfoResponse, ChannelsListResponse};
use anyhow::Result;

/// Resolves a channel identifier to a channel ID.
/// Accepts channel IDs (C123), names (general), or names with # prefix (#general).
/// Returns the channel ID.
pub async fn resolve_channel_id(client: &SlackClient, identifier: &str) -> Result<String> {
    // Remove # prefix if present
    let clean_identifier = identifier.strip_prefix('#').unwrap_or(identifier);

    // If it looks like a channel ID (starts with C), return as-is
    if clean_identifier.starts_with('C') && clean_identifier.len() > 1 {
        return Ok(clean_identifier.to_string());
    }

    // Otherwise, it's a channel name - we need to look it up
    list_channels_and_find(client, clean_identifier).await
}

async fn list_channels_and_find(client: &SlackClient, name: &str) -> Result<String> {
    // Search for channel with pagination, stopping when found
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

        total_checked += response.channels.len();

        // Check if we found the channel in this batch
        if let Some(channel) = response.channels.iter().find(|ch| ch.name == name) {
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

async fn fetch_all_channels(client: &SlackClient, include_archived: bool) -> Result<Vec<Channel>> {
    let exclude_archived = if include_archived { "false" } else { "true" };
    let mut all_channels = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut query = vec![
            ("limit", "200".to_string()), // Slack's recommended page size
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

        all_channels.extend(response.channels);

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

pub async fn list_channels(client: &SlackClient, include_archived: bool) -> Result<Vec<Channel>> {
    fetch_all_channels(client, include_archived).await
}

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
        let client = SlackClient::with_base_url(&server.url(), false).unwrap();
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

    #[tokio::test]
    async fn test_resolve_channel_id_with_id() {
        let (_server, client) = setup().await;

        // Should return the ID as-is if it starts with C
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

        let channels = list_channels(&client, false).await.unwrap();
        assert_eq!(channels.len(), 3);
        assert_eq!(channels[0].id, "C1");
        assert_eq!(channels[1].id, "C2");
        assert_eq!(channels[2].id, "C3");
    }
}
