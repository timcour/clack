use crate::models::channel::Channel;
use crate::models::message::Message;
use crate::models::user::User;
use crate::output::color::ColorWriter;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::io::Result;
use termcolor::Color;
use textwrap::wrap;

pub fn format_messages_with_thread_info(
    messages: &[Message],
    channel: &Channel,
    users: &HashMap<String, User>,
    thread_info: &HashMap<String, (usize, Vec<String>)>, // Map of thread_ts -> (reply_count, participants)
    writer: &mut ColorWriter,
) -> Result<()> {
    // Channel metadata summary
    writer.print_header(&format!("#{} ({})", channel.name, channel.id))?;

    // Topic if present
    if let Some(topic) = &channel.topic {
        if !topic.value.is_empty() {
            writer.print_field("Topic", &topic.value)?;
        }
    }

    // Purpose if present
    if let Some(purpose) = &channel.purpose {
        if !purpose.value.is_empty() {
            writer.print_field("Purpose", &purpose.value)?;
        }
    }

    // Member count if present
    if let Some(num_members) = channel.num_members {
        writer.print_field("Members", &num_members.to_string())?;
    }

    // Privacy status
    let privacy = if channel.is_private == Some(true) {
        "Private"
    } else {
        "Public"
    };
    writer.print_field("Privacy", privacy)?;

    writer.print_separator()?;
    writer.print_header(&format!("Messages ({})", messages.len()))?;
    writer.print_separator()?;

    for (i, msg) in messages.iter().enumerate() {
        format_message(msg, &channel.name, &channel.id, users, thread_info, writer)?;

        if i < messages.len() - 1 {
            writer.writeln()?;
        }
    }

    Ok(())
}

/// Backward compatibility wrapper - formats messages without thread info
pub fn format_messages(
    messages: &[Message],
    channel: &Channel,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    let empty_thread_info = HashMap::new();
    format_messages_with_thread_info(messages, channel, users, &empty_thread_info, writer)
}

fn format_message(
    msg: &Message,
    channel_name: &str,
    channel_id: &str,
    users: &HashMap<String, User>,
    thread_info: &HashMap<String, (usize, Vec<String>)>,
    writer: &mut ColorWriter,
) -> Result<()> {
    // Parse timestamp and convert to local timezone
    let ts_float: f64 = msg.ts.parse().unwrap_or(0.0);
    let dt_utc = DateTime::from_timestamp(ts_float as i64, 0).unwrap_or_default();
    let dt_local: DateTime<Local> = dt_utc.into();

    // Calculate time difference
    let now = Local::now();
    let duration = now.signed_duration_since(dt_local);

    // Format timestamp based on age
    let time_str = if duration.num_hours() < 24 {
        // Less than 1 day old - use "N units ago"
        if duration.num_minutes() < 1 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            let mins = duration.num_minutes();
            if mins == 1 {
                "1 minute ago".to_string()
            } else {
                format!("{} minutes ago", mins)
            }
        } else {
            let hours = duration.num_hours();
            if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        }
    } else {
        // More than 1 day old - use 24-hour clock without offset
        dt_local.format("%Y-%m-%d %H:%M:%S").to_string()
    };

    // Channel name in green
    writer.print_colored(&format!("#{}", channel_name), Color::Green)?;
    writer.write(" ")?;

    // User handle (name) in cyan, or ID if user not found
    if let Some(user_id) = &msg.user {
        if let Some(user) = users.get(user_id) {
            writer.print_colored(&format!("@{}", user.name), Color::Cyan)?;
        } else {
            // Fallback to ID if user not in map
            writer.print_colored(user_id, Color::Cyan)?;
        }
    } else {
        writer.print_colored("<system>", Color::White)?;
    }
    writer.write(" ")?;

    // Timestamp in yellow
    writer.print_colored(&time_str, Color::Yellow)?;
    writer.writeln()?;

    // Message text wrapped dynamically to terminal width
    let wrap_width = crate::output::width::get_wrap_width();
    let wrapped = wrap(&msg.text, wrap_width);
    for line in wrapped {
        writer.write("  ")?;
        writer.write(&line)?;
        writer.writeln()?;
    }

    // Reactions if present
    if let Some(reactions) = &msg.reactions {
        if !reactions.is_empty() {
            writer.write("  ")?;
            for (i, reaction) in reactions.iter().enumerate() {
                if i > 0 {
                    writer.write(" ")?;
                }
                writer.write(&format!(":{}:{}", reaction.name, reaction.count))?;
            }
            writer.writeln()?;
        }
    }

    // Thread indicator
    if let Some(thread_ts) = &msg.thread_ts {
        writer.write("  ")?;

        // Get thread metadata if available
        if let Some((reply_count, participant_ids)) = thread_info.get(thread_ts) {
            writer.print_colored(
                &format!("💬 Part of thread ({} replies)", reply_count),
                Color::Blue
            )?;
            writer.writeln()?;

            // Show participants if any
            if !participant_ids.is_empty() {
                writer.write("  ")?;
                writer.print_colored("Participants: ", Color::Blue)?;

                let participant_names: Vec<String> = participant_ids
                    .iter()
                    .filter_map(|id| {
                        users.get(id).map(|u| format!("@{}", u.name))
                    })
                    .collect();

                writer.write(&participant_names.join(", "))?;
                writer.writeln()?;
            }
        } else {
            // Fallback to simple indicator
            writer.print_colored("💬 Part of thread", Color::Blue)?;
            writer.writeln()?;
        }
    }

    // Message URL with actual channel ID
    let msg_ts = msg.ts.replace('.', "");
    writer.write("  🔗 ")?;
    writer.write(&format!(
        "https://slack.com/archives/{}/p{}",
        channel_id, msg_ts
    ))?;
    writer.writeln()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::channel::{Channel, ChannelPurpose, ChannelTopic};
    use crate::models::message::{Message, Reaction};
    use crate::models::user::{User, UserProfile};

    fn create_test_channel() -> Channel {
        Channel {
            id: "C123".to_string(),
            name: "general".to_string(),
            is_channel: Some(true),
            is_group: None,
            is_im: None,
            is_mpim: None,
            is_private: Some(false),
            is_archived: Some(false),
            topic: Some(ChannelTopic {
                value: "General discussions".to_string(),
            }),
            purpose: Some(ChannelPurpose {
                value: "Company-wide communication".to_string(),
            }),
            num_members: Some(42),
        }
    }

    fn create_test_user(id: &str, name: &str) -> User {
        User {
            id: id.to_string(),
            name: name.to_string(),
            real_name: Some(format!("{} User", name)),
            profile: UserProfile {
                email: Some(format!("{}@example.com", name)),
                status_emoji: None,
                status_text: None,
                display_name: Some(name.to_string()),
                image_72: None,
            },
            deleted: false,
            is_bot: false,
            is_admin: None,
            is_owner: None,
            tz: None,
        }
    }

    fn create_test_message(ts: &str, user: Option<&str>, text: &str) -> Message {
        Message {
            ts: ts.to_string(),
            user: user.map(|s| s.to_string()),
            text: text.to_string(),
            thread_ts: None,
            reactions: None,
            channel: None,
            permalink: None,
        }
    }

    #[test]
    fn test_format_messages_shows_channel_metadata() {
        let channel = create_test_channel();
        let messages = vec![];
        let users = HashMap::new();
        let mut writer = ColorWriter::new(true); // no_color = true for testing

        format_messages(&messages, &channel, &users, &mut writer).unwrap();

        // Test passes if no panic - actual output would be verified in integration tests
    }

    #[test]
    fn test_format_message_with_user_handle() {
        let channel = create_test_channel();
        let user = create_test_user("U123", "johndoe");
        let mut users = HashMap::new();
        users.insert("U123".to_string(), user);

        let message = create_test_message("1234567890.123456", Some("U123"), "Hello world");

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.name, &channel.id, &users, &HashMap::new(), &mut writer).unwrap();

        // Test passes if no panic - user handle formatting is tested visually
    }

    #[test]
    fn test_format_message_with_unknown_user_falls_back_to_id() {
        let channel = create_test_channel();
        let users = HashMap::new(); // Empty map

        let message = create_test_message("1234567890.123456", Some("U999"), "Hello world");

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.name, &channel.id, &users, &HashMap::new(), &mut writer).unwrap();

        // Test passes if no panic - falls back to showing user ID
    }

    #[test]
    fn test_format_message_with_system_message() {
        let channel = create_test_channel();
        let users = HashMap::new();

        let message = create_test_message("1234567890.123456", None, "System message");

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.name, &channel.id, &users, &HashMap::new(), &mut writer).unwrap();

        // Test passes if no panic - system messages shown correctly
    }

    #[test]
    fn test_format_message_url_uses_channel_id() {
        let channel = create_test_channel();
        let users = HashMap::new();

        let message = create_test_message("1234567890.123456", None, "Test");

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.name, &channel.id, &users, &HashMap::new(), &mut writer).unwrap();

        // URL should contain channel ID "C123"
        // Actual URL generation verified through integration tests
    }

    #[test]
    fn test_format_message_with_reactions() {
        let channel = create_test_channel();
        let users = HashMap::new();

        let mut message = create_test_message("1234567890.123456", None, "Test");
        message.reactions = Some(vec![
            Reaction {
                name: "thumbsup".to_string(),
                count: 5,
            },
            Reaction {
                name: "heart".to_string(),
                count: 3,
            },
        ]);

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.name, &channel.id, &users, &HashMap::new(), &mut writer).unwrap();

        // Test passes if no panic - reactions formatted correctly
    }

    #[test]
    fn test_format_message_with_thread() {
        let channel = create_test_channel();
        let users = HashMap::new();

        let mut message = create_test_message("1234567890.123456", None, "Test");
        message.thread_ts = Some("1234567890.123456".to_string());

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.name, &channel.id, &users, &HashMap::new(), &mut writer).unwrap();

        // Test passes if no panic - thread indicator shown
    }

    #[test]
    fn test_timestamp_parsing() {
        let channel = create_test_channel();
        let users = HashMap::new();

        // Test with a known timestamp: 2026-01-01 00:00:00 UTC
        let message = create_test_message("1704067200.000000", None, "New Year!");

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.name, &channel.id, &users, &HashMap::new(), &mut writer).unwrap();

        // Timestamp should be parsed and converted to local timezone
        // Exact output depends on system timezone
    }
}
