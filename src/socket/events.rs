use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use termcolor::Color;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

use crate::api::client::SlackClient;
use crate::api::users::get_user;
use crate::models::event::{Event, EventCallback, MessageEvent};
use crate::models::user::User;
use crate::output::color::ColorWriter;
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

            // Cache the event
            if let Some(pool) = client.cache_pool() {
                if let Ok(mut conn) = crate::cache::db::get_connection(pool).await {
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
    use chrono::{DateTime, Local};

    // Parse message timestamp for display
    let ts_float: f64 = msg.ts.parse().unwrap_or(0.0);
    let dt_utc = DateTime::from_timestamp(ts_float as i64, 0).unwrap_or_default();
    let dt_local: DateTime<Local> = dt_utc.into();

    // Timestamp prefix
    writer.print_colored(
        &format!("[{}] ", dt_local.format("%Y-%m-%d %H:%M:%S")),
        Color::White,
    )?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_filter_no_filters() {
        let filter = EventFilter::default();
        let msg = MessageEvent {
            channel: "C123".to_string(),
            user: Some("U123".to_string()),
            text: Some("test".to_string()),
            ts: "123.456".to_string(),
            thread_ts: None,
            subtype: None,
            bot_id: None,
            app_id: None,
            channel_type: None,
            event_ts: None,
            blocks: None,
            attachments: None,
            files: None,
            edited: None,
            hidden: None,
            message: None,
            previous_message: None,
            deleted_ts: None,
        };
        assert!(filter.matches_message(&msg));
    }

    #[test]
    fn test_event_filter_channel_match() {
        let filter = EventFilter {
            channels: vec!["C123".to_string()],
            users: vec![],
        };
        let msg = MessageEvent {
            channel: "C123".to_string(),
            user: Some("U123".to_string()),
            text: Some("test".to_string()),
            ts: "123.456".to_string(),
            thread_ts: None,
            subtype: None,
            bot_id: None,
            app_id: None,
            channel_type: None,
            event_ts: None,
            blocks: None,
            attachments: None,
            files: None,
            edited: None,
            hidden: None,
            message: None,
            previous_message: None,
            deleted_ts: None,
        };
        assert!(filter.matches_message(&msg));
    }

    #[test]
    fn test_event_filter_channel_no_match() {
        let filter = EventFilter {
            channels: vec!["C456".to_string()],
            users: vec![],
        };
        let msg = MessageEvent {
            channel: "C123".to_string(),
            user: Some("U123".to_string()),
            text: Some("test".to_string()),
            ts: "123.456".to_string(),
            thread_ts: None,
            subtype: None,
            bot_id: None,
            app_id: None,
            channel_type: None,
            event_ts: None,
            blocks: None,
            attachments: None,
            files: None,
            edited: None,
            hidden: None,
            message: None,
            previous_message: None,
            deleted_ts: None,
        };
        assert!(!filter.matches_message(&msg));
    }

    #[test]
    fn test_event_filter_user_match() {
        let filter = EventFilter {
            channels: vec![],
            users: vec!["U123".to_string()],
        };
        let msg = MessageEvent {
            channel: "C123".to_string(),
            user: Some("U123".to_string()),
            text: Some("test".to_string()),
            ts: "123.456".to_string(),
            thread_ts: None,
            subtype: None,
            bot_id: None,
            app_id: None,
            channel_type: None,
            event_ts: None,
            blocks: None,
            attachments: None,
            files: None,
            edited: None,
            hidden: None,
            message: None,
            previous_message: None,
            deleted_ts: None,
        };
        assert!(filter.matches_message(&msg));
    }
}
