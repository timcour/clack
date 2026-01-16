use crate::models::channel::Channel;
use crate::models::search::{FileResult, SearchAllResponse, SearchFilesResponse, SearchMessagesResponse};
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub fn format_search_messages(response: &SearchMessagesResponse, no_color: bool) -> std::io::Result<()> {
    let color_choice = if no_color {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };
    let mut stdout = StandardStream::stdout(color_choice);

    // Header
    let mut header_spec = ColorSpec::new();
    header_spec.set_bold(true).set_fg(Some(Color::Cyan));
    stdout.set_color(&header_spec)?;
    writeln!(
        stdout,
        "\nFound {} message{} matching '{}'",
        response.messages.total,
        if response.messages.total == 1 { "" } else { "s" },
        response.query
    )?;
    stdout.reset()?;

    if response.messages.matches.is_empty() {
        return Ok(());
    }

    writeln!(stdout)?;

    // Display each message
    for msg in &response.messages.matches {
        // Timestamp
        let mut ts_spec = ColorSpec::new();
        ts_spec.set_fg(Some(Color::Blue));
        stdout.set_color(&ts_spec)?;
        write!(stdout, "{}", msg.ts)?;
        stdout.reset()?;

        // User
        if let Some(ref user) = msg.user {
            let mut user_spec = ColorSpec::new();
            user_spec.set_fg(Some(Color::Green)).set_bold(true);
            stdout.set_color(&user_spec)?;
            write!(stdout, " @{}", user)?;
            stdout.reset()?;
        }

        // Channel
        if let Some(ref channel) = msg.channel {
            let mut channel_spec = ColorSpec::new();
            channel_spec.set_fg(Some(Color::Yellow));
            stdout.set_color(&channel_spec)?;
            if let Some(ref name) = channel.name {
                write!(stdout, " #{}", name)?;
            } else {
                write!(stdout, " {}", channel.id)?;
            }
            stdout.reset()?;
        }

        writeln!(stdout)?;

        // Message text
        writeln!(stdout, "  {}", msg.text)?;

        // Permalink
        if let Some(ref permalink) = msg.permalink {
            let mut link_spec = ColorSpec::new();
            link_spec.set_fg(Some(Color::Cyan)).set_dimmed(true);
            stdout.set_color(&link_spec)?;
            writeln!(stdout, "  {}", permalink)?;
            stdout.reset()?;
        }

        writeln!(stdout)?;
    }

    Ok(())
}

pub fn format_search_files(response: &SearchFilesResponse, no_color: bool) -> std::io::Result<()> {
    let color_choice = if no_color {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };
    let mut stdout = StandardStream::stdout(color_choice);

    // Header
    let mut header_spec = ColorSpec::new();
    header_spec.set_bold(true).set_fg(Some(Color::Cyan));
    stdout.set_color(&header_spec)?;
    writeln!(
        stdout,
        "\nFound {} file{} matching '{}'",
        response.files.total,
        if response.files.total == 1 { "" } else { "s" },
        response.query
    )?;
    stdout.reset()?;

    if response.files.matches.is_empty() {
        return Ok(());
    }

    writeln!(stdout)?;

    // Display each file
    for file in &response.files.matches {
        format_file(file, &mut stdout)?;
        writeln!(stdout)?;
    }

    Ok(())
}

pub fn format_search_all(response: &SearchAllResponse, no_color: bool) -> std::io::Result<()> {
    let color_choice = if no_color {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };
    let mut stdout = StandardStream::stdout(color_choice);

    // Header
    let mut header_spec = ColorSpec::new();
    header_spec.set_bold(true).set_fg(Some(Color::Cyan));
    stdout.set_color(&header_spec)?;
    writeln!(
        stdout,
        "\nSearch results for '{}'",
        response.query
    )?;
    stdout.reset()?;

    // Messages section
    if response.messages.total > 0 {
        let mut section_spec = ColorSpec::new();
        section_spec.set_bold(true).set_fg(Some(Color::Yellow));
        stdout.set_color(&section_spec)?;
        writeln!(
            stdout,
            "\n{} Message{}:",
            response.messages.total,
            if response.messages.total == 1 { "" } else { "s" }
        )?;
        stdout.reset()?;

        for msg in &response.messages.matches {
            writeln!(stdout)?;

            // Timestamp
            let mut ts_spec = ColorSpec::new();
            ts_spec.set_fg(Some(Color::Blue));
            stdout.set_color(&ts_spec)?;
            write!(stdout, "{}", msg.ts)?;
            stdout.reset()?;

            // User
            if let Some(ref user) = msg.user {
                let mut user_spec = ColorSpec::new();
                user_spec.set_fg(Some(Color::Green)).set_bold(true);
                stdout.set_color(&user_spec)?;
                write!(stdout, " @{}", user)?;
                stdout.reset()?;
            }

            // Channel
            if let Some(ref channel) = msg.channel {
                let mut channel_spec = ColorSpec::new();
                channel_spec.set_fg(Some(Color::Yellow));
                stdout.set_color(&channel_spec)?;
                if let Some(ref name) = channel.name {
                    write!(stdout, " #{}", name)?;
                } else {
                    write!(stdout, " {}", channel.id)?;
                }
                stdout.reset()?;
            }

            writeln!(stdout)?;

            // Message text
            writeln!(stdout, "  {}", msg.text)?;

            // Permalink
            if let Some(ref permalink) = msg.permalink {
                let mut link_spec = ColorSpec::new();
                link_spec.set_fg(Some(Color::Cyan)).set_dimmed(true);
                stdout.set_color(&link_spec)?;
                writeln!(stdout, "  {}", permalink)?;
                stdout.reset()?;
            }
        }
    }

    // Files section
    if response.files.total > 0 {
        let mut section_spec = ColorSpec::new();
        section_spec.set_bold(true).set_fg(Some(Color::Yellow));
        stdout.set_color(&section_spec)?;
        writeln!(
            stdout,
            "\n{} File{}:",
            response.files.total,
            if response.files.total == 1 { "" } else { "s" }
        )?;
        stdout.reset()?;

        for file in &response.files.matches {
            writeln!(stdout)?;
            format_file(file, &mut stdout)?;
        }
    }

    if response.messages.total == 0 && response.files.total == 0 {
        writeln!(stdout, "\nNo results found.")?;
    }

    writeln!(stdout)?;

    Ok(())
}

fn format_file(file: &FileResult, stdout: &mut StandardStream) -> std::io::Result<()> {
    // File name and type
    let mut name_spec = ColorSpec::new();
    name_spec.set_fg(Some(Color::Green)).set_bold(true);
    stdout.set_color(&name_spec)?;
    write!(stdout, "{}", file.name)?;
    stdout.reset()?;

    let mut type_spec = ColorSpec::new();
    type_spec.set_fg(Some(Color::Magenta));
    stdout.set_color(&type_spec)?;
    write!(stdout, " ({})", file.pretty_type)?;
    stdout.reset()?;

    // Size
    let size_kb = file.size as f64 / 1024.0;
    if size_kb < 1024.0 {
        write!(stdout, " - {:.1} KB", size_kb)?;
    } else {
        write!(stdout, " - {:.1} MB", size_kb / 1024.0)?;
    }

    writeln!(stdout)?;

    // Title (if different from name)
    if file.title != file.name && !file.title.is_empty() {
        writeln!(stdout, "  Title: {}", file.title)?;
    }

    // User
    let mut user_spec = ColorSpec::new();
    user_spec.set_fg(Some(Color::Green));
    stdout.set_color(&user_spec)?;
    write!(stdout, "  Uploaded by: @{}", file.user)?;
    stdout.reset()?;

    // Timestamp
    let datetime = chrono::DateTime::from_timestamp(file.timestamp as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| file.timestamp.to_string());
    write!(stdout, " on {}", datetime)?;
    writeln!(stdout)?;

    // Permalink
    if let Some(ref permalink) = file.permalink {
        let mut link_spec = ColorSpec::new();
        link_spec.set_fg(Some(Color::Cyan)).set_dimmed(true);
        stdout.set_color(&link_spec)?;
        writeln!(stdout, "  {}", permalink)?;
        stdout.reset()?;
    }

    Ok(())
}

pub fn format_channel_search_results(
    query: &str,
    channels: &[Channel],
    no_color: bool,
) -> std::io::Result<()> {
    let color_choice = if no_color {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };
    let mut stdout = StandardStream::stdout(color_choice);

    // Header
    let mut header_spec = ColorSpec::new();
    header_spec.set_bold(true).set_fg(Some(Color::Cyan));
    stdout.set_color(&header_spec)?;
    writeln!(
        stdout,
        "\nFound {} channel{} matching '{}'",
        channels.len(),
        if channels.len() == 1 { "" } else { "s" },
        query
    )?;
    stdout.reset()?;

    if channels.is_empty() {
        return Ok(());
    }

    writeln!(stdout)?;

    // Display each channel
    for channel in channels {
        // Channel name with #
        let mut name_spec = ColorSpec::new();
        name_spec.set_fg(Some(Color::Yellow)).set_bold(true);
        stdout.set_color(&name_spec)?;
        write!(stdout, "#{}", channel.name)?;
        stdout.reset()?;

        // Channel ID
        let mut id_spec = ColorSpec::new();
        id_spec.set_fg(Some(Color::Blue)).set_dimmed(true);
        stdout.set_color(&id_spec)?;
        write!(stdout, " ({})", channel.id)?;
        stdout.reset()?;

        // Privacy indicator
        if channel.is_private.unwrap_or(false) {
            write!(stdout, " 🔒")?;
        }

        // Archived indicator
        if channel.is_archived.unwrap_or(false) {
            write!(stdout, " 📦")?;
        }

        writeln!(stdout)?;

        // Topic
        if let Some(ref topic) = channel.topic {
            if !topic.value.is_empty() {
                let mut topic_spec = ColorSpec::new();
                topic_spec.set_fg(Some(Color::White));
                stdout.set_color(&topic_spec)?;
                writeln!(stdout, "  Topic: {}", topic.value)?;
                stdout.reset()?;
            }
        }

        // Purpose
        if let Some(ref purpose) = channel.purpose {
            if !purpose.value.is_empty() {
                let mut purpose_spec = ColorSpec::new();
                purpose_spec.set_fg(Some(Color::White));
                stdout.set_color(&purpose_spec)?;
                writeln!(stdout, "  Purpose: {}", purpose.value)?;
                stdout.reset()?;
            }
        }

        // Member count
        if let Some(num_members) = channel.num_members {
            let mut members_spec = ColorSpec::new();
            members_spec.set_fg(Some(Color::Green));
            stdout.set_color(&members_spec)?;
            writeln!(stdout, "  Members: {}", num_members)?;
            stdout.reset()?;
        }

        writeln!(stdout)?;
    }

    Ok(())
}
