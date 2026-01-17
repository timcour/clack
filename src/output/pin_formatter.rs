use crate::models::pin::PinItem;
use crate::output::color::ColorWriter;
use std::io::Result;
use termcolor::Color;

pub fn format_pins_list(pins: &[PinItem], writer: &mut ColorWriter) -> Result<()> {
    writer.print_header(&format!("Pinned Items ({})", pins.len()))?;
    writer.print_separator()?;

    if pins.is_empty() {
        writer.write("No pinned items in this channel")?;
        writer.writeln()?;
        return Ok(());
    }

    for (i, pin) in pins.iter().enumerate() {
        // Pin type
        writer.print_colored("📌 ", Color::Yellow)?;
        writer.print_colored(&format!("{}", pin.pin_type), Color::Cyan)?;
        writer.writeln()?;

        // Pinned by and when
        writer.write("  ")?;
        writer.print_colored("Pinned by: ", Color::Blue)?;
        writer.write(&pin.created_by)?;
        writer.write(" on ")?;
        let datetime = chrono::DateTime::from_timestamp(pin.created as i64, 0)
            .unwrap_or_else(|| chrono::Utc::now());
        writer.write(&datetime.format("%Y-%m-%d %H:%M:%S").to_string())?;
        writer.writeln()?;

        // Message content if available
        if let Some(ref message) = pin.message {
            writer.write("  ")?;
            writer.print_colored("Message: ", Color::Blue)?;
            writer.write(&message.text)?;
            writer.writeln()?;

            if let Some(ref ts) = Some(&message.ts) {
                writer.write("  ")?;
                writer.print_colored("Timestamp: ", Color::Blue)?;
                writer.write(ts)?;
                writer.writeln()?;
            }
        }

        // Add spacing between pins
        if i < pins.len() - 1 {
            writer.writeln()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::pin::PinItem;

    #[test]
    fn test_format_empty_pins_list() {
        let pins: Vec<PinItem> = vec![];
        let mut writer = ColorWriter::new(true);
        format_pins_list(&pins, &mut writer).unwrap();
    }
}
