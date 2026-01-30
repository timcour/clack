use anyhow::{Context, Result};
use futures_util::SinkExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};
use url::Url;

/// Response from apps.connections.open
#[derive(Debug, Deserialize)]
struct ConnectionOpenResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

/// Hello message received after WebSocket connection
#[derive(Debug, Deserialize)]
pub struct HelloMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub num_connections: Option<u32>,
    pub debug_info: Option<DebugInfo>,
    pub connection_info: Option<ConnectionInfo>,
}

#[derive(Debug, Deserialize)]
pub struct DebugInfo {
    pub host: Option<String>,
    pub approximate_connection_time: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionInfo {
    pub app_id: Option<String>,
}

/// Disconnect message
#[derive(Debug, Deserialize)]
pub struct DisconnectMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub reason: String,
    pub debug_info: Option<DebugInfo>,
}

/// Envelope wrapper for all Socket Mode messages
#[derive(Debug, Deserialize)]
pub struct SocketEnvelope {
    pub envelope_id: String,
    #[serde(rename = "type")]
    pub envelope_type: String,
    pub accepts_response_payload: Option<bool>,
    pub retry_attempt: Option<u32>,
    pub retry_reason: Option<String>,
    pub payload: Option<serde_json::Value>,
}

/// Acknowledgment message to send back
#[derive(Debug, Serialize)]
struct Acknowledgment {
    envelope_id: String,
}

pub struct SocketModeClient {
    app_token: String,
    verbose: bool,
}

impl SocketModeClient {
    pub fn new(app_token: String, verbose: bool) -> Self {
        Self { app_token, verbose }
    }

    /// Get WebSocket URL from apps.connections.open
    pub async fn get_websocket_url(&self) -> Result<String> {
        let client = reqwest::Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.app_token))?,
        );

        let response = client
            .post("https://slack.com/api/apps.connections.open")
            .headers(headers)
            .send()
            .await
            .context("Failed to call apps.connections.open")?;

        let result: ConnectionOpenResponse = response
            .json()
            .await
            .context("Failed to parse apps.connections.open response")?;

        if !result.ok {
            anyhow::bail!(
                "apps.connections.open failed: {}",
                result.error.unwrap_or_else(|| "unknown error".to_string())
            );
        }

        result
            .url
            .context("apps.connections.open returned ok but no URL")
    }

    /// Connect to WebSocket and return the stream
    pub async fn connect(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let ws_url = self.get_websocket_url().await?;

        if self.verbose {
            eprintln!("[SOCKET] Connecting to WebSocket...");
        }

        let url = Url::parse(&ws_url).context("Invalid WebSocket URL")?;
        let (ws_stream, _) = connect_async(url)
            .await
            .context("Failed to connect to WebSocket")?;

        if self.verbose {
            eprintln!("[SOCKET] WebSocket connected");
        }

        Ok(ws_stream)
    }

    /// Connect with automatic reconnection on failure
    pub async fn connect_with_retry(
        &self,
        max_retries: u32,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut retry_count = 0;
        let mut backoff = Duration::from_secs(1);

        loop {
            match self.connect().await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    retry_count += 1;
                    if retry_count > max_retries {
                        return Err(e)
                            .context(format!("Failed to connect after {} retries", max_retries));
                    }

                    if self.verbose {
                        eprintln!(
                            "[SOCKET] Connection failed (attempt {}/{}): {}",
                            retry_count, max_retries, e
                        );
                        eprintln!("[SOCKET] Retrying in {:?}...", backoff);
                    }

                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
                }
            }
        }
    }
}

/// Send acknowledgment for an envelope
pub async fn acknowledge(
    ws_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    envelope_id: &str,
) -> Result<()> {
    let ack = Acknowledgment {
        envelope_id: envelope_id.to_string(),
    };
    let ack_json = serde_json::to_string(&ack)?;
    ws_stream.send(WsMessage::Text(ack_json)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hello_message() {
        let json = r#"{
            "type": "hello",
            "num_connections": 1,
            "debug_info": {
                "host": "applink-123",
                "approximate_connection_time": 3600
            },
            "connection_info": {
                "app_id": "A123"
            }
        }"#;

        let hello: HelloMessage = serde_json::from_str(json).unwrap();
        assert_eq!(hello.msg_type, "hello");
        assert_eq!(hello.num_connections, Some(1));
        assert_eq!(
            hello
                .debug_info
                .as_ref()
                .unwrap()
                .approximate_connection_time,
            Some(3600)
        );
    }

    #[test]
    fn test_parse_disconnect_message() {
        let json = r#"{
            "type": "disconnect",
            "reason": "refresh_requested"
        }"#;

        let disconnect: DisconnectMessage = serde_json::from_str(json).unwrap();
        assert_eq!(disconnect.msg_type, "disconnect");
        assert_eq!(disconnect.reason, "refresh_requested");
    }

    #[test]
    fn test_parse_socket_envelope() {
        let json = r#"{
            "envelope_id": "env-123",
            "type": "events_api",
            "accepts_response_payload": false,
            "payload": {
                "type": "event_callback",
                "team_id": "T123",
                "api_app_id": "A123",
                "event": {
                    "type": "message",
                    "channel": "C123",
                    "user": "U123",
                    "text": "test",
                    "ts": "123.456"
                },
                "event_id": "Ev123",
                "event_time": 1234567890
            }
        }"#;

        let envelope: SocketEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(envelope.envelope_id, "env-123");
        assert_eq!(envelope.envelope_type, "events_api");
        assert!(envelope.payload.is_some());
    }

    #[test]
    fn test_socket_mode_client_new() {
        let client = SocketModeClient::new("xapp-test-token".to_string(), true);
        assert!(client.verbose);
    }
}
