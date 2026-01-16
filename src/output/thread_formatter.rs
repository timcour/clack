use crate::models::channel::Channel;
use crate::models::message::Message;
use crate::models::user::User;
use crate::output::color::ColorWriter;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::io::Result;
use termcolor::Color;
use textwrap::wrap;

pub fn format_thread(
    messages: &[Message],
    channel: &Channel,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    if messages.is_empty() {
        writer.print_error("Thread not found or empty")?;
        return Ok(());
    }

    // Get root message
    let root = &messages[0];
    let thread_ts = root.thread_ts.as_ref().unwrap_or(&root.ts);

    // Thread header
    writer.print_header(&format!(
        "Thread in #{} ({} messages)",
        channel.name,
        messages.len()
    ))?;
    writer.print_separator()?;

    // Format root message
    writer.print_colored("ROOT MESSAGE", Color::Green)?;
    writer.writeln()?;
    writer.print_separator()?;
    format_message(root, &channel.id, users, writer, false)?;

    // Format replies if there are any
    if messages.len() > 1 {
        writer.writeln()?;
        writer.print_colored(
            &format!("REPLIES ({})", messages.len() - 1),
            Color::Green,
        )?;
        writer.writeln()?;
        writer.print_separator()?;

        for (i, msg) in messages.iter().skip(1).enumerate() {
            format_message(msg, &channel.id, users, writer, true)?;

            if i < messages.len() - 2 {
                writer.writeln()?;
            }
        }
    }

    // Thread URL
    writer.writeln()?;
    writer.print_separator()?;
    let thread_ts_clean = thread_ts.replace('.', "");
    writer.write("🔗 Thread URL: ")?;
    writer.write(&format!(
        "https://slack.com/archives/{}/p{}",
        channel.id, thread_ts_clean
    ))?;
    writer.writeln()?;

    Ok(())
}

fn format_message(
    msg: &Message,
    channel_id: &str,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
    is_reply: bool,
) -> Result<()> {
    // Indent for replies
    let indent = if is_reply { "  " } else { "" };

    // Parse timestamp and convert to local timezone
    let ts_float: f64 = msg.ts.parse().unwrap_or(0.0);
    let dt_utc = DateTime::from_timestamp(ts_float as i64, 0).unwrap_or_default();
    let dt_local: DateTime<Local> = dt_utc.into();
    let time_str = dt_local.format("%Y-%m-%d %H:%M:%S %Z").to_string();

    // Timestamp in yellow
    writer.write(indent)?;
    writer.print_colored(&time_str, Color::Yellow)?;
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
    writer.writeln()?;

    // Message text wrapped to appropriate width (accounting for indent)
    let wrap_width = if is_reply { 74 } else { 78 };
    let text_indent = format!("{}  ", indent);
    let wrapped = wrap(&msg.text, wrap_width);
    for line in wrapped {
        writer.write(&text_indent)?;
        writer.write(&line)?;
        writer.writeln()?;
    }

    // Reactions if present
    if let Some(reactions) = &msg.reactions {
        if !reactions.is_empty() {
            writer.write(&text_indent)?;
            for (i, reaction) in reactions.iter().enumerate() {
                if i > 0 {
                    writer.write(" ")?;
                }
                writer.write(&format!(":{}:{}", reaction.name, reaction.count))?;
            }
            writer.writeln()?;
        }
    }

    // Message URL
    let msg_ts = msg.ts.replace('.', "");
    writer.write(&text_indent)?;
    writer.write("🔗 ")?;
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

    fn create_test_message(ts: &str, user: Option<&str>, text: &str, thread_ts: Option<&str>) -> Message {
        Message {
            ts: ts.to_string(),
            user: user.map(|s| s.to_string()),
            text: text.to_string(),
            thread_ts: thread_ts.map(|s| s.to_string()),
            reactions: None,
        }
    }

    #[test]
    fn test_format_thread_with_root_and_replies() {
        let channel = create_test_channel();
        let user1 = create_test_user("U123", "alice");
        let user2 = create_test_user("U456", "bob");
        let mut users = HashMap::new();
        users.insert("U123".to_string(), user1);
        users.insert("U456".to_string(), user2);

        let messages = vec![
            create_test_message("1234567890.123456", Some("U123"), "Root message", Some("1234567890.123456")),
            create_test_message("1234567891.123456", Some("U456"), "Reply 1", Some("1234567890.123456")),
            create_test_message("1234567892.123456", Some("U123"), "Reply 2", Some("1234567890.123456")),
        ];

        let mut writer = ColorWriter::new(true); // no_color = true for testing
        format_thread(&messages, &channel, &users, &mut writer).unwrap();

        // Test passes if no panic
    }

    #[test]
    fn test_format_thread_with_only_root() {
        let channel = create_test_channel();
        let user = create_test_user("U123", "alice");
        let mut users = HashMap::new();
        users.insert("U123".to_string(), user);

        let messages = vec![
            create_test_message("1234567890.123456", Some("U123"), "Root message", Some("1234567890.123456")),
        ];

        let mut writer = ColorWriter::new(true);
        format_thread(&messages, &channel, &users, &mut writer).unwrap();

        // Test passes if no panic
    }

    #[test]
    fn test_format_thread_empty() {
        let channel = create_test_channel();
        let users = HashMap::new();
        let messages: Vec<Message> = vec![];

        let mut writer = ColorWriter::new(true);
        format_thread(&messages, &channel, &users, &mut writer).unwrap();

        // Should handle empty thread gracefully
    }

    #[test]
    fn test_format_message_reply_indentation() {
        let channel = create_test_channel();
        let user = create_test_user("U123", "alice");
        let mut users = HashMap::new();
        users.insert("U123".to_string(), user);

        let message = create_test_message("1234567891.123456", Some("U123"), "This is a reply", Some("1234567890.123456"));

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.id, &users, &mut writer, true).unwrap();

        // Test that reply formatting works (indented)
    }

    #[test]
    fn test_format_message_with_reactions_in_thread() {
        let channel = create_test_channel();
        let users = HashMap::new();

        let mut message = create_test_message("1234567890.123456", None, "Test", Some("1234567890.123456"));
        message.reactions = Some(vec![
            Reaction {
                name: "thumbsup".to_string(),
                count: 5,
            },
        ]);

        let mut writer = ColorWriter::new(true);
        format_message(&message, &channel.id, &users, &mut writer, false).unwrap();

        // Test passes if no panic
    }
}
