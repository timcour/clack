use crate::models::channel::Channel;
use crate::output::color::ColorWriter;
use std::io::Result;
use termcolor::Color;

pub fn format_channels_list(channels: &[Channel], writer: &mut ColorWriter) -> Result<()> {
    writer.print_header(&format!("Channels ({})", channels.len()))?;
    writer.print_separator()?;

    // Sort channels by name for easier reading
    let mut sorted_channels = channels.to_vec();
    sorted_channels.sort_by(|a, b| a.name.cmp(&b.name));

    for (i, channel) in sorted_channels.iter().enumerate() {
        // Channel name with # prefix
        writer.print_colored(&format!("#{}", channel.name), Color::Cyan)?;
        writer.write(" ")?;

        // Channel ID in yellow
        writer.print_colored(&format!("({})", channel.id), Color::Yellow)?;

        // Privacy indicator
        if channel.is_private == Some(true) {
            writer.write(" ")?;
            writer.print_colored("🔒 Private", Color::Blue)?;
        }

        // Archived indicator
        if channel.is_archived == Some(true) {
            writer.write(" ")?;
            writer.print_colored("📦 Archived", Color::White)?;
        }

        writer.writeln()?;

        // Topic on second line if present
        if let Some(topic) = &channel.topic {
            if !topic.value.is_empty() {
                writer.write("  ")?;
                writer.print_colored("Topic: ", Color::Blue)?;
                writer.write(&topic.value)?;
                writer.writeln()?;
            }
        }

        // Member count if available
        if let Some(num_members) = channel.num_members {
            writer.write("  ")?;
            writer.print_colored(&format!("{} members", num_members), Color::Green)?;
            writer.writeln()?;
        }

        // Add spacing between channels
        if i < sorted_channels.len() - 1 {
            writer.writeln()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::channel::{Channel, ChannelPurpose, ChannelTopic};

    fn create_test_channel(name: &str, is_private: bool) -> Channel {
        Channel {
            id: format!("C{}", name.to_uppercase()),
            name: name.to_string(),
            is_channel: Some(true),
            is_group: None,
            is_im: None,
            is_mpim: None,
            is_private: Some(is_private),
            is_archived: Some(false),
            topic: Some(ChannelTopic {
                value: format!("{} discussion", name),
            }),
            purpose: Some(ChannelPurpose {
                value: format!("Purpose for {}", name),
            }),
            num_members: Some(42),
        }
    }

    #[test]
    fn test_format_channels_list() {
        let channels = vec![
            create_test_channel("general", false),
            create_test_channel("random", false),
        ];

        let mut writer = ColorWriter::new(true); // no_color = true for testing
        format_channels_list(&channels, &mut writer).unwrap();

        // Test passes if no panic
    }

    #[test]
    fn test_format_empty_channels_list() {
        let channels: Vec<Channel> = vec![];

        let mut writer = ColorWriter::new(true);
        format_channels_list(&channels, &mut writer).unwrap();

        // Test passes if no panic
    }

    #[test]
    fn test_format_private_channel() {
        let channels = vec![create_test_channel("secret", true)];

        let mut writer = ColorWriter::new(true);
        format_channels_list(&channels, &mut writer).unwrap();

        // Should show private indicator
    }
}
