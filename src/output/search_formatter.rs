use crate::models::channel::Channel;
use crate::models::message::Message;
use crate::models::search::{FileResult, SearchAllResponse, SearchFilesResponse, SearchMessagesResponse, SearchPagination};
use crate::models::user::User;
use crate::output::color::ColorWriter;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::io::Result;
use termcolor::Color;
use textwrap::wrap;

/// Format pagination info in a standard way
fn format_pagination(pagination: &SearchPagination, writer: &mut ColorWriter) -> Result<()> {
    writer.writeln()?;
    writer.print_colored(
        &format!(
            "Page {} of {} ({}-{} of {} results)",
            pagination.page,
            pagination.page_count,
            pagination.first,
            pagination.last,
            pagination.total_count
        ),
        Color::White,
    )?;
    writer.writeln()?;
    Ok(())
}

pub fn format_search_messages(
    response: &SearchMessagesResponse,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    writer.print_header(&format!(
        "Found {} message{} matching '{}'",
        response.messages.total,
        if response.messages.total == 1 { "" } else { "s" },
        response.query
    ))?;
    writer.print_separator()?;

    for (i, msg) in response.messages.matches.iter().enumerate() {
        format_search_message(msg, users, writer)?;

        if i < response.messages.matches.len() - 1 {
            writer.writeln()?;
        }
    }

    // Display pagination info if available
    if let Some(ref pagination) = response.messages.pagination {
        format_pagination(pagination, writer)?;
    }

    Ok(())
}

pub fn format_search_message(
    msg: &Message,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    // Parse timestamp and convert to local timezone (same as message_formatter)
    let ts_float: f64 = msg.ts.parse().unwrap_or(0.0);
    let dt_utc = DateTime::from_timestamp(ts_float as i64, 0).unwrap_or_default();
    let dt_local: DateTime<Local> = dt_utc.into();

    // Calculate time difference
    let now = Local::now();
    let duration = now.signed_duration_since(dt_local);

    // Format timestamp based on age (same logic as message_formatter)
    let time_str = if duration.num_hours() < 24 {
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
        dt_local.format("%Y-%m-%d %H:%M:%S").to_string()
    };

    // Channel name in green (if available)
    if let Some(channel) = &msg.channel {
        if let Some(name) = channel.name() {
            writer.print_colored(&format!("#{}", name), Color::Green)?;
        } else {
            writer.print_colored(&format!("#{}", channel.id()), Color::Green)?;
        }
        writer.write(" ")?;
    }

    // User handle (name) in cyan, or ID if user not found
    if let Some(user_id) = &msg.user {
        if let Some(user) = users.get(user_id) {
            writer.print_colored(&format!("@{}", user.name), Color::Cyan)?;
        } else {
            writer.print_colored(user_id, Color::Cyan)?;
        }
    } else {
        writer.print_colored("<system>", Color::White)?;
    }
    writer.write(" ")?;

    // Timestamp in yellow
    writer.print_colored(&time_str, Color::Yellow)?;
    writer.writeln()?;

    // Message text wrapped dynamically
    let wrap_width = crate::output::width::get_wrap_width();
    let wrapped = wrap(&msg.text, wrap_width);
    for line in wrapped {
        writer.write("  ")?;
        writer.write(&line)?;
        writer.writeln()?;
    }

    // Permalink if available
    if let Some(permalink) = &msg.permalink {
        writer.write("  🔗 ")?;
        writer.write(permalink)?;
        writer.writeln()?;
    }

    Ok(())
}

pub fn format_search_files(
    response: &SearchFilesResponse,
    writer: &mut ColorWriter,
) -> Result<()> {
    writer.print_header(&format!(
        "Found {} file{} matching '{}'",
        response.files.total,
        if response.files.total == 1 { "" } else { "s" },
        response.query
    ))?;

    if response.files.matches.is_empty() {
        return Ok(());
    }

    writer.print_separator()?;

    for (i, file) in response.files.matches.iter().enumerate() {
        format_file(file, writer)?;

        if i < response.files.matches.len() - 1 {
            writer.writeln()?;
        }
    }

    // Display pagination info if available
    if let Some(ref pagination) = response.files.pagination {
        format_pagination(pagination, writer)?;
    }

    Ok(())
}

pub fn format_search_all(
    response: &SearchAllResponse,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    writer.print_header(&format!("Search results for '{}'", response.query))?;
    writer.print_separator()?;

    // Messages section
    if response.messages.total > 0 {
        writer.print_colored(
            &format!(
                "{} Message{}:",
                response.messages.total,
                if response.messages.total == 1 { "" } else { "s" }
            ),
            Color::Yellow,
        )?;
        writer.writeln()?;
        writer.print_separator()?;

        for (i, msg) in response.messages.matches.iter().enumerate() {
            format_search_message(msg, users, writer)?;

            if i < response.messages.matches.len() - 1 {
                writer.writeln()?;
            }
        }

        // Display messages pagination
        if let Some(ref pagination) = response.messages.pagination {
            format_pagination(pagination, writer)?;
        }
    }

    // Files section
    if response.files.total > 0 {
        if response.messages.total > 0 {
            writer.writeln()?;
            writer.print_separator()?;
        }

        writer.print_colored(
            &format!(
                "{} File{}:",
                response.files.total,
                if response.files.total == 1 { "" } else { "s" }
            ),
            Color::Yellow,
        )?;
        writer.writeln()?;
        writer.print_separator()?;

        for (i, file) in response.files.matches.iter().enumerate() {
            format_file(file, writer)?;

            if i < response.files.matches.len() - 1 {
                writer.writeln()?;
            }
        }

        // Display files pagination
        if let Some(ref pagination) = response.files.pagination {
            format_pagination(pagination, writer)?;
        }
    }

    if response.messages.total == 0 && response.files.total == 0 {
        writer.writeln()?;
        writer.write("No results found.")?;
        writer.writeln()?;
    }

    Ok(())
}

fn format_file(file: &FileResult, writer: &mut ColorWriter) -> Result<()> {
    // File name and type
    writer.print_colored(&file.name, Color::Green)?;
    writer.write(" ")?;
    writer.print_colored(&format!("({})", file.pretty_type), Color::Magenta)?;

    // Size
    let size_kb = file.size as f64 / 1024.0;
    if size_kb < 1024.0 {
        writer.write(&format!(" - {:.1} KB", size_kb))?;
    } else {
        writer.write(&format!(" - {:.1} MB", size_kb / 1024.0))?;
    }
    writer.writeln()?;

    // Title (if different from name)
    if file.title != file.name && !file.title.is_empty() {
        writer.write("  Title: ")?;
        writer.write(&file.title)?;
        writer.writeln()?;
    }

    // User
    writer.write("  Uploaded by: ")?;
    writer.print_colored(&format!("@{}", file.user), Color::Green)?;

    // Timestamp
    let datetime = chrono::DateTime::from_timestamp(file.timestamp as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| file.timestamp.to_string());
    writer.write(&format!(" on {}", datetime))?;
    writer.writeln()?;

    // Permalink
    if let Some(permalink) = &file.permalink {
        writer.write("  🔗 ")?;
        writer.write(permalink)?;
        writer.writeln()?;
    }

    Ok(())
}

pub fn format_channel_search_results(
    query: &str,
    channels: &[Channel],
    writer: &mut ColorWriter,
) -> Result<()> {
    writer.print_header(&format!(
        "Found {} channel{} matching '{}'",
        channels.len(),
        if channels.len() == 1 { "" } else { "s" },
        query
    ))?;

    if channels.is_empty() {
        return Ok(());
    }

    writer.print_separator()?;

    for (i, channel) in channels.iter().enumerate() {
        // Channel name with #
        writer.print_colored(&format!("#{}", channel.name), Color::Yellow)?;
        writer.write(" ")?;

        // Channel ID
        writer.print_colored(&format!("({})", channel.id), Color::Blue)?;

        // Privacy indicator
        if channel.is_private.unwrap_or(false) {
            writer.write(" 🔒")?;
        }

        // Archived indicator
        if channel.is_archived.unwrap_or(false) {
            writer.write(" 📦")?;
        }
        writer.writeln()?;

        // Topic
        if let Some(ref topic) = channel.topic {
            if !topic.value.is_empty() {
                writer.write("  Topic: ")?;
                writer.write(&topic.value)?;
                writer.writeln()?;
            }
        }

        // Purpose
        if let Some(ref purpose) = channel.purpose {
            if !purpose.value.is_empty() {
                writer.write("  Purpose: ")?;
                writer.write(&purpose.value)?;
                writer.writeln()?;
            }
        }

        // Member count
        if let Some(num_members) = channel.num_members {
            writer.print_colored(&format!("  Members: {}", num_members), Color::Green)?;
            writer.writeln()?;
        }

        if i < channels.len() - 1 {
            writer.writeln()?;
        }
    }

    Ok(())
}
