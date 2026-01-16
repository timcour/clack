use crate::models::workspace::AuthTestResponse;
use crate::output::color::ColorWriter;
use std::io::Result;
use termcolor::Color;

pub fn format_auth_test(auth: &AuthTestResponse, writer: &mut ColorWriter) -> Result<()> {
    writer.print_header("Workspace Authentication")?;
    writer.print_separator()?;

    // Workspace info
    writer.print_field("Workspace", &auth.team)?;
    writer.print_field("Workspace ID", &auth.team_id)?;
    writer.print_field("Workspace URL", &auth.url)?;

    writer.writeln()?;

    // User info
    writer.print_field("User", &auth.user)?;
    writer.print_field("User ID", &auth.user_id)?;

    // Bot ID if present
    if let Some(bot_id) = &auth.bot_id {
        writer.print_field("Bot ID", bot_id)?;
    }

    // Enterprise install flag if present
    if let Some(is_enterprise) = auth.is_enterprise_install {
        if is_enterprise {
            writer.writeln()?;
            writer.write("  ")?;
            writer.print_colored("Enterprise Install", Color::Cyan)?;
            writer.writeln()?;
        }
    }

    Ok(())
}
