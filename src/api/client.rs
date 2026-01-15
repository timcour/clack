use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
struct SlackErrorResponse {
    ok: bool,
    error: Option<String>,
    needed: Option<String>,
    provided: Option<String>,
}

pub struct SlackClient {
    client: reqwest::Client,
    base_url: String,
}

impl SlackClient {
    pub fn new() -> Result<Self> {
        Self::with_base_url("https://slack.com/api")
    }

    pub fn with_base_url(base_url: &str) -> Result<Self> {
        let token = env::var("SLACK_TOKEN").context(
            "SLACK_TOKEN environment variable not set\n\n\
             Please set your Slack API token:\n  \
             export SLACK_TOKEN=xoxb-your-token-here\n\n\
             To create a token, visit: https://api.slack.com/authentication/token-types"
        )?;

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.to_string(),
        })
    }

    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let response = self.client.get(&url).query(query).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("API request failed: {}", response.status());
        }

        // Get the response body as text
        let body = response.text().await?;

        // First, check if this is an error response
        if let Ok(error_response) = serde_json::from_str::<SlackErrorResponse>(&body) {
            if !error_response.ok {
                let error_msg = error_response.error.as_deref().unwrap_or("unknown error");

                // Provide helpful error messages for common errors
                let helpful_message = match error_msg {
                    "invalid_auth" => {
                        "Invalid authentication token.\n\n\
                         Your SLACK_TOKEN may be expired or invalid.\n\
                         Please check your token at: https://api.slack.com/apps"
                    }
                    "missing_scope" => {
                        let needed = error_response.needed.as_deref().unwrap_or("unknown");
                        let provided = error_response.provided.as_deref().unwrap_or("none");

                        // Provide additional context for common scope confusion
                        let additional_help = if needed.contains("history") {
                            "\nNote: *:read scopes only provide metadata (names, members, etc.).\n\
                             *:history scopes are required to read actual message content."
                        } else {
                            ""
                        };

                        return Err(anyhow::anyhow!(
                            "Missing required OAuth scope.\n\n\
                             Required: {}\n\
                             You have: {}{}\n\n\
                             Please add the required scope to your Slack app at:\n\
                             https://api.slack.com/apps",
                            needed, provided, additional_help
                        ));
                    }
                    "not_authed" => {
                        "Not authenticated.\n\n\
                         Please set your SLACK_TOKEN environment variable:\n\
                         export SLACK_TOKEN=xoxb-your-token-here"
                    }
                    "account_inactive" => "Your Slack account is inactive.",
                    "token_revoked" => "Your authentication token has been revoked.",
                    "no_permission" => "You don't have permission to access this resource.",
                    "org_login_required" => "Organization login is required.",
                    "ekm_access_denied" => "Access denied by enterprise key management.",
                    "ratelimited" => "Rate limited. Please wait a moment and try again.",
                    _ => error_msg,
                };

                anyhow::bail!("Slack API error: {}", helpful_message);
            }
        }

        // Parse the successful response
        let data = serde_json::from_str::<T>(&body)
            .with_context(|| format!("Failed to parse API response from {}", endpoint))?;
        Ok(data)
    }
}
