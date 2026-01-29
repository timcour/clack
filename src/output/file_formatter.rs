use crate::models::file::File;
use crate::models::user::User;
use crate::output::color::ColorWriter;
use std::collections::HashMap;
use std::io::Result;
use termcolor::Color;

pub fn format_files_list(files: &[File], users: &HashMap<String, User>, writer: &mut ColorWriter) -> Result<()> {
    writer.print_header(&format!("Files ({})", files.len()))?;
    writer.print_separator()?;

    for (i, file) in files.iter().enumerate() {
        // File name and type
        writer.print_colored(&file.name, Color::Cyan)?;
        writer.write(" ")?;
        writer.print_colored(&format!("({})", file.pretty_type), Color::Yellow)?;
        writer.writeln()?;

        // File ID
        writer.write("  ")?;
        writer.print_colored("ID: ", Color::Blue)?;
        writer.write(&file.id)?;
        writer.writeln()?;

        // Size
        writer.write("  ")?;
        writer.print_colored("Size: ", Color::Blue)?;
        writer.write(&format_size(file.size))?;
        writer.writeln()?;

        // User and timestamp
        writer.write("  ")?;
        writer.print_colored("Uploaded by: ", Color::Blue)?;
        if let Some(user) = users.get(&file.user) {
            writer.write(&format!("@{} ({})", user.name, file.user))?;
        } else {
            writer.write(&file.user)?; // Fallback to ID if user not found
        }
        writer.write(" on ")?;
        let datetime = chrono::DateTime::from_timestamp(file.created as i64, 0)
            .unwrap_or_else(|| chrono::Utc::now());
        writer.write(&datetime.format("%Y-%m-%d %H:%M:%S").to_string())?;
        writer.writeln()?;

        // Permalink
        if let Some(ref permalink) = file.permalink {
            writer.write("  ")?;
            writer.print_colored("Link: ", Color::Blue)?;
            writer.write(permalink)?;
            writer.writeln()?;
        }

        // Add spacing between files
        if i < files.len() - 1 {
            writer.writeln()?;
        }
    }

    Ok(())
}

pub fn format_file(file: &File, users: &HashMap<String, User>, writer: &mut ColorWriter) -> Result<()> {
    format_files_list(&vec![file.clone()], users, writer)
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::file::File;

    fn create_test_file() -> File {
        File {
            id: "F123".to_string(),
            created: 1234567890,
            timestamp: 1234567890,
            name: "test.txt".to_string(),
            title: "Test File".to_string(),
            mimetype: "text/plain".to_string(),
            filetype: "txt".to_string(),
            pretty_type: "Text".to_string(),
            user: "U123".to_string(),
            size: 1024,
            url_private: None,
            url_private_download: None,
            permalink: Some("https://example.slack.com/files/F123".to_string()),
            permalink_public: None,
            is_external: Some(false),
            is_public: Some(false),
            channels: None,
            groups: None,
            ims: None,
        }
    }

    #[test]
    fn test_format_files_list() {
        let files = vec![create_test_file()];
        let users = HashMap::new();
        let mut writer = ColorWriter::new(true);
        format_files_list(&files, &users, &mut writer).unwrap();
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1572864), "1.50 MB");
        assert_eq!(format_size(1610612736), "1.50 GB");
    }
}
