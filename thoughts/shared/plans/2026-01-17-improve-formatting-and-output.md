# Improve Response Formatting and Scrollable Output Implementation Plan

## Overview

This plan improves the clack CLI's human-readable output formatting and adds scrollable output similar to `git log`. The primary goals are:
1. Display human-readable names before IDs in all formatters
2. Add automatic pager support (default ON) for all output formats
3. Dynamically detect terminal width for text wrapping (max 120 chars)
4. Enhance thread indicators with participant information
5. Make search results consistent with message formatting
6. Use shorter URL formats for better readability
7. (Optional) Add theme configuration support

## Current State Analysis

### Key Issues Identified:

1. **Inconsistent ID Display**:
   - User lists show `U1234ABCD johndoe` instead of `@johndoe (U1234ABCD)` (`user_formatter.rs:70`)
   - User details show `User ID: U1234ABCD` without name context (`user_formatter.rs:11`)
   - File uploader shows raw user ID instead of username (`file_formatter.rs:32`)
   - Message fallback shows raw user ID when user not found (`message_formatter.rs:113`)

2. **No Scrollable Output**:
   - All output dumps directly to `StandardStream::stdout()` (`color.rs:18`)
   - No pager library in `Cargo.toml`
   - Users must manually pipe to `less`

3. **Hard-coded Terminal Width**:
   - Messages wrap at 78 chars (`message_formatter.rs:125`)
   - Thread replies wrap at 74 chars (`thread_formatter.rs:141-143`)
   - No dynamic terminal size detection

4. **Limited Thread Context**:
   - Shows only `💬 Part of thread` without participant info (`message_formatter.rs:149`)

5. **Search Results Inconsistency**:
   - Search formatter doesn't match message formatter patterns (`search_formatter.rs`)

6. **Long URLs**:
   - Full URLs displayed: `https://slack.com/archives/C123/p1234567890` (`message_formatter.rs:156-159`)

## Desired End State

After implementing this plan:

1. **All IDs will be human-readable**: `@johndoe (U1234ABCD)`, `#general (C1234ABCD)`
2. **Scrollable output by default**: Using a pager for all formats (human, json, yaml)
3. **Smart pipe detection**: Pager automatically disabled when output is piped
4. **Dynamic text wrapping**: Adapts to terminal width (max 120 chars)
5. **Rich thread context**: Shows reply count and participants
6. **Consistent search formatting**: Matches message formatter patterns
7. **Concise URLs**: `🔗 View message` instead of full URLs
8. **(Optional) Theme support**: Configurable color schemes

### Verification:
- Run `clack conversations history general` - should open in pager with names before IDs
- Run `clack conversations history general | grep foo` - pager should be disabled
- Run `clack users list --no-pager` - should output directly without pager
- All IDs should show human-readable context

## What We're NOT Doing

- We are NOT changing JSON/YAML data structure (only display formatting)
- We are NOT adding interactive features to the pager (just scrolling)
- We are NOT implementing custom pager - using existing system pagers
- We are NOT changing the API data fetching logic
- We are NOT adding pagination to API calls (they already handle that)

## Implementation Approach

We'll implement in phases, with each phase being independently testable. The pager integration is placed early (Phase 2) because it affects all subsequent formatting work, and we want to ensure the pager works correctly before making extensive formatting changes.

---

## Phase 1: ID Formatting - Human-Readable Names

### Overview
Update all formatters to display human-readable names before IDs, making the output more user-friendly while preserving technical IDs for copy-paste purposes.

### Changes Required:

#### 1. User List Formatter
**File**: `src/output/user_formatter.rs`
**Changes**: Update `format_users_list()` to show name before ID

**Current code** (lines 68-72):
```rust
for (i, user) in users.iter().enumerate() {
    // ID and name
    writer.print_colored(&user.id, Color::Yellow)?;
    writer.write(" ")?;
    writer.print_bold(&user.name)?;
```

**New code**:
```rust
for (i, user) in users.iter().enumerate() {
    // Name in bold with @ prefix, then ID in parentheses
    writer.write("@")?;
    writer.print_bold(&user.name)?;
    writer.write(" ")?;
    writer.print_colored(&format!("({})", user.id), Color::Yellow)?;
```

#### 2. User Details Formatter
**File**: `src/output/user_formatter.rs`
**Changes**: Update `format_user()` to show username with ID

**Current code** (line 11):
```rust
writer.print_field("User ID", &user.id)?;
```

**New code**:
```rust
writer.print_field("User ID", &format!("@{} ({})", user.name, user.id))?;
```

#### 3. File Uploader Lookup
**File**: `src/output/file_formatter.rs`
**Changes**: Add user lookup parameter and display username

**Update function signature**:
```rust
pub fn format_files_list(
    files: &[File],
    users: &HashMap<String, User>,
    writer: &mut ColorWriter
) -> Result<()>
```

**Current code** (lines 30-33):
```rust
writer.write("  ")?;
writer.print_colored("Uploaded by: ", Color::Blue)?;
writer.write(&file.user)?;
writer.write(" on ")?;
```

**New code**:
```rust
writer.write("  ")?;
writer.print_colored("Uploaded by: ", Color::Blue)?;
if let Some(user) = users.get(&file.user) {
    writer.write(&format!("@{} ({})", user.name, file.user))?;
} else {
    writer.write(&file.user)?; // Fallback to ID if user not found
}
writer.write(" on ")?;
```

**Update `format_file()` signature**:
```rust
pub fn format_file(file: &File, users: &HashMap<String, User>, writer: &mut ColorWriter) -> Result<()> {
    format_files_list(&vec![file.clone()], users, writer)
}
```

#### 4. Update main.rs to pass user map to file formatters
**File**: `src/main.rs`
**Changes**: Build user map for files list and pass to formatter

**Find the files list command** (around lines 296-303):
```rust
FilesCommands::List { limit, user, channel } => {
    let files = api::files::list_files(&client, limit, user, channel).await?;

    match cli.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&files)?),
        "yaml" => println!("{}", serde_yaml::to_string(&files)?),
        _ => {
            // Build user lookup map
            let mut user_map: std::collections::HashMap<String, models::user::User> =
                std::collections::HashMap::new();

            for file in &files {
                if !user_map.contains_key(&file.user) {
                    if let Ok(user) = api::users::get_user(&client, &file.user).await {
                        user_map.insert(user.id.clone(), user);
                    }
                }
            }

            let mut writer = output::color::ColorWriter::new(cli.no_color);
            output::file_formatter::format_files_list(&files, &user_map, &mut writer)?;
        }
    }
}
```

**Same for FilesCommands::Info** (around lines 308):
```rust
FilesCommands::Info { file_id } => {
    let file = api::files::get_file(&client, &file_id).await?;

    match cli.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&file)?),
        "yaml" => println!("{}", serde_yaml::to_string(&file)?),
        _ => {
            // Build user lookup map for the single file uploader
            let mut user_map: std::collections::HashMap<String, models::user::User> =
                std::collections::HashMap::new();

            if let Ok(user) = api::users::get_user(&client, &file.user).await {
                user_map.insert(user.id.clone(), user);
            }

            let mut writer = output::color::ColorWriter::new(cli.no_color);
            output::file_formatter::format_file(&file, &user_map, &mut writer)?;
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] All tests pass: `make test`
- [ ] Code compiles without errors: `make build`
- [ ] Type checking passes: `cargo check`
- [ ] Linting passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] `clack users list` shows `@username (U1234ABCD)` format
- [ ] `clack users info U123` shows `User ID: @username (U1234ABCD)`
- [ ] `clack files list` shows uploader as `@username (U1234ABCD)`
- [ ] Channel references remain `#general (C1234ABCD)` (already correct)
- [ ] Fallback to raw ID when user not found works correctly

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 2: Pager Integration

### Overview
Add automatic pager support for all output formats (human, json, yaml) with smart pipe detection and manual override flags.

### Changes Required:

#### 1. Add Dependencies
**File**: `Cargo.toml`
**Changes**: Add pager crate

Add to `[dependencies]` section (after line 16):
```toml
minus = { version = "5.5", features = ["static_output", "search"] }
atty = "0.2"
```

**Note**: `minus` is a pure Rust pager library that works cross-platform. `atty` detects if stdout is a TTY.

#### 2. Add CLI Flag
**File**: `src/cli.rs`
**Changes**: Add `--no-pager` global flag

Add after line 17:
```rust
/// Disable pager for scrollable output
#[arg(long, global = true)]
pub no_pager: bool,
```

#### 3. Create Pager Module
**File**: `src/output/pager.rs` (new file)
**Changes**: Create pager wrapper with smart detection

```rust
use anyhow::Result;
use minus::{MinusError, Pager};
use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;

pub enum OutputDestination {
    Pager(Pager),
    Direct(Vec<u8>), // Buffer for direct output
}

impl OutputDestination {
    /// Create a new output destination
    /// - Uses pager if: stdout is TTY AND no_pager=false AND PAGER env var exists or default is available
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
                buffer.write_all(b"\n")?;
                Ok(())
            }
        }
    }

    /// Flush and display the output
    pub fn finish(self) -> Result<()> {
        match self {
            OutputDestination::Pager(pager) => {
                // Run the pager - this will block until user exits
                minus::page_all(pager).map_err(|e| match e {
                    MinusError::HandleNotSet => anyhow::anyhow!("Pager handle not set"),
                    MinusError::InvalidSetup(_) => anyhow::anyhow!("Invalid pager setup"),
                    MinusError::Communication(_) => anyhow::anyhow!("Pager communication error"),
                    MinusError::InvalidTerminal => anyhow::anyhow!("Invalid terminal"),
                    _ => anyhow::anyhow!("Unknown pager error"),
                })?;
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

/// Helper to capture ColorWriter output to a string
pub struct StringWriter {
    buffer: String,
}

impl StringWriter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn write(&mut self, s: &str) -> std::io::Result<()> {
        self.buffer.push_str(s);
        Ok(())
    }

    pub fn into_string(self) -> String {
        self.buffer
    }
}

impl std::io::Write for StringWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = std::str::from_utf8(buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.buffer.push_str(s);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
```

#### 4. Update output/mod.rs
**File**: `src/output/mod.rs`
**Changes**: Export pager module

Add to exports:
```rust
pub mod pager;
```

#### 5. Update main.rs to use pager
**File**: `src/main.rs`
**Changes**: Wrap all output with pager

**Strategy**: Instead of calling formatters directly, we'll:
1. Create output destination (pager or direct)
2. Capture formatter output to string
3. Write to output destination
4. Finish output destination (triggers pager or stdout)

**Add at the top of main() after line 16**:
```rust
// Create output destination (pager or direct output)
let mut output_dest = output::pager::OutputDestination::new(cli.no_pager)?;
```

**For each output block, replace the pattern**:

**OLD PATTERN**:
```rust
match cli.format.as_str() {
    "json" => println!("{}", serde_json::to_string_pretty(&data)?),
    "yaml" => println!("{}", serde_yaml::to_string(&data)?),
    _ => {
        let mut writer = output::color::ColorWriter::new(cli.no_color);
        output::formatter::format_data(&data, &mut writer)?;
    }
}
```

**NEW PATTERN**:
```rust
let output_str = match cli.format.as_str() {
    "json" => serde_json::to_string_pretty(&data)?,
    "yaml" => serde_yaml::to_string(&data)?,
    _ => {
        // Capture formatter output to string
        let mut string_writer = output::pager::StringWriter::new();
        {
            let mut writer = output::color::ColorWriter::new(cli.no_color);
            // Note: We need to update ColorWriter to write to a custom Write impl
            output::formatter::format_data(&data, &mut writer)?;
        }
        string_writer.into_string()
    }
};
output_dest.write_str(&output_str)?;
```

**WAIT - Better approach**: We need to refactor ColorWriter to accept any `Write` implementation instead of hardcoding `StandardStream`. Let me revise:

#### 5. Refactor ColorWriter (revised approach)
**File**: `src/output/color.rs`
**Changes**: Make ColorWriter generic over Write

**Current implementation** uses `StandardStream::stdout()` directly. We need to make it configurable.

**Replace the entire ColorWriter implementation**:
```rust
use std::io::{self, Write};
use termcolor::{Buffer, Color, ColorChoice, ColorSpec, WriteColor};

pub struct ColorWriter {
    buffer: Buffer,
    no_color: bool,
}

impl ColorWriter {
    pub fn new(no_color: bool) -> Self {
        let colors_enabled = !no_color && std::env::var("NO_COLOR").is_err();
        let choice = if colors_enabled {
            ColorChoice::Always // Use Always to preserve colors in buffer
        } else {
            ColorChoice::Never
        };

        Self {
            buffer: Buffer::ansi(), // Use ANSI buffer for color codes
            no_color,
        }
    }

    /// Get the buffer contents as a string
    pub fn into_string(self) -> Result<String, std::io::Error> {
        String::from_utf8(self.buffer.into_inner())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

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

    pub fn print_field(&mut self, label: &str, value: &str) -> io::Result<()> {
        if !self.no_color {
            let mut spec = ColorSpec::new();
            spec.set_fg(Some(Color::Blue));
            self.buffer.set_color(&spec)?;
        }
        write!(self.buffer, "{}: ", label)?;
        if !self.no_color {
            self.buffer.reset()?;
        }
        writeln!(self.buffer, "{}", value)?;
        Ok(())
    }

    pub fn print_error(&mut self, text: &str) -> io::Result<()> {
        if !self.no_color {
            let mut spec = ColorSpec::new();
            spec.set_fg(Some(Color::Red));
            self.buffer.set_color(&spec)?;
        }
        writeln!(self.buffer, "✗ {}", text)?;
        if !self.no_color {
            self.buffer.reset()?;
        }
        Ok(())
    }

    pub fn print_separator(&mut self) -> io::Result<()> {
        if !self.no_color {
            let mut spec = ColorSpec::new();
            spec.set_fg(Some(Color::White));
            self.buffer.set_color(&spec)?;
        }
        writeln!(self.buffer, "{}", "-".repeat(80))?;
        if !self.no_color {
            self.buffer.reset()?;
        }
        Ok(())
    }

    pub fn write(&mut self, text: &str) -> io::Result<()> {
        write!(self.buffer, "{}", text)?;
        Ok(())
    }

    pub fn writeln(&mut self) -> io::Result<()> {
        writeln!(self.buffer)?;
        Ok(())
    }
}
```

#### 6. Update main.rs with simplified pager usage
**File**: `src/main.rs`
**Changes**: Use new ColorWriter buffer approach

**Wrap the entire command match block** (starting around line 25):

**Before the match statement** (line 24):
```rust
// Will accumulate all output here
let mut final_output = String::new();
```

**For each human format block**, change from:
```rust
_ => {
    let mut writer = output::color::ColorWriter::new(cli.no_color);
    output::user_formatter::format_users_list(&users, &mut writer)?;
}
```

**To**:
```rust
_ => {
    let mut writer = output::color::ColorWriter::new(cli.no_color);
    output::user_formatter::format_users_list(&users, &mut writer)?;
    final_output = writer.into_string()?;
}
```

**For JSON/YAML**, change from:
```rust
"json" => println!("{}", serde_json::to_string_pretty(&users)?),
"yaml" => println!("{}", serde_yaml::to_string(&users)?),
```

**To**:
```rust
"json" => final_output = serde_json::to_string_pretty(&users)?,
"yaml" => final_output = serde_yaml::to_string(&users)?,
```

**After the entire match statement** (end of main function, before the final `Ok(())`):
```rust
// Output with pager if enabled
if !final_output.is_empty() {
    let mut output_dest = output::pager::OutputDestination::new(cli.no_pager)?;
    output_dest.write_str(&final_output)?;
    output_dest.finish()?;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] All dependencies install successfully: `cargo build`
- [ ] All tests pass: `make test`
- [ ] Code compiles without errors: `make build`
- [ ] Type checking passes: `cargo check`
- [ ] Linting passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] `clack users list` opens in pager (scrollable with arrow keys, q to quit)
- [ ] `clack users list --no-pager` outputs directly without pager
- [ ] `clack users list | grep foo` auto-disables pager (grep works)
- [ ] `clack users list --format json` opens in pager
- [ ] `clack users list --format json | jq .` auto-disables pager
- [ ] `PAGER=less clack users list` uses system less pager
- [ ] Pager respects NO_COLOR environment variable

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 3: Dynamic Terminal Width

### Overview
Detect terminal width and dynamically wrap text up to a maximum of 120 characters, improving readability on different screen sizes.

### Changes Required:

#### 1. Add Dependency
**File**: `Cargo.toml`
**Changes**: Add terminal size detection crate

Add to `[dependencies]` section:
```toml
terminal_size = "0.3"
```

#### 2. Create Width Detection Utility
**File**: `src/output/width.rs` (new file)
**Changes**: Create terminal width detection

```rust
use terminal_size::{terminal_size, Width};

/// Get the optimal text width for wrapping
/// - Detects terminal width
/// - Caps at 120 characters maximum
/// - Defaults to 80 if detection fails
pub fn get_wrap_width() -> usize {
    const MAX_WIDTH: usize = 120;
    const DEFAULT_WIDTH: usize = 80;
    const MARGIN: usize = 2; // Leave margin for padding/indentation

    if let Some((Width(w), _)) = terminal_size() {
        let width = w as usize;
        // Use terminal width minus margin, but cap at MAX_WIDTH
        std::cmp::min(width.saturating_sub(MARGIN), MAX_WIDTH)
    } else {
        DEFAULT_WIDTH
    }
}

/// Get wrap width for indented text (e.g., threaded replies)
/// - Accounts for indentation level
pub fn get_wrap_width_with_indent(indent_size: usize) -> usize {
    get_wrap_width().saturating_sub(indent_size)
}
```

#### 3. Update output/mod.rs
**File**: `src/output/mod.rs`
**Changes**: Export width module

Add:
```rust
pub mod width;
```

#### 4. Update Message Formatter
**File**: `src/output/message_formatter.rs`
**Changes**: Use dynamic width instead of hard-coded 78

**Replace line 125**:
```rust
// OLD
let wrapped = wrap(&msg.text, 78);

// NEW
let wrap_width = crate::output::width::get_wrap_width();
let wrapped = wrap(&msg.text, wrap_width);
```

#### 5. Update Thread Formatter
**File**: `src/output/thread_formatter.rs`
**Changes**: Use dynamic width with indent calculation

**Find the text wrapping section** (around lines 140-148):
```rust
// OLD
let wrap_width = if is_reply { 74 } else { 78 };
let text_indent = format!("{}  ", indent);
let wrapped = wrap(&msg.text, wrap_width);

// NEW
let base_width = crate::output::width::get_wrap_width();
let indent_size = if is_reply { 4 } else { 2 }; // 2 spaces for root, 4 for replies
let wrap_width = base_width.saturating_sub(indent_size);
let text_indent = format!("{}  ", indent);
let wrapped = wrap(&msg.text, wrap_width);
```

### Success Criteria:

#### Automated Verification:
- [ ] All tests pass: `make test`
- [ ] Code compiles without errors: `make build`
- [ ] Type checking passes: `cargo check`
- [ ] Linting passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] On wide terminal (>120 chars): Messages wrap at 120 chars max
- [ ] On narrow terminal (80 chars): Messages wrap at ~78 chars
- [ ] On very narrow terminal (60 chars): Messages wrap at ~58 chars
- [ ] Thread replies properly account for indentation
- [ ] No text cutoff or overflow issues

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 4: Enhanced Thread Indicators

### Overview
Improve thread indicators to show reply count and participating users, providing better context about thread activity.

### Changes Required:

#### 1. Add Thread Metadata to API Response
**File**: `src/api/messages.rs`
**Changes**: Fetch thread metadata (reply count, participants)

We need to enhance the thread fetching to include participant information. The Slack API's `conversations.replies` endpoint returns all messages, so we can derive this information.

**Add helper function**:
```rust
use std::collections::HashSet;

/// Extract thread metadata from messages
pub fn get_thread_metadata(messages: &[Message]) -> (usize, Vec<String>) {
    let reply_count = messages.len().saturating_sub(1); // Exclude root message

    // Collect unique user IDs
    let mut participants = HashSet::new();
    for msg in messages {
        if let Some(user_id) = &msg.user {
            participants.insert(user_id.clone());
        }
    }

    let participant_ids: Vec<String> = participants.into_iter().collect();
    (reply_count, participant_ids)
}
```

#### 2. Update Message Formatter for Inline Thread Indicator
**File**: `src/output/message_formatter.rs`
**Changes**: Add function to show enhanced thread indicator

**Add new function signature to accept thread info**:
```rust
pub fn format_messages_with_thread_info(
    messages: &[Message],
    channel: &Channel,
    users: &HashMap<String, User>,
    thread_info: &HashMap<String, (usize, Vec<String>)>, // Map of thread_ts -> (reply_count, participants)
    writer: &mut ColorWriter,
) -> Result<()>
```

**Update thread indicator section** (currently lines 146-151):
```rust
// OLD
if msg.thread_ts.is_some() {
    writer.write("  ")?;
    writer.print_colored("💬 Part of thread", Color::Blue)?;
    writer.writeln()?;
}

// NEW
if let Some(thread_ts) = &msg.thread_ts {
    writer.write("  ")?;

    // Get thread metadata if available
    if let Some((reply_count, participant_ids)) = thread_info.get(thread_ts) {
        writer.print_colored(
            &format!("💬 Part of thread ({} replies)", reply_count),
            Color::Blue
        )?;
        writer.writeln()?;

        // Show participants if any
        if !participant_ids.is_empty() {
            writer.write("  ")?;
            writer.print_colored("Participants: ", Color::Blue)?;

            let participant_names: Vec<String> = participant_ids
                .iter()
                .filter_map(|id| {
                    users.get(id).map(|u| format!("@{}", u.name))
                })
                .collect();

            writer.write(&participant_names.join(", "))?;
            writer.writeln()?;
        }
    } else {
        // Fallback to simple indicator
        writer.print_colored("💬 Part of thread", Color::Blue)?;
        writer.writeln()?;
    }
}
```

**Keep the original `format_messages` for backward compatibility**:
```rust
pub fn format_messages(
    messages: &[Message],
    channel: &Channel,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    let empty_thread_info = HashMap::new();
    format_messages_with_thread_info(messages, channel, users, &empty_thread_info, writer)
}
```

#### 3. Update Thread Formatter Header
**File**: `src/output/thread_formatter.rs`
**Changes**: Show participants in thread header

**Update the thread header section** (around lines 26-48):
```rust
// Add after the thread header (around line 31)
pub fn format_thread(
    messages: &[Message],
    channel: &Channel,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    writer.print_header(&format!(
        "Thread in #{} ({} messages)",
        channel.name,
        messages.len()
    ))?;

    // Calculate participants
    let mut participant_ids = std::collections::HashSet::new();
    for msg in messages {
        if let Some(user_id) = &msg.user {
            participant_ids.insert(user_id);
        }
    }

    // Show participants
    if !participant_ids.is_empty() {
        writer.print_field("Participants", &{
            let names: Vec<String> = participant_ids
                .iter()
                .filter_map(|id| users.get(*id).map(|u| format!("@{}", u.name)))
                .collect();
            names.join(", ")
        })?;
    }

    writer.print_separator()?;

    // ... rest of the function
```

#### 4. Update main.rs to fetch thread metadata
**File**: `src/main.rs`
**Changes**: Build thread metadata map for message history

For the `ConversationsCommands::History` block (around lines 97-139), we need to:
1. Identify messages that are part of threads
2. Fetch thread replies for each thread
3. Build a thread_info map

**Add after building user_map** (around line 129):
```rust
// Build thread metadata map
let mut thread_info: std::collections::HashMap<String, (usize, Vec<String>)> =
    std::collections::HashMap::new();

// Identify unique threads
let thread_timestamps: std::collections::HashSet<&String> = messages
    .iter()
    .filter_map(|m| m.thread_ts.as_ref())
    .collect();

// Fetch metadata for each thread
for thread_ts in thread_timestamps {
    if let Ok(thread_messages) = api::messages::get_thread(&client, &channel_id, thread_ts).await {
        let (reply_count, participant_ids) = api::messages::get_thread_metadata(&thread_messages);
        thread_info.insert(thread_ts.clone(), (reply_count, participant_ids));

        // Also add participants to user_map
        for user_id in &participant_ids {
            if !user_map.contains_key(user_id) {
                if let Ok(user) = api::users::get_user(&client, user_id).await {
                    user_map.insert(user.id.clone(), user);
                }
            }
        }
    }
}

// Update formatter call to use new function
let mut writer = output::color::ColorWriter::new(cli.no_color);
output::message_formatter::format_messages_with_thread_info(
    &messages,
    &channel_info,
    &user_map,
    &thread_info,
    &mut writer,
)?;
final_output = writer.into_string()?;
```

### Success Criteria:

#### Automated Verification:
- [ ] All tests pass: `make test`
- [ ] Code compiles without errors: `make build`
- [ ] Type checking passes: `cargo check`
- [ ] Linting passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] Messages part of a thread show `💬 Part of thread (N replies)`
- [ ] Thread participants are listed: `Participants: @alice, @bob, @charlie`
- [ ] Thread detail view shows participants in header
- [ ] Participant usernames are resolved correctly
- [ ] No performance degradation with many threads

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Search Results Consistency

### Overview
Update the search results formatter to match the message formatter patterns, providing consistent timestamp formatting and user references.

### Changes Required:

#### 1. Update Search Message Formatter
**File**: `src/output/search_formatter.rs`
**Changes**: Rewrite to match message_formatter patterns

**Find the `format_search_messages` function** and replace with:

```rust
use crate::models::message::Message;
use crate::models::search::{SearchMessages, SearchFiles, SearchAll};
use crate::models::user::User;
use crate::output::color::ColorWriter;
use std::collections::HashMap;
use std::io::Result;
use termcolor::Color;
use chrono::{DateTime, Local};
use textwrap::wrap;

pub fn format_search_messages(
    results: &SearchMessages,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    writer.print_header(&format!(
        "Found {} message(s) matching '{}'",
        results.matches.len(),
        results.query
    ))?;
    writer.print_separator()?;

    for (i, msg) in results.matches.iter().enumerate() {
        // Use same formatting as message_formatter
        format_search_message(msg, users, writer)?;

        if i < results.matches.len() - 1 {
            writer.writeln()?;
        }
    }

    Ok(())
}

fn format_search_message(
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
        writer.print_colored(&format!("#{}", channel), Color::Green)?;
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
```

#### 2. Update main.rs to build user map for search results
**File**: `src/main.rs`
**Changes**: Build user lookup map for search commands

**Find search messages command** (around line 220-230):
```rust
SearchType::Messages { query, from, channel, after, before, limit } => {
    let results = api::search::search_messages(
        &client, &query, from.as_deref(), channel.as_deref(),
        after.as_deref(), before.as_deref(), *limit
    ).await?;

    match cli.format.as_str() {
        "json" => final_output = serde_json::to_string_pretty(&results)?,
        "yaml" => final_output = serde_yaml::to_string(&results)?,
        _ => {
            // Build user lookup map from search results
            let mut user_map: std::collections::HashMap<String, models::user::User> =
                std::collections::HashMap::new();

            for message in &results.matches {
                if let Some(user_id) = &message.user {
                    if !user_map.contains_key(user_id) {
                        if let Ok(user) = api::users::get_user(&client, user_id).await {
                            user_map.insert(user.id.clone(), user);
                        }
                    }
                }
            }

            let mut writer = output::color::ColorWriter::new(cli.no_color);
            output::search_formatter::format_search_messages(&results, &user_map, &mut writer)?;
            final_output = writer.into_string()?;
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] All tests pass: `make test`
- [ ] Code compiles without errors: `make build`
- [ ] Type checking passes: `cargo check`
- [ ] Linting passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] `clack search messages "test"` shows consistent formatting with conversations history
- [ ] Timestamps use same "X ago" or date format as messages
- [ ] User names are resolved: `@username` instead of `U1234ABCD`
- [ ] Text wrapping matches terminal width (max 120 chars)
- [ ] Search results are readable and consistent

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 6: Shorter URL Format

### Overview
Replace long URLs with concise `🔗 View message` format while keeping links functional.

### Changes Required:

#### 1. Update Message Formatter
**File**: `src/output/message_formatter.rs`
**Changes**: Use shorter URL display format

**Replace the message URL section** (currently lines 153-160):
```rust
// OLD
let msg_ts = msg.ts.replace('.', "");
writer.write("  🔗 ")?;
writer.write(&format!(
    "https://slack.com/archives/{}/p{}",
    channel_id, msg_ts
))?;
writer.writeln()?;

// NEW
writer.write("  ")?;
writer.print_colored("🔗 View message", Color::Blue)?;
writer.writeln()?;

// Note: We could make this clickable in terminals that support hyperlinks
// using OSC 8 escape sequences, but that's beyond the scope of this phase
```

#### 2. Update Thread Formatter
**File**: `src/output/thread_formatter.rs`
**Changes**: Use shorter URL display format

**Find the thread URL section** (around lines 60-68):
```rust
// OLD
writer.write("🔗 Thread URL: ")?;
writer.write(&format!(
    "https://slack.com/archives/{}/p{}",
    channel.id,
    thread_ts.replace('.', "")
))?;
writer.writeln()?;

// NEW
writer.print_colored("🔗 View thread", Color::Blue)?;
writer.writeln()?;
```

#### 3. Update Search Formatter
**File**: `src/output/search_formatter.rs`
**Changes**: Use shorter format for permalinks

**In the `format_search_message` function** (permalink section):
```rust
// OLD
if let Some(permalink) = &msg.permalink {
    writer.write("  🔗 ")?;
    writer.write(permalink)?;
    writer.writeln()?;
}

// NEW
if msg.permalink.is_some() {
    writer.write("  ")?;
    writer.print_colored("🔗 View message", Color::Blue)?;
    writer.writeln()?;
}
```

#### 4. (Optional Enhancement) Add Clickable Hyperlinks
**File**: `src/output/color.rs`
**Changes**: Add method for clickable hyperlinks using OSC 8

**Add new method** (optional, for terminals that support it):
```rust
/// Print a clickable hyperlink (OSC 8 terminal sequence)
/// Supported by: iTerm2, GNOME Terminal, Windows Terminal, etc.
pub fn print_hyperlink(&mut self, text: &str, url: &str, color: Color) -> io::Result<()> {
    // OSC 8 format: \033]8;;URL\033\\TEXT\033]8;;\033\\
    self.print_colored(&format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text), color)?;
    Ok(())
}
```

**Then update message_formatter.rs to use it**:
```rust
let msg_ts = msg.ts.replace('.', "");
let url = format!("https://slack.com/archives/{}/p{}", channel_id, msg_ts);
writer.write("  ")?;
writer.print_hyperlink("🔗 View message", &url, Color::Blue)?;
writer.writeln()?;
```

**Note**: The hyperlink enhancement is optional and may not work in all terminals.

### Success Criteria:

#### Automated Verification:
- [ ] All tests pass: `make test`
- [ ] Code compiles without errors: `make build`
- [ ] Type checking passes: `cargo check`
- [ ] Linting passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] Message URLs show as `🔗 View message` instead of full URL
- [ ] Thread URLs show as `🔗 View thread`
- [ ] Search result URLs show as `🔗 View message`
- [ ] Output is cleaner and less cluttered
- [ ] (Optional) Clickable links work in supported terminals

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 7 (Optional): Theme Configuration

### Overview
Add support for custom color themes via configuration file. This phase is optional and may be deferred based on user needs.

### Changes Required:

#### 1. Add Configuration Dependencies
**File**: `Cargo.toml`
**Changes**: Add config parsing crates

Add to `[dependencies]`:
```toml
serde = { version = "1.0", features = ["derive"] }  # Already exists
toml = "0.8"
```

#### 2. Create Configuration Module
**File**: `src/config/mod.rs` (new file)
**Changes**: Define theme configuration structure

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: ThemeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub channel_name: String,      // Color name: "green", "cyan", etc.
    pub user_name: String,
    pub timestamp: String,
    pub id: String,
    pub header: String,
    pub field_label: String,
    pub error: String,
    pub link: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            channel_name: "green".to_string(),
            user_name: "cyan".to_string(),
            timestamp: "yellow".to_string(),
            id: "yellow".to_string(),
            header: "cyan".to_string(),
            field_label: "blue".to_string(),
            error: "red".to_string(),
            link: "blue".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_file_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            // Return default config
            Ok(Self {
                theme: ThemeConfig::default(),
            })
        }
    }

    pub fn config_file_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        Ok(config_dir.join("clack").join("config.toml"))
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path()?;

        // Create directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)?;
        Ok(())
    }
}

/// Convert color name string to termcolor::Color
pub fn parse_color(name: &str) -> termcolor::Color {
    match name.to_lowercase().as_str() {
        "black" => termcolor::Color::Black,
        "blue" => termcolor::Color::Blue,
        "green" => termcolor::Color::Green,
        "red" => termcolor::Color::Red,
        "cyan" => termcolor::Color::Cyan,
        "magenta" => termcolor::Color::Magenta,
        "yellow" => termcolor::Color::Yellow,
        "white" => termcolor::Color::White,
        _ => termcolor::Color::White, // Default fallback
    }
}
```

#### 3. Update main.rs Module Declaration
**File**: `src/main.rs`
**Changes**: Add config module

Add after line 4:
```rust
mod config;
```

#### 4. Load Configuration in main()
**File**: `src/main.rs`
**Changes**: Load theme config at startup

Add after line 16:
```rust
// Load configuration
let config = config::Config::load().unwrap_or_else(|_| config::Config {
    theme: config::ThemeConfig::default(),
});
```

#### 5. Update ColorWriter to Accept Theme
**File**: `src/output/color.rs`
**Changes**: Use theme colors instead of hard-coded colors

This requires significant refactoring of ColorWriter to accept a theme parameter. For brevity, the key change is:

**Add theme field to ColorWriter**:
```rust
pub struct ColorWriter {
    buffer: Buffer,
    no_color: bool,
    theme: crate::config::ThemeConfig,
}

impl ColorWriter {
    pub fn new(no_color: bool, theme: crate::config::ThemeConfig) -> Self {
        // ... existing code ...
        Self {
            buffer: Buffer::ansi(),
            no_color,
            theme,
        }
    }

    // Update methods to use theme colors
    pub fn print_header(&mut self, text: &str) -> io::Result<()> {
        let color = crate::config::parse_color(&self.theme.header);
        // ... use color ...
    }
}
```

#### 6. Add CLI Flag for Theme Selection
**File**: `src/cli.rs`
**Changes**: Add `--theme` flag

Add after line 17:
```rust
/// Override theme (overrides config file)
#[arg(long, global = true)]
pub theme: Option<String>,
```

#### 7. Create Sample Config File
**File**: `config.example.toml` (new file in project root)
**Changes**: Provide example configuration

```toml
[theme]
channel_name = "green"
user_name = "cyan"
timestamp = "yellow"
id = "yellow"
header = "cyan"
field_label = "blue"
error = "red"
link = "blue"
```

### Success Criteria:

#### Automated Verification:
- [ ] All tests pass: `make test`
- [ ] Code compiles without errors: `make build`
- [ ] Type checking passes: `cargo check`
- [ ] Linting passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] Default theme works without config file
- [ ] Config file at `~/.config/clack/config.toml` is respected
- [ ] `--theme` flag overrides config file (if implemented)
- [ ] Invalid color names fall back to defaults gracefully
- [ ] Theme colors apply to all formatters consistently

**Implementation Note**: This phase is **optional** and can be deferred. After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful.

---

## Testing Strategy

### Unit Tests:
- Add tests for `width::get_wrap_width()` with mocked terminal sizes
- Add tests for `pager::OutputDestination` pipe detection
- Add tests for `config::parse_color()` with various color names
- Update existing formatter tests to use new signatures

### Integration Tests:
- Test full command output with pager enabled/disabled
- Test pipe detection: `clack users list | cat`
- Test width detection in different terminal sizes
- Test theme loading from config file

### Manual Testing Steps:
1. Run `clack conversations history general` and verify pager behavior
2. Run `clack users list --no-pager` and verify direct output
3. Resize terminal and verify text wraps correctly
4. Check thread indicators show participant information
5. Verify search results match message formatting
6. Check URL format is concise

## Performance Considerations

- **Thread metadata fetching**: May add latency when displaying messages with many threads. Consider adding a `--skip-thread-info` flag if this becomes an issue.
- **Pager buffer memory**: Large outputs may consume memory in buffer before paging. This is acceptable for CLI usage.
- **User lookup caching**: Already implemented via cache module, no additional optimization needed.

## Migration Notes

- **Backward compatibility**: All changes are additive or improve existing behavior. No breaking changes to command syntax.
- **Configuration**: Config file is optional; default theme works without any configuration.
- **Pager opt-out**: Users who prefer direct output can use `--no-pager` flag or pipe output.

## References

- Clack codebase analysis: Current implementation at `src/output/` directory
- `minus` crate documentation: https://docs.rs/minus/
- `atty` crate for TTY detection: https://docs.rs/atty/
- `terminal_size` crate: https://docs.rs/terminal_size/
- OSC 8 hyperlinks spec: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
