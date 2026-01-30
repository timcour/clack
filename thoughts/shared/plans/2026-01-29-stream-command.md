# Plan: Stream Command for Real-time Monitoring

## Overview

Add a top-level `clack stream` command that continuously polls for new data and outputs formatted results in real-time until the user sends SIGINT (Ctrl+C).

## Example Usage

```bash
# Stream new messages mentioning a user
clack stream search messages '<@U123ABC>'

# Stream messages in a channel
clack stream search messages --channel general "deploy"

# Stream all messages from a user
clack stream search messages --from alice ""

# With custom poll interval
clack stream --interval 30 search messages "keyword"
```

## Architecture

### Phase 1: CLI Structure

Add new `Stream` command variant to `cli.rs`:

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...

    /// Stream real-time updates (runs until Ctrl+C)
    Stream {
        /// Poll interval in seconds
        #[arg(long, default_value = "10")]
        interval: u64,

        /// Output format (defaults to human-compact for streaming)
        #[arg(long)]
        format: Option<String>,  // None means use "human-compact"

        #[command(subcommand)]
        stream_type: StreamType,
    },
}

#[derive(Subcommand)]
pub enum StreamType {
    /// Stream search results
    Search {
        #[command(subcommand)]
        search_type: StreamSearchType,
    },
}

#[derive(Subcommand)]
pub enum StreamSearchType {
    /// Stream message search results
    Messages {
        query: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, alias = "in")]
        channel: Option<String>,
        #[arg(long)]
        has: Option<String>,
    },
}
```

### Phase 2: Signal Handling

Create `src/stream/mod.rs` with SIGINT handling:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn setup_signal_handler() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        eprintln!("\nStopping stream...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    running
}
```

### Phase 3: Stream State Management

Track seen messages to avoid duplicates:

```rust
pub struct StreamState {
    /// Set of seen message timestamps (ts values are unique per channel)
    seen_messages: HashSet<(String, String)>, // (channel_id, ts)

    /// Last poll timestamp
    last_poll: Instant,

    /// Poll interval
    interval: Duration,
}

impl StreamState {
    pub fn new(interval_secs: u64) -> Self {
        Self {
            seen_messages: HashSet::new(),
            last_poll: Instant::now(),
            interval: Duration::from_secs(interval_secs),
        }
    }

    /// Returns true if this message is new (not seen before)
    pub fn is_new(&mut self, channel_id: &str, ts: &str) -> bool {
        self.seen_messages.insert((channel_id.to_string(), ts.to_string()))
    }

    /// Wait for next poll interval
    pub async fn wait_for_next_poll(&mut self) {
        let elapsed = self.last_poll.elapsed();
        if elapsed < self.interval {
            tokio::time::sleep(self.interval - elapsed).await;
        }
        self.last_poll = Instant::now();
    }
}
```

### Phase 4: Stream Loop Implementation

Main streaming logic in `src/stream/search.rs`:

```rust
pub async fn stream_search_messages(
    client: &SlackClient,
    query: &str,
    interval_secs: u64,
    format: &str,
    no_color: bool,
) -> Result<()> {
    let running = setup_signal_handler();
    let mut state = StreamState::new(interval_secs);

    eprintln!("Streaming messages matching '{}' (Ctrl+C to stop)...\n", query);

    while running.load(Ordering::SeqCst) {
        // Fetch latest results
        let response = search_messages(client, query, Some(20), Some(1)).await?;

        // Cache ALL fetched messages immediately (before filtering)
        cache_search_messages(client, &response.messages.matches).await;

        // Filter to only new messages (for display)
        let new_messages: Vec<_> = response.messages.matches
            .iter()
            .filter(|msg| {
                if let Some(ref channel) = msg.channel {
                    state.is_new(channel.id(), &msg.ts)
                } else {
                    state.is_new("unknown", &msg.ts)
                }
            })
            .collect();

        // Format and output new messages
        if !new_messages.is_empty() {
            // Fetch user info for formatting
            let mut user_map = HashMap::new();
            for msg in &new_messages {
                if let Some(ref user_id) = msg.user {
                    if !user_map.contains_key(user_id) {
                        if let Ok(user) = get_user(client, user_id).await {
                            user_map.insert(user.id.clone(), user);
                        }
                    }
                }
            }

            // Output based on format
            match format {
                "json" => {
                    for msg in &new_messages {
                        println!("{}", serde_json::to_string(msg)?);
                    }
                }
                "yaml" => {
                    for msg in &new_messages {
                        println!("{}", serde_yaml::to_string(msg)?);
                    }
                }
                "human-compact" => {
                    let mut writer = ColorWriter::new(no_color);
                    for msg in &new_messages {
                        format_message_compact(msg, &user_map, &mut writer)?;
                    }
                    print!("{}", writer.into_string()?);
                }
                _ => {
                    // "human" - full format
                    let mut writer = ColorWriter::new(no_color);
                    for msg in &new_messages {
                        format_search_message(msg, &user_map, &mut writer)?;
                        writer.writeln()?;
                    }
                    print!("{}", writer.into_string()?);
                }
            }
        }

        // Wait for next poll
        state.wait_for_next_poll().await;
    }

    eprintln!("Stream stopped.");
    Ok(())
}
```

### Phase 5: Output Formatting

Add `human-compact` format option to the global `--format` choices in `cli.rs`:

```rust
/// Output format
#[arg(long, global = true, default_value = "human")]
pub format: String,  // "human", "human-compact", "json", "yaml"
```

For streaming, default to `human-compact` unless overridden:

```rust
Commands::Stream { interval, format, stream_type } => {
    // Default to human-compact for streaming if not specified
    let effective_format = format.as_deref().unwrap_or("human-compact");
    // ...
}
```

Add compact formatter in `src/output/message_formatter.rs`:

```rust
/// Format a single message in compact single-line format
/// Includes: timestamp, channel, user, truncated text, and permalink
pub fn format_message_compact(
    msg: &Message,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    // Parse message timestamp for display
    let ts_float: f64 = msg.ts.parse().unwrap_or(0.0);
    let dt_utc = DateTime::from_timestamp(ts_float as i64, 0).unwrap_or_default();
    let dt_local: DateTime<Local> = dt_utc.into();

    // Timestamp prefix
    writer.print_colored(&format!("[{}] ", dt_local.format("%Y-%m-%d %H:%M")), Color::White)?;

    // Channel
    if let Some(channel) = &msg.channel {
        if let Some(name) = channel.name() {
            writer.print_colored(&format!("#{}", name), Color::Green)?;
        } else {
            writer.print_colored(&format!("#{}", channel.id()), Color::Green)?;
        }
        writer.write(" ")?;
    }

    // User
    if let Some(user_id) = &msg.user {
        if let Some(user) = users.get(user_id) {
            writer.print_colored(&format!("@{}", user.name), Color::Cyan)?;
        } else {
            writer.print_colored(user_id, Color::Cyan)?;
        }
    }
    writer.write(": ")?;

    // Message text (single line, truncated if needed)
    let text = msg.text.replace('\n', " ");
    let max_len = 80;
    let truncated = if text.len() > max_len {
        format!("{}...", &text[..max_len - 3])
    } else {
        text
    };
    writer.write(&truncated)?;

    // Permalink (always include for compact format)
    if let Some(permalink) = &msg.permalink {
        writer.write(" ")?;
        writer.print_colored(permalink, Color::Blue)?;
    }

    writer.writeln()?;
    Ok(())
}
```

**Example output:**

```
[2026-01-29 14:32] #general @alice: Just deployed the new feature to production... https://myteam.slack.com/archives/C123/p1234567890
[2026-01-29 14:35] #engineering @bob: @alice looks good! Tests are passing https://myteam.slack.com/archives/C456/p1234567891
```

The `human-compact` format can also be used outside of streaming:

```bash
# Use compact format for regular search
clack --format human-compact search messages "deploy"
```
```

### Phase 6: Main Integration

Update `main.rs` to handle stream command:

```rust
Commands::Stream { interval, stream_type } => {
    match stream_type {
        StreamType::Search { search_type } => {
            match search_type {
                StreamSearchType::Messages { query, from, to, channel, has } => {
                    // Build query with filters
                    let search_query = build_search_query_full(
                        &query,
                        from.as_deref(),
                        to.as_deref(),
                        channel.as_deref(),
                        has.as_deref(),
                        None, None, None,
                    );

                    stream::search::stream_search_messages(
                        &client,
                        &search_query,
                        interval,
                        cli.no_color,
                    ).await?;
                }
            }
        }
    }
}
```

## Dependencies

Add to `Cargo.toml`:

```toml
ctrlc = { version = "3.4", features = ["termination"] }
```

## File Structure

```
src/
├── stream/
│   ├── mod.rs          # Signal handling, StreamState
│   └── search.rs       # Search streaming implementation
├── output/
│   └── stream_formatter.rs  # Stream-specific formatting
```

## Future Extensions

1. **WebSocket support**: Use Slack's RTM API or Socket Mode for true real-time (requires different auth scope)
2. **Stream other data types**: Files, reactions, channel activity
3. **Filters**: Time-based filters (--since), count limits
4. **Output modes**: JSON streaming (newline-delimited), quiet mode (just counts)
5. **Notifications**: Desktop notifications for matches

## Testing Strategy

1. **Unit tests**: StreamState deduplication logic
2. **Integration tests**: Mock server with changing results
3. **Manual testing**: Real Slack workspace streaming

## Success Criteria

1. `clack stream search messages "keyword"` polls and shows new messages
2. Ctrl+C gracefully stops the stream
3. No duplicate messages are shown
4. Output is formatted consistently with existing formatters
5. Messages are cached for offline access
