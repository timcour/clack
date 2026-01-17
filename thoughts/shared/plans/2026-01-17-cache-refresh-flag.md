# Cache Refresh Flag Implementation Plan

## Overview

Add a `--refresh-cache` global flag to force fresh API queries and update the cache with the latest data from Slack's API, bypassing stale or cached data.

## Current State Analysis

The clack CLI tool currently implements a write-through cache pattern:
- Cache is stored in a SQLite database using Diesel ORM
- TTL-based freshness checking (7 days for users, conversations, messages)
- Cache-reading functions check cache first, fall back to API if stale/missing
- All successful API responses write through to cache (best-effort)

### Key Discoveries

**Cache-Reading Functions** (need modification):
- `src/api/users.rs::get_user()` - Line 46: tries cache before API
- `src/api/channels.rs::get_channel()` - Line 199: tries cache before API
- `src/api/channels.rs::resolve_channel_id()` - Line 48: tries cache when resolving channel names

**List Functions** (no changes needed):
- `list_users()`, `list_channels()`, `list_messages()` already bypass cache and always query API
- Already write-through to cache after fetching from API

**CLI Global Flags Pattern**:
- Defined in `src/cli.rs` with `#[arg(long, global = true)]`
- Current global flags: `no_color`, `format`, `verbose`, `debug_response`
- Passed to `SlackClient` constructor in `src/main.rs` (line 19)
- Stored in `SlackClient` struct in `src/api/client.rs` (lines 19-20)

## Desired End State

When users specify `--refresh-cache`:
- All API calls bypass cache reading
- API is queried directly for all requests
- Cache is updated with fresh data from successful API responses
- Flag can be placed anywhere in command (before or after subcommands)

### Success Verification

**Automated:**
- Run all tests: `cargo test`
- Verify CLI parsing accepts flag in multiple positions
- Verify cache-skipping behavior in unit tests

**Manual:**
- Run command with `--refresh-cache` flag
- Verify with `--verbose` that cache reads are skipped
- Verify cache is still updated with fresh data
- Test flag positioning: both `clack --refresh-cache users info U123` and `clack users info --refresh-cache U123` work

## What We're NOT Doing

- Not changing the cache write-through behavior
- Not modifying cache TTL values
- Not changing list operations (they already bypass cache)
- Not adding cache invalidation logic
- Not modifying the cache database schema
- Not changing how `--verbose` logs cache operations

## Implementation Approach

Use the existing global flag pattern (`verbose`, `debug_response`) as a model. Add `refresh_cache` flag to CLI, pass it through to `SlackClient`, and conditionally skip cache reads in API functions.

## Phase 1: Add Global Flag and Thread Through Application

### Overview
Add the `--refresh-cache` CLI flag and thread it through to `SlackClient`.

### Changes Required

#### 1. Add CLI Flag Definition
**File**: `src/cli.rs`
**Changes**: Add `refresh_cache` field to `Cli` struct

```rust
/// Force cache refresh - bypass cache and query API directly
#[arg(long, global = true)]
pub refresh_cache: bool,
```

**Location**: After line 25 (after `debug_response` field)

#### 2. Update SlackClient Constructor Call
**File**: `src/main.rs`
**Changes**: Pass `refresh_cache` to `SlackClient::new()`

Find line 19:
```rust
let client = SlackClient::new(cli.verbose, cli.debug_response).await?;
```

Replace with:
```rust
let client = SlackClient::new(cli.verbose, cli.debug_response, cli.refresh_cache).await?;
```

#### 3. Update SlackClient Struct and Methods
**File**: `src/api/client.rs`

**Changes**:

a) Add field to struct (line 20, after `debug_response`):
```rust
pub struct SlackClient {
    client: reqwest::Client,
    base_url: String,
    verbose: bool,
    debug_response: bool,
    refresh_cache: bool,  // ADD THIS LINE
    workspace_id: Option<String>,
    cache_pool: Option<CachePool>,
}
```

b) Update `new()` method signature (line 30):
```rust
pub async fn new(verbose: bool, debug_response: bool, refresh_cache: bool) -> Result<Self> {
    Self::with_base_url("https://slack.com/api", verbose, debug_response, refresh_cache).await
}
```

c) Update `new_verbose()` method (line 26):
```rust
pub async fn new_verbose(verbose: bool) -> Result<Self> {
    Self::with_base_url("https://slack.com/api", verbose, false, false).await
}
```

d) Update `with_base_url()` method signature and body (line 34):
```rust
pub async fn with_base_url(base_url: &str, verbose: bool, debug_response: bool, refresh_cache: bool) -> Result<Self> {
    // ... existing token and client setup code ...

    Ok(Self {
        client,
        base_url: base_url.to_string(),
        verbose,
        debug_response,
        refresh_cache,  // ADD THIS LINE
        workspace_id: None,
        cache_pool,
    })
}
```

e) Add getter method (after line 260):
```rust
/// Check if cache refresh is enabled
pub fn refresh_cache(&self) -> bool {
    self.refresh_cache
}
```

### Success Criteria

#### Automated Verification:
- [ ] Code compiles: `cargo build`
- [ ] All existing tests pass: `cargo test`

#### Manual Verification:
- [ ] Application starts without errors
- [ ] `--help` shows the new `--refresh-cache` flag

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation that the application builds and runs correctly before proceeding to Phase 2.

---

## Phase 2: Update Cache-Reading Functions

### Overview
Modify API functions that read from cache to skip cache when `refresh_cache` is enabled.

### Changes Required

#### 1. Update `get_user()` Function
**File**: `src/api/users.rs`
**Changes**: Skip cache check when `refresh_cache` is true

Find the cache check block (lines 45-69):
```rust
// Try cache first
if let Some(pool) = client.cache_pool() {
    match cache::get_connection(pool).await {
        Ok(mut conn) => {
            match cache::operations::get_user(&mut conn, workspace_id, user_id, client.verbose()) {
                Ok(Some(cached_user)) => {
                    return Ok(cached_user);
                }
                Ok(None) => {
                    // Cache miss, continue to API
                }
                Err(e) => {
                    if client.verbose() {
                        eprintln!("[CACHE] Error reading cache: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            if client.verbose() {
                eprintln!("[CACHE] Failed to get connection: {}", e);
            }
        }
    }
}
```

Replace with:
```rust
// Try cache first (unless refresh requested)
if !client.refresh_cache() {
    if let Some(pool) = client.cache_pool() {
        match cache::get_connection(pool).await {
            Ok(mut conn) => {
                match cache::operations::get_user(&mut conn, workspace_id, user_id, client.verbose()) {
                    Ok(Some(cached_user)) => {
                        return Ok(cached_user);
                    }
                    Ok(None) => {
                        // Cache miss, continue to API
                    }
                    Err(e) => {
                        if client.verbose() {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                if client.verbose() {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }
} else if client.verbose() {
    eprintln!("[CACHE] User {} - SKIP (refresh requested)", user_id);
}
```

#### 2. Update `get_channel()` Function
**File**: `src/api/channels.rs`
**Changes**: Skip cache check when `refresh_cache` is true

Find the cache check block (lines 198-220):
```rust
// Try cache first
if let Some(pool) = client.cache_pool() {
    match cache::get_connection(pool).await {
        Ok(mut conn) => {
            match cache::operations::get_conversation(&mut conn, workspace_id, channel_id, client.verbose()) {
                Ok(Some(cached_channel)) => {
                    return Ok(cached_channel);
                }
                Ok(None) => {
                    // Cache miss, continue to API
                }
                Err(e) => {
                    if client.verbose() {
                        eprintln!("[CACHE] Error reading cache: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            if client.verbose() {
                eprintln!("[CACHE] Failed to get connection: {}", e);
            }
        }
    }
}
```

Replace with:
```rust
// Try cache first (unless refresh requested)
if !client.refresh_cache() {
    if let Some(pool) = client.cache_pool() {
        match cache::get_connection(pool).await {
            Ok(mut conn) => {
                match cache::operations::get_conversation(&mut conn, workspace_id, channel_id, client.verbose()) {
                    Ok(Some(cached_channel)) => {
                        return Ok(cached_channel);
                    }
                    Ok(None) => {
                        // Cache miss, continue to API
                    }
                    Err(e) => {
                        if client.verbose() {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                if client.verbose() {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }
} else if client.verbose() {
    eprintln!("[CACHE] Conversation {} - SKIP (refresh requested)", channel_id);
}
```

#### 3. Update `resolve_channel_id()` Function
**File**: `src/api/channels.rs`
**Changes**: Skip cache search when `refresh_cache` is true

Find the cache check for name resolution (lines 45-56):
```rust
// Try cache first for name-based lookup
if let Some(pool) = client.cache_pool() {
    if let Ok(mut conn) = cache::get_connection(pool).await {
        if let Ok(Some(cached_channels)) = cache::operations::get_conversations(&mut conn, workspace_id, client.verbose()) {
            // Search cached channels first
            if let Some(channel) = cached_channels.iter().find(|ch| ch.name == name) {
                return Ok(channel.id.clone());
            }
        }
    }
}
```

Replace with:
```rust
// Try cache first for name-based lookup (unless refresh requested)
if !client.refresh_cache() {
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = cache::get_connection(pool).await {
            if let Ok(Some(cached_channels)) = cache::operations::get_conversations(&mut conn, workspace_id, client.verbose()) {
                // Search cached channels first
                if let Some(channel) = cached_channels.iter().find(|ch| ch.name == name) {
                    return Ok(channel.id.clone());
                }
            }
        }
    }
}
```

### Success Criteria

#### Automated Verification:
- [ ] Code compiles: `cargo build`
- [ ] All existing tests pass: `cargo test`

#### Manual Verification:
- [ ] Run `clack --verbose --refresh-cache users info U123` and verify cache is skipped
- [ ] Run `clack --verbose users info U123` (without flag) and verify cache is used
- [ ] Verify cache still gets updated after API calls with `--refresh-cache`

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual testing to verify cache-skipping behavior before proceeding to Phase 3.

---

## Phase 3: Add Unit Tests

### Overview
Add comprehensive unit tests for the new `--refresh-cache` flag behavior.

### Changes Required

#### 1. Add CLI Parsing Tests
**File**: `src/cli.rs`
**Changes**: Add tests to verify flag parsing

Add after line 674 (in the `#[cfg(test)]` module):

```rust
#[test]
fn test_global_refresh_cache_option() {
    let cli = Cli::parse_from(["clack", "--refresh-cache", "users", "list"]);
    assert!(cli.refresh_cache);
}

#[test]
fn test_refresh_cache_before_subcommand() {
    let cli = Cli::parse_from(["clack", "--refresh-cache", "conversations", "info", "C123"]);
    assert!(cli.refresh_cache);
    match cli.command {
        Commands::Conversations { command } => match command {
            ConversationsCommands::Info { channel } => {
                assert_eq!(channel, "C123");
            }
            _ => panic!("Expected Conversations Info command"),
        },
        _ => panic!("Expected Conversations command"),
    }
}

#[test]
fn test_refresh_cache_after_subcommand() {
    let cli = Cli::parse_from(["clack", "conversations", "info", "--refresh-cache", "C123"]);
    assert!(cli.refresh_cache);
}

#[test]
fn test_refresh_cache_default_false() {
    let cli = Cli::parse_from(["clack", "users", "list"]);
    assert!(!cli.refresh_cache);
}
```

#### 2. Add Cache-Skipping Test for Users
**File**: `src/api/users.rs`
**Changes**: Add test to verify cache is skipped when flag is set

Add after line 296 (in the `#[cfg(test)]` module):

```rust
#[tokio::test]
async fn test_get_user_with_refresh_cache() {
    let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let workspace_id = format!("T{}", test_id);

    let mut server = mockito::Server::new_async().await;
    std::env::set_var("SLACK_TOKEN", "xoxb-test-token");

    // Create client with refresh_cache=true
    let mut client = SlackClient::with_base_url(&server.url(), false, false, true).await.unwrap();

    // Mock auth.test
    let auth_body = format!(
        r#"{{"ok": true, "url": "https://test.slack.com/", "team_id": "{}", "team": "Test Team", "user": "testuser", "user_id": "U123"}}"#,
        workspace_id
    );
    let _auth_mock = server
        .mock("GET", "/auth.test")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(auth_body)
        .create();

    client.init_workspace().await.unwrap();

    // Pre-populate cache with stale data
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = cache::get_connection(pool).await {
            let stale_user = User {
                id: "U123".to_string(),
                name: "staleuser".to_string(),
                real_name: "Stale User".to_string(),
                deleted: false,
                is_bot: false,
                profile: crate::models::user::UserProfile {
                    email: Some("stale@example.com".to_string()),
                    display_name: "staleuser".to_string(),
                    real_name: "Stale User".to_string(),
                    ..Default::default()
                },
            };
            let _ = cache::operations::upsert_user(&mut conn, &workspace_id, &stale_user, false);
        }
    }

    // Mock API response with fresh data
    let _mock = server
        .mock("GET", "/users.info?user=U123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
            "ok": true,
            "user": {
                "id": "U123",
                "name": "freshuser",
                "real_name": "Fresh User",
                "deleted": false,
                "is_bot": false,
                "profile": {
                    "email": "fresh@example.com",
                    "display_name": "freshuser"
                }
            }
        }"#,
        )
        .create_async()
        .await;

    // Call get_user - should skip cache and get fresh data from API
    let user = get_user(&client, "U123").await.unwrap();
    assert_eq!(user.name, "freshuser", "Should get fresh data from API, not stale cache");
    assert_eq!(user.profile.email, Some("fresh@example.com".to_string()));
}
```

#### 3. Add Cache-Skipping Test for Channels
**File**: `src/api/channels.rs`
**Changes**: Add test to verify cache is skipped when flag is set

Find the `#[cfg(test)]` module and add:

```rust
#[tokio::test]
async fn test_get_channel_with_refresh_cache() {
    let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let workspace_id = format!("T{}", test_id);

    let mut server = mockito::Server::new_async().await;
    std::env::set_var("SLACK_TOKEN", "xoxb-test-token");

    // Create client with refresh_cache=true
    let mut client = SlackClient::with_base_url(&server.url(), false, false, true).await.unwrap();

    // Mock auth.test
    let auth_body = format!(
        r#"{{"ok": true, "url": "https://test.slack.com/", "team_id": "{}", "team": "Test Team", "user": "testuser", "user_id": "U123"}}"#,
        workspace_id
    );
    let _auth_mock = server
        .mock("GET", "/auth.test")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(auth_body)
        .create();

    client.init_workspace().await.unwrap();

    // Pre-populate cache with stale data
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = crate::cache::get_connection(pool).await {
            let stale_channel = Channel {
                id: "C123".to_string(),
                name: "stale-channel".to_string(),
                is_channel: true,
                is_private: false,
                is_archived: false,
                ..Default::default()
            };
            let _ = crate::cache::operations::upsert_conversation(&mut conn, &workspace_id, &stale_channel, false);
        }
    }

    // Mock API response with fresh data
    let _mock = server
        .mock("GET", "/conversations.info?channel=C123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
            "ok": true,
            "channel": {
                "id": "C123",
                "name": "fresh-channel",
                "is_channel": true,
                "is_private": false,
                "is_archived": false
            }
        }"#,
        )
        .create_async()
        .await;

    // Call get_channel - should skip cache and get fresh data from API
    let channel = get_channel(&client, "C123").await.unwrap();
    assert_eq!(channel.name, "fresh-channel", "Should get fresh data from API, not stale cache");
}
```

### Success Criteria

#### Automated Verification:
- [ ] All new tests pass: `cargo test test_global_refresh_cache_option`
- [ ] All new tests pass: `cargo test test_refresh_cache_before_subcommand`
- [ ] All new tests pass: `cargo test test_refresh_cache_after_subcommand`
- [ ] All new tests pass: `cargo test test_refresh_cache_default_false`
- [ ] All new tests pass: `cargo test test_get_user_with_refresh_cache`
- [ ] All new tests pass: `cargo test test_get_channel_with_refresh_cache`
- [ ] All existing tests still pass: `cargo test`
- [ ] Code compiles without warnings: `cargo build`

#### Manual Verification:
- [ ] Run test suite and verify all tests pass
- [ ] Verify test coverage includes both flag positions (before and after subcommand)

**Implementation Note**: After completing this phase and all automated verification passes, the implementation is complete.

---

## Testing Strategy

### Unit Tests
- CLI parsing tests verify flag is recognized in multiple positions
- API tests verify cache is skipped when `refresh_cache=true`
- API tests verify cache is still used when `refresh_cache=false`
- API tests verify cache is updated after API calls even with `refresh_cache=true`

### Integration Tests
Already covered by existing integration tests in `tests/integration_test.rs`. No new integration tests needed since we're using existing patterns.

### Manual Testing Steps
1. Run `clack --help` and verify `--refresh-cache` appears in global options
2. Run `clack --verbose users info U123` (without flag) - verify cache hit on second run
3. Run `clack --verbose --refresh-cache users info U123` - verify cache skip message
4. Test both flag positions:
   - `clack --refresh-cache conversations info C123`
   - `clack conversations info --refresh-cache C123`
5. Verify cache is still updated by checking verbose output

## Performance Considerations

**No Negative Impact:**
- Flag check is a simple boolean comparison (negligible overhead)
- Only affects functions that already check cache
- Cache write-through behavior unchanged
- No additional API calls for list operations (already bypass cache)

**Expected Behavior:**
- With flag: slightly slower (no cache hit possible) but guarantees fresh data
- Without flag: same performance as before

## References

- Original task: `tasks/2026-01-17-cache-refresh.md`
- Related cache implementation: `tasks/2026-01-15-object-caching.md`
- Cache operations: `src/cache/operations.rs`
- CLI parsing: `src/cli.rs`
- API client: `src/api/client.rs`
