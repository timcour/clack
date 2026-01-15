# Clack - Slack CLI Tool Implementation Plan

## Overview

Clack is a command-line interface tool for querying the Slack API with human-readable output by default, supporting JSON and YAML formats for machine parsing. The tool follows git-style CLI conventions and is built in Rust using Tokio for async operations.

## Current State Analysis

This is a greenfield project. The repository currently contains:
- `README.md` - Project overview and goals
- `CLI.md` - Complete CLI interface documentation (Phase 1 complete)
- `tasks/2026-01-14-first-steps.md` - Implementation roadmap

No Rust code or build infrastructure exists yet.

## Desired End State

A working CLI tool that:
- Allows users to query Slack users and messages via simple commands
- Outputs human-readable colorized text by default
- Supports JSON and YAML export formats
- Includes comprehensive unit tests with mocked API calls
- Has a clean Makefile-based build system
- Implements basic message search functionality

### Key Technical Decisions:
- **CLI Framework**: clap (v4) with derive macros
- **HTTP Client**: reqwest with tokio async runtime
- **Colorization**: termcolor with custom abstraction layer
- **Testing**: mockito for HTTP mocking
- **Output Formats**: JSON (via serde_json) and YAML (via serde_yaml)
- **Authentication**: Bearer token via SLACK_TOKEN environment variable

## What We're NOT Doing

- SQLite caching (deferred to future implementation)
- ORM integration (future consideration)
- Channel listing/management beyond message retrieval
- OAuth flow (using pre-configured tokens only)
- Interactive/TUI mode (CLI only)
- Webhook management
- File uploads
- Advanced message search filters (Phase 6 covers basics only)

## Implementation Approach

We'll build this in 6 phases, each building upon the previous:
1. ~~CLI Interface Design~~ (Complete - CLI.md created)
2. Scaffolding - Set up Rust project structure and build system
3. Core Functionality - Implement users and messages commands
4. Testing - Add comprehensive unit tests with mocked APIs
5. Output Formatting - Add colorization and format options
6. Message Search - Implement basic search functionality

Each phase includes automated and manual verification steps.

---

## Phase 2: Project Scaffolding

### Overview
Set up the Rust project structure with a hello-world binary, unit tests, and a Makefile for build automation.

### Changes Required:

#### 1. Initialize Cargo Project
**Command**: `cargo init --name clack`

**Result**: Creates basic Rust project structure:
- `Cargo.toml` - Project manifest
- `src/main.rs` - Entry point with hello world
- `.gitignore` - Rust-specific ignore patterns

#### 2. Create src/main.rs
**File**: `src/main.rs`
**Content**:

```rust
fn main() {
    println!("Hello from Clack!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        // Simple test that always passes
        assert_eq!(2 + 2, 4);
    }
}
```

#### 3. Update Cargo.toml with Dependencies
**File**: `Cargo.toml`
**Changes**: Add initial dependencies (minimal set for Phase 2)

```toml
[package]
name = "clack"
version = "0.1.0"
edition = "2021"

[dependencies]
# Dependencies will be added in Phase 3

[dev-dependencies]
# Test dependencies will be added in Phase 4
```

#### 4. Create Makefile
**File**: `Makefile`
**Content**:

```makefile
.PHONY: clack test deps all clean

# Default target
clack:
	cargo build --release
	@echo "Binary built: target/release/clack"

test:
	cargo test --all-features

deps:
	@echo "Installing Rust toolchain if needed..."
	@command -v rustc >/dev/null 2>&1 || { \
		echo "Rust not found. Please install from https://rustup.rs/"; \
		exit 1; \
	}
	@echo "Rust toolchain is installed"
	@rustc --version
	@cargo --version

all: deps clack test

clean:
	cargo clean
```

#### 5. Create .gitignore (if not exists)
**File**: `.gitignore`
**Content**:

```
# Rust build artifacts
target/
Cargo.lock

# IDE files
.vscode/
.idea/
*.swp
*.swo
*~

# OS files
.DS_Store
Thumbs.db
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo init` completes successfully
- [ ] `make deps` verifies Rust toolchain exists and shows versions
- [ ] `make clack` builds the binary without errors
- [ ] `make test` runs and passes the hello-world test
- [ ] `make all` runs deps, build, and test in sequence
- [ ] Binary exists at `target/release/clack`
- [ ] Running `./target/release/clack` outputs "Hello from Clack!"

#### Manual Verification:
- [ ] Project structure looks clean and organized
- [ ] Makefile targets work as documented in README
- [ ] Git status shows only expected files

---

## Phase 3: Core Functionality Implementation

### Overview
Implement the `clack users`, `clack user <id>`, and `clack messages <channel>` commands with Slack API integration. This phase focuses on functionality without colorization (Phase 5) or advanced formatting.

### Changes Required:

#### 1. Update Cargo.toml Dependencies
**File**: `Cargo.toml`
**Changes**: Add core dependencies

```toml
[dependencies]
clap = { version = "4.5", features = ["derive", "env"] }
reqwest = { version = "0.11", features = ["json"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
anyhow = "1.0"
```

#### 2. Create Module Structure
**Files to create**:
- `src/cli.rs` - CLI argument parsing with clap
- `src/api/mod.rs` - Slack API client module
- `src/api/client.rs` - HTTP client wrapper
- `src/api/users.rs` - Users API methods
- `src/api/messages.rs` - Messages API methods
- `src/models/mod.rs` - Data models
- `src/models/user.rs` - User struct
- `src/models/message.rs` - Message struct
- `src/output/mod.rs` - Output formatting
- `src/output/formatter.rs` - Format trait and implementations

#### 3. Implement CLI Argument Parsing
**File**: `src/cli.rs`

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clack")]
#[command(about = "A Slack API CLI tool", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Disable colorized output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Output format (human, json, yaml)
    #[arg(long, global = true, default_value = "human")]
    pub format: String,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all users
    Users {
        /// Maximum number of users to return
        #[arg(long)]
        limit: Option<u32>,

        /// Include deleted/deactivated users
        #[arg(long)]
        include_deleted: bool,
    },
    /// Get a specific user by ID
    User {
        /// Slack user ID (e.g., U1234ABCD)
        user_id: String,
    },
    /// List messages from a channel
    Messages {
        /// Channel ID or name
        channel: String,

        /// Number of messages to retrieve
        #[arg(long, default_value = "100")]
        limit: u32,

        /// End of time range (Unix timestamp)
        #[arg(long)]
        latest: Option<String>,

        /// Start of time range (Unix timestamp)
        #[arg(long)]
        oldest: Option<String>,
    },
}
```

#### 4. Implement Slack API Client
**File**: `src/api/client.rs`

```rust
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use std::env;

pub struct SlackClient {
    client: reqwest::Client,
    token: String,
    base_url: String,
}

impl SlackClient {
    pub fn new() -> Result<Self> {
        let token = env::var("SLACK_TOKEN").context(
            "SLACK_TOKEN environment variable not set\n\n\
             Please set your Slack API token:\n  \
             export SLACK_TOKEN=xoxb-your-token-here\n\n\
             To create a token, visit: https://api.slack.com/authentication/token-types"
        )?;

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            token,
            base_url: "https://slack.com/api".to_string(),
        })
    }

    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let response = self.client.get(&url).query(query).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("API request failed: {}", response.status());
        }

        let data = response.json::<T>().await?;
        Ok(data)
    }
}
```

#### 5. Implement User Models and API
**File**: `src/models/user.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub real_name: Option<String>,
    pub profile: UserProfile,
    pub deleted: bool,
    pub is_bot: bool,
    pub is_admin: Option<bool>,
    pub is_owner: Option<bool>,
    pub tz: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserProfile {
    pub email: Option<String>,
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
    pub display_name: Option<String>,
    pub image_72: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UsersListResponse {
    pub ok: bool,
    pub members: Vec<User>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserInfoResponse {
    pub ok: bool,
    pub user: User,
    pub error: Option<String>,
}
```

**File**: `src/api/users.rs`

```rust
use super::client::SlackClient;
use crate::models::user::{UserInfoResponse, UsersListResponse};
use anyhow::Result;

pub async fn list_users(
    client: &SlackClient,
    limit: Option<u32>,
    include_deleted: bool,
) -> Result<Vec<crate::models::user::User>> {
    let mut query = vec![];

    if let Some(limit) = limit {
        query.push(("limit", limit.to_string()));
    }

    let response: UsersListResponse = client.get("users.list", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let mut users = response.members;
    if !include_deleted {
        users.retain(|u| !u.deleted);
    }

    Ok(users)
}

pub async fn get_user(
    client: &SlackClient,
    user_id: &str,
) -> Result<crate::models::user::User> {
    let query = vec![("user", user_id.to_string())];
    let response: UserInfoResponse = client.get("users.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.user)
}
```

#### 6. Implement Message Models and API
**File**: `src/models/message.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub ts: String,
    pub user: Option<String>,
    pub text: String,
    pub thread_ts: Option<String>,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Reaction {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Deserialize)]
pub struct MessagesResponse {
    pub ok: bool,
    pub messages: Vec<Message>,
    pub error: Option<String>,
}
```

**File**: `src/api/messages.rs`

```rust
use super::client::SlackClient;
use crate::models::message::MessagesResponse;
use anyhow::Result;

pub async fn list_messages(
    client: &SlackClient,
    channel: &str,
    limit: u32,
    latest: Option<String>,
    oldest: Option<String>,
) -> Result<Vec<crate::models::message::Message>> {
    let mut query = vec![
        ("channel", channel.to_string()),
        ("limit", limit.to_string()),
    ];

    if let Some(latest) = latest {
        query.push(("latest", latest));
    }
    if let Some(oldest) = oldest {
        query.push(("oldest", oldest));
    }

    let response: MessagesResponse = client.get("conversations.history", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.messages)
}
```

#### 7. Implement Basic Output Formatting
**File**: `src/output/formatter.rs`

```rust
use anyhow::Result;
use serde::Serialize;

pub trait Formatter {
    fn format<T: Serialize>(&self, data: &T) -> Result<String>;
}

pub struct JsonFormatter;
pub struct YamlFormatter;
pub struct HumanFormatter;

impl Formatter for JsonFormatter {
    fn format<T: Serialize>(&self, data: &T) -> Result<String> {
        Ok(serde_json::to_string_pretty(data)?)
    }
}

impl Formatter for YamlFormatter {
    fn format<T: Serialize>(&self, data: &T) -> Result<String> {
        Ok(serde_yaml::to_string(data)?)
    }
}

impl Formatter for HumanFormatter {
    fn format<T: Serialize>(&self, data: &T) -> Result<String> {
        // For now, just output JSON - Phase 5 will make this human-friendly
        Ok(serde_json::to_string_pretty(data)?)
    }
}

pub fn get_formatter(format: &str) -> Box<dyn Formatter> {
    match format {
        "json" => Box::new(JsonFormatter),
        "yaml" => Box::new(YamlFormatter),
        _ => Box::new(HumanFormatter),
    }
}
```

#### 8. Update main.rs
**File**: `src/main.rs`

```rust
mod api;
mod cli;
mod models;
mod output;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client
    let client = api::client::SlackClient::new()?;

    // Get appropriate formatter
    let formatter = output::formatter::get_formatter(&cli.format);

    // Execute command
    match cli.command {
        Commands::Users { limit, include_deleted } => {
            let users = api::users::list_users(&client, limit, include_deleted).await?;
            let output = formatter.format(&users)?;
            println!("{}", output);
        }
        Commands::User { user_id } => {
            let user = api::users::get_user(&client, &user_id).await?;
            let output = formatter.format(&user)?;
            println!("{}", output);
        }
        Commands::Messages { channel, limit, latest, oldest } => {
            let messages = api::messages::list_messages(&client, &channel, limit, latest, oldest).await?;
            let output = formatter.format(&messages)?;
            println!("{}", output);
        }
    }

    Ok(())
}
```

#### 9. Create Module Index Files
**File**: `src/api/mod.rs`
```rust
pub mod client;
pub mod users;
pub mod messages;
```

**File**: `src/models/mod.rs`
```rust
pub mod user;
pub mod message;
```

**File**: `src/output/mod.rs`
```rust
pub mod formatter;
```

### Success Criteria:

#### Automated Verification:
- [ ] `make clack` builds successfully with all dependencies
- [ ] `cargo check` passes with no errors
- [ ] `cargo clippy` runs with no warnings
- [ ] Binary runs with `--help` flag and displays usage

#### Manual Verification:
- [ ] `export SLACK_TOKEN=xoxb-test` and verify error message appears correctly when token is invalid
- [ ] With valid `SLACK_TOKEN`:
  - [ ] `clack users` lists users from workspace
  - [ ] `clack user <valid-id>` displays user details
  - [ ] `clack messages <valid-channel>` displays messages
  - [ ] `clack users --format json` outputs valid JSON
  - [ ] `clack users --format yaml` outputs valid YAML
- [ ] Without `SLACK_TOKEN`, command exits with code -1 and helpful error message
- [ ] Invalid user ID shows appropriate error message
- [ ] Invalid channel ID shows appropriate error message

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 4: Comprehensive Unit Testing

### Overview
Add unit tests for all public functions and CLI interfaces using mockito to mock Slack API responses. No actual network requests should be made during testing.

### Changes Required:

#### 1. Update Cargo.toml Dev Dependencies
**File**: `Cargo.toml`
**Changes**: Add test dependencies

```toml
[dev-dependencies]
mockito = "1.2"
tokio-test = "0.4"
```

#### 2. Refactor API Client for Testability
**File**: `src/api/client.rs`
**Changes**: Make base_url configurable for testing

```rust
impl SlackClient {
    pub fn new() -> Result<Self> {
        Self::with_base_url("https://slack.com/api")
    }

    #[cfg(test)]
    pub fn with_base_url(base_url: &str) -> Result<Self> {
        // ... existing code but use provided base_url
    }
}
```

#### 3. Add Tests for User API
**File**: `src/api/users.rs`
**Add at end of file**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Mock, ServerGuard};

    async fn setup() -> (ServerGuard, SlackClient) {
        let server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let client = SlackClient::with_base_url(&server.url()).unwrap();
        (server, client)
    }

    #[tokio::test]
    async fn test_list_users_success() {
        let (mut server, client) = setup().await;

        let mock = server.mock("GET", "/users.list")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "ok": true,
                "members": [{
                    "id": "U123",
                    "name": "testuser",
                    "real_name": "Test User",
                    "deleted": false,
                    "is_bot": false,
                    "profile": {
                        "email": "test@example.com",
                        "display_name": "testuser"
                    }
                }]
            }"#)
            .create_async()
            .await;

        let users = list_users(&client, None, false).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].id, "U123");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_users_filters_deleted() {
        // Test that deleted users are filtered when include_deleted is false
    }

    #[tokio::test]
    async fn test_get_user_success() {
        // Test successful user fetch
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        // Test error handling for non-existent user
    }
}
```

#### 4. Add Tests for Message API
**File**: `src/api/messages.rs`
**Add similar test structure**:

```rust
#[cfg(test)]
mod tests {
    // Test list_messages with various parameters
    // Test error handling
    // Test timestamp filtering
}
```

#### 5. Add Tests for CLI Parsing
**File**: `src/cli.rs`
**Add at end**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_users_command_parsing() {
        let cli = Cli::parse_from(["clack", "users"]);
        assert!(matches!(cli.command, Commands::Users { .. }));
    }

    #[test]
    fn test_user_command_with_id() {
        let cli = Cli::parse_from(["clack", "user", "U123"]);
        match cli.command {
            Commands::User { user_id } => assert_eq!(user_id, "U123"),
            _ => panic!("Expected User command"),
        }
    }

    #[test]
    fn test_messages_command_with_options() {
        let cli = Cli::parse_from([
            "clack", "messages", "C123",
            "--limit", "50",
            "--latest", "1234567890"
        ]);
        match cli.command {
            Commands::Messages { channel, limit, latest, .. } => {
                assert_eq!(channel, "C123");
                assert_eq!(limit, 50);
                assert_eq!(latest, Some("1234567890".to_string()));
            },
            _ => panic!("Expected Messages command"),
        }
    }

    #[test]
    fn test_global_format_option() {
        let cli = Cli::parse_from(["clack", "--format", "json", "users"]);
        assert_eq!(cli.format, "json");
    }
}
```

#### 6. Add Tests for Output Formatters
**File**: `src/output/formatter.rs`
**Add tests**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_json_formatter() {
        let formatter = JsonFormatter;
        let data = TestData { name: "test".to_string(), value: 42 };
        let output = formatter.format(&data).unwrap();
        assert!(output.contains("\"name\": \"test\""));
        assert!(output.contains("\"value\": 42"));
    }

    #[test]
    fn test_yaml_formatter() {
        let formatter = YamlFormatter;
        let data = TestData { name: "test".to_string(), value: 42 };
        let output = formatter.format(&data).unwrap();
        assert!(output.contains("name: test"));
        assert!(output.contains("value: 42"));
    }

    #[test]
    fn test_formatter_selection() {
        assert!(matches!(get_formatter("json"), JsonFormatter));
        assert!(matches!(get_formatter("yaml"), YamlFormatter));
        assert!(matches!(get_formatter("human"), HumanFormatter));
    }
}
```

#### 7. Add Integration Test
**File**: `tests/integration_test.rs` (new directory)

```rust
use assert_cmd::Command;

#[test]
fn test_missing_slack_token() {
    std::env::remove_var("SLACK_TOKEN");

    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("users")
        .assert()
        .failure()
        .stderr(predicates::str::contains("SLACK_TOKEN"));
}

#[test]
fn test_help_output() {
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Slack API CLI tool"));
}
```

#### 8. Update Cargo.toml for Integration Tests
**File**: `Cargo.toml`
**Add**:

```toml
[dev-dependencies]
mockito = "1.2"
tokio-test = "0.4"
assert_cmd = "2.0"
predicates = "3.0"
```

### Success Criteria:

#### Automated Verification:
- [ ] `make test` runs all unit tests successfully
- [ ] `cargo test` shows passing tests for:
  - [ ] CLI argument parsing (5+ tests)
  - [ ] User API methods (4+ tests)
  - [ ] Message API methods (3+ tests)
  - [ ] Output formatters (3+ tests)
  - [ ] Integration tests (2+ tests)
- [ ] `cargo test -- --test-threads=1` passes (ensures no test interference)
- [ ] `cargo test --no-fail-fast` shows all test results
- [ ] No actual HTTP requests are made during test execution (verify with network monitoring if needed)

#### Manual Verification:
- [ ] Review test coverage - all public functions have tests
- [ ] Tests use mockito correctly with no real API calls
- [ ] Test names clearly describe what they're testing
- [ ] Error cases are covered (invalid tokens, missing data, API errors)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Human-Readable Output with Colorization

### Overview
Enhance the human output format with colorization, improved formatting, and better information hierarchy. Create a color abstraction layer using termcolor that's simple for common use cases.

### Changes Required:

#### 1. Update Cargo.toml Dependencies
**File**: `Cargo.toml`
**Add**:

```toml
[dependencies]
# ... existing dependencies
termcolor = "1.4"
textwrap = "0.16"  # For text wrapping
chrono = "0.4"     # For timestamp formatting
```

#### 2. Create Color Abstraction Layer
**File**: `src/output/color.rs`

```rust
use std::io::Write;
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
    pub fn print_colored(&mut self, text: &str, color: Color) -> std::io::Result<()> {
        self.stdout.set_color(ColorSpec::new().set_fg(Some(color)))?;
        write!(self.stdout, "{}", text)?;
        self.stdout.reset()?;
        Ok(())
    }

    /// Print bold text
    pub fn print_bold(&mut self, text: &str) -> std::io::Result<()> {
        self.stdout.set_color(ColorSpec::new().set_bold(true))?;
        write!(self.stdout, "{}", text)?;
        self.stdout.reset()?;
        Ok(())
    }

    /// Print a header (bold + color)
    pub fn print_header(&mut self, text: &str) -> std::io::Result<()> {
        self.stdout.set_color(
            ColorSpec::new()
                .set_fg(Some(Color::Cyan))
                .set_bold(true)
        )?;
        writeln!(self.stdout, "{}", text)?;
        self.stdout.reset()?;
        Ok(())
    }

    /// Print a label: value pair
    pub fn print_field(&mut self, label: &str, value: &str) -> std::io::Result<()> {
        self.print_colored(label, Color::Blue)?;
        write!(self.stdout, ": ")?;
        writeln!(self.stdout, "{}", value)?;
        Ok(())
    }

    /// Print success message
    pub fn print_success(&mut self, text: &str) -> std::io::Result<()> {
        self.print_colored("✓ ", Color::Green)?;
        writeln!(self.stdout, "{}", text)?;
        Ok(())
    }

    /// Print error message
    pub fn print_error(&mut self, text: &str) -> std::io::Result<()> {
        self.print_colored("✗ ", Color::Red)?;
        writeln!(self.stdout, "{}", text)?;
        Ok(())
    }

    /// Print separator line
    pub fn print_separator(&mut self) -> std::io::Result<()> {
        self.print_colored(&"─".repeat(80), Color::White)?;
        writeln!(self.stdout)?;
        Ok(())
    }

    /// Print regular text (no color)
    pub fn print(&mut self, text: &str) -> std::io::Result<()> {
        writeln!(self.stdout, "{}", text)
    }
}
```

#### 3. Create Human-Readable User Formatter
**File**: `src/output/user_formatter.rs`

```rust
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

    // Profile URL
    let profile_url = format!("https://slack.com/app_redirect?team=<team>&channel={}", user.id);
    writer.print_field("Profile URL", &profile_url)?;

    Ok(())
}

pub fn format_users_list(users: &[User], writer: &mut ColorWriter) -> Result<()> {
    writer.print_header(&format!("Users ({})", users.len()))?;
    writer.print_separator()?;

    for (i, user) in users.iter().enumerate() {
        // ID and name
        writer.print_colored(&user.id, Color::Yellow)?;
        write!(std::io::stdout(), " ")?;
        writer.print_bold(&user.name)?;

        // Real name in parentheses
        if let Some(real_name) = &user.real_name {
            print!(" ({})", real_name);
        }

        // Status emoji if present
        if let Some(emoji) = &user.profile.status_emoji {
            print!(" {}", emoji);
        }

        println!();

        // Email on second line if present
        if let Some(email) = &user.profile.email {
            print!("  ");
            writer.print_colored("✉", Color::Blue)?;
            println!(" {}", email);
        }

        // Add spacing between users
        if i < users.len() - 1 {
            println!();
        }
    }

    Ok(())
}
```

#### 4. Create Human-Readable Message Formatter
**File**: `src/output/message_formatter.rs`

```rust
use crate::models::message::Message;
use crate::output::color::ColorWriter;
use chrono::{DateTime, Utc};
use std::io::Result;
use termcolor::Color;
use textwrap::wrap;

pub fn format_messages(messages: &[Message], writer: &mut ColorWriter) -> Result<()> {
    writer.print_header(&format!("Messages ({})", messages.len()))?;
    writer.print_separator()?;

    for (i, msg) in messages.iter().enumerate() {
        format_message(msg, writer)?;

        if i < messages.len() - 1 {
            println!();
        }
    }

    Ok(())
}

fn format_message(msg: &Message, writer: &mut ColorWriter) -> Result<()> {
    // Parse timestamp
    let ts_float: f64 = msg.ts.parse().unwrap_or(0.0);
    let dt = DateTime::<Utc>::from_timestamp(ts_float as i64, 0)
        .unwrap_or_default();
    let time_str = dt.format("%Y-%m-%d %H:%M:%S UTC").to_string();

    // Timestamp in gray
    writer.print_colored(&time_str, Color::White)?;
    print!(" ");

    // User ID in cyan
    if let Some(user) = &msg.user {
        writer.print_colored(user, Color::Cyan)?;
    } else {
        writer.print_colored("<system>", Color::White)?;
    }
    println!();

    // Message text wrapped to 80 chars
    let wrapped = wrap(&msg.text, 78);
    for line in wrapped {
        println!("  {}", line);
    }

    // Reactions if present
    if let Some(reactions) = &msg.reactions {
        print!("  ");
        for (i, reaction) in reactions.iter().enumerate() {
            if i > 0 {
                print!(" ");
            }
            print!(":{}:{}", reaction.name, reaction.count);
        }
        println!();
    }

    // Thread indicator
    if msg.thread_ts.is_some() {
        writer.print_colored("  💬 Part of thread", Color::Blue)?;
        println!();
    }

    // Message URL
    let msg_ts = msg.ts.replace('.', "");
    println!("  🔗 https://slack.com/archives/<channel>/p{}", msg_ts);

    Ok(())
}
```

#### 5. Update HumanFormatter Implementation
**File**: `src/output/formatter.rs`
**Changes**: Update HumanFormatter to use new color formatters

```rust
use crate::output::color::ColorWriter;
use crate::output::{user_formatter, message_formatter};

impl HumanFormatter {
    fn new(no_color: bool) -> Self {
        Self {
            writer: ColorWriter::new(no_color),
        }
    }
}

impl Formatter for HumanFormatter {
    fn format<T: Serialize>(&self, data: &T) -> Result<String> {
        // Use type_id or serde_value to determine type
        // Route to appropriate formatter
        // This requires some runtime type detection

        // For now, return a marker that main.rs can use to call
        // the right formatter
        unimplemented!("HumanFormatter needs refactoring")
    }
}
```

**Note**: The Formatter trait may need refactoring to better handle different types. Consider creating separate format functions that main.rs can call directly.

#### 6. Refactor main.rs to Use Formatters
**File**: `src/main.rs`
**Changes**: Update to use color writers for human format

```rust
match cli.command {
    Commands::Users { limit, include_deleted } => {
        let users = api::users::list_users(&client, limit, include_deleted).await?;

        match cli.format.as_str() {
            "json" => println!("{}", serde_json::to_string_pretty(&users)?),
            "yaml" => println!("{}", serde_yaml::to_string(&users)?),
            _ => {
                let mut writer = output::color::ColorWriter::new(cli.no_color);
                output::user_formatter::format_users_list(&users, &mut writer)?;
            }
        }
    }
    // Similar for other commands...
}
```

#### 7. Update Module Index
**File**: `src/output/mod.rs`
```rust
pub mod formatter;
pub mod color;
pub mod user_formatter;
pub mod message_formatter;
```

### Success Criteria:

#### Automated Verification:
- [ ] `make clack` builds successfully
- [ ] `cargo clippy` passes with no warnings
- [ ] Existing unit tests still pass
- [ ] NO_COLOR=1 environment variable disables colors
- [ ] `--no-color` flag disables colors

#### Manual Verification:
- [ ] `clack users` displays colorized output with:
  - [ ] User IDs in yellow
  - [ ] Names in bold
  - [ ] Email addresses with envelope icon
  - [ ] Clear visual separation between users
- [ ] `clack user <id>` displays:
  - [ ] Header in cyan/bold
  - [ ] Field labels in blue
  - [ ] Separator lines
  - [ ] Profile URL at bottom
- [ ] `clack messages <channel>` displays:
  - [ ] Timestamps in gray
  - [ ] User IDs in cyan
  - [ ] Message text wrapped appropriately
  - [ ] Reactions displayed inline
  - [ ] Thread indicators
  - [ ] Message URLs
- [ ] Colors respect terminal capabilities (don't break on non-color terminals)
- [ ] Output is readable and well-formatted
- [ ] JSON and YAML formats still work correctly

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 6: Basic Message Search

### Overview
Implement `clack search` command with basic text query, user filtering, and channel filtering capabilities.

### Changes Required:

#### 1. Extend CLI Commands
**File**: `src/cli.rs`
**Add to Commands enum**:

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands

    /// Search for messages
    Search {
        /// Search query text
        query: String,

        /// Filter by user (can be user ID or @username)
        #[arg(long)]
        from: Option<String>,

        /// Search in specific channel
        #[arg(long)]
        channel: Option<String>,

        /// Sort results (score, timestamp)
        #[arg(long, default_value = "score")]
        sort: String,

        /// Number of results to return
        #[arg(long, default_value = "20")]
        count: u32,
    },
}
```

#### 2. Add Search Model
**File**: `src/models/search.rs`

```rust
use serde::{Deserialize, Serialize};
use super::message::Message;

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchResponse {
    pub ok: bool,
    pub query: String,
    pub messages: SearchMatches,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchMatches {
    pub total: u32,
    pub matches: Vec<SearchMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchMessage {
    #[serde(flatten)]
    pub message: Message,
    pub channel: ChannelInfo,
    pub permalink: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
}
```

#### 3. Implement Search API
**File**: `src/api/search.rs`

```rust
use super::client::SlackClient;
use crate::models::search::SearchResponse;
use anyhow::Result;

pub async fn search_messages(
    client: &SlackClient,
    query: &str,
    from_user: Option<&str>,
    in_channel: Option<&str>,
    sort: &str,
    count: u32,
) -> Result<SearchResponse> {
    // Build search query with filters
    let mut search_query = query.to_string();

    if let Some(user) = from_user {
        // Handle both @username and user ID formats
        let user_filter = if user.starts_with('@') {
            format!(" from:{}", user)
        } else if user.starts_with('U') {
            format!(" from:<@{}>", user)
        } else {
            format!(" from:@{}", user)
        };
        search_query.push_str(&user_filter);
    }

    if let Some(channel) = in_channel {
        // Handle both #channel and channel ID formats
        let channel_filter = if channel.starts_with('#') {
            format!(" in:{}", channel)
        } else if channel.starts_with('C') {
            format!(" in:<#{}>", channel)
        } else {
            format!(" in:#{}", channel)
        };
        search_query.push_str(&channel_filter);
    }

    let query_params = vec![
        ("query", search_query),
        ("sort", sort.to_string()),
        ("count", count.to_string()),
    ];

    let response: SearchResponse = client
        .get("search.messages", &query_params)
        .await?;

    if !response.ok {
        anyhow::bail!(
            "Search failed: {}",
            response.error.unwrap_or_default()
        );
    }

    Ok(response)
}
```

#### 4. Create Search Result Formatter
**File**: `src/output/search_formatter.rs`

```rust
use crate::models::search::SearchResponse;
use crate::output::color::ColorWriter;
use std::io::Result;
use termcolor::Color;

pub fn format_search_results(
    response: &SearchResponse,
    writer: &mut ColorWriter,
) -> Result<()> {
    writer.print_header(&format!(
        "Search Results for \"{}\" ({} total)",
        response.query,
        response.messages.total
    ))?;
    writer.print_separator()?;

    if response.messages.matches.is_empty() {
        writer.print("No messages found")?;
        return Ok(());
    }

    for (i, result) in response.messages.matches.iter().enumerate() {
        // Channel name in cyan
        writer.print_colored(&format!("#{}", result.channel.name), Color::Cyan)?;
        print!(" ");

        // User
        if let Some(user) = &result.message.user {
            writer.print_colored(user, Color::Yellow)?;
        }
        println!();

        // Message text with query terms highlighted
        // (Simple implementation - just show the text)
        println!("  {}", result.message.text);

        // Permalink
        writer.print_colored("  🔗 ", Color::Blue)?;
        println!("{}", result.permalink);

        if i < response.messages.matches.len() - 1 {
            println!();
        }
    }

    Ok(())
}
```

#### 5. Update main.rs for Search Command
**File**: `src/main.rs`
**Add to match statement**:

```rust
Commands::Search { query, from, channel, sort, count } => {
    let response = api::search::search_messages(
        &client,
        &query,
        from.as_deref(),
        channel.as_deref(),
        &sort,
        count,
    ).await?;

    match cli.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&response)?),
        "yaml" => println!("{}", serde_yaml::to_string(&response)?),
        _ => {
            let mut writer = output::color::ColorWriter::new(cli.no_color);
            output::search_formatter::format_search_results(&response, &mut writer)?;
        }
    }
}
```

#### 6. Update Module Indexes
**File**: `src/api/mod.rs`
```rust
pub mod client;
pub mod users;
pub mod messages;
pub mod search;
```

**File**: `src/models/mod.rs`
```rust
pub mod user;
pub mod message;
pub mod search;
```

**File**: `src/output/mod.rs`
```rust
pub mod formatter;
pub mod color;
pub mod user_formatter;
pub mod message_formatter;
pub mod search_formatter;
```

#### 7. Add Search Tests
**File**: `src/api/search.rs`
**Add tests**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_basic_text_search() {
        // Mock search.messages endpoint
        // Verify query parameter
    }

    #[tokio::test]
    async fn test_search_with_from_filter() {
        // Test user filtering with @username
        // Test user filtering with user ID
    }

    #[tokio::test]
    async fn test_search_with_channel_filter() {
        // Test channel filtering with #channel
        // Test channel filtering with channel ID
    }

    #[tokio::test]
    async fn test_search_with_multiple_filters() {
        // Test combining query + from + in
    }
}
```

#### 8. Update CLI.md Documentation
**File**: `CLI.md`
**Add search command documentation**:

```markdown
### Search

#### Search for messages
\`\`\`bash
clack search <query>
\`\`\`

Search for messages across all accessible channels.

**Arguments:**
- `<query>` - Text to search for

**Options:**
- `--from <user>` - Filter by user (@username or user ID)
- `--channel <channel>` - Search in specific channel (#name or channel ID)
- `--sort <sort>` - Sort by `score` (default) or `timestamp`
- `--count <n>` - Number of results (default: 20)
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
\`\`\`bash
# Basic text search
clack search "error message"

# Search messages from specific user
clack search "bug" --from @john

# Search in specific channel
clack search "deploy" --channel #general

# Combine filters
clack search "error" --from U1234 --channel C5678 --count 10

# Export search results as JSON
clack search "api" --format json
\`\`\`

**Required Scopes:**
- `search:read` - Required to search messages
```

### Success Criteria:

#### Automated Verification:
- [ ] `make clack` builds successfully
- [ ] `make test` passes all tests including new search tests
- [ ] `clack search --help` displays usage information
- [ ] Search command accepts all specified arguments
- [ ] Unit tests cover search query building and filtering

#### Manual Verification:
- [ ] `clack search "keyword"` returns relevant messages
- [ ] `clack search "test" --from @username` filters by user
- [ ] `clack search "test" --from U1234` filters by user ID
- [ ] `clack search "test" --channel #general` filters by channel name
- [ ] `clack search "test" --channel C1234` filters by channel ID
- [ ] Combining `--from` and `--channel` works correctly
- [ ] Search results show channel name, user, message text, and permalink
- [ ] `--format json` and `--format yaml` work for search results
- [ ] Error handling works for invalid search parameters
- [ ] Results are sorted correctly based on `--sort` option
- [ ] `--count` parameter limits results appropriately

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Testing Strategy

### Unit Tests
- **CLI Parsing**: Test all command variations, options, and argument combinations
- **API Methods**: Mock all Slack API endpoints using mockito
- **Data Models**: Test serialization/deserialization with real-world API response samples
- **Formatters**: Test output generation for various data structures
- **Color Writer**: Test color enabling/disabling based on flags and environment

### Integration Tests
- **Binary Execution**: Use assert_cmd to test actual binary behavior
- **Environment Variables**: Test SLACK_TOKEN handling
- **Error Cases**: Invalid tokens, missing arguments, API errors
- **Output Verification**: Check that help/version flags work correctly

### Manual Testing Steps
1. Test each command with valid SLACK_TOKEN:
   - List users
   - Get specific user
   - List messages from channel
   - Search messages with various filters
2. Test output formats (human, JSON, YAML) for each command
3. Test colorization and `--no-color` flag
4. Test error messages for:
   - Missing SLACK_TOKEN
   - Invalid user/channel IDs
   - API rate limiting
   - Network errors
5. Test edge cases:
   - Empty results
   - Very long message text
   - Special characters in queries
   - Unicode in user names/messages

## Performance Considerations

- **Async Operations**: All API calls use async/await for potential concurrent requests
- **HTTP Connection Pooling**: reqwest automatically handles connection reuse
- **Pagination**: Initial implementation fetches single pages; future work could add pagination support
- **Rate Limiting**: Not implemented in initial version; future consideration for high-volume usage

## Migration Notes

Not applicable - this is a new project with no existing data or systems to migrate.

## Future Enhancements (Out of Scope for Initial Release)

- Channel listing and management commands
- SQLite caching layer with ORM (diesel or sqlx)
- Pagination support for large result sets
- Advanced search filters (date ranges, attachments, etc.)
- Message posting capabilities
- File upload/download
- Configuration file support (~/.clackrc)
- Shell completion scripts
- Webhook management
- Interactive/TUI mode using ratatui

## References

**Slack API Documentation:**
- [Slack API Methods](https://docs.slack.dev/reference/methods/)
- [users.list method](https://docs.slack.dev/reference/methods/users.list/)
- [users.info method](https://api.slack.com/methods/users.info)
- [conversations.history method](https://api.slack.com/methods/conversations.history)
- [search.messages method](https://api.slack.com/methods/search.messages)
- [Token Types](https://api.slack.com/authentication/token-types)
- [OAuth Scopes](https://api.slack.com/scopes)

**Project Files:**
- Task specification: `tasks/2026-01-14-first-steps.md`
- CLI documentation: `CLI.md`
- Project README: `README.md`

**Rust Crates:**
- [clap documentation](https://docs.rs/clap/)
- [reqwest documentation](https://docs.rs/reqwest/)
- [tokio documentation](https://docs.rs/tokio/)
- [termcolor documentation](https://docs.rs/termcolor/)
- [mockito documentation](https://docs.rs/mockito/)
