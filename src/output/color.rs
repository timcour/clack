use std::io::{self, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub struct ColorWriter {
    stdout: StandardStream,
    colors_enabled: bool,
}

impl ColorWriter {
    pub fn new(no_color: bool) -> Self {
        let colors_enabled = !no_color && std::env::var("NO_COLOR").is_err();
        let choice = if colors_enabled {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        };

        Self {
            stdout: StandardStream::stdout(choice),
            colors_enabled,
        }
    }

    /// Print text in a specific color
    pub fn print_colored(&mut self, text: &str, color: Color) -> io::Result<()> {
        self.stdout.set_color(ColorSpec::new().set_fg(Some(color)))?;
        write!(self.stdout, "{}", text)?;
        self.stdout.reset()?;
        Ok(())
    }

    /// Print bold text
    pub fn print_bold(&mut self, text: &str) -> io::Result<()> {
        self.stdout.set_color(ColorSpec::new().set_bold(true))?;
        write!(self.stdout, "{}", text)?;
        self.stdout.reset()?;
        Ok(())
    }

    /// Print a header (bold + color)
    pub fn print_header(&mut self, text: &str) -> io::Result<()> {
        self.stdout
            .set_color(ColorSpec::new().set_fg(Some(Color::Cyan)).set_bold(true))?;
        writeln!(self.stdout, "{}", text)?;
        self.stdout.reset()?;
        Ok(())
    }

    /// Print a label: value pair
    pub fn print_field(&mut self, label: &str, value: &str) -> io::Result<()> {
        self.print_colored(label, Color::Blue)?;
        write!(self.stdout, ": ")?;
        writeln!(self.stdout, "{}", value)?;
        Ok(())
    }

    /// Print success message
    pub fn print_success(&mut self, text: &str) -> io::Result<()> {
        self.print_colored("✓ ", Color::Green)?;
        writeln!(self.stdout, "{}", text)?;
        Ok(())
    }

    /// Print error message
    pub fn print_error(&mut self, text: &str) -> io::Result<()> {
        self.print_colored("✗ ", Color::Red)?;
        writeln!(self.stdout, "{}", text)?;
        Ok(())
    }

    /// Print separator line
    pub fn print_separator(&mut self) -> io::Result<()> {
        self.print_colored(&"─".repeat(80), Color::White)?;
        writeln!(self.stdout)?;
        Ok(())
    }

    /// Print regular text (no color)
    pub fn print(&mut self, text: &str) -> io::Result<()> {
        writeln!(self.stdout, "{}", text)
    }

    /// Write text without newline
    pub fn write(&mut self, text: &str) -> io::Result<()> {
        write!(self.stdout, "{}", text)
    }

    /// Write newline
    pub fn writeln(&mut self) -> io::Result<()> {
        writeln!(self.stdout)
    }
}
