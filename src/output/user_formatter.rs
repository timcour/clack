use crate::models::user::User;
use crate::output::color::ColorWriter;
use std::io::Result;
use termcolor::Color;

pub fn format_user(user: &User, writer: &mut ColorWriter) -> Result<()> {
    writer.print_header(&format!("User: {}", user.name))?;
    writer.print_separator()?;

    // Basic info
    writer.print_field("User ID", &user.id)?;

    if let Some(real_name) = &user.real_name {
        writer.print_field("Real Name", real_name)?;
    }

    if let Some(display_name) = &user.profile.display_name {
        if !display_name.is_empty() {
            writer.print_field("Display Name", display_name)?;
        }
    }

    // Contact info
    if let Some(email) = &user.profile.email {
        writer.print_field("Email", email)?;
    }

    // Status
    if let Some(status_emoji) = &user.profile.status_emoji {
        let status_text = user.profile.status_text.as_deref().unwrap_or("");
        writer.print_field("Status", &format!("{} {}", status_emoji, status_text))?;
    }

    // Metadata
    if let Some(tz) = &user.tz {
        writer.print_field("Timezone", tz)?;
    }

    // Flags
    let mut flags = Vec::new();
    if user.is_bot {
        flags.push("Bot");
    }
    if user.is_admin == Some(true) {
        flags.push("Admin");
    }
    if user.is_owner == Some(true) {
        flags.push("Owner");
    }
    if user.deleted {
        flags.push("Deleted");
    }
    if !flags.is_empty() {
        writer.print_field("Flags", &flags.join(", "))?;
    }

    // Profile URL - note: team ID would need to be fetched separately in real implementation
    let profile_url = format!("https://slack.com/app_redirect?channel={}", user.id);
    writer.print_field("Profile URL", &profile_url)?;

    Ok(())
}

pub fn format_users_list(users: &[User], writer: &mut ColorWriter) -> Result<()> {
    writer.print_header(&format!("Users ({})", users.len()))?;
    writer.print_separator()?;

    for (i, user) in users.iter().enumerate() {
        // ID and name
        writer.print_colored(&user.id, Color::Yellow)?;
        writer.write(" ")?;
        writer.print_bold(&user.name)?;

        // Real name in parentheses
        if let Some(real_name) = &user.real_name {
            writer.write(&format!(" ({})", real_name))?;
        }

        // Status emoji if present
        if let Some(emoji) = &user.profile.status_emoji {
            writer.write(&format!(" {}", emoji))?;
        }

        writer.writeln()?;

        // Email on second line if present
        if let Some(email) = &user.profile.email {
            writer.write("  ")?;
            writer.print_colored("✉", Color::Blue)?;
            writer.write(&format!(" {}", email))?;
            writer.writeln()?;
        }

        // Add spacing between users
        if i < users.len() - 1 {
            writer.writeln()?;
        }
    }

    Ok(())
}
