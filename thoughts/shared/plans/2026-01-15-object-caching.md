# Object Caching Implementation Plan

## Overview

Implement local SQLite caching for Slack API objects to reduce API calls and improve performance. The cache will store users, conversations (channels), messages, files, and reactions with workspace isolation, TTL-based freshness checks, and automatic write-through caching.

## Current State Analysis

**Existing Architecture:**
- All Slack API data is fetched on every command execution (no caching)
- `SlackClient` (`src/api/client.rs:14-18`) uses `reqwest` for HTTP requests
- Models (`src/models/`) are simple serde structs matching Slack API responses
- No workspace tracking - only `SLACK_TOKEN` environment variable
- Tokio async runtime already in place
- Error handling uses `anyhow::Result` with silent fallback patterns

**Data Flow:**
1. CLI parses command → `main.rs:18`
2. Creates `SlackClient` → `main.rs:15`
3. Calls API functions (e.g., `api::users::list_users`) → `src/api/users.rs:5`
4. HTTP GET via `client.get()` → `src/api/client.rs:50`
5. Deserializes JSON to model structs → `serde_json::from_str`
6. Formats output → `src/output/`

**Key Constraints:**
- Must maintain backward compatibility with existing CLI commands
- Cache must be workspace-aware (multi-workspace support)
- Message caching is opt-in only (via `--use-cache` flag)
- Silent fallback to API on cache errors (unless `--verbose`)

## Desired End State

### User Experience
- **Faster command execution**: Users/channels loaded from cache when fresh
- **Reduced API calls**: Significant reduction in rate limiting issues
- **Opt-in message caching**: Users can choose to use cached messages via `--use-cache`
- **Cache transparency**: `--debug` flag shows cache statistics
- **Cache control**: `clack cache clear` command for manual cache invalidation

### Technical Specification
- SQLite database at platform-specific cache directory
- Workspace isolation via `workspace_id` column on all tables
- TTL-based freshness: Users (1hr), Channels (30min), Messages (5min)
- Write-through caching: All API responses automatically update cache
- Connection pooling via deadpool for concurrent access
- WAL mode for better read performance
- Comprehensive test coverage with in-memory SQLite

### Verification
**Automated:**
- All existing tests pass
- New cache tests pass (hit/miss, TTL, workspace isolation)
- Migration runs successfully
- No compilation errors

**Manual:**
- Cache directory created on first use
- `clack users` faster on second run (cache hit)
- `--debug` shows cache statistics
- `clack cache clear` removes cached data
- Multiple workspaces don't interfere with each other

## What We're NOT Doing

- ❌ Caching search results (too dynamic per spec)
- ❌ Default message caching (opt-in only via `--use-cache`)
- ❌ Encryption of cache database (local CLI app)
- ❌ Cache size limits (can add later if needed)
- ❌ Background cache refresh/warming
- ❌ Cache versioning for clack upgrades (migrations handle schema changes)
- ❌ Distributed caching or network storage
- ❌ Caching pagination cursors

## Implementation Approach

### High-Level Strategy
1. **Non-Breaking**: All changes are additive - existing commands work unchanged
2. **Layered**: Cache sits between API layer and database, transparent to CLI
3. **Graceful Degradation**: Cache failures fall back to API silently
4. **Test-Driven**: Each phase includes tests before moving forward

### Technology Choices
- **diesel-async**: Async ORM matching our tokio runtime
- **deadpool**: Connection pooling for concurrent access
- **diesel_migrations**: Schema version management
- **dirs**: Platform-specific cache directory location
- **SQLite**: Embedded, zero-config, portable database

---

## Phase 1: Database Foundation & Dependencies

### Overview
Set up diesel-async, create migration infrastructure, and implement the initial database schema with workspace-aware tables for users, conversations, and messages.

### Changes Required

#### 1. Add Dependencies
**File**: `Cargo.toml`
**Changes**: Add diesel-async, connection pooling, and platform utilities

```toml
[dependencies]
# Existing dependencies...
diesel = { version = "2.1", features = ["sqlite", "returning_clauses_for_sqlite_3_35"] }
diesel-async = { version = "0.4", features = ["async-connection-wrapper", "deadpool"] }
diesel_migrations = "2.1"
dirs = "5.0"

[dev-dependencies]
# Existing dev dependencies...
tempfile = "3.8"  # For creating temporary test databases
```

#### 2. Create Migration Infrastructure
**File**: `diesel.toml` (new file)
**Changes**: Configure diesel for SQLite

```toml
[print_schema]
file = "src/cache/schema.rs"
custom_type_derives = ["diesel::query_builder::QueryId"]

[migrations_directory]
dir = "migrations"
```

#### 3. Create Initial Migration
**Command**: `diesel migration generate initial_schema`
**File**: `migrations/YYYYMMDDHHMMSS_initial_schema/up.sql`
**Changes**: Create users, conversations, and messages tables with workspace_id

```sql
-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Enable WAL mode for better concurrent read performance
PRAGMA journal_mode = WAL;

-- Users table
CREATE TABLE users (
    id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL,
    real_name TEXT,
    deleted BOOLEAN NOT NULL DEFAULT 0,

    -- User flags
    is_bot BOOLEAN NOT NULL DEFAULT 0,
    is_admin BOOLEAN,
    is_owner BOOLEAN,

    -- User preferences
    tz TEXT,

    -- Profile fields (flattened from user.profile object)
    profile_email TEXT,
    profile_display_name TEXT,
    profile_status_emoji TEXT,
    profile_status_text TEXT,
    profile_image_72 TEXT,

    -- Cache metadata
    full_object TEXT NOT NULL,
    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,

    PRIMARY KEY (id, workspace_id)
);

CREATE INDEX idx_users_workspace_id ON users(workspace_id);
CREATE INDEX idx_users_name ON users(workspace_id, name);
CREATE INDEX idx_users_cached_at ON users(cached_at);
CREATE INDEX idx_users_deleted ON users(workspace_id, deleted, deleted_at);

-- Conversations table
CREATE TABLE conversations (
    id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL,

    -- Conversation type flags
    is_channel BOOLEAN,
    is_group BOOLEAN,
    is_im BOOLEAN,
    is_mpim BOOLEAN,
    is_private BOOLEAN,
    is_archived BOOLEAN NOT NULL DEFAULT 0,

    -- Conversation metadata
    topic_value TEXT,
    topic_creator TEXT,
    topic_last_set INTEGER,
    purpose_value TEXT,
    purpose_creator TEXT,
    purpose_last_set INTEGER,

    num_members INTEGER,

    -- Cache metadata
    full_object TEXT NOT NULL,
    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,

    PRIMARY KEY (id, workspace_id)
);

CREATE INDEX idx_conversations_workspace_id ON conversations(workspace_id);
CREATE INDEX idx_conversations_name ON conversations(workspace_id, name);
CREATE INDEX idx_conversations_is_archived ON conversations(workspace_id, is_archived);
CREATE INDEX idx_conversations_type ON conversations(workspace_id, is_channel, is_group, is_im, is_mpim);
CREATE INDEX idx_conversations_cached_at ON conversations(cached_at);

-- Messages table
CREATE TABLE messages (
    conversation_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    ts TEXT NOT NULL,

    -- Message content
    user_id TEXT,
    text TEXT NOT NULL,
    thread_ts TEXT,

    -- Message metadata
    permalink TEXT,

    -- Cache metadata
    full_object TEXT NOT NULL,
    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,

    PRIMARY KEY (conversation_id, workspace_id, ts),
    FOREIGN KEY (conversation_id, workspace_id) REFERENCES conversations(id, workspace_id),
    FOREIGN KEY (user_id, workspace_id) REFERENCES users(id, workspace_id)
);

CREATE INDEX idx_messages_workspace_id ON messages(workspace_id);
CREATE INDEX idx_messages_conversation_id ON messages(workspace_id, conversation_id);
CREATE INDEX idx_messages_user_id ON messages(workspace_id, user_id);
CREATE INDEX idx_messages_thread_ts ON messages(workspace_id, thread_ts);
CREATE INDEX idx_messages_ts ON messages(workspace_id, ts);
CREATE INDEX idx_messages_cached_at ON messages(cached_at);
```

**File**: `migrations/YYYYMMDDHHMMSS_initial_schema/down.sql`
**Changes**: Drop all tables

```sql
DROP INDEX IF EXISTS idx_messages_cached_at;
DROP INDEX IF EXISTS idx_messages_ts;
DROP INDEX IF EXISTS idx_messages_thread_ts;
DROP INDEX IF EXISTS idx_messages_user_id;
DROP INDEX IF EXISTS idx_messages_conversation_id;
DROP INDEX IF EXISTS idx_messages_workspace_id;
DROP TABLE IF EXISTS messages;

DROP INDEX IF EXISTS idx_conversations_cached_at;
DROP INDEX IF EXISTS idx_conversations_type;
DROP INDEX IF EXISTS idx_conversations_is_archived;
DROP INDEX IF EXISTS idx_conversations_name;
DROP INDEX IF EXISTS idx_conversations_workspace_id;
DROP TABLE IF EXISTS conversations;

DROP INDEX IF EXISTS idx_users_deleted;
DROP INDEX IF EXISTS idx_users_cached_at;
DROP INDEX IF EXISTS idx_users_name;
DROP INDEX IF EXISTS idx_users_workspace_id;
DROP TABLE IF EXISTS users;
```

#### 4. Create Cache Module Structure
**File**: `src/cache/mod.rs` (new file)
**Changes**: Module declaration

```rust
pub mod db;
pub mod models;
pub mod schema;

pub use db::CachePool;
```

#### 5. Generate Diesel Schema
**File**: `src/cache/schema.rs` (auto-generated by diesel)
**Changes**: Will be auto-generated by running migrations

#### 6. Create Database Initialization
**File**: `src/cache/db.rs` (new file)
**Changes**: Database connection pool setup with platform-specific cache directory

```rust
use anyhow::{Context, Result};
use deadpool::managed::{Object, Pool};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::Pool as AsyncPool;
use diesel_async::AsyncConnection;
use diesel_async::RunQueryDsl;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::path::PathBuf;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub type CachePool = AsyncPool<SqliteConnection>;
pub type CacheConnection = Object<AsyncDieselConnectionManager<SqliteConnection>>;

/// Get platform-specific cache directory
pub fn get_cache_dir() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .context("Failed to determine cache directory for this platform")?;

    let clack_cache = cache_dir.join("clack");

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&clack_cache)
        .context("Failed to create clack cache directory")?;

    Ok(clack_cache)
}

/// Get full path to cache database file
pub fn get_cache_db_path() -> Result<PathBuf> {
    let cache_dir = get_cache_dir()?;
    Ok(cache_dir.join("cache.db"))
}

/// Initialize the cache database and run migrations
pub fn init_cache_db(verbose: bool) -> Result<()> {
    let db_path = get_cache_db_path()?;
    let db_url = format!("sqlite://{}", db_path.display());

    if verbose {
        eprintln!("Initializing cache database at: {}", db_path.display());
    }

    // Create synchronous connection for migrations
    let mut conn = SqliteConnection::establish(&db_url)
        .context("Failed to connect to cache database")?;

    // Run pending migrations
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;

    if verbose {
        eprintln!("Cache database initialized successfully");
    }

    Ok(())
}

/// Create a connection pool for the cache database
pub async fn create_cache_pool(verbose: bool) -> Result<CachePool> {
    // Initialize database and run migrations
    init_cache_db(verbose)?;

    let db_path = get_cache_db_path()?;
    let db_url = format!("sqlite://{}", db_path.display());

    // Create async connection manager
    let config = AsyncDieselConnectionManager::<SqliteConnection>::new(db_url);

    // Create pool with reasonable defaults
    let pool = Pool::builder(config)
        .max_size(8) // Max 8 connections
        .build()
        .context("Failed to create connection pool")?;

    if verbose {
        eprintln!("Cache connection pool created");
    }

    Ok(pool)
}
```

#### 7. Update Main Module
**File**: `src/main.rs`
**Changes**: Add cache module declaration

```rust
mod api;
mod cache;  // ← Add this line
mod cli;
mod models;
mod output;
```

#### 8. Create lib.rs for Tests
**File**: `src/lib.rs` (new file)
**Changes**: Expose cache module for integration tests

```rust
pub mod cache;
```

#### 9. Add Test Helper Function
**File**: `src/cache/db.rs`
**Changes**: Add function for tests to specify custom database path

```rust
/// Initialize cache database at a specific path (for testing)
pub fn init_cache_db_at_path(db_path: &PathBuf, verbose: bool) -> Result<()> {
    let db_url = format!("sqlite://{}", db_path.display());

    // ... same implementation as init_cache_db but uses provided path
}
```

#### 10. Create Integration Test
**File**: `tests/cache_init_test.rs` (new file)
**Changes**: Test database initialization with temporary directory

```rust
use clack::cache::db::init_cache_db_at_path;
use tempfile::tempdir;

#[test]
fn test_cache_db_initialization() {
    // Create a temporary directory for this test
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_cache.db");

    // Initialize the cache at the temp path
    let result = init_cache_db_at_path(&db_path, true);
    assert!(result.is_ok(), "Failed to initialize cache: {:?}", result);

    // Verify database file was created
    assert!(db_path.exists(), "Database file was not created");

    // Verify WAL files were created
    let wal_path = temp_dir.path().join("test_cache.db-wal");
    let shm_path = temp_dir.path().join("test_cache.db-shm");
    assert!(wal_path.exists(), "WAL file was not created");
    assert!(shm_path.exists(), "SHM file was not created");

    // temp_dir automatically cleaned up when it goes out of scope
}
```

### Success Criteria

#### Automated Verification:
- [ ] Dependencies compile: `make build`
- [ ] Migration files exist: `ls migrations/`
- [ ] Schema generated: `ls src/cache/schema.rs`
- [ ] All existing tests pass: `make test`
- [ ] No compiler warnings: `make clippy`

#### Manual Verification:
- [ ] Database created at correct platform-specific location (check with `clack --verbose users`)
- [ ] Database has WAL mode enabled: `sqlite3 ~/Library/Caches/clack/cache.db "PRAGMA journal_mode;"`
- [ ] Tables created: `sqlite3 ~/Library/Caches/clack/cache.db ".tables"`
- [ ] Indexes exist: `sqlite3 ~/Library/Caches/clack/cache.db ".indexes"`

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 2: Workspace Context Integration

### Overview
Add support for retrieving workspace ID via the `auth.test` Slack API endpoint and store it in the `SlackClient` for use in all cache operations.

### Changes Required

#### 1. Add Workspace ID Model
**File**: `src/models/workspace.rs` (new file)
**Changes**: Create model for auth.test response

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthTestResponse {
    pub ok: bool,
    pub url: String,
    pub team: String,
    pub user: String,
    pub team_id: String,
    pub user_id: String,
    pub bot_id: Option<String>,
    pub is_enterprise_install: Option<bool>,
    pub error: Option<String>,
}
```

#### 2. Update Models Module
**File**: `src/models/mod.rs`
**Changes**: Add workspace module

```rust
pub mod channel;
pub mod message;
pub mod search;
pub mod user;
pub mod workspace;  // ← Add this line
```

#### 3. Add auth.test API Function
**File**: `src/api/auth.rs` (new file)
**Changes**: Implement auth.test endpoint

```rust
use super::client::SlackClient;
use crate::models::workspace::AuthTestResponse;
use anyhow::Result;

pub async fn test_auth(client: &SlackClient) -> Result<AuthTestResponse> {
    let query = vec![];
    let response: AuthTestResponse = client.get("auth.test", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (mockito::ServerGuard, SlackClient) {
        let server = mockito::Server::new_async().await;
        std::env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let client = SlackClient::with_base_url(&server.url(), false).unwrap();
        (server, client)
    }

    #[tokio::test]
    async fn test_auth_test_success() {
        let (mut server, client) = setup().await;

        let _mock = server
            .mock("GET", "/auth.test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "ok": true,
                "url": "https://test-workspace.slack.com/",
                "team": "Test Workspace",
                "user": "testuser",
                "team_id": "T12345678",
                "user_id": "U12345678"
            }"#,
            )
            .create_async()
            .await;

        let response = test_auth(&client).await.unwrap();
        assert_eq!(response.team_id, "T12345678");
        assert_eq!(response.team, "Test Workspace");
    }
}
```

#### 4. Update API Module
**File**: `src/api/mod.rs`
**Changes**: Add auth module

```rust
pub mod auth;      // ← Add this line
pub mod channels;
pub mod client;
pub mod messages;
pub mod search;
pub mod users;
```

#### 5. Update SlackClient to Store Workspace ID
**File**: `src/api/client.rs`
**Changes**: Add workspace_id field and initialization

```rust
pub struct SlackClient {
    client: reqwest::Client,
    base_url: String,
    verbose: bool,
    workspace_id: Option<String>,  // ← Add this field
}

impl SlackClient {
    pub fn new_verbose(verbose: bool) -> Result<Self> {
        Self::with_base_url("https://slack.com/api", verbose)
    }

    pub fn with_base_url(base_url: &str, verbose: bool) -> Result<Self> {
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
            base_url: base_url.to_string(),
            verbose,
            workspace_id: None,  // ← Initialize as None
        })
    }

    // Add method to initialize workspace context
    pub async fn init_workspace(&mut self) -> Result<String> {
        if let Some(ref id) = self.workspace_id {
            return Ok(id.clone());
        }

        // Import moved inside function to avoid circular dependency
        use crate::api::auth::test_auth;

        let auth_response = test_auth(self).await?;
        self.workspace_id = Some(auth_response.team_id.clone());

        if self.verbose {
            eprintln!("Workspace: {} ({})", auth_response.team, auth_response.team_id);
        }

        Ok(auth_response.team_id)
    }

    // Add getter for workspace_id
    pub fn workspace_id(&self) -> Option<&str> {
        self.workspace_id.as_deref()
    }
}
```

#### 6. Update Main to Initialize Workspace Context
**File**: `src/main.rs`
**Changes**: Call init_workspace after creating client

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client with verbose flag
    let mut client = api::client::SlackClient::new_verbose(cli.verbose)?;

    // Initialize workspace context (fetches team_id)
    client.init_workspace().await?;  // ← Add this line

    // Execute command
    match cli.command {
        // ... rest of the match statement
```

### Success Criteria

#### Automated Verification:
- [ ] Code compiles: `make build`
- [ ] Unit tests pass: `make test`
- [ ] auth.test mock test passes
- [ ] No clippy warnings: `make clippy`

#### Manual Verification:
- [ ] `clack --verbose users` shows workspace name and ID in output
- [ ] Workspace ID is a valid Slack team ID (format: T[A-Z0-9]{8,})
- [ ] Different SLACK_TOKEN values yield different workspace IDs

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 3: Cache Layer Core Infrastructure

### Overview
Create the cache layer with diesel models, CRUD operations, TTL checking, and connection pool management.

### Changes Required

#### 1. Create Diesel Models
**File**: `src/cache/models.rs` (new file)
**Changes**: Create diesel models matching schema

```rust
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use super::schema::{conversations, messages, users};

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CachedUser {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub real_name: Option<String>,
    pub deleted: bool,

    pub is_bot: bool,
    pub is_admin: Option<bool>,
    pub is_owner: Option<bool>,

    pub tz: Option<String>,

    pub profile_email: Option<String>,
    pub profile_display_name: Option<String>,
    pub profile_status_emoji: Option<String>,
    pub profile_status_text: Option<String>,
    pub profile_image_72: Option<String>,

    pub full_object: String,
    pub cached_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CachedConversation {
    pub id: String,
    pub workspace_id: String,
    pub name: String,

    pub is_channel: Option<bool>,
    pub is_group: Option<bool>,
    pub is_im: Option<bool>,
    pub is_mpim: Option<bool>,
    pub is_private: Option<bool>,
    pub is_archived: bool,

    pub topic_value: Option<String>,
    pub topic_creator: Option<String>,
    pub topic_last_set: Option<i32>,
    pub purpose_value: Option<String>,
    pub purpose_creator: Option<String>,
    pub purpose_last_set: Option<i32>,

    pub num_members: Option<i32>,

    pub full_object: String,
    pub cached_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CachedMessage {
    pub conversation_id: String,
    pub workspace_id: String,
    pub ts: String,

    pub user_id: Option<String>,
    pub text: String,
    pub thread_ts: Option<String>,

    pub permalink: Option<String>,

    pub full_object: String,
    pub cached_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

// Helper functions to convert between API models and cache models
impl CachedUser {
    pub fn from_api_user(user: &crate::models::user::User, workspace_id: &str) -> Self {
        Self {
            id: user.id.clone(),
            workspace_id: workspace_id.to_string(),
            name: user.name.clone(),
            real_name: user.real_name.clone(),
            deleted: user.deleted,
            is_bot: user.is_bot,
            is_admin: user.is_admin,
            is_owner: user.is_owner,
            tz: user.tz.clone(),
            profile_email: user.profile.email.clone(),
            profile_display_name: user.profile.display_name.clone(),
            profile_status_emoji: user.profile.status_emoji.clone(),
            profile_status_text: user.profile.status_text.clone(),
            profile_image_72: user.profile.image_72.clone(),
            full_object: serde_json::to_string(user).unwrap_or_default(),
            cached_at: chrono::Utc::now().naive_utc(),
            deleted_at: None,
        }
    }

    pub fn to_api_user(&self) -> anyhow::Result<crate::models::user::User> {
        serde_json::from_str(&self.full_object)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize cached user: {}", e))
    }
}

impl CachedConversation {
    pub fn from_api_channel(channel: &crate::models::channel::Channel, workspace_id: &str) -> Self {
        Self {
            id: channel.id.clone(),
            workspace_id: workspace_id.to_string(),
            name: channel.name.clone(),
            is_channel: channel.is_channel,
            is_group: channel.is_group,
            is_im: channel.is_im,
            is_mpim: channel.is_mpim,
            is_private: channel.is_private,
            is_archived: channel.is_archived.unwrap_or(false),
            topic_value: channel.topic.as_ref().map(|t| t.value.clone()),
            topic_creator: None,
            topic_last_set: None,
            purpose_value: channel.purpose.as_ref().map(|p| p.value.clone()),
            purpose_creator: None,
            purpose_last_set: None,
            num_members: channel.num_members.map(|n| n as i32),
            full_object: serde_json::to_string(channel).unwrap_or_default(),
            cached_at: chrono::Utc::now().naive_utc(),
            deleted_at: None,
        }
    }

    pub fn to_api_channel(&self) -> anyhow::Result<crate::models::channel::Channel> {
        serde_json::from_str(&self.full_object)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize cached conversation: {}", e))
    }
}

impl CachedMessage {
    pub fn from_api_message(message: &crate::models::message::Message, conversation_id: &str, workspace_id: &str) -> Self {
        Self {
            conversation_id: conversation_id.to_string(),
            workspace_id: workspace_id.to_string(),
            ts: message.ts.clone(),
            user_id: message.user.clone(),
            text: message.text.clone(),
            thread_ts: message.thread_ts.clone(),
            permalink: None,
            full_object: serde_json::to_string(message).unwrap_or_default(),
            cached_at: chrono::Utc::now().naive_utc(),
            deleted_at: None,
        }
    }

    pub fn to_api_message(&self) -> anyhow::Result<crate::models::message::Message> {
        serde_json::from_str(&self.full_object)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize cached message: {}", e))
    }
}
```

#### 2. Create Cache Operations
**File**: `src/cache/operations.rs` (new file)
**Changes**: Implement cache CRUD with TTL checking

```rust
use anyhow::Result;
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use super::db::CacheConnection;
use super::models::{CachedConversation, CachedMessage, CachedUser};
use super::schema::{conversations, messages, users};
use crate::models::channel::Channel;
use crate::models::message::Message;
use crate::models::user::User;

// TTL constants (in seconds)
const USER_TTL_SECONDS: i64 = 3600; // 1 hour
const CONVERSATION_TTL_SECONDS: i64 = 1800; // 30 minutes
const MESSAGE_TTL_SECONDS: i64 = 300; // 5 minutes

/// Check if a cached item is fresh based on TTL
fn is_fresh(cached_at: chrono::NaiveDateTime, ttl_seconds: i64) -> bool {
    let cached_at_utc = chrono::DateTime::<Utc>::from_naive_utc_and_offset(cached_at, Utc);
    let age = Utc::now().signed_duration_since(cached_at_utc);
    age.num_seconds() < ttl_seconds
}

// User operations

pub async fn get_user(
    conn: &mut CacheConnection,
    workspace_id: &str,
    user_id: &str,
    verbose: bool,
) -> Result<Option<User>> {
    use super::schema::users::dsl::*;

    let cached_user: Option<CachedUser> = users
        .filter(id.eq(user_id))
        .filter(workspace_id.eq(workspace_id))
        .filter(deleted_at.is_null())
        .first(conn)
        .await
        .optional()?;

    match cached_user {
        Some(cached) => {
            if is_fresh(cached.cached_at, USER_TTL_SECONDS) {
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

pub async fn get_users(
    conn: &mut CacheConnection,
    workspace_id: &str,
    verbose: bool,
) -> Result<Option<Vec<User>>> {
    use super::schema::users::dsl::*;

    let cached_users: Vec<CachedUser> = users
        .filter(workspace_id.eq(workspace_id))
        .filter(deleted_at.is_null())
        .load(conn)
        .await?;

    if cached_users.is_empty() {
        if verbose {
            eprintln!("[CACHE] Users - MISS (empty)");
        }
        return Ok(None);
    }

    // Check if all users are fresh
    let all_fresh = cached_users
        .iter()
        .all(|u| is_fresh(u.cached_at, USER_TTL_SECONDS));

    if all_fresh {
        if verbose {
            eprintln!("[CACHE] Users - HIT ({} users)", cached_users.len());
        }
        let api_users: Result<Vec<User>> = cached_users
            .iter()
            .map(|u| u.to_api_user())
            .collect();
        Ok(Some(api_users?))
    } else {
        if verbose {
            eprintln!("[CACHE] Users - MISS (some stale)");
        }
        Ok(None)
    }
}

pub async fn upsert_user(
    conn: &mut CacheConnection,
    workspace_id: &str,
    user: &User,
    verbose: bool,
) -> Result<()> {
    let cached = CachedUser::from_api_user(user, workspace_id);

    diesel::replace_into(users::table)
        .values(&cached)
        .execute(conn)
        .await?;

    if verbose {
        eprintln!("[CACHE] User {} - UPSERTED", user.id);
    }

    Ok(())
}

pub async fn upsert_users(
    conn: &mut CacheConnection,
    workspace_id: &str,
    user_list: &[User],
    verbose: bool,
) -> Result<()> {
    let cached_users: Vec<CachedUser> = user_list
        .iter()
        .map(|u| CachedUser::from_api_user(u, workspace_id))
        .collect();

    for cached in cached_users {
        diesel::replace_into(users::table)
            .values(&cached)
            .execute(conn)
            .await?;
    }

    if verbose {
        eprintln!("[CACHE] Users - UPSERTED {} users", user_list.len());
    }

    Ok(())
}

// Conversation operations

pub async fn get_conversation(
    conn: &mut CacheConnection,
    workspace_id: &str,
    conversation_id: &str,
    verbose: bool,
) -> Result<Option<Channel>> {
    use super::schema::conversations::dsl::*;

    let cached_conv: Option<CachedConversation> = conversations
        .filter(id.eq(conversation_id))
        .filter(workspace_id.eq(workspace_id))
        .filter(deleted_at.is_null())
        .first(conn)
        .await
        .optional()?;

    match cached_conv {
        Some(cached) => {
            if is_fresh(cached.cached_at, CONVERSATION_TTL_SECONDS) {
                if verbose {
                    eprintln!("[CACHE] Conversation {} - HIT (fresh)", conversation_id);
                }
                Ok(Some(cached.to_api_channel()?))
            } else {
                if verbose {
                    eprintln!("[CACHE] Conversation {} - MISS (stale)", conversation_id);
                }
                Ok(None)
            }
        }
        None => {
            if verbose {
                eprintln!("[CACHE] Conversation {} - MISS (not found)", conversation_id);
            }
            Ok(None)
        }
    }
}

pub async fn get_conversations(
    conn: &mut CacheConnection,
    workspace_id: &str,
    verbose: bool,
) -> Result<Option<Vec<Channel>>> {
    use super::schema::conversations::dsl::*;

    let cached_convs: Vec<CachedConversation> = conversations
        .filter(workspace_id.eq(workspace_id))
        .filter(deleted_at.is_null())
        .load(conn)
        .await?;

    if cached_convs.is_empty() {
        if verbose {
            eprintln!("[CACHE] Conversations - MISS (empty)");
        }
        return Ok(None);
    }

    let all_fresh = cached_convs
        .iter()
        .all(|c| is_fresh(c.cached_at, CONVERSATION_TTL_SECONDS));

    if all_fresh {
        if verbose {
            eprintln!("[CACHE] Conversations - HIT ({} conversations)", cached_convs.len());
        }
        let api_channels: Result<Vec<Channel>> = cached_convs
            .iter()
            .map(|c| c.to_api_channel())
            .collect();
        Ok(Some(api_channels?))
    } else {
        if verbose {
            eprintln!("[CACHE] Conversations - MISS (some stale)");
        }
        Ok(None)
    }
}

pub async fn upsert_conversation(
    conn: &mut CacheConnection,
    workspace_id: &str,
    channel: &Channel,
    verbose: bool,
) -> Result<()> {
    let cached = CachedConversation::from_api_channel(channel, workspace_id);

    diesel::replace_into(conversations::table)
        .values(&cached)
        .execute(conn)
        .await?;

    if verbose {
        eprintln!("[CACHE] Conversation {} - UPSERTED", channel.id);
    }

    Ok(())
}

pub async fn upsert_conversations(
    conn: &mut CacheConnection,
    workspace_id: &str,
    channel_list: &[Channel],
    verbose: bool,
) -> Result<()> {
    for channel in channel_list {
        let cached = CachedConversation::from_api_channel(channel, workspace_id);
        diesel::replace_into(conversations::table)
            .values(&cached)
            .execute(conn)
            .await?;
    }

    if verbose {
        eprintln!("[CACHE] Conversations - UPSERTED {} conversations", channel_list.len());
    }

    Ok(())
}

// Message operations

pub async fn get_messages(
    conn: &mut CacheConnection,
    workspace_id: &str,
    conversation_id: &str,
    verbose: bool,
) -> Result<Option<Vec<Message>>> {
    use super::schema::messages::dsl::*;

    let cached_msgs: Vec<CachedMessage> = messages
        .filter(conversation_id.eq(conversation_id))
        .filter(workspace_id.eq(workspace_id))
        .filter(deleted_at.is_null())
        .load(conn)
        .await?;

    if cached_msgs.is_empty() {
        if verbose {
            eprintln!("[CACHE] Messages (conv {}) - MISS (empty)", conversation_id);
        }
        return Ok(None);
    }

    let all_fresh = cached_msgs
        .iter()
        .all(|m| is_fresh(m.cached_at, MESSAGE_TTL_SECONDS));

    if all_fresh {
        if verbose {
            eprintln!("[CACHE] Messages (conv {}) - HIT ({} messages)", conversation_id, cached_msgs.len());
        }
        let api_messages: Result<Vec<Message>> = cached_msgs
            .iter()
            .map(|m| m.to_api_message())
            .collect();
        Ok(Some(api_messages?))
    } else {
        if verbose {
            eprintln!("[CACHE] Messages (conv {}) - MISS (some stale)", conversation_id);
        }
        Ok(None)
    }
}

pub async fn upsert_messages(
    conn: &mut CacheConnection,
    workspace_id: &str,
    conversation_id: &str,
    message_list: &[Message],
    verbose: bool,
) -> Result<()> {
    for message in message_list {
        let cached = CachedMessage::from_api_message(message, conversation_id, workspace_id);
        diesel::replace_into(messages::table)
            .values(&cached)
            .execute(conn)
            .await?;
    }

    if verbose {
        eprintln!("[CACHE] Messages (conv {}) - UPSERTED {} messages", conversation_id, message_list.len());
    }

    Ok(())
}

// Cache clearing operations

pub async fn clear_workspace_cache(
    conn: &mut CacheConnection,
    workspace_id: &str,
    verbose: bool,
) -> Result<()> {
    use super::schema::{conversations, messages, users};

    diesel::delete(messages::table.filter(messages::workspace_id.eq(workspace_id)))
        .execute(conn)
        .await?;

    diesel::delete(conversations::table.filter(conversations::workspace_id.eq(workspace_id)))
        .execute(conn)
        .await?;

    diesel::delete(users::table.filter(users::workspace_id.eq(workspace_id)))
        .execute(conn)
        .await?;

    if verbose {
        eprintln!("[CACHE] Cleared all cache for workspace {}", workspace_id);
    }

    Ok(())
}

pub async fn clear_all_cache(
    conn: &mut CacheConnection,
    verbose: bool,
) -> Result<()> {
    use super::schema::{conversations, messages, users};

    diesel::delete(messages::table).execute(conn).await?;
    diesel::delete(conversations::table).execute(conn).await?;
    diesel::delete(users::table).execute(conn).await?;

    if verbose {
        eprintln!("[CACHE] Cleared all cache");
    }

    Ok(())
}
```

#### 3. Update Cache Module
**File**: `src/cache/mod.rs`
**Changes**: Add operations module

```rust
pub mod db;
pub mod models;
pub mod operations;
pub mod schema;

pub use db::{create_cache_pool, CachePool};
```

### Success Criteria

#### Automated Verification:
- [ ] Code compiles: `make build`
- [ ] All tests pass: `make test`
- [ ] No clippy warnings: `make clippy`
- [ ] Schema file generated correctly

#### Manual Verification:
- [ ] Cache operations can insert and retrieve users
- [ ] TTL expiration works correctly (cached_at timestamp checked)
- [ ] Workspace isolation works (different workspace_id values don't interfere)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 4: API Integration (Users & Channels)

### Overview
Integrate cache layer into users and channels API functions with check-cache-first pattern and automatic write-through caching.

### Changes Required

#### 1. Update SlackClient with Cache Pool
**File**: `src/api/client.rs`
**Changes**: Add cache pool to client struct

```rust
use crate::cache::CachePool;  // ← Add import

pub struct SlackClient {
    client: reqwest::Client,
    base_url: String,
    verbose: bool,
    workspace_id: Option<String>,
    cache_pool: Option<CachePool>,  // ← Add this field
}

impl SlackClient {
    pub async fn new_verbose(verbose: bool) -> Result<Self> {
        Self::with_base_url("https://slack.com/api", verbose).await
    }

    pub async fn with_base_url(base_url: &str, verbose: bool) -> Result<Self> {
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

        // Initialize cache pool (with error handling)
        let cache_pool = match crate::cache::create_cache_pool(verbose).await {
            Ok(pool) => Some(pool),
            Err(e) => {
                if verbose {
                    eprintln!("Warning: Failed to initialize cache: {}", e);
                    eprintln!("Continuing without cache...");
                }
                None
            }
        };

        Ok(Self {
            client,
            base_url: base_url.to_string(),
            verbose,
            workspace_id: None,
            cache_pool,
        })
    }

    pub fn cache_pool(&self) -> Option<&CachePool> {
        self.cache_pool.as_ref()
    }
}
```

#### 2. Update Users API with Caching
**File**: `src/api/users.rs`
**Changes**: Add cache check before API call, write-through after

```rust
use super::client::SlackClient;
use crate::cache::operations;
use crate::models::user::{User, UserInfoResponse, UsersListResponse};
use anyhow::Result;

pub async fn list_users(
    client: &SlackClient,
    limit: u32,
    include_deleted: bool,
) -> Result<Vec<User>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first (if pool available)
    if let Some(pool) = client.cache_pool() {
        match pool.get().await {
            Ok(mut conn) => {
                match operations::get_users(&mut conn, workspace_id, client.verbose).await {
                    Ok(Some(cached_users)) => {
                        let mut users = cached_users;
                        if !include_deleted {
                            users.retain(|u| !u.deleted);
                        }
                        return Ok(users);
                    }
                    Ok(None) => {
                        // Cache miss or stale, continue to API
                    }
                    Err(e) => {
                        if client.verbose {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                        // Fall through to API
                    }
                }
            }
            Err(e) => {
                if client.verbose {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }

    // Cache miss or error - fetch from API
    let query = vec![("limit", limit.to_string())];
    let response: UsersListResponse = client.get("users.list", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let users = response.members;

    // Write through to cache (best effort, don't fail on cache errors)
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = pool.get().await {
            let _ = operations::upsert_users(&mut conn, workspace_id, &users, client.verbose).await;
        }
    }

    let mut result = users;
    if !include_deleted {
        result.retain(|u| !u.deleted);
    }

    Ok(result)
}

pub async fn get_user(client: &SlackClient, user_id: &str) -> Result<User> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first
    if let Some(pool) = client.cache_pool() {
        match pool.get().await {
            Ok(mut conn) => {
                match operations::get_user(&mut conn, workspace_id, user_id, client.verbose).await {
                    Ok(Some(cached_user)) => {
                        return Ok(cached_user);
                    }
                    Ok(None) => {
                        // Cache miss, continue to API
                    }
                    Err(e) => {
                        if client.verbose {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                if client.verbose {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }

    // Fetch from API
    let query = vec![("user", user_id.to_string())];
    let response: UserInfoResponse = client.get("users.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let user = response.user;

    // Write through to cache
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = pool.get().await {
            let _ = operations::upsert_user(&mut conn, workspace_id, &user, client.verbose).await;
        }
    }

    Ok(user)
}
```

#### 3. Update Channels API with Caching
**File**: `src/api/channels.rs`
**Changes**: Add cache check before API call, write-through after

```rust
// Add to imports
use crate::cache::operations;

// Update list_channels function
pub async fn list_channels(client: &SlackClient, include_archived: bool) -> Result<Vec<Channel>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first
    if let Some(pool) = client.cache_pool() {
        match pool.get().await {
            Ok(mut conn) => {
                match operations::get_conversations(&mut conn, workspace_id, client.verbose).await {
                    Ok(Some(cached_channels)) => {
                        let mut channels = cached_channels;
                        if !include_archived {
                            channels.retain(|c| !c.is_archived.unwrap_or(false));
                        }
                        return Ok(channels);
                    }
                    Ok(None) => {
                        // Cache miss, continue to API
                    }
                    Err(e) => {
                        if client.verbose {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                if client.verbose {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }

    // Fetch from API
    let channels = fetch_all_channels(client, include_archived).await?;

    // Write through to cache
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = pool.get().await {
            let _ = operations::upsert_conversations(&mut conn, workspace_id, &channels, client.verbose).await;
        }
    }

    Ok(channels)
}

// Update get_channel function
pub async fn get_channel(client: &SlackClient, channel_id: &str) -> Result<Channel> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Try cache first
    if let Some(pool) = client.cache_pool() {
        match pool.get().await {
            Ok(mut conn) => {
                match operations::get_conversation(&mut conn, workspace_id, channel_id, client.verbose).await {
                    Ok(Some(cached_channel)) => {
                        return Ok(cached_channel);
                    }
                    Ok(None) => {
                        // Cache miss, continue to API
                    }
                    Err(e) => {
                        if client.verbose {
                            eprintln!("[CACHE] Error reading cache: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                if client.verbose {
                    eprintln!("[CACHE] Failed to get connection: {}", e);
                }
            }
        }
    }

    // Fetch from API
    let query = vec![("channel", channel_id.to_string())];
    let response: ChannelInfoResponse = client.get("conversations.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let channel = response.channel;

    // Write through to cache
    if let Some(pool) = client.cache_pool() {
        if let Ok(mut conn) = pool.get().await {
            let _ = operations::upsert_conversation(&mut conn, workspace_id, &channel, client.verbose).await;
        }
    }

    Ok(channel)
}
```

#### 4. Update Main to Use Async Client Creation
**File**: `src/main.rs`
**Changes**: Use async client creation

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client with verbose flag (now async)
    let mut client = api::client::SlackClient::new_verbose(cli.verbose).await?;

    // Initialize workspace context (fetches team_id)
    client.init_workspace().await?;

    // Rest of the code remains the same...
```

### Success Criteria

#### Automated Verification:
- [ ] Code compiles: `make build`
- [ ] All existing tests pass: `make test`
- [ ] No clippy warnings: `make clippy`

#### Manual Verification:
- [ ] First `clack users` call fetches from API (slower)
- [ ] Second `clack users` call within 1 hour uses cache (faster)
- [ ] `clack --verbose users` shows cache HIT/MISS messages
- [ ] First `clack channels` call fetches from API
- [ ] Second `clack channels` call within 30 minutes uses cache
- [ ] Cache persists across command invocations

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Message Caching with Opt-in Flag

### Overview
Add `--use-cache` flag to Messages and Thread commands for opt-in message caching with 5-minute TTL.

### Changes Required

#### 1. Add --use-cache Flag to CLI
**File**: `src/cli.rs`
**Changes**: Add use_cache flag to Messages and Thread commands

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... other commands ...

    /// List messages from a channel
    Messages {
        /// Channel ID or name
        channel: String,

        /// Number of messages to retrieve
        #[arg(long, default_value = "200")]
        limit: u32,

        /// End of time range (Unix timestamp)
        #[arg(long)]
        latest: Option<String>,

        /// Start of time range (Unix timestamp)
        #[arg(long)]
        oldest: Option<String>,

        /// Use cached messages if available (within 5 minute TTL)
        #[arg(long)]
        use_cache: bool,  // ← Add this field
    },

    /// Get a conversation thread and all its replies
    Thread {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Message timestamp/ID (e.g., 1234567890.123456)
        message_ts: String,

        /// Use cached thread if available (within 5 minute TTL)
        #[arg(long)]
        use_cache: bool,  // ← Add this field
    },

    // ... other commands ...
}
```

#### 2. Update Messages API with Opt-in Caching
**File**: `src/api/messages.rs`
**Changes**: Add cache check when use_cache=true

```rust
use super::client::SlackClient;
use crate::cache::operations;
use crate::models::message::{Message, MessagesResponse};
use anyhow::Result;

pub async fn list_messages(
    client: &SlackClient,
    channel_id: &str,
    limit: u32,
    latest: Option<String>,
    oldest: Option<String>,
    use_cache: bool,  // ← Add parameter
) -> Result<Vec<Message>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Only check cache if explicitly requested
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            match pool.get().await {
                Ok(mut conn) => {
                    match operations::get_messages(&mut conn, workspace_id, channel_id, client.verbose).await {
                        Ok(Some(cached_messages)) => {
                            if client.verbose {
                                eprintln!("[CACHE] Using cached messages (--use-cache enabled)");
                            }
                            return Ok(cached_messages);
                        }
                        Ok(None) => {
                            if client.verbose {
                                eprintln!("[CACHE] No fresh cached messages, fetching from API");
                            }
                        }
                        Err(e) => {
                            if client.verbose {
                                eprintln!("[CACHE] Error reading cache: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    if client.verbose {
                        eprintln!("[CACHE] Failed to get connection: {}", e);
                    }
                }
            }
        }
    } else if client.verbose {
        eprintln!("[CACHE] Message caching disabled (use --use-cache to enable)");
    }

    // Fetch from API
    let mut query = vec![("channel", channel_id.to_string()), ("limit", limit.to_string())];

    if let Some(latest_ts) = latest {
        query.push(("latest", latest_ts));
    }
    if let Some(oldest_ts) = oldest {
        query.push(("oldest", oldest_ts));
    }

    let response: MessagesResponse = client.get("conversations.history", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let messages = response.messages;

    // Write through to cache if use_cache is enabled
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = pool.get().await {
                let _ = operations::upsert_messages(&mut conn, workspace_id, channel_id, &messages, client.verbose).await;
            }
        }
    }

    Ok(messages)
}

pub async fn get_thread(
    client: &SlackClient,
    channel_id: &str,
    message_ts: &str,
    use_cache: bool,  // ← Add parameter
) -> Result<Vec<Message>> {
    let workspace_id = client
        .workspace_id()
        .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;

    // Only check cache if explicitly requested
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            match pool.get().await {
                Ok(mut conn) => {
                    // For threads, we could implement thread-specific caching
                    // For now, we'll skip cache for threads (more complex)
                    if client.verbose {
                        eprintln!("[CACHE] Thread caching not yet implemented");
                    }
                }
                Err(e) => {
                    if client.verbose {
                        eprintln!("[CACHE] Failed to get connection: {}", e);
                    }
                }
            }
        }
    }

    // Fetch from API
    let query = vec![
        ("channel", channel_id.to_string()),
        ("ts", message_ts.to_string()),
    ];

    let response: MessagesResponse = client.get("conversations.replies", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let messages = response.messages;

    // Write through to cache if use_cache is enabled
    if use_cache {
        if let Some(pool) = client.cache_pool() {
            if let Ok(mut conn) = pool.get().await {
                let _ = operations::upsert_messages(&mut conn, workspace_id, channel_id, &messages, client.verbose).await;
            }
        }
    }

    Ok(messages)
}
```

#### 3. Update Main to Pass use_cache Flag
**File**: `src/main.rs`
**Changes**: Pass use_cache to API functions

```rust
Commands::Messages {
    channel,
    limit,
    latest,
    oldest,
    use_cache,  // ← Add this
} => {
    let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

    let messages = api::messages::list_messages(
        &client,
        &channel_id,
        limit,
        latest,
        oldest,
        use_cache,  // ← Pass to API
    ).await?;

    // ... rest of formatting code
}

Commands::Thread {
    channel,
    message_ts,
    use_cache,  // ← Add this
} => {
    let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

    let messages = api::messages::get_thread(
        &client,
        &channel_id,
        &message_ts,
        use_cache,  // ← Pass to API
    ).await?;

    // ... rest of formatting code
}
```

### Success Criteria

#### Automated Verification:
- [ ] Code compiles: `make build`
- [ ] All tests pass: `make test`
- [ ] CLI parsing tests updated for new flag

#### Manual Verification:
- [ ] `clack messages #general` fetches from API (no cache)
- [ ] `clack messages #general --use-cache` fetches from API (first time)
- [ ] Second `clack messages #general --use-cache` uses cache (within 5 min)
- [ ] `clack --verbose messages #general --use-cache` shows cache status
- [ ] Without `--use-cache`, messages are never cached

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 6: CLI Enhancements & Cache Management

### Overview
Add `--debug` global flag for cache statistics and `cache clear` subcommand for manual cache invalidation.

### Changes Required

#### 1. Add --debug Global Flag
**File**: `src/cli.rs`
**Changes**: Add debug flag to Cli struct

```rust
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

    /// Enable debug mode with cache statistics
    #[arg(long, global = true)]
    pub debug: bool,  // ← Add this field
}
```

#### 2. Add Cache Command
**File**: `src/cli.rs`
**Changes**: Add Cache command with Clear subcommand

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...

    /// Cache management commands
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand)]
pub enum CacheAction {
    /// Clear cache for current workspace or all workspaces
    Clear {
        /// Clear cache for all workspaces (default: current workspace only)
        #[arg(long)]
        all: bool,
    },
}
```

#### 3. Create Cache Statistics Tracker
**File**: `src/cache/stats.rs` (new file)
**Changes**: Implement cache statistics tracking

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub user_hits: Arc<AtomicU64>,
    pub user_misses: Arc<AtomicU64>,
    pub conversation_hits: Arc<AtomicU64>,
    pub conversation_misses: Arc<AtomicU64>,
    pub message_hits: Arc<AtomicU64>,
    pub message_misses: Arc<AtomicU64>,
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            user_hits: Arc::new(AtomicU64::new(0)),
            user_misses: Arc::new(AtomicU64::new(0)),
            conversation_hits: Arc::new(AtomicU64::new(0)),
            conversation_misses: Arc::new(AtomicU64::new(0)),
            message_hits: Arc::new(AtomicU64::new(0)),
            message_misses: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn record_user_hit(&self) {
        self.user_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_user_miss(&self) {
        self.user_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_conversation_hit(&self) {
        self.conversation_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_conversation_miss(&self) {
        self.conversation_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_message_hit(&self) {
        self.message_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_message_miss(&self) {
        self.message_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn print_summary(&self) {
        let user_hits = self.user_hits.load(Ordering::Relaxed);
        let user_misses = self.user_misses.load(Ordering::Relaxed);
        let conv_hits = self.conversation_hits.load(Ordering::Relaxed);
        let conv_misses = self.conversation_misses.load(Ordering::Relaxed);
        let msg_hits = self.message_hits.load(Ordering::Relaxed);
        let msg_misses = self.message_misses.load(Ordering::Relaxed);

        eprintln!("\n[CACHE STATISTICS]");

        if user_hits + user_misses > 0 {
            let user_total = user_hits + user_misses;
            let user_hit_rate = (user_hits as f64 / user_total as f64) * 100.0;
            eprintln!(
                "  users: {} hits, {} misses ({:.1}% hit rate)",
                user_hits, user_misses, user_hit_rate
            );
        }

        if conv_hits + conv_misses > 0 {
            let conv_total = conv_hits + conv_misses;
            let conv_hit_rate = (conv_hits as f64 / conv_total as f64) * 100.0;
            eprintln!(
                "  channels: {} hits, {} misses ({:.1}% hit rate)",
                conv_hits, conv_misses, conv_hit_rate
            );
        }

        if msg_hits + msg_misses > 0 {
            let msg_total = msg_hits + msg_misses;
            let msg_hit_rate = (msg_hits as f64 / msg_total as f64) * 100.0;
            eprintln!(
                "  messages: {} hits, {} misses ({:.1}% hit rate)",
                msg_hits, msg_misses, msg_hit_rate
            );
        }

        if user_hits + user_misses + conv_hits + conv_misses + msg_hits + msg_misses == 0 {
            eprintln!("  No cache operations performed");
        }
    }
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}
```

#### 4. Update Cache Module
**File**: `src/cache/mod.rs`
**Changes**: Add stats module

```rust
pub mod db;
pub mod models;
pub mod operations;
pub mod schema;
pub mod stats;  // ← Add this line

pub use db::{create_cache_pool, CachePool};
pub use stats::CacheStats;
```

#### 5. Add Stats to SlackClient
**File**: `src/api/client.rs`
**Changes**: Add stats tracking

```rust
use crate::cache::{CachePool, CacheStats};  // ← Update import

pub struct SlackClient {
    client: reqwest::Client,
    base_url: String,
    verbose: bool,
    workspace_id: Option<String>,
    cache_pool: Option<CachePool>,
    cache_stats: CacheStats,  // ← Add this field
}

impl SlackClient {
    pub async fn with_base_url(base_url: &str, verbose: bool) -> Result<Self> {
        // ... existing code ...

        Ok(Self {
            client,
            base_url: base_url.to_string(),
            verbose,
            workspace_id: None,
            cache_pool,
            cache_stats: CacheStats::new(),  // ← Initialize stats
        })
    }

    pub fn cache_stats(&self) -> &CacheStats {
        &self.cache_stats
    }
}
```

#### 6. Update Operations to Track Stats
**File**: `src/cache/operations.rs`
**Changes**: Add stats parameter and tracking

```rust
use super::stats::CacheStats;

// Update function signatures to accept optional stats
pub async fn get_user(
    conn: &mut CacheConnection,
    workspace_id: &str,
    user_id: &str,
    verbose: bool,
    stats: Option<&CacheStats>,  // ← Add parameter
) -> Result<Option<User>> {
    // ... existing code ...

    match cached_user {
        Some(cached) => {
            if is_fresh(cached.cached_at, USER_TTL_SECONDS) {
                if let Some(s) = stats {
                    s.record_user_hit();
                }
                if verbose {
                    eprintln!("[CACHE] User {} - HIT (fresh)", user_id);
                }
                Ok(Some(cached.to_api_user()?))
            } else {
                if let Some(s) = stats {
                    s.record_user_miss();
                }
                if verbose {
                    eprintln!("[CACHE] User {} - MISS (stale)", user_id);
                }
                Ok(None)
            }
        }
        None => {
            if let Some(s) = stats {
                s.record_user_miss();
            }
            if verbose {
                eprintln!("[CACHE] User {} - MISS (not found)", user_id);
            }
            Ok(None)
        }
    }
}

// Repeat similar changes for get_users, get_conversation, get_conversations, get_messages
```

#### 7. Update API Functions to Pass Stats
**File**: `src/api/users.rs`, `src/api/channels.rs`, `src/api/messages.rs`
**Changes**: Pass stats to cache operations

```rust
// Example from users.rs
match operations::get_user(
    &mut conn,
    workspace_id,
    user_id,
    client.verbose,
    Some(client.cache_stats()),  // ← Pass stats
).await {
    // ...
}
```

#### 8. Implement Cache Clear Command Handler
**File**: `src/main.rs`
**Changes**: Add handler for cache clear command

```rust
use cli::{CacheAction, Cli, Commands, SearchType};  // ← Update import

// ... in match statement ...

Commands::Cache { action } => match action {
    CacheAction::Clear { all } => {
        if let Some(pool) = client.cache_pool() {
            let mut conn = pool.get().await?;

            if *all {
                crate::cache::operations::clear_all_cache(&mut conn, cli.verbose).await?;
                println!("Cache cleared for all workspaces");
            } else {
                let workspace_id = client
                    .workspace_id()
                    .ok_or_else(|| anyhow::anyhow!("Workspace ID not initialized"))?;
                crate::cache::operations::clear_workspace_cache(&mut conn, workspace_id, cli.verbose).await?;
                println!("Cache cleared for current workspace");
            }
        } else {
            eprintln!("Cache is not available");
        }
    }
},
```

#### 9. Add Debug Stats Output
**File**: `src/main.rs`
**Changes**: Print stats at end of execution if --debug

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client with verbose flag (now async)
    let mut client = api::client::SlackClient::new_verbose(cli.verbose).await?;

    // Initialize workspace context (fetches team_id)
    client.init_workspace().await?;

    // Execute command
    match cli.command {
        // ... all command handlers ...
    }

    // Print cache stats if debug mode enabled
    if cli.debug {
        client.cache_stats().print_summary();
    }

    Ok(())
}
```

### Success Criteria

#### Automated Verification:
- [ ] Code compiles: `make build`
- [ ] All tests pass: `make test`
- [ ] CLI parsing tests include debug flag

#### Manual Verification:
- [ ] `clack --debug users` shows cache statistics at end
- [ ] `clack cache clear` clears current workspace cache
- [ ] `clack cache clear --all` clears all workspace caches
- [ ] Cache stats show correct hit/miss counts
- [ ] Cache stats show correct hit rate percentages

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 7: Extended Schema (Files & Reactions)

### Overview
Add files, file_conversations, and reactions tables to support caching file and reaction data from search results.

### Changes Required

#### 1. Create Files Migration
**Command**: `diesel migration generate add_files_and_reactions`
**File**: `migrations/YYYYMMDDHHMMSS_add_files_and_reactions/up.sql`
**Changes**: Create files, file_conversations, and reactions tables

```sql
-- Files table
CREATE TABLE files (
    id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,

    -- File metadata
    name TEXT NOT NULL,
    title TEXT,
    mimetype TEXT NOT NULL,
    filetype TEXT NOT NULL,
    pretty_type TEXT,

    -- File content
    size INTEGER NOT NULL,
    created INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,

    -- User who uploaded
    user_id TEXT,

    -- URLs
    url_private TEXT,
    url_private_download TEXT,
    permalink TEXT,
    permalink_public TEXT,

    -- Cache metadata
    full_object TEXT NOT NULL,
    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,

    PRIMARY KEY (id, workspace_id),
    FOREIGN KEY (user_id, workspace_id) REFERENCES users(id, workspace_id)
);

CREATE INDEX idx_files_workspace_id ON files(workspace_id);
CREATE INDEX idx_files_user_id ON files(workspace_id, user_id);
CREATE INDEX idx_files_created ON files(workspace_id, created);
CREATE INDEX idx_files_filetype ON files(workspace_id, filetype);
CREATE INDEX idx_files_cached_at ON files(cached_at);

-- File Conversations junction table
CREATE TABLE file_conversations (
    file_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,

    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (file_id, workspace_id, conversation_id),
    FOREIGN KEY (file_id, workspace_id) REFERENCES files(id, workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (conversation_id, workspace_id) REFERENCES conversations(id, workspace_id) ON DELETE CASCADE
);

CREATE INDEX idx_file_conversations_file ON file_conversations(workspace_id, file_id);
CREATE INDEX idx_file_conversations_conversation ON file_conversations(workspace_id, conversation_id);

-- Reactions table
CREATE TABLE reactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Message identification
    conversation_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    message_ts TEXT NOT NULL,

    -- Reaction details
    name TEXT NOT NULL,
    count INTEGER NOT NULL,

    -- Cache metadata
    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (conversation_id, workspace_id, message_ts) REFERENCES messages(conversation_id, workspace_id, ts) ON DELETE CASCADE,
    UNIQUE(conversation_id, workspace_id, message_ts, name)
);

CREATE INDEX idx_reactions_message ON reactions(workspace_id, conversation_id, message_ts);
```

**File**: `migrations/YYYYMMDDHHMMSS_add_files_and_reactions/down.sql`
**Changes**: Drop new tables

```sql
DROP INDEX IF EXISTS idx_reactions_message;
DROP TABLE IF EXISTS reactions;

DROP INDEX IF EXISTS idx_file_conversations_conversation;
DROP INDEX IF EXISTS idx_file_conversations_file;
DROP TABLE IF EXISTS file_conversations;

DROP INDEX IF EXISTS idx_files_cached_at;
DROP INDEX IF EXISTS idx_files_filetype;
DROP INDEX IF EXISTS idx_files_created;
DROP INDEX IF EXISTS idx_files_user_id;
DROP INDEX IF EXISTS idx_files_workspace_id;
DROP TABLE IF EXISTS files;
```

#### 2. Regenerate Schema
**Command**: Run migrations to update schema
**File**: `src/cache/schema.rs` will be auto-updated by diesel

#### 3. Add Diesel Models for Files and Reactions
**File**: `src/cache/models.rs`
**Changes**: Add models for files and reactions

```rust
use super::schema::{file_conversations, files, reactions};

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = files)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CachedFile {
    pub id: String,
    pub workspace_id: String,

    pub name: String,
    pub title: Option<String>,
    pub mimetype: String,
    pub filetype: String,
    pub pretty_type: Option<String>,

    pub size: i32,
    pub created: i32,
    pub timestamp: i32,

    pub user_id: Option<String>,

    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub permalink: Option<String>,
    pub permalink_public: Option<String>,

    pub full_object: String,
    pub cached_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = file_conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CachedFileConversation {
    pub file_id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub cached_at: NaiveDateTime,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = reactions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CachedReaction {
    pub id: Option<i32>,
    pub conversation_id: String,
    pub workspace_id: String,
    pub message_ts: String,
    pub name: String,
    pub count: i32,
    pub cached_at: NaiveDateTime,
}
```

#### 4. Add Cache Operations for Files and Reactions
**File**: `src/cache/operations.rs`
**Changes**: Add CRUD operations for files and reactions

```rust
// Add file operations similar to user/conversation patterns
pub async fn upsert_file(
    conn: &mut CacheConnection,
    workspace_id: &str,
    file: &crate::models::search::FileResult,
    verbose: bool,
) -> Result<()> {
    // Implementation similar to upsert_user
    // Convert FileResult to CachedFile and insert
    // ...
}

pub async fn upsert_reactions(
    conn: &mut CacheConnection,
    workspace_id: &str,
    conversation_id: &str,
    message_ts: &str,
    reactions: &[crate::models::message::Reaction],
    verbose: bool,
) -> Result<()> {
    // Implementation to insert reactions
    // ...
}
```

### Success Criteria

#### Automated Verification:
- [ ] Migration runs successfully: `diesel migration run`
- [ ] Code compiles: `make build`
- [ ] All tests pass: `make test`
- [ ] Schema updated with new tables

#### Manual Verification:
- [ ] New tables exist: `sqlite3 ~/Library/Caches/clack/cache.db ".tables"`
- [ ] Files and reactions can be inserted and queried
- [ ] Foreign key constraints work correctly

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 8: Testing & Verification

### Overview
Add comprehensive test coverage for cache operations, TTL expiration, workspace isolation, and concurrent access.

### Changes Required

#### 1. Add Test Utilities
**File**: `src/cache/test_utils.rs` (new file)
**Changes**: Helper functions for testing with in-memory SQLite

```rust
#![cfg(test)]

use anyhow::Result;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::AsyncConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Create an in-memory SQLite database for testing
pub async fn create_test_pool() -> Result<Pool<SqliteConnection>> {
    // Create in-memory database
    let db_url = ":memory:";

    // Run migrations on sync connection first
    let mut sync_conn = SqliteConnection::establish(db_url)?;
    sync_conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;

    // Create async pool
    let config = AsyncDieselConnectionManager::<SqliteConnection>::new(db_url);
    let pool = Pool::builder(config)
        .max_size(1)
        .build()?;

    Ok(pool)
}
```

#### 2. Add Cache Operations Tests
**File**: `src/cache/operations.rs`
**Changes**: Add comprehensive test module

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::test_utils::create_test_pool;
    use crate::models::user::{User, UserProfile};

    #[tokio::test]
    async fn test_user_cache_miss() {
        let pool = create_test_pool().await.unwrap();
        let mut conn = pool.get().await.unwrap();

        let result = get_user(&mut conn, "T123", "U123", false, None).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_user_cache_hit() {
        let pool = create_test_pool().await.unwrap();
        let mut conn = pool.get().await.unwrap();

        let user = User {
            id: "U123".to_string(),
            name: "testuser".to_string(),
            real_name: Some("Test User".to_string()),
            deleted: false,
            is_bot: false,
            is_admin: None,
            is_owner: None,
            tz: None,
            profile: UserProfile {
                email: Some("test@example.com".to_string()),
                display_name: Some("testuser".to_string()),
                status_emoji: None,
                status_text: None,
                image_72: None,
            },
        };

        upsert_user(&mut conn, "T123", &user, false).await.unwrap();

        let cached = get_user(&mut conn, "T123", "U123", false, None).await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().id, "U123");
    }

    #[tokio::test]
    async fn test_workspace_isolation() {
        let pool = create_test_pool().await.unwrap();
        let mut conn = pool.get().await.unwrap();

        let user = User {
            id: "U123".to_string(),
            name: "testuser".to_string(),
            real_name: None,
            deleted: false,
            is_bot: false,
            is_admin: None,
            is_owner: None,
            tz: None,
            profile: UserProfile {
                email: None,
                display_name: None,
                status_emoji: None,
                status_text: None,
                image_72: None,
            },
        };

        // Insert into workspace T123
        upsert_user(&mut conn, "T123", &user, false).await.unwrap();

        // Should find in T123
        let result1 = get_user(&mut conn, "T123", "U123", false, None).await.unwrap();
        assert!(result1.is_some());

        // Should NOT find in T456
        let result2 = get_user(&mut conn, "T456", "U123", false, None).await.unwrap();
        assert!(result2.is_none());
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        // Note: This test would require mocking time or waiting
        // For now, we verify the is_fresh function logic
        let old_time = chrono::Utc::now().naive_utc() - chrono::Duration::seconds(7200);
        let recent_time = chrono::Utc::now().naive_utc() - chrono::Duration::seconds(30);

        assert!(!is_fresh(old_time, 3600)); // 2 hours old, 1 hour TTL - stale
        assert!(is_fresh(recent_time, 3600)); // 30 seconds old, 1 hour TTL - fresh
    }

    #[tokio::test]
    async fn test_clear_workspace_cache() {
        let pool = create_test_pool().await.unwrap();
        let mut conn = pool.get().await.unwrap();

        let user = User {
            id: "U123".to_string(),
            name: "testuser".to_string(),
            real_name: None,
            deleted: false,
            is_bot: false,
            is_admin: None,
            is_owner: None,
            tz: None,
            profile: UserProfile {
                email: None,
                display_name: None,
                status_emoji: None,
                status_text: None,
                image_72: None,
            },
        };

        upsert_user(&mut conn, "T123", &user, false).await.unwrap();

        let before = get_user(&mut conn, "T123", "U123", false, None).await.unwrap();
        assert!(before.is_some());

        clear_workspace_cache(&mut conn, "T123", false).await.unwrap();

        let after = get_user(&mut conn, "T123", "U123", false, None).await.unwrap();
        assert!(after.is_none());
    }
}
```

#### 3. Add Integration Tests
**File**: `tests/cache_integration_test.rs` (new file)
**Changes**: End-to-end cache integration tests

```rust
use clack::cache::{create_cache_pool, operations};
use clack::models::user::{User, UserProfile};

#[tokio::test]
async fn test_cache_pool_creation() {
    // Test that cache pool can be created
    let result = create_cache_pool(false).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let pool = create_cache_pool(false).await.unwrap();

    let user = User {
        id: "U123".to_string(),
        name: "testuser".to_string(),
        real_name: None,
        deleted: false,
        is_bot: false,
        is_admin: None,
        is_owner: None,
        tz: None,
        profile: UserProfile {
            email: None,
            display_name: None,
            status_emoji: None,
            status_text: None,
            image_72: None,
        },
    };

    // Spawn multiple concurrent tasks
    let mut handles = vec![];

    for _ in 0..10 {
        let pool_clone = pool.clone();
        let user_clone = user.clone();

        let handle = tokio::spawn(async move {
            let mut conn = pool_clone.get().await.unwrap();
            operations::upsert_user(&mut conn, "T123", &user_clone, false)
                .await
                .unwrap();
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify user exists
    let mut conn = pool.get().await.unwrap();
    let cached = operations::get_user(&mut conn, "T123", "U123", false, None)
        .await
        .unwrap();
    assert!(cached.is_some());
}
```

#### 4. Update Makefile
**File**: `Makefile`
**Changes**: Ensure test target runs all tests

```makefile
test:
	cargo test --all-features

test-verbose:
	cargo test --all-features -- --nocapture

test-cache:
	cargo test --all-features --lib cache
	cargo test --all-features --test cache_integration_test
```

### Success Criteria

#### Automated Verification:
- [ ] All unit tests pass: `make test`
- [ ] Cache operations tests pass: `make test-cache`
- [ ] Integration tests pass
- [ ] Tests cover: cache hit, miss, TTL, workspace isolation, concurrent access
- [ ] No test failures or panics

#### Manual Verification:
- [ ] Tests run in reasonable time (< 10 seconds total)
- [ ] In-memory SQLite works correctly for tests
- [ ] Concurrent access tests don't deadlock or race

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Testing Strategy

### Unit Tests
- **Location**: Inline `#[cfg(test)]` modules in each source file
- **Focus**: Individual functions and operations
- **Database**: In-memory SQLite (`:memory:`) for fast, isolated tests
- **Coverage**:
  - Cache CRUD operations (insert, get, update, delete)
  - TTL checking logic
  - Workspace isolation
  - Model conversion (API ↔ Cache)

### Integration Tests
- **Location**: `tests/` directory
- **Focus**: End-to-end scenarios
- **Database**: Temporary directory within build directory (NOT system cache location)
  - Use `tempfile` crate to create isolated test databases
  - Each test gets its own temporary database file
  - Automatically cleaned up after test completion
  - Prevents pollution of production cache directory
- **Coverage**:
  - Pool creation and connection management
  - Concurrent access scenarios
  - Migration application
  - Full API → Cache → API roundtrip
  - WAL mode verification
  - File-based database operations

### Test Database Strategy
- **Production**: Uses platform-specific cache directory (`~/Library/Caches/clack/cache.db` on macOS)
- **Tests**: Use temporary files in build directory
  - Integration tests: `tempfile::tempdir()` for isolated file-based tests
  - Unit tests: `:memory:` for fast in-memory tests
- **Benefits**:
  - No pollution of user's cache directory during development
  - Each test run is completely isolated
  - Parallel test execution without conflicts
  - Automatic cleanup on test completion

### Manual Testing Checklist
- [ ] First run creates cache directory at correct platform location
- [ ] Database file created with correct permissions
- [ ] WAL mode enabled (check with `PRAGMA journal_mode`)
- [ ] Foreign key constraints enforced
- [ ] Cache improves performance (measure with time command)
- [ ] `--debug` shows accurate statistics
- [ ] `--verbose` shows detailed cache operations
- [ ] Multiple workspaces don't interfere
- [ ] Cache clear removes data correctly

## Performance Considerations

### Optimization Strategy
- **Keep it simple initially** - optimize based on actual usage patterns
- **Bulk operations**: Use transactions for batch inserts (messages)
- **Connection pooling**: deadpool handles concurrency efficiently
- **Indexes**: Already created on frequently queried columns
- **WAL mode**: Better concurrent read performance

### Expected Performance Gains
- **Users list**: ~90% faster on cache hit (no API roundtrip)
- **Channels list**: ~85% faster on cache hit
- **Messages (with --use-cache)**: ~95% faster on cache hit

### Memory Usage
- **SQLite database**: Grows with usage, typically < 10MB for normal usage
- **Connection pool**: Max 8 connections, minimal memory overhead
- **No size limits initially** - can add if needed

## Migration Notes

### Database Schema Versioning
- **diesel_migrations** tracks applied migrations in `__diesel_schema_migrations` table
- **Forward-only**: Up migrations create schema, down migrations revert
- **Automatic**: Migrations run on first cache access (in `init_cache_db`)

### Upgrading clack
- New versions may include new migrations
- Migrations run automatically on first use
- Old cache data preserved (schema extended, not replaced)
- Users don't need to manually migrate

### Rollback Strategy
- If migration fails, cache is disabled (silent fallback to API)
- User can manually delete cache database to start fresh
- Down migrations available but not automatically applied

## References

- Original task: `/Users/tim.courrejou/timcode/clack/tasks/2026-01-15-object-caching.md`
- Slack API auth.test: https://api.slack.com/methods/auth.test
- diesel-async docs: https://docs.rs/diesel-async/
- deadpool docs: https://docs.rs/deadpool/
- SQLite WAL mode: https://www.sqlite.org/wal.html
