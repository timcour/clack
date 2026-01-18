use std::io::{self, Write};
use termcolor::{Buffer, Color, ColorSpec, WriteColor};

pub struct ColorWriter {
    buffer: Buffer,
    no_color: bool,
}

impl ColorWriter {
    pub fn new(no_color: bool) -> Self {
        let colors_enabled = !no_color && std::env::var("NO_COLOR").is_err();

        Self {
            buffer: Buffer::ansi(), // Use ANSI buffer for color codes
            no_color: !colors_enabled,
        }
    }

    /// Get the buffer contents as a string
    pub fn into_string(self) -> Result<String, std::io::Error> {
        String::from_utf8(self.buffer.into_inner())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Print text in a specific color
    pub fn print_colored(&mut self, text: &str, color: Color) -> io::Result<()> {
        if !self.no_color {
            let mut spec = ColorSpec::new();
            spec.set_fg(Some(color));
            self.buffer.set_color(&spec)?;
        }
        write!(self.buffer, "{}", text)?;
        if !self.no_color {
            self.buffer.reset()?;
        }
        Ok(())
    }

    /// Print bold text
    pub fn print_bold(&mut self, text: &str) -> io::Result<()> {
        if !self.no_color {
            let mut spec = ColorSpec::new();
            spec.set_bold(true);
            self.buffer.set_color(&spec)?;
        }
        write!(self.buffer, "{}", text)?;
        if !self.no_color {
            self.buffer.reset()?;
        }
        Ok(())
    }

    /// Print a header (bold + color)
    pub fn print_header(&mut self, text: &str) -> io::Result<()> {
        if !self.no_color {
            let mut spec = ColorSpec::new();
            spec.set_fg(Some(Color::Cyan)).set_bold(true);
            self.buffer.set_color(&spec)?;
        }
        writeln!(self.buffer, "{}", text)?;
        if !self.no_color {
            self.buffer.reset()?;
        }
        Ok(())
    }

    /// Print a label: value pair
    pub fn print_field(&mut self, label: &str, value: &str) -> io::Result<()> {
        self.print_colored(label, Color::Blue)?;
        write!(self.buffer, ": ")?;
        writeln!(self.buffer, "{}", value)?;
        Ok(())
    }

    /// Print error message
    pub fn print_error(&mut self, text: &str) -> io::Result<()> {
        self.print_colored("✗ ", Color::Red)?;
        writeln!(self.buffer, "{}", text)?;
        Ok(())
    }

    /// Print separator line
    pub fn print_separator(&mut self) -> io::Result<()> {
        self.print_colored(&"─".repeat(80), Color::White)?;
        writeln!(self.buffer)?;
        Ok(())
    }

    /// Write text without newline
    pub fn write(&mut self, text: &str) -> io::Result<()> {
        write!(self.buffer, "{}", text)
    }

    /// Write newline
    pub fn writeln(&mut self) -> io::Result<()> {
        writeln!(self.buffer)
    }
}
