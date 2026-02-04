use anyhow::{anyhow, Result};
use url::Url;

/// Parsed Slack URL components
#[derive(Debug)]
pub struct SlackUrl {
    #[allow(dead_code)]
    pub workspace: String,
    pub channel_id: String,
    pub message_ts: Option<String>,
}

impl SlackUrl {
    /// Parse a Slack URL into its components
    ///
    /// Supports formats:
    /// - https://workspace.slack.com/archives/C123/p1234567890123456
    /// - https://workspace.slack.com/archives/C123
    pub fn parse(url_str: &str) -> Result<Self> {
        let url = Url::parse(url_str).map_err(|e| anyhow!("Invalid URL: {}", e))?;

        // Extract workspace from host
        let host = url.host_str().ok_or_else(|| anyhow!("URL missing host"))?;

        if !host.ends_with(".slack.com") {
            return Err(anyhow!("Not a Slack URL: {}", host));
        }

        let workspace = host.trim_end_matches(".slack.com").to_string();

        // Parse path: /archives/CHANNEL_ID/pTIMESTAMP
        let path_segments: Vec<&str> = url.path().split('/').filter(|s| !s.is_empty()).collect();

        if path_segments.is_empty() || path_segments[0] != "archives" {
            return Err(anyhow!(
                "Invalid Slack URL path format. Expected /archives/CHANNEL_ID/..."
            ));
        }

        if path_segments.len() < 2 {
            return Err(anyhow!("URL missing channel ID"));
        }

        let channel_id = path_segments[1].to_string();

        // Parse message timestamp if present
        let message_ts = if path_segments.len() >= 3 && path_segments[2].starts_with('p') {
            // Convert p1234567890123456 to 1234567890.123456
            let ts_str = &path_segments[2][1..]; // Remove 'p' prefix
            if ts_str.len() >= 10 {
                let (secs, micros) = ts_str.split_at(10);
                if micros.is_empty() {
                    Some(format!("{}.000000", secs))
                } else {
                    Some(format!("{}.{}", secs, micros))
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(SlackUrl {
            workspace,
            channel_id,
            message_ts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_url() {
        let url = SlackUrl::parse(
            "https://fulfilsolutions.slack.com/archives/C08A6HHTC1F/p1770234618715029",
        )
        .unwrap();

        assert_eq!(url.workspace, "fulfilsolutions");
        assert_eq!(url.channel_id, "C08A6HHTC1F");
        assert_eq!(url.message_ts, Some("1770234618.715029".to_string()));
    }

    #[test]
    fn test_parse_channel_url() {
        let url = SlackUrl::parse("https://myworkspace.slack.com/archives/C123ABC").unwrap();

        assert_eq!(url.workspace, "myworkspace");
        assert_eq!(url.channel_id, "C123ABC");
        assert_eq!(url.message_ts, None);
    }

    #[test]
    fn test_parse_message_url_short_ts() {
        let url =
            SlackUrl::parse("https://test.slack.com/archives/C123/p1234567890").unwrap();

        assert_eq!(url.channel_id, "C123");
        assert_eq!(url.message_ts, Some("1234567890.000000".to_string()));
    }

    #[test]
    fn test_invalid_not_slack() {
        let result = SlackUrl::parse("https://example.com/archives/C123");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not a Slack URL"));
    }

    #[test]
    fn test_invalid_no_archives() {
        let result = SlackUrl::parse("https://test.slack.com/messages/C123");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_url() {
        let result = SlackUrl::parse("not a url");
        assert!(result.is_err());
    }
}
