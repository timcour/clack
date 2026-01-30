use crate::api::client::SlackClient;
use crate::api::search::{cache_search_messages, search_messages};
use crate::api::users::get_user;
use crate::models::user::User;
use crate::output::color::ColorWriter;
use crate::output::message_formatter::format_message_compact;
use crate::output::search_formatter::format_search_message;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

use super::{setup_signal_handler, StreamState};

/// Stream search messages continuously until interrupted
pub async fn stream_search_messages(
    client: &SlackClient,
    query: &str,
    interval_secs: u64,
    format: &str,
    no_color: bool,
) -> Result<()> {
    let running = setup_signal_handler();
    let mut state = StreamState::new(interval_secs);

    eprintln!(
        "Streaming messages matching '{}' (Ctrl+C to stop)...\n",
        query
    );

    while running.load(Ordering::SeqCst) {
        // Fetch latest results
        let response = match search_messages(client, query, Some(20), Some(1)).await {
            Ok(r) => r,
            Err(e) => {
                if client.verbose() {
                    eprintln!("[STREAM] Error fetching results: {}", e);
                }
                state.wait_for_next_poll().await;
                continue;
            }
        };

        // Cache ALL fetched messages immediately (before filtering)
        cache_search_messages(client, &response.messages.matches).await;

        // Filter to only new messages (for display)
        let new_messages: Vec<_> = response
            .messages
            .matches
            .iter()
            .filter(|msg| {
                if let Some(ref channel) = msg.channel {
                    state.is_new(channel.id(), &msg.ts)
                } else {
                    state.is_new("unknown", &msg.ts)
                }
            })
            .collect();

        // Format and output new messages
        if !new_messages.is_empty() {
            // Fetch user info for formatting
            let mut user_map: HashMap<String, User> = HashMap::new();
            for msg in &new_messages {
                if let Some(ref user_id) = msg.user {
                    if !user_map.contains_key(user_id) {
                        if let Ok(user) = get_user(client, user_id).await {
                            user_map.insert(user.id.clone(), user);
                        }
                    }
                }
            }

            // Output based on format
            match format {
                "json" => {
                    for msg in &new_messages {
                        println!("{}", serde_json::to_string(msg)?);
                    }
                }
                "yaml" => {
                    for msg in &new_messages {
                        println!("{}", serde_yaml::to_string(msg)?);
                    }
                }
                "human" => {
                    let mut writer = ColorWriter::new(no_color);
                    for msg in &new_messages {
                        format_search_message(msg, &user_map, &mut writer)?;
                        writer.writeln()?;
                    }
                    print!("{}", writer.into_string()?);
                }
                _ => {
                    // "human-compact" is the default
                    let mut writer = ColorWriter::new(no_color);
                    for msg in &new_messages {
                        format_message_compact(msg, &user_map, &mut writer)?;
                    }
                    print!("{}", writer.into_string()?);
                }
            }
        }

        // Wait for next poll
        state.wait_for_next_poll().await;
    }

    eprintln!("Stream stopped.");
    Ok(())
}
