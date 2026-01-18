use anyhow::Result;
use minus::Pager;
use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;

pub enum OutputDestination {
    Pager(Pager),
    Direct(Vec<u8>), // Buffer for direct output
}

impl OutputDestination {
    /// Create a new output destination
    /// - Uses pager if: stdout is TTY AND no_pager=false
    /// - Uses direct output if: stdout is piped OR no_pager=true
    pub fn new(no_pager: bool) -> Result<Self> {
        // Check if stdout is a TTY (not piped)
        let is_tty = atty::is(atty::Stream::Stdout);

        // Check if paging should be disabled
        let should_page = !no_pager && is_tty;

        if should_page {
            // Create pager instance
            let pager = Pager::new();
            Ok(OutputDestination::Pager(pager))
        } else {
            // Direct output to stdout
            Ok(OutputDestination::Direct(Vec::new()))
        }
    }

    /// Write a string to the output destination
    pub fn write_str(&mut self, s: &str) -> Result<()> {
        match self {
            OutputDestination::Pager(pager) => {
                writeln!(pager, "{}", s).map_err(|e| anyhow::anyhow!("Pager write error: {}", e))?;
                Ok(())
            }
            OutputDestination::Direct(buffer) => {
                buffer.write_all(s.as_bytes())?;
                if !s.ends_with('\n') {
                    buffer.write_all(b"\n")?;
                }
                Ok(())
            }
        }
    }

    /// Flush and display the output
    pub fn finish(self) -> Result<()> {
        match self {
            OutputDestination::Pager(pager) => {
                // Run the pager - this will block until user exits
                minus::page_all(pager).map_err(|e| anyhow::anyhow!("Pager error: {}", e))?;
                Ok(())
            }
            OutputDestination::Direct(buffer) => {
                // Write directly to stdout
                std::io::stdout().write_all(&buffer)?;
                Ok(())
            }
        }
    }
}
