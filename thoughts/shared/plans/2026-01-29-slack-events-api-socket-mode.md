# Plan: Slack Events API with Socket Mode

## Overview

Implement Slack Events API support using Socket Mode (WebSocket-based) to receive real-time events. This adds a new `events listen` command that connects via WebSocket, streams events to stdout, and caches them for later querying. The first event type to support is `message`.

## Example Usage

```bash
# Listen to all message events
clack events listen

# Listen with JSON output
clack --format json events listen

# Filter to specific channels
clack events listen --channel general --channel engineering

# Filter to specific users
clack events listen --from alice

# Query cached events
clack events list --limit 50
clack events list --channel general --since "2026-01-29"
```

## Current State Analysis

### Existing Infrastructure
- **No HTTP server**: CLI-only application, uses polling for `stream` feature
- **SQLite cache**: Messages/users/channels cached with 1-week TTL (`src/cache/`)
- **Stream pattern**: Existing `stream` command uses polling with signal handling (`src/stream/`)
- **Token auth**: `SLACK_TOKEN` (xoxb-) environment variable for API calls

### Key Discoveries
- `src/stream/mod.rs:9-20`: Signal handling for graceful shutdown already implemented
- `src/stream/mod.rs:23-58`: `StreamState` for deduplication already exists
- `src/models/message.rs:4-13`: `Message` struct can be extended for event-specific fields
- `src/api/client.rs:35-41`: Token handling pattern to follow for app token

## Desired End State

After implementation:
1. Users can run `clack events listen` to receive real-time Slack events via WebSocket
2. Events stream to stdout in all supported formats (json/yaml/human/human-compact)
3. Events are cached in SQLite for offline querying via `clack events list`
4. Graceful reconnection on disconnect with exponential backoff
5. Client-side filtering by channel and user

### Verification
- `clack events listen` connects and receives events
- Events appear within seconds of being posted in Slack
- `clack events list` shows cached events after listening session ends
- Ctrl+C gracefully disconnects

## What We're NOT Doing

- HTTP Request URL mode (webhook-based) - Socket Mode is sufficient for this use case
- Other event types beyond `message` (can be added later)
- Event subscriptions management via CLI (done in Slack app settings)
- Interactive message handling (slash commands, button clicks)
- Marketplace distribution (Socket Mode apps not allowed)

## Prerequisites

Before implementation, the user must:
1. Create a Slack app at https://api.slack.com/apps (if not already done)
2. Enable Socket Mode in app settings
3. Generate an app-level token with `connections:write` scope
4. Subscribe to `message.channels`, `message.groups`, `message.im`, `message.mpim` events
5. Set `SLACK_APP_TOKEN=xapp-...` environment variable

## Implementation Approach

Socket Mode uses WebSocket for bidirectional communication:
1. Call `apps.connections.open` with app-level token to get WebSocket URL
2. Connect to WebSocket, receive `hello` message
3. Receive events as `events_api` messages containing event payloads
4. Acknowledge each event by sending `{"envelope_id": "..."}` back
5. Handle `disconnect` messages and reconnect as needed

---

## Phase 1: Add WebSocket Dependencies & App Token Support

### Overview
Add the WebSocket client library and support for the app-level token required by Socket Mode.

### Changes Required

#### 1. Cargo.toml
**File**: `Cargo.toml`
**Changes**: Add WebSocket client dependency

```toml
[dependencies]
# ... existing dependencies ...
tokio-tungstenite = { version = "0.21", features = ["native-tls"] }
futures-util = "0.3"
url = "2.5"
```

#### 2. API Client - App Token Support
**File**: `src/api/client.rs`
**Changes**: Add method to get app token for Socket Mode

After line 74 (end of `with_base_url` function), add:

```rust
/// Get the app-level token for Socket Mode (xapp-...)
/// This is separate from the bot token (xoxb-) used for API calls
pub fn get_app_token() -> Result<String> {
    env::var("SLACK_APP_TOKEN").context(
        "SLACK_APP_TOKEN environment variable not set\n\n\
         Socket Mode requires an app-level token:\n  \
         export SLACK_APP_TOKEN=xapp-your-token-here\n\n\
         To create an app-level token:\n\
         1. Go to https://api.slack.com/apps\n\
         2. Select your app > Basic Information\n\
         3. Under 'App-Level Tokens', click 'Generate Token and Scopes'\n\
         4. Add the 'connections:write' scope\n\
         5. Copy the token (starts with xapp-)"
    )
}
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds with new dependencies
- [x] `cargo test` passes

#### Manual Verification:
- [ ] N/A for this phase

---

## Phase 2: Implement Socket Mode Connection

### Overview
Create the Socket Mode connection infrastructure including the `apps.connections.open` API call and WebSocket connection management.

### Changes Required

#### 1. Socket Module Structure
**File**: `src/socket/mod.rs` (new file)
**Changes**: Create module with connection management

```rust
pub mod connection;
pub mod events;

pub use connection::SocketModeClient;
```

#### 2. Socket Mode Connection
**File**: `src/socket/connection.rs` (new file)
**Changes**: Implement WebSocket connection with reconnection logic

```rust
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};
use url::Url;

use crate::api::client::SlackClient;

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
    pub async fn connect(
        &self,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
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
    pub async fn connect_with_retry(&self, max_retries: u32) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut retry_count = 0;
        let mut backoff = Duration::from_secs(1);

        loop {
            match self.connect().await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    retry_count += 1;
                    if retry_count > max_retries {
                        return Err(e).context(format!(
                            "Failed to connect after {} retries",
                            max_retries
                        ));
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
```

#### 3. Update lib.rs
**File**: `src/lib.rs`
**Changes**: Export socket module

```rust
pub mod socket;
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes
- [x] Unit tests for `SocketModeClient::new()` pass

#### Manual Verification:
- [ ] N/A for this phase (connection tested in Phase 4)

---

## Phase 3: Event Models & Parsing

### Overview
Create data models for Events API payloads, focusing on the `message` event type.

### Changes Required

#### 1. Event Models
**File**: `src/models/event.rs` (new file)
**Changes**: Define event payload structures

```rust
use serde::{Deserialize, Serialize};

/// The outer event callback wrapper from Events API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EventCallback {
    /// Always "event_callback" for events
    #[serde(rename = "type")]
    pub callback_type: String,

    /// Workspace ID
    pub team_id: String,

    /// App ID
    pub api_app_id: String,

    /// The actual event payload
    pub event: Event,

    /// Unique event ID
    pub event_id: String,

    /// Unix timestamp of when event occurred
    pub event_time: i64,

    /// Event context (for newer events)
    pub event_context: Option<String>,

    /// Authorizations for this event
    pub authorizations: Option<Vec<Authorization>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Authorization {
    pub enterprise_id: Option<String>,
    pub team_id: Option<String>,
    pub user_id: Option<String>,
    pub is_bot: Option<bool>,
    pub is_enterprise_install: Option<bool>,
}

/// Event payload - currently only message type supported
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "message")]
    Message(MessageEvent),

    /// Catch-all for unsupported event types
    #[serde(other)]
    Unknown,
}

/// Message event payload
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MessageEvent {
    /// Channel ID where message was posted
    pub channel: String,

    /// User ID who sent the message (None for bot messages without user)
    pub user: Option<String>,

    /// Message text content
    pub text: Option<String>,

    /// Message timestamp (unique identifier)
    pub ts: String,

    /// Thread parent timestamp (if this is a reply)
    pub thread_ts: Option<String>,

    /// Message subtype (e.g., "bot_message", "channel_join")
    pub subtype: Option<String>,

    /// Bot ID if sent by a bot
    pub bot_id: Option<String>,

    /// App ID if sent by an app
    pub app_id: Option<String>,

    /// Channel type: "channel", "group", "im", "mpim"
    pub channel_type: Option<String>,

    /// Event timestamp
    pub event_ts: Option<String>,

    /// Blocks for rich message content
    pub blocks: Option<serde_json::Value>,

    /// Attachments
    pub attachments: Option<serde_json::Value>,

    /// Files attached to message
    pub files: Option<serde_json::Value>,

    /// If message was edited
    pub edited: Option<EditedInfo>,

    /// Hidden flag (for some message types)
    pub hidden: Option<bool>,

    /// For message_changed subtype - the actual message
    pub message: Option<Box<MessageEvent>>,

    /// For message_changed subtype - previous message
    pub previous_message: Option<Box<MessageEvent>>,

    /// For message_deleted subtype
    pub deleted_ts: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EditedInfo {
    pub user: String,
    pub ts: String,
}

impl MessageEvent {
    /// Get the effective user ID (handles bot messages)
    pub fn effective_user(&self) -> Option<&str> {
        self.user.as_deref().or(self.bot_id.as_deref())
    }

    /// Check if this is a bot message
    pub fn is_bot(&self) -> bool {
        self.bot_id.is_some() || self.subtype.as_deref() == Some("bot_message")
    }

    /// Check if this is a thread reply
    pub fn is_thread_reply(&self) -> bool {
        self.thread_ts.is_some() && self.thread_ts.as_deref() != Some(&self.ts)
    }

    /// Get display text (handles None case)
    pub fn display_text(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_event() {
        let json = r#"{
            "type": "message",
            "channel": "C123ABC456",
            "user": "U123ABC456",
            "text": "Hello world",
            "ts": "1355517523.000005",
            "event_ts": "1355517523.000005",
            "channel_type": "channel"
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Message(msg) => {
                assert_eq!(msg.channel, "C123ABC456");
                assert_eq!(msg.user, Some("U123ABC456".to_string()));
                assert_eq!(msg.text, Some("Hello world".to_string()));
                assert_eq!(msg.ts, "1355517523.000005");
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_parse_bot_message() {
        let json = r#"{
            "type": "message",
            "subtype": "bot_message",
            "channel": "C123ABC456",
            "bot_id": "B123",
            "text": "Bot says hello",
            "ts": "1355517523.000005"
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Message(msg) => {
                assert!(msg.is_bot());
                assert_eq!(msg.effective_user(), Some("B123"));
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_parse_thread_reply() {
        let json = r#"{
            "type": "message",
            "channel": "C123ABC456",
            "user": "U123ABC456",
            "text": "Reply in thread",
            "ts": "1355517524.000005",
            "thread_ts": "1355517523.000005"
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Message(msg) => {
                assert!(msg.is_thread_reply());
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_parse_event_callback() {
        let json = r#"{
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
        }"#;

        let callback: EventCallback = serde_json::from_str(json).unwrap();
        assert_eq!(callback.team_id, "T123");
        assert_eq!(callback.event_id, "Ev123");
    }
}
```

#### 2. Update models/mod.rs
**File**: `src/models/mod.rs`
**Changes**: Export event module

```rust
pub mod channel;
pub mod event;  // Add this line
pub mod file;
pub mod message;
pub mod pin;
pub mod search;
pub mod user;
pub mod workspace;
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes
- [x] Event parsing tests pass: `cargo test models::event`

#### Manual Verification:
- [ ] N/A for this phase

---

## Phase 4: Event Streaming Command

### Overview
Implement the `events listen` command that connects to Socket Mode and streams events to stdout.

### Changes Required

#### 1. Socket Events Handler
**File**: `src/socket/events.rs` (new file)
**Changes**: Implement event listening loop

```rust
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

use crate::api::client::SlackClient;
use crate::api::users::get_user;
use crate::models::event::{Event, EventCallback, MessageEvent};
use crate::models::user::User;
use crate::output::color::ColorWriter;
use crate::output::message_formatter::format_message_compact;
use crate::socket::connection::{acknowledge, HelloMessage, SocketEnvelope, SocketModeClient};
use crate::stream::setup_signal_handler;

/// Filter criteria for events
#[derive(Default)]
pub struct EventFilter {
    pub channels: Vec<String>,
    pub users: Vec<String>,
}

impl EventFilter {
    pub fn matches_message(&self, msg: &MessageEvent) -> bool {
        // If no filters, match everything
        if self.channels.is_empty() && self.users.is_empty() {
            return true;
        }

        // Check channel filter
        if !self.channels.is_empty() && !self.channels.contains(&msg.channel) {
            return false;
        }

        // Check user filter
        if !self.users.is_empty() {
            let user_id = msg.effective_user().unwrap_or("");
            if !self.users.iter().any(|u| u == user_id) {
                return false;
            }
        }

        true
    }
}

/// Listen to Socket Mode events and stream to stdout
pub async fn listen_events(
    client: &SlackClient,
    app_token: &str,
    filter: EventFilter,
    format: &str,
    no_color: bool,
    verbose: bool,
) -> Result<()> {
    let running = setup_signal_handler();
    let socket_client = SocketModeClient::new(app_token.to_string(), verbose);

    eprintln!("Connecting to Slack Socket Mode...");

    // Connect with retry
    let mut ws_stream = socket_client.connect_with_retry(3).await?;

    // User cache for formatting
    let mut user_map: HashMap<String, User> = HashMap::new();

    eprintln!("Connected! Listening for events (Ctrl+C to stop)...\n");

    while running.load(Ordering::SeqCst) {
        tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        if verbose {
                            eprintln!("[SOCKET] Received: {}", text);
                        }

                        // Try to parse as different message types
                        if let Ok(envelope) = serde_json::from_str::<SocketEnvelope>(&text) {
                            // Acknowledge the envelope
                            if let Err(e) = acknowledge(&mut ws_stream, &envelope.envelope_id).await {
                                if verbose {
                                    eprintln!("[SOCKET] Failed to acknowledge: {}", e);
                                }
                            }

                            // Handle based on envelope type
                            match envelope.envelope_type.as_str() {
                                "events_api" => {
                                    if let Some(payload) = envelope.payload {
                                        if let Ok(callback) = serde_json::from_value::<EventCallback>(payload) {
                                            handle_event_callback(
                                                client,
                                                &callback,
                                                &filter,
                                                format,
                                                no_color,
                                                &mut user_map,
                                            ).await?;
                                        }
                                    }
                                }
                                "disconnect" => {
                                    if verbose {
                                        eprintln!("[SOCKET] Received disconnect, reconnecting...");
                                    }
                                    // Reconnect
                                    ws_stream = socket_client.connect_with_retry(3).await?;
                                }
                                _ => {
                                    if verbose {
                                        eprintln!("[SOCKET] Unknown envelope type: {}", envelope.envelope_type);
                                    }
                                }
                            }
                        } else if let Ok(hello) = serde_json::from_str::<HelloMessage>(&text) {
                            if hello.msg_type == "hello" {
                                if verbose {
                                    eprintln!("[SOCKET] Received hello message");
                                    if let Some(info) = &hello.debug_info {
                                        if let Some(time) = info.approximate_connection_time {
                                            eprintln!("[SOCKET] Connection will refresh in ~{} seconds", time);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) => {
                        if verbose {
                            eprintln!("[SOCKET] Connection closed, reconnecting...");
                        }
                        ws_stream = socket_client.connect_with_retry(3).await?;
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        // Respond to ping
                        let _ = ws_stream.send(WsMessage::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        if verbose {
                            eprintln!("[SOCKET] WebSocket error: {}", e);
                        }
                        // Try to reconnect
                        ws_stream = socket_client.connect_with_retry(3).await?;
                    }
                    None => {
                        if verbose {
                            eprintln!("[SOCKET] Stream ended, reconnecting...");
                        }
                        ws_stream = socket_client.connect_with_retry(3).await?;
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                // Keepalive - send ping if no activity
                let _ = ws_stream.send(WsMessage::Ping(vec![])).await;
            }
        }
    }

    // Clean shutdown
    let _ = ws_stream.close(None).await;
    eprintln!("Disconnected.");
    Ok(())
}

async fn handle_event_callback(
    client: &SlackClient,
    callback: &EventCallback,
    filter: &EventFilter,
    format: &str,
    no_color: bool,
    user_map: &mut HashMap<String, User>,
) -> Result<()> {
    match &callback.event {
        Event::Message(msg) => {
            // Skip hidden messages and message_changed/deleted for now
            if msg.hidden == Some(true) {
                return Ok(());
            }
            if let Some(subtype) = &msg.subtype {
                if subtype == "message_changed" || subtype == "message_deleted" {
                    return Ok(());
                }
            }

            // Apply filter
            if !filter.matches_message(msg) {
                return Ok(());
            }

            // Fetch user info if needed
            if let Some(user_id) = msg.effective_user() {
                if !user_map.contains_key(user_id) {
                    if let Ok(user) = get_user(client, user_id).await {
                        user_map.insert(user.id.clone(), user);
                    }
                }
            }

            // Output based on format
            output_message_event(callback, msg, user_map, format, no_color)?;
        }
        Event::Unknown => {
            // Ignore unknown event types for now
        }
    }
    Ok(())
}

fn output_message_event(
    callback: &EventCallback,
    msg: &MessageEvent,
    user_map: &HashMap<String, User>,
    format: &str,
    no_color: bool,
) -> Result<()> {
    match format {
        "json" => {
            // Output the full event callback as JSON
            println!("{}", serde_json::to_string(callback)?);
        }
        "yaml" => {
            println!("{}", serde_yaml::to_string(callback)?);
        }
        _ => {
            // Human-readable format
            let mut writer = ColorWriter::new(no_color);
            format_event_message(msg, user_map, &mut writer)?;
            print!("{}", writer.into_string()?);
        }
    }
    Ok(())
}

fn format_event_message(
    msg: &MessageEvent,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    use chrono::{DateTime, Local, TimeZone, Utc};
    use termcolor::Color;

    // Parse message timestamp for display
    let ts_float: f64 = msg.ts.parse().unwrap_or(0.0);
    let dt_utc = DateTime::from_timestamp(ts_float as i64, 0).unwrap_or_default();
    let dt_local: DateTime<Local> = dt_utc.into();

    // Timestamp prefix
    writer.print_colored(&format!("[{}] ", dt_local.format("%Y-%m-%d %H:%M:%S")), Color::White)?;

    // Channel
    writer.print_colored(&format!("#{}", &msg.channel), Color::Green)?;
    writer.write(" ")?;

    // User
    if let Some(user_id) = msg.effective_user() {
        if let Some(user) = users.get(user_id) {
            writer.print_colored(&format!("@{}", user.name), Color::Cyan)?;
        } else if msg.is_bot() {
            writer.print_colored(&format!("[bot:{}]", user_id), Color::Yellow)?;
        } else {
            writer.print_colored(user_id, Color::Cyan)?;
        }
    }
    writer.write(": ")?;

    // Thread indicator
    if msg.is_thread_reply() {
        writer.print_colored("[reply] ", Color::Magenta)?;
    }

    // Message text (single line)
    let text = msg.display_text().replace('\n', " ");
    writer.write(&text)?;

    writer.writeln()?;
    Ok(())
}
```

#### 2. CLI Command Definition
**File**: `src/cli.rs`
**Changes**: Add Events command and subcommands

After the `Stream` command definition (around line 87), add:

```rust
    /// Listen to Slack events in real-time via Socket Mode
    Events {
        #[command(subcommand)]
        command: EventsCommands,
    },
```

Add after `StreamType` enum:

```rust
#[derive(Subcommand)]
pub enum EventsCommands {
    /// Listen to events in real-time (requires Socket Mode app token)
    Listen {
        /// Filter to specific channels (can be specified multiple times)
        #[arg(long)]
        channel: Vec<String>,

        /// Filter to specific users (can be specified multiple times)
        #[arg(long)]
        from: Vec<String>,
    },
}
```

#### 3. Main Integration
**File**: `src/main.rs`
**Changes**: Handle Events command

Add import at top:
```rust
use clack::socket::events::{listen_events, EventFilter};
use clack::api::client::SlackClient;
```

Add in the main match statement for commands:

```rust
Commands::Events { command } => {
    match command {
        EventsCommands::Listen { channel, from } => {
            // Get app token for Socket Mode
            let app_token = SlackClient::get_app_token()?;

            // Resolve channel names to IDs if needed
            let mut channel_ids = Vec::new();
            for ch in &channel {
                let channel_id = crate::api::channels::resolve_channel_id(&client, ch).await?;
                channel_ids.push(channel_id);
            }

            // Resolve user names to IDs if needed
            let mut user_ids = Vec::new();
            for user in &from {
                let user_id = crate::api::users::resolve_user_to_id(&client, user).await?;
                user_ids.push(user_id);
            }

            let filter = EventFilter {
                channels: channel_ids,
                users: user_ids,
            };

            listen_events(
                &client,
                &app_token,
                filter,
                &cli.format,
                cli.no_color,
                cli.verbose,
            )
            .await?;
        }
    }
}
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes
- [x] CLI help shows events command: `cargo run -- events --help`

#### Manual Verification:
- [x] `clack events listen` connects to Socket Mode
- [x] Events appear in stdout within seconds of being posted
- [x] Ctrl+C gracefully disconnects
- [x] `--format json` outputs valid JSON per event
- [x] `--channel` and `--from` filters work correctly

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Event Caching

### Overview
Store received events in SQLite cache for offline querying.

### Changes Required

#### 1. Database Migration
**File**: `migrations/2026-01-29-000001_add_events/up.sql` (new file)
**Changes**: Create events table

```sql
-- Events table for caching received Socket Mode events
CREATE TABLE IF NOT EXISTS events (
    -- Composite primary key
    event_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,

    -- Event metadata
    event_type TEXT NOT NULL,
    event_time INTEGER NOT NULL,
    api_app_id TEXT NOT NULL,

    -- Message event fields (nullable for other event types)
    channel_id TEXT,
    user_id TEXT,
    message_ts TEXT,
    message_text TEXT,
    thread_ts TEXT,
    subtype TEXT,

    -- Full event payload as JSON
    full_payload TEXT NOT NULL,

    -- Cache metadata
    cached_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (event_id, workspace_id)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_events_workspace_time ON events(workspace_id, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_channel ON events(workspace_id, channel_id, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_user ON events(workspace_id, user_id, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_cached_at ON events(cached_at);
```

**File**: `migrations/2026-01-29-000001_add_events/down.sql` (new file)
```sql
DROP TABLE IF EXISTS events;
```

#### 2. Cache Schema
**File**: `src/cache/schema.rs`
**Changes**: Add events table definition

Add after existing table definitions:

```rust
diesel::table! {
    events (event_id, workspace_id) {
        event_id -> Text,
        workspace_id -> Text,
        event_type -> Text,
        event_time -> BigInt,
        api_app_id -> Text,
        channel_id -> Nullable<Text>,
        user_id -> Nullable<Text>,
        message_ts -> Nullable<Text>,
        message_text -> Nullable<Text>,
        thread_ts -> Nullable<Text>,
        subtype -> Nullable<Text>,
        full_payload -> Text,
        cached_at -> Timestamp,
    }
}
```

#### 3. Cache Models
**File**: `src/cache/models.rs`
**Changes**: Add CachedEvent struct

Add after existing model definitions:

```rust
use crate::models::event::EventCallback;

#[derive(Debug, Clone, Queryable, Insertable, AsChangeset)]
#[diesel(table_name = crate::cache::schema::events)]
pub struct CachedEvent {
    pub event_id: String,
    pub workspace_id: String,
    pub event_type: String,
    pub event_time: i64,
    pub api_app_id: String,
    pub channel_id: Option<String>,
    pub user_id: Option<String>,
    pub message_ts: Option<String>,
    pub message_text: Option<String>,
    pub thread_ts: Option<String>,
    pub subtype: Option<String>,
    pub full_payload: String,
    pub cached_at: chrono::NaiveDateTime,
}

impl CachedEvent {
    pub fn from_event_callback(callback: &EventCallback, workspace_id: &str) -> Self {
        use crate::models::event::Event;

        let (channel_id, user_id, message_ts, message_text, thread_ts, subtype) = match &callback.event {
            Event::Message(msg) => (
                Some(msg.channel.clone()),
                msg.user.clone(),
                Some(msg.ts.clone()),
                msg.text.clone(),
                msg.thread_ts.clone(),
                msg.subtype.clone(),
            ),
            Event::Unknown => (None, None, None, None, None, None),
        };

        Self {
            event_id: callback.event_id.clone(),
            workspace_id: workspace_id.to_string(),
            event_type: "message".to_string(), // For now, only message events
            event_time: callback.event_time,
            api_app_id: callback.api_app_id.clone(),
            channel_id,
            user_id,
            message_ts,
            message_text,
            thread_ts,
            subtype,
            full_payload: serde_json::to_string(callback).unwrap_or_default(),
            cached_at: chrono::Utc::now().naive_utc(),
        }
    }

    pub fn to_event_callback(&self) -> Option<EventCallback> {
        serde_json::from_str(&self.full_payload).ok()
    }
}
```

#### 4. Cache Operations
**File**: `src/cache/operations.rs`
**Changes**: Add event caching operations

Add after existing operations:

```rust
use crate::cache::models::CachedEvent;
use crate::cache::schema::events;
use crate::models::event::EventCallback;

/// Cache an event callback
pub fn cache_event(
    conn: &mut SqliteConnection,
    callback: &EventCallback,
    workspace_id: &str,
    verbose: bool,
) -> Result<(), diesel::result::Error> {
    use diesel::prelude::*;

    let cached = CachedEvent::from_event_callback(callback, workspace_id);

    diesel::insert_into(events::table)
        .values(&cached)
        .on_conflict((events::event_id, events::workspace_id))
        .do_nothing()
        .execute(conn)?;

    if verbose {
        eprintln!("[CACHE] Cached event {}", callback.event_id);
    }

    Ok(())
}

/// Get cached events with optional filters
pub fn get_cached_events(
    conn: &mut SqliteConnection,
    workspace_id: &str,
    channel_id: Option<&str>,
    user_id: Option<&str>,
    since: Option<i64>,
    limit: i64,
    verbose: bool,
) -> Result<Vec<CachedEvent>, diesel::result::Error> {
    use diesel::prelude::*;

    let mut query = events::table
        .filter(events::workspace_id.eq(workspace_id))
        .order(events::event_time.desc())
        .limit(limit)
        .into_boxed();

    if let Some(ch) = channel_id {
        query = query.filter(events::channel_id.eq(ch));
    }

    if let Some(u) = user_id {
        query = query.filter(events::user_id.eq(u));
    }

    if let Some(ts) = since {
        query = query.filter(events::event_time.ge(ts));
    }

    let results = query.load::<CachedEvent>(conn)?;

    if verbose {
        eprintln!("[CACHE] Retrieved {} events", results.len());
    }

    Ok(results)
}
```

#### 5. Integrate Caching into Event Listener
**File**: `src/socket/events.rs`
**Changes**: Cache events as they're received

Add to `handle_event_callback` function, after the filter check and before output:

```rust
// Cache the event
if let Some(pool) = client.cache_pool() {
    if let Ok(mut conn) = pool.get_connection() {
        if let Some(workspace_id) = client.workspace_id() {
            let _ = crate::cache::operations::cache_event(
                &mut conn,
                callback,
                workspace_id,
                client.verbose(),
            );
        }
    }
}
```

#### 6. Events List Command
**File**: `src/cli.rs`
**Changes**: Add list subcommand to EventsCommands

```rust
#[derive(Subcommand)]
pub enum EventsCommands {
    /// Listen to events in real-time (requires Socket Mode app token)
    Listen {
        #[arg(long)]
        channel: Vec<String>,
        #[arg(long)]
        from: Vec<String>,
    },
    /// List cached events
    List {
        /// Filter to specific channel
        #[arg(long)]
        channel: Option<String>,

        /// Filter to specific user
        #[arg(long)]
        from: Option<String>,

        /// Only show events since this Unix timestamp or date (e.g., "2026-01-29")
        #[arg(long)]
        since: Option<String>,

        /// Maximum number of events to return
        #[arg(long, default_value = "50")]
        limit: i64,
    },
}
```

**File**: `src/main.rs`
**Changes**: Handle events list command

```rust
EventsCommands::List { channel, from, since, limit } => {
    // Resolve channel name if provided
    let channel_id = if let Some(ch) = &channel {
        Some(crate::api::channels::resolve_channel_id(&client, ch).await?)
    } else {
        None
    };

    // Resolve user name if provided
    let user_id = if let Some(u) = &from {
        Some(crate::api::users::resolve_user_to_id(&client, u).await?)
    } else {
        None
    };

    // Parse since timestamp
    let since_ts = if let Some(s) = &since {
        // Try parsing as Unix timestamp first
        if let Ok(ts) = s.parse::<i64>() {
            Some(ts)
        } else {
            // Try parsing as date
            use chrono::NaiveDate;
            if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
            } else {
                anyhow::bail!("Invalid --since value. Use Unix timestamp or YYYY-MM-DD format.");
            }
        }
    } else {
        None
    };

    // Get cached events
    let events = if let Some(pool) = client.cache_pool() {
        let mut conn = pool.get_connection()?;
        let workspace_id = client.workspace_id().context("No workspace ID")?;
        crate::cache::operations::get_cached_events(
            &mut conn,
            workspace_id,
            channel_id.as_deref(),
            user_id.as_deref(),
            since_ts,
            limit,
            cli.verbose,
        )?
    } else {
        vec![]
    };

    // Output
    if events.is_empty() {
        eprintln!("No cached events found.");
    } else {
        for cached in &events {
            if let Some(callback) = cached.to_event_callback() {
                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string(&callback)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&callback)?),
                    _ => {
                        // Human-readable format
                        if let crate::models::event::Event::Message(msg) = &callback.event {
                            println!(
                                "[{}] #{} {}: {}",
                                chrono::DateTime::from_timestamp(callback.event_time, 0)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                    .unwrap_or_else(|| callback.event_time.to_string()),
                                msg.channel,
                                msg.effective_user().unwrap_or("unknown"),
                                msg.display_text()
                            );
                        }
                    }
                }
            }
        }
    }
}
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes
- [x] Migration runs: embedded migrations run automatically on cache init
- [x] Events list command works: `cargo run -- events list --help`

#### Manual Verification:
- [x] After running `clack events listen`, events are cached
- [x] `clack events list` shows previously received events
- [x] Filters (`--channel`, `--from`, `--since`) work correctly
- [x] Cache persists across CLI invocations

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 6: Testing & Documentation

### Overview
Add comprehensive tests and update help text.

### Changes Required

#### 1. Unit Tests for Socket Connection
**File**: `src/socket/connection.rs`
**Changes**: Add tests at bottom of file

```rust
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
        assert_eq!(hello.debug_info.as_ref().unwrap().approximate_connection_time, Some(3600));
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
}
```

#### 2. Integration Test
**File**: `tests/events_test.rs` (new file)
**Changes**: Add integration tests

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_events_help() {
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("events").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Listen to Slack events"));
}

#[test]
fn test_events_listen_help() {
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("events").arg("listen").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--channel"))
        .stdout(predicate::str::contains("--from"));
}

#[test]
fn test_events_list_help() {
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("events").arg("list").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--since"))
        .stdout(predicate::str::contains("--limit"));
}

#[test]
fn test_events_listen_requires_app_token() {
    // Remove app token to test error message
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.env_remove("SLACK_APP_TOKEN")
        .env("SLACK_TOKEN", "xoxb-test")
        .arg("events")
        .arg("listen");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("SLACK_APP_TOKEN"));
}
```

#### 3. Update README/Help
**File**: Update CLI help text in `src/cli.rs` as needed

Ensure the Events command has clear, helpful descriptions:

```rust
/// Listen to Slack events in real-time via Socket Mode
///
/// Requires Socket Mode to be enabled in your Slack app settings
/// and a SLACK_APP_TOKEN environment variable (starts with xapp-).
///
/// Example: clack events listen --channel general
Events {
    #[command(subcommand)]
    command: EventsCommands,
},
```

### Success Criteria

#### Automated Verification:
- [x] All tests pass: `cargo test`
- [x] Integration tests pass: `cargo test --test events_test`
- [x] No clippy warnings: `cargo clippy` (minor warnings acceptable)
- [x] Code is formatted: `cargo fmt --check`

#### Manual Verification:
- [x] `clack events --help` shows clear usage instructions
- [x] Error message for missing `SLACK_APP_TOKEN` is helpful

---

## Testing Strategy

### Unit Tests
- Event model parsing (various subtypes, edge cases)
- Socket envelope parsing
- Filter matching logic
- Cache model conversions

### Integration Tests
- CLI argument parsing
- Help text verification
- Error message quality

### Manual Testing Steps
1. Create/configure a Slack app with Socket Mode enabled
2. Generate app-level token with `connections:write` scope
3. Subscribe to message events in app settings
4. Set `SLACK_APP_TOKEN` and `SLACK_TOKEN` environment variables
5. Run `clack events listen` and post a message in Slack
6. Verify message appears in stdout within 1-2 seconds
7. Press Ctrl+C and verify graceful shutdown
8. Run `clack events list` and verify the event was cached
9. Test filters: `--channel`, `--from`
10. Test output formats: `--format json`, `--format yaml`

## Performance Considerations

- **Connection keepalive**: Send WebSocket ping every 30 seconds to prevent idle disconnects
- **User caching**: Cache user info to avoid repeated API calls during event formatting
- **Event deduplication**: Socket Mode may send duplicate events during reconnection; use `event_id` to deduplicate in cache

## Security Considerations

- App-level token (`xapp-`) should be treated as sensitive as bot token
- Socket Mode connection is pre-authenticated, no additional verification needed
- Events may contain sensitive message content - respect user privacy

## References

- [Slack Events API](https://docs.slack.dev/apis/events-api/)
- [Socket Mode Documentation](https://docs.slack.dev/apis/events-api/using-socket-mode/)
- [apps.connections.open API](https://docs.slack.dev/reference/methods/apps.connections.open/)
- [Message Event Reference](https://docs.slack.dev/reference/events/message)
- [slack-morphism-rust](https://github.com/abdolence/slack-morphism-rust) - Rust Slack library with Socket Mode
