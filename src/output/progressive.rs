use anyhow::Result;
use std::io::{self, Write};

/// Progressive output helper that writes immediately to stdout
/// bypassing the pager for incremental display.
///
/// Used for commands like `conversations history`, `users list`, etc.
/// where we want to show results as they stream in from the API.
#[allow(dead_code)]
pub struct ProgressiveOutput {
    items_written: usize,
}

#[allow(dead_code)]
impl ProgressiveOutput {
    pub fn new() -> Self {
        Self { items_written: 0 }
    }

    /// Write formatted output immediately to stdout
    pub fn write(&mut self, output: &str) -> Result<()> {
        print!("{}", output);
        io::stdout().flush()?;
        self.items_written += 1;
        Ok(())
    }

    /// Write a line immediately to stdout
    pub fn writeln(&mut self, output: &str) -> Result<()> {
        println!("{}", output);
        io::stdout().flush()?;
        Ok(())
    }

    /// Write just a newline
    pub fn newline(&mut self) -> Result<()> {
        println!();
        io::stdout().flush()?;
        Ok(())
    }

    /// Get the number of items written
    pub fn items_written(&self) -> usize {
        self.items_written
    }
}

impl Default for ProgressiveOutput {
    fn default() -> Self {
        Self::new()
    }
}
