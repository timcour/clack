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
    verbose: bool,
    workspace_id: Option<String>,
}

impl SlackClient {
    pub fn new_verbose(verbose: bool) -> Result<Self> {
        Self::with_base_url("https://slack.com/api", verbose)
    }

    pub fn with_base_url(base_url: &str, verbose: bool) -> Result<Self> {
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
            verbose,
            workspace_id: None,
        })
    }

    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        self.get_with_retry(endpoint, query, 3).await
    }

    async fn get_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &[(&str, String)],
        max_retries: u32,
    ) -> Result<T> {
        let mut retry_count = 0;

        loop {
            let url = format!("{}/{}", self.base_url, endpoint);

            // Log request if verbose
            if self.verbose {
                let query_str = query
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                eprintln!("→ GET {}", url);
                if !query_str.is_empty() {
                    eprintln!("  Query: {}", query_str);
                }
            }

            let start = std::time::Instant::now();
            let response = self.client.get(&url).query(query).send().await?;
            let duration = start.elapsed();
            let status = response.status();

            // Handle rate limiting (429 Too Many Requests)
            if status.as_u16() == 429 {
                if self.verbose {
                    eprintln!("← {} ({}ms) - Rate limited", status.as_u16(), duration.as_millis());
                }
                if retry_count >= max_retries {
                    anyhow::bail!(
                        "Rate limit exceeded. Maximum retries ({}) reached.\n\n\
                         Slack API rate limits have been hit. Please wait a moment before trying again.\n\
                         For large workspaces, consider making fewer API calls or adding delays between commands.",
                    max_retries
                    );
                }

                // Get the Retry-After header (in seconds)
                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1); // Default to 1 second if header is missing

                eprintln!(
                    "Rate limited. Waiting {} second(s) before retry {}/{}...",
                    retry_after,
                    retry_count + 1,
                    max_retries
                );

                tokio::time::sleep(tokio::time::Duration::from_secs(retry_after)).await;
                retry_count += 1;
                continue;
            }

            if !status.is_success() {
                if self.verbose {
                    eprintln!("← {} ({}ms) - Failed", status.as_u16(), duration.as_millis());
                }
                anyhow::bail!("API request failed: {}", status);
            }

            // Get the response body as text
            let body = response.text().await?;
            let body_size = body.len();

            // Log response if verbose
            if self.verbose {
                eprintln!("← {} ({}ms, {} bytes)", status.as_u16(), duration.as_millis(), body_size);
            }

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
            return Ok(data);
        }
    }

    /// Initialize workspace context by calling auth.test
    pub async fn init_workspace(&mut self) -> Result<String> {
        if let Some(ref id) = self.workspace_id {
            return Ok(id.clone());
        }

        // Import moved inside function to avoid circular dependency
        use crate::api::auth::test_auth;

        let auth_response = test_auth(self).await?;
        self.workspace_id = Some(auth_response.team_id.clone());

        if self.verbose {
            eprintln!("Workspace: {} ({})", auth_response.team, auth_response.team_id);
        }

        Ok(auth_response.team_id)
    }

    /// Get the workspace ID if initialized
    pub fn workspace_id(&self) -> Option<&str> {
        self.workspace_id.as_deref()
    }
}
