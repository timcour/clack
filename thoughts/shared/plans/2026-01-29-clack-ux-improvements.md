# Clack UX Improvements Implementation Plan

## Overview

Implement three UX improvements for the clack Slack CLI:
1. Skip workspace API query when `CLACK_WORKSPACE_ID` env var is set
2. Add direct conversation cache lookup by name (avoiding full table scan)
3. Enhance search with pagination, additional modifiers, and name-to-ID resolution

## Current State Analysis

### Workspace ID Initialization
- `client.rs:231-247`: `init_workspace()` always calls `auth.test` API
- Called unconditionally in `main.rs:22` for every command
- No environment variable check exists

### Conversation Cache
- `cache/operations.rs:153-189`: `get_conversation()` only looks up by ID
- `channels.rs:40-60`: `list_channels_and_find()` fetches ALL cached conversations via `get_conversations()`, then filters in memory
- Schema already has `name` column that can be queried directly

### Search Implementation
- CLI options exist for `--from`, `--channel`/`--in`, `--after`, `--before`
- Missing: `--page`, `--has`, `--to`, `--during`
- `SearchPagination` struct exists in models but not displayed in output
- `build_search_query()` doesn't resolve names to IDs

## Desired End State

After implementation:
1. Commands with `CLACK_WORKSPACE_ID` set skip the `auth.test` API call entirely
2. Channel name lookups query the cache directly by name (O(1) vs O(n))
3. Search supports `--page`, `--has`, `--to`, `--during` with validated values
4. Search displays full pagination metadata (page X of Y, per_page, total_count)
5. Search modifiers like `--from @john.smith` resolve to `from:<@USERID>` using cached data
6. Multiple user matches prompt user to specify exact ID

### Verification
- `CLACK_WORKSPACE_ID=T123 clack --verbose users list` shows no `auth.test` call
- Search with `--page 2` returns second page of results
- Search output shows "Page 1 of 5 (20 per page, 100 total)"
- `--from @john` with ambiguous match lists options and exits with error

## What We're NOT Doing

- Adding new database migrations (using existing schema)
- Changing the cache TTL defaults (only adding override capability)
- Adding fuzzy/partial name matching (exact case-insensitive only)
- Caching workspace ID to disk (only env var support)

## Implementation Approach

We'll implement in 6 phases, each building on the previous:
1. Workspace ID env var check (standalone)
2. Cache TTL override support (foundation for later phases)
3. Conversation lookup by name (uses TTL override)
4. User lookup by name (uses TTL override)
5. Search CLI options expansion
6. Search query building with name resolution
7. Search output pagination display

---

## Phase 1: Workspace ID Environment Variable

### Overview
Check `CLACK_WORKSPACE_ID` environment variable before calling `auth.test` API.

### Changes Required

#### 1. SlackClient initialization
**File**: `src/api/client.rs`

Modify `init_workspace()` to check env var first:

```rust
/// Initialize workspace context by calling auth.test (or using env var)
pub async fn init_workspace(&mut self) -> Result<String> {
    if let Some(ref id) = self.workspace_id {
        return Ok(id.clone());
    }

    // Check for CLACK_WORKSPACE_ID environment variable first
    if let Ok(ws_id) = env::var("CLACK_WORKSPACE_ID") {
        if self.verbose {
            eprintln!("Workspace ID from env: {}", ws_id);
        }
        self.workspace_id = Some(ws_id.clone());
        return Ok(ws_id);
    }

    // Fall back to auth.test API call
    use crate::api::auth::test_auth;

    let auth_response = test_auth(self).await?;
    self.workspace_id = Some(auth_response.team_id.clone());

    if self.verbose {
        eprintln!("Workspace: {} ({})", auth_response.team, auth_response.team_id);
    }

    Ok(auth_response.team_id)
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

#### Manual Verification
- [ ] `CLACK_WORKSPACE_ID=T123 clack --verbose users list` shows "Workspace ID from env: T123" and no `auth.test` API call
- [ ] Without env var set, normal `auth.test` behavior continues

---

## Phase 2: Cache TTL Override Support

### Overview
Add optional TTL override parameter to cache lookup functions to support lookups that ignore staleness.

### Changes Required

#### 1. Update cache operations with TTL override
**File**: `src/cache/operations.rs`

Update `get_user()` to accept optional TTL override:

```rust
pub fn get_user(
    conn: &mut CacheConnection,
    ws_id: &str,
    user_id: &str,
    verbose: bool,
    ttl_override: Option<i64>,
) -> Result<Option<User>> {
    use super::schema::users::dsl::*;

    let cached_user: Option<CachedUser> = users
        .filter(id.eq(user_id))
        .filter(workspace_id.eq(ws_id))
        .filter(deleted_at.is_null())
        .first(conn)
        .optional()?;

    match cached_user {
        Some(cached) => {
            let ttl = ttl_override.unwrap_or(USER_TTL_SECONDS);
            if is_fresh(cached.cached_at, ttl) {
                if verbose {
                    eprintln!("[CACHE] User {} - HIT (fresh)", user_id);
                }
                Ok(Some(cached.to_api_user()?))
            } else {
                if verbose {
                    eprintln!("[CACHE] User {} - MISS (stale)", user_id);
                }
                Ok(None)
            }
        }
        None => {
            if verbose {
                eprintln!("[CACHE] User {} - MISS (not found)", user_id);
            }
            Ok(None)
        }
    }
}
```

Similarly update:
- `get_users()` - add `ttl_override: Option<i64>` parameter
- `get_conversation()` - add `ttl_override: Option<i64>` parameter
- `get_conversations()` - add `ttl_override: Option<i64>` parameter

#### 2. Update all callers to pass `None` for TTL override
**Files**: `src/api/users.rs`, `src/api/channels.rs`

Update all existing calls to pass `None` as the TTL override to maintain current behavior:

```rust
// In users.rs get_user()
cache::operations::get_user(&mut conn, workspace_id, user_id, client.verbose(), None)

// In channels.rs get_channel()
cache::operations::get_conversation(&mut conn, workspace_id, channel_id, client.verbose(), None)
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

#### Manual Verification
- [ ] Existing cache behavior unchanged (verify with `--verbose`)

---

## Phase 3: Conversation Cache Lookup by Name

### Overview
Add direct name-based lookup to avoid loading all conversations into memory.

### Changes Required

#### 1. Add get_conversation_by_name function
**File**: `src/cache/operations.rs`

```rust
/// Look up a conversation by name (case-insensitive)
/// Returns all matches to handle ambiguous names
pub fn get_conversation_by_name(
    conn: &mut CacheConnection,
    ws_id: &str,
    name: &str,
    verbose: bool,
    ttl_override: Option<i64>,
) -> Result<Vec<Channel>> {
    use super::schema::conversations::dsl::*;

    // SQLite LIKE is case-insensitive by default for ASCII
    let cached_convs: Vec<CachedConversation> = conversations
        .filter(workspace_id.eq(ws_id))
        .filter(name.eq(name.to_lowercase()))
        .filter(deleted_at.is_null())
        .load(conn)?;

    if cached_convs.is_empty() {
        if verbose {
            eprintln!("[CACHE] Conversation '{}' - MISS (not found by name)", name);
        }
        return Ok(vec![]);
    }

    let ttl = ttl_override.unwrap_or(CONVERSATION_TTL_SECONDS);
    let fresh_convs: Vec<Channel> = cached_convs
        .iter()
        .filter(|c| is_fresh(c.cached_at, ttl))
        .filter_map(|c| c.to_api_channel().ok())
        .collect();

    if fresh_convs.is_empty() {
        if verbose {
            eprintln!("[CACHE] Conversation '{}' - MISS (stale)", name);
        }
        Ok(vec![])
    } else {
        if verbose {
            eprintln!("[CACHE] Conversation '{}' - HIT ({} matches)", name, fresh_convs.len());
        }
        Ok(fresh_convs)
    }
}
```

**Note**: Slack channel names are lowercase, but we'll use case-insensitive comparison for robustness.

#### 2. Update list_channels_and_find to use direct lookup
**File**: `src/api/channels.rs`

```rust
async fn list_channels_and_find(client: &SlackClient, name: &str) -> Result<String> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try direct name lookup in cache first (unless refresh requested)
    if !client.refresh_cache() {
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = cache::get_connection(pool).await {
                let matches = cache::operations::get_conversation_by_name(
                    &mut conn,
                    workspace_id,
                    name,
                    client.verbose(),
                    None,  // Use default TTL
                )?;

                if matches.len() == 1 {
                    if client.verbose() {
                        eprintln!("[CACHE] Channel '{}' resolved from cache to {}", name, matches[0].id);
                    }
                    return Ok(matches[0].id.clone());
                }
                // If 0 or >1 matches, fall through to API
            }
        }
    }

    // Not in cache or ambiguous - search with pagination via API
    // ... existing pagination logic ...
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

#### Manual Verification
- [ ] `clack --verbose conversations info general` shows direct cache lookup message
- [ ] Performance improvement visible for repeated lookups

---

## Phase 4: User Cache Lookup by Name

### Overview
Add name-based user lookup for search modifier resolution.

### Changes Required

#### 1. Add get_user_by_name function
**File**: `src/cache/operations.rs`

```rust
/// Look up users by name (case-insensitive)
/// Matches against both `name` and `profile_display_name` fields
/// Returns all matches to handle ambiguous names
pub fn get_user_by_name(
    conn: &mut CacheConnection,
    ws_id: &str,
    name: &str,
    verbose: bool,
    ttl_override: Option<i64>,
) -> Result<Vec<User>> {
    use super::schema::users::dsl::*;
    use diesel::dsl::sql;
    use diesel::sql_types::Bool;

    let name_lower = name.to_lowercase();

    // Match name or display_name case-insensitively
    let cached_users: Vec<CachedUser> = users
        .filter(workspace_id.eq(ws_id))
        .filter(deleted_at.is_null())
        .filter(
            sql::<Bool>("LOWER(name) = ")
                .bind::<diesel::sql_types::Text, _>(&name_lower)
                .sql(" OR LOWER(profile_display_name) = ")
                .bind::<diesel::sql_types::Text, _>(&name_lower)
        )
        .load(conn)?;

    if cached_users.is_empty() {
        if verbose {
            eprintln!("[CACHE] User '{}' - MISS (not found by name)", name);
        }
        return Ok(vec![]);
    }

    let ttl = ttl_override.unwrap_or(USER_TTL_SECONDS);
    let fresh_users: Vec<User> = cached_users
        .iter()
        .filter(|u| is_fresh(u.cached_at, ttl))
        .filter_map(|u| u.to_api_user().ok())
        .collect();

    if fresh_users.is_empty() {
        if verbose {
            eprintln!("[CACHE] User '{}' - MISS (stale)", name);
        }
        Ok(vec![])
    } else {
        if verbose {
            eprintln!("[CACHE] User '{}' - HIT ({} matches)", name, fresh_users.len());
        }
        Ok(fresh_users)
    }
}
```

#### 2. Add resolve_user_to_id function
**File**: `src/api/users.rs`

```rust
use crate::output::color::ColorWriter;
use crate::output::user_formatter;

/// Resolve a user identifier to a user ID.
/// Accepts user IDs (U123), names (@john.smith or john.smith).
/// Uses cache lookup (ignoring TTL) for name resolution.
/// Returns error with formatted user list if multiple matches found.
pub async fn resolve_user_to_id(client: &SlackClient, identifier: &str) -> Result<String> {
    // Strip @ prefix if present
    let clean_identifier = identifier.strip_prefix('@').unwrap_or(identifier);

    // Check if it looks like a user ID (starts with U or W)
    if clean_identifier.starts_with('U') || clean_identifier.starts_with('W') {
        return Ok(clean_identifier.to_string());
    }

    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Look up by name in cache (use very long TTL to find any cached record)
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = cache::get_connection(pool).await {
            let matches = cache::operations::get_user_by_name(
                &mut conn,
                workspace_id,
                clean_identifier,
                client.verbose(),
                Some(i64::MAX),  // Ignore TTL - use any cached record
            )?;

            match matches.len() {
                0 => {
                    // Not in cache - could fetch from API, but for search modifiers
                    // we want fast resolution. User should run `clack users list` first.
                    anyhow::bail!(
                        "User '{}' not found in cache.\n\n\
                         Run 'clack users list' to populate the cache, then try again.\n\
                         Or specify the user ID directly (e.g., U1234ABCD).",
                        clean_identifier
                    );
                }
                1 => {
                    if client.verbose() {
                        eprintln!("[CACHE] User '{}' resolved to {}", clean_identifier, matches[0].id);
                    }
                    return Ok(matches[0].id.clone());
                }
                _ => {
                    // Multiple matches - format them for display
                    let mut writer = ColorWriter::new(false);
                    writeln!(writer, "Multiple users match '{}':\n", clean_identifier)?;
                    user_formatter::format_users_list(&matches, &mut writer)?;
                    writeln!(writer, "\nPlease specify the exact user ID.")?;

                    anyhow::bail!("{}", writer.into_string()?);
                }
            }
        }
    }

    anyhow::bail!("Cache not available for user lookup")
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
- [ ] Add unit tests for `get_user_by_name` with single/multiple/no matches

#### Manual Verification
- [ ] `resolve_user_to_id` correctly resolves known username
- [ ] Multiple matches displays formatted user list

---

## Phase 5: Search CLI Options Expansion

### Overview
Add `--page`, `--has`, `--to`, `--during` options to search commands.

### Changes Required

#### 1. Update SearchType enum variants
**File**: `src/cli.rs`

Add new fields to `SearchType::Messages`:

```rust
#[derive(Subcommand)]
pub enum SearchType {
    /// Search messages
    Messages {
        /// Search query
        query: String,

        /// Filter by sender (user ID, @username, or display name)
        #[arg(long)]
        from: Option<String>,

        /// Filter by recipient in DMs (user ID, @username, or display name)
        #[arg(long)]
        to: Option<String>,

        /// Filter by channel (channel ID, #name, or name)
        #[arg(long, alias = "in")]
        channel: Option<String>,

        /// Filter by attachment type (link, file, image, etc.)
        #[arg(long)]
        has: Option<String>,

        /// Filter messages after date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        after: Option<String>,

        /// Filter messages before date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        before: Option<String>,

        /// Filter by time period (today, yesterday, week, month, year)
        #[arg(long)]
        during: Option<String>,

        /// Page number (1-indexed)
        #[arg(long, default_value = "1")]
        page: u32,

        /// Maximum number of results per page
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    // ... similar updates for Files and All variants
}
```

#### 2. Add during value validation
**File**: `src/api/search.rs`

```rust
const VALID_DURING_VALUES: &[&str] = &["today", "yesterday", "week", "month", "year"];

pub fn validate_during(value: &str) -> Result<()> {
    if VALID_DURING_VALUES.contains(&value.to_lowercase().as_str()) {
        Ok(())
    } else {
        anyhow::bail!(
            "Invalid --during value: '{}'\n\nValid values are: {}",
            value,
            VALID_DURING_VALUES.join(", ")
        )
    }
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test` passes - add parsing tests for new options
- [ ] `cargo clippy` has no warnings

#### Manual Verification
- [ ] `clack search messages "test" --page 2` parses correctly
- [ ] `clack search messages "test" --during invalid` shows validation error
- [ ] `clack search messages "test" --has link` parses correctly

---

## Phase 6: Search Query Building with Name Resolution

### Overview
Update `build_search_query()` to resolve names to IDs and pass page parameter to API.

### Changes Required

#### 1. Update search functions to accept page parameter
**File**: `src/api/search.rs`

```rust
pub async fn search_messages(
    client: &SlackClient,
    query: &str,
    count: Option<u32>,
    page: Option<u32>,
) -> Result<SearchMessagesResponse> {
    let mut params = vec![("query", query.to_string())];

    if let Some(c) = count {
        params.push(("count", c.to_string()));
    }

    if let Some(p) = page {
        params.push(("page", p.to_string()));
    }

    let response: SearchMessagesResponse = client.get("search.messages", &params).await?;
    // ... rest unchanged
}
```

#### 2. Create async build_search_query with resolution
**File**: `src/api/search.rs`

```rust
/// Builds a Slack search query with filters, resolving names to IDs
pub async fn build_search_query_with_resolution(
    client: &SlackClient,
    text: &str,
    from_user: Option<&str>,
    to_user: Option<&str>,
    in_channel: Option<&str>,
    has: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    during: Option<&str>,
) -> Result<String> {
    let mut query = text.to_string();

    // Resolve and add from: modifier
    if let Some(user) = from_user {
        let user_id = crate::api::users::resolve_user_to_id(client, user).await?;
        query.push_str(&format!(" from:<@{}>", user_id));
    }

    // Resolve and add to: modifier
    if let Some(user) = to_user {
        let user_id = crate::api::users::resolve_user_to_id(client, user).await?;
        query.push_str(&format!(" to:<@{}>", user_id));
    }

    // Resolve and add in: modifier
    if let Some(channel) = in_channel {
        let channel_id = crate::api::channels::resolve_channel_id(client, channel).await?;
        query.push_str(&format!(" in:<#{}>", channel_id));
    }

    // Add has: modifier (no resolution needed)
    if let Some(has_type) = has {
        query.push_str(&format!(" has:{}", has_type));
    }

    // Add date filters
    if let Some(after_date) = after {
        query.push_str(&format!(" after:{}", after_date));
    }

    if let Some(before_date) = before {
        query.push_str(&format!(" before:{}", before_date));
    }

    // Add during filter (validated earlier in CLI handling)
    if let Some(during_period) = during {
        query.push_str(&format!(" during:{}", during_period));
    }

    Ok(query)
}
```

#### 3. Update main.rs to use new search flow
**File**: `src/main.rs`

```rust
Commands::Search { search_type } => match search_type {
    SearchType::Messages {
        query,
        from,
        to,
        channel,
        has,
        after,
        before,
        during,
        page,
        limit,
    } => {
        // Validate during if provided
        if let Some(ref d) = during {
            api::search::validate_during(d)?;
        }

        // Build search query with name resolution
        let search_query = api::search::build_search_query_with_resolution(
            &client,
            &query,
            from.as_deref(),
            to.as_deref(),
            channel.as_deref(),
            has.as_deref(),
            after.as_deref(),
            before.as_deref(),
            during.as_deref(),
        ).await?;

        let response = api::search::search_messages(&client, &search_query, Some(limit), Some(page)).await?;
        // ... rest of formatting
    }
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test` passes
- [ ] Add test for `build_search_query_with_resolution`
- [ ] `cargo clippy` has no warnings

#### Manual Verification
- [ ] `clack search messages "test" --from @john.smith` resolves to user ID
- [ ] `clack search messages "test" --in general` resolves to channel ID
- [ ] `clack search messages "test" --page 2` returns page 2

---

## Phase 7: Search Output Pagination Display

### Overview
Display full pagination metadata in human-formatted search output.

### Changes Required

#### 1. Update format_search_messages
**File**: `src/output/search_formatter.rs`

```rust
pub fn format_search_messages(
    response: &SearchMessagesResponse,
    users: &HashMap<String, User>,
    writer: &mut ColorWriter,
) -> Result<()> {
    // Header with result count
    writer.print_header(&format!(
        "Search: '{}'",
        response.query
    ))?;

    // Pagination metadata
    if let Some(ref pagination) = response.messages.pagination {
        writer.print_colored(
            &format!(
                "Page {} of {} ({} per page, {} total results)",
                pagination.page,
                pagination.page_count,
                pagination.per_page,
                pagination.total_count,
            ),
            Color::Blue,
        )?;
        writer.writeln()?;
    } else {
        writer.write(&format!(
            "Found {} message{}",
            response.messages.total,
            if response.messages.total == 1 { "" } else { "s" }
        ))?;
        writer.writeln()?;
    }

    writer.print_separator()?;

    // ... rest of message formatting unchanged
}
```

Apply similar changes to:
- `format_search_files()`
- `format_search_all()`

### Success Criteria

#### Automated Verification
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

#### Manual Verification
- [ ] Search output shows "Page 1 of 5 (20 per page, 100 total results)"
- [ ] Works correctly when pagination data is missing (fallback to simple count)

---

## Testing Strategy

### Unit Tests
- `get_user_by_name` with 0, 1, and multiple matches
- `get_conversation_by_name` with 0, 1, and multiple matches
- `validate_during` with valid and invalid values
- `build_search_query_with_resolution` query string construction
- CLI parsing for new options

### Integration Tests
- End-to-end search with `--from @username` resolution
- Workspace ID from env var skips API call
- Cache lookup by name performance

### Manual Testing Steps
1. Set `CLACK_WORKSPACE_ID=T123` and run `clack --verbose users list` - verify no auth.test call
2. Run `clack users list` to populate cache
3. Run `clack search messages "test" --from @<known_user>` - verify ID resolution
4. Run `clack search messages "test" --from ambiguous` with multiple matches - verify error display
5. Run `clack search messages "test" --page 2 --limit 10` - verify pagination output
6. Run `clack search messages "test" --during invalid` - verify validation error

## Performance Considerations

- Direct name lookup in SQLite is O(log n) with index vs O(n) full scan
- Consider adding index on `conversations.name` if not already present
- TTL override with `i64::MAX` avoids cache miss for stale but valid records

## References

- Slack search modifiers: https://slack.com/help/articles/202528808-Search-in-Slack
- Current implementation: `src/api/search.rs`, `src/cache/operations.rs`
