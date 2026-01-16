# Gist
Many Slack API objects infrequently change. Let's cache them locally!

Below are some requirements and considerations. Give feedback when
there may be a better option (library, architectural decision,
implementation detail, etc.).

After reading the requirements, suggestions, and considerations,
suggest adding to any of the sections that seems missing. Then once I
approve, propose a plan that takes all of it into account. Explain the
trade-offs, feedback and concrete implementation proposal.

# Requirements
 1. Any [Slack object](https://docs.slack.dev/reference/objects) that
    is received in an API call's response should be cached locally.
 2. For DRY structuring to leverage an existing query language, the
    objects should be stored in a relational database (suggestion:
    Probably the latest SQLite).
 3. Associated with every cached object should be the timestamp when
    it was last refreshed, and whether it has been deleted upstream
    (do not delete messages that were deleted upstream).


# Suggestions
 - SQLite is a good candidate for the relational database because of
   ease of install, portability and existing SQL implementation.
 - plural table names are nice.
 - Use diesel-async as the ORM (we already have an async runtime in place).
 - Use a migration framework that works with SQLite and diesel-async,
   with support for version tracking and management (e.g., diesel migrations).

# Considerations
  - naming consistency with the slack api object and field names

# Cache Behavior
 - **Message lists**: Do NOT use cache by default. Only use cache when
   `--use-cache` flag is explicitly passed. This ensures users always
   get fresh message data unless they opt into using cached data.
 - **Other objects** (users, channels):
   - Check cache first, return if fresh (< TTL)
   - If stale or missing, fetch from API and update cache
   - TTLs:
     - Users: 1 hour (change infrequently)
     - Channels: 30 minutes
     - Messages: 5 minutes (when cached via --use-cache)
 - **Search results**: Never cache (too dynamic)
 - **Write path**: All API responses automatically update cache
 - **Conflict resolution**: If cached data conflicts with API response,
   API response wins (overwrite cached data)
 - **Partial updates**: Do NOT mark non-returned messages as deleted.
   Only mark objects as deleted when API explicitly indicates deletion.

# Cache Storage
 - **Location**: Platform-specific cache directory (using `dirs` crate):
   - Linux: `~/.cache/clack/cache.db`
   - macOS: `~/Library/Caches/clack/cache.db`
   - Windows: `%LOCALAPPDATA%\clack\cache.db`
 - **Initialization**: Create database on first use
 - **Concurrent access**: Use SQLite's built-in locking mechanisms
   (WAL mode for better concurrent read performance)
 - **Security**: No encryption (local CLI application), use default
   file permissions
 - **Size limits**: None initially (can add later if needed)

# Schema Design
 - **Normalized tables** matching Slack object types:
   - `users` table
   - `channels` table
   - `messages` table
   - etc.
 - **Each table includes**:
   - All relevant Slack API fields as typed columns
   - `full_object` JSON blob column (for flexibility/forward compatibility)
   - `cached_at` TIMESTAMP (when object was last fetched)
   - `deleted_at` TIMESTAMP NULL (when object was marked as deleted, NULL if active)
 - **Indexes**: Create indexes on commonly queried fields:
   - Primary keys (id)
   - User/channel names
   - Message timestamps
   - Channel IDs in messages (for foreign key lookups)

# Testing Strategy
 - **Unit tests**: Use mock cache layer to test caching logic independently
 - **Integration tests**: Use in-memory SQLite database (`:memory:`)
   for testing actual cache operations end-to-end
 - **Coverage**: Test cache hits, misses, TTL expiration, conflict
   resolution, concurrent access scenarios

# Performance Considerations
 - Keep it simple initially, optimize later based on actual usage patterns
 - Bulk insert operations for message lists (single transaction)
 - Connection pooling handled by diesel-async

# Database Schema

## Table: users
Stores Slack workspace users. Profile fields are flattened into this table for easier querying.

```sql
CREATE TABLE users (
    id TEXT PRIMARY KEY,              -- Slack user ID (e.g., U1234ABCD)
    name TEXT NOT NULL,               -- Username/handle
    real_name TEXT,                   -- Full/real name
    deleted BOOLEAN NOT NULL DEFAULT 0, -- Slack's deleted flag (from user.deleted)

    -- User flags
    is_bot BOOLEAN NOT NULL DEFAULT 0,-- Whether user is a bot
    is_admin BOOLEAN,                 -- Whether user is workspace admin
    is_owner BOOLEAN,                 -- Whether user is workspace owner

    -- User preferences
    tz TEXT,                          -- Timezone identifier (e.g., "America/Los_Angeles")

    -- Profile fields (flattened from user.profile object)
    profile_email TEXT,               -- Email address (may be NULL if not accessible)
    profile_display_name TEXT,        -- Display name from profile
    profile_status_emoji TEXT,        -- Current status emoji (e.g., ":palm_tree:")
    profile_status_text TEXT,         -- Current status text (e.g., "On vacation")
    profile_image_72 TEXT,            -- URL to 72x72 profile image

    -- Cache metadata
    full_object TEXT NOT NULL,        -- Complete JSON of Slack user object
    cached_at TIMESTAMP NOT NULL,     -- When this record was last fetched from API
    deleted_at TIMESTAMP,             -- When we mark as deleted in cache (NULL if active)

    UNIQUE(id)
);

CREATE INDEX idx_users_name ON users(name);
CREATE INDEX idx_users_profile_email ON users(profile_email);
CREATE INDEX idx_users_cached_at ON users(cached_at);
CREATE INDEX idx_users_deleted ON users(deleted, deleted_at);
```

## Table: conversations
Stores Slack conversations (channels, private channels, DMs, group DMs).
This follows Slack's modern "conversations" API terminology.

```sql
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,              -- Slack conversation ID (e.g., C1234ABCD, D123, G456)
    name TEXT NOT NULL,               -- Conversation name (without #, or user ID for DMs)

    -- Conversation type flags
    is_channel BOOLEAN,               -- True for public channels
    is_group BOOLEAN,                 -- True for private channels
    is_im BOOLEAN,                    -- True for direct messages (1:1)
    is_mpim BOOLEAN,                  -- True for multi-person DMs
    is_private BOOLEAN,               -- True for private channels
    is_archived BOOLEAN NOT NULL DEFAULT 0, -- Slack's archived flag

    -- Conversation metadata (flattened from topic/purpose objects)
    topic_value TEXT,                 -- Channel topic text (from conversation.topic.value)
    topic_creator TEXT,               -- User ID who set topic (optional, for future use)
    topic_last_set INTEGER,           -- Unix timestamp when topic was set (optional)
    purpose_value TEXT,               -- Channel purpose text (from conversation.purpose.value)
    purpose_creator TEXT,             -- User ID who set purpose (optional)
    purpose_last_set INTEGER,         -- Unix timestamp when purpose was set (optional)

    num_members INTEGER,              -- Number of members in conversation

    -- Cache metadata
    full_object TEXT NOT NULL,        -- Complete JSON of Slack conversation object
    cached_at TIMESTAMP NOT NULL,     -- When this record was last fetched from API
    deleted_at TIMESTAMP,             -- When marked as deleted (NULL if active)

    UNIQUE(id)
);

CREATE INDEX idx_conversations_name ON conversations(name);
CREATE INDEX idx_conversations_is_archived ON conversations(is_archived);
CREATE INDEX idx_conversations_type ON conversations(is_channel, is_group, is_im, is_mpim);
CREATE INDEX idx_conversations_cached_at ON conversations(cached_at);
```

## Table: messages
Stores Slack messages. Uses composite primary key (conversation_id, ts) since
timestamps are unique within a conversation but not globally.

```sql
CREATE TABLE messages (
    -- Composite primary key: conversation + timestamp uniquely identifies a message
    conversation_id TEXT NOT NULL,    -- Conversation where message was posted
    ts TEXT NOT NULL,                 -- Message timestamp (Slack's unique ID: "1234567890.123456")

    -- Message content
    user_id TEXT,                     -- User who posted (NULL for system messages, bot messages)
    text TEXT NOT NULL,               -- Message text content
    thread_ts TEXT,                   -- Parent message ts if this is a thread reply (NULL if root)

    -- Message metadata (from search results or API responses)
    permalink TEXT,                   -- Permanent URL to message (when available)

    -- Cache metadata
    full_object TEXT NOT NULL,        -- Complete JSON of Slack message object
    cached_at TIMESTAMP NOT NULL,     -- When this record was last fetched from API
    deleted_at TIMESTAMP,             -- When marked as deleted (NULL if active)

    PRIMARY KEY (conversation_id, ts),
    FOREIGN KEY (conversation_id) REFERENCES conversations(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_messages_conversation_id ON messages(conversation_id);
CREATE INDEX idx_messages_user_id ON messages(user_id);
CREATE INDEX idx_messages_thread_ts ON messages(thread_ts);
CREATE INDEX idx_messages_ts ON messages(ts);
CREATE INDEX idx_messages_cached_at ON messages(cached_at);
CREATE INDEX idx_messages_conversation_ts ON messages(conversation_id, ts); -- For thread queries
```

## Table: reactions
Stores emoji reactions on messages. Reactions are aggregated counts, not individual user reactions.

```sql
CREATE TABLE reactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Message identification (composite foreign key)
    conversation_id TEXT NOT NULL,    -- Conversation of parent message
    message_ts TEXT NOT NULL,         -- Timestamp of parent message

    -- Reaction details
    name TEXT NOT NULL,               -- Emoji name (e.g., "thumbsup", "heart")
    count INTEGER NOT NULL,           -- Number of users who reacted with this emoji

    -- Cache metadata
    cached_at TIMESTAMP NOT NULL,     -- When this record was last fetched from API

    FOREIGN KEY (conversation_id, message_ts) REFERENCES messages(conversation_id, ts) ON DELETE CASCADE,
    UNIQUE(conversation_id, message_ts, name)
);

CREATE INDEX idx_reactions_message ON reactions(conversation_id, message_ts);
```

## Table: files
Stores Slack file uploads (documents, images, etc.).

```sql
CREATE TABLE files (
    id TEXT PRIMARY KEY,              -- Slack file ID (e.g., F1234ABCD)

    -- File metadata
    name TEXT NOT NULL,               -- Filename (e.g., "document.pdf")
    title TEXT,                       -- File title (user-provided, may differ from filename)
    mimetype TEXT NOT NULL,           -- MIME type (e.g., "application/pdf")
    filetype TEXT NOT NULL,           -- File type (e.g., "pdf", "png", "mp4")
    pretty_type TEXT,                 -- Human-readable type (e.g., "PDF", "PNG Image")

    -- File content
    size INTEGER NOT NULL,            -- File size in bytes
    created INTEGER NOT NULL,         -- Unix timestamp when file was created
    timestamp INTEGER NOT NULL,       -- Unix timestamp (duplicate of created, kept for compatibility)

    -- User who uploaded
    user_id TEXT,                     -- User who uploaded the file
    FOREIGN KEY (user_id) REFERENCES users(id),

    -- URLs (all optional, depend on permissions and file type)
    url_private TEXT,                 -- Private URL (requires auth)
    url_private_download TEXT,        -- Private download URL (requires auth)
    permalink TEXT,                   -- Public permalink
    permalink_public TEXT,            -- Public permalink (if enabled)

    -- Cache metadata
    full_object TEXT NOT NULL,        -- Complete JSON of Slack file object
    cached_at TIMESTAMP NOT NULL,     -- When this record was last fetched from API
    deleted_at TIMESTAMP,             -- When marked as deleted (NULL if active)

    UNIQUE(id)
);

CREATE INDEX idx_files_user_id ON files(user_id);
CREATE INDEX idx_files_created ON files(created);
CREATE INDEX idx_files_filetype ON files(filetype);
CREATE INDEX idx_files_cached_at ON files(cached_at);
```

## Table: file_conversations
Junction table for many-to-many relationship between files and conversations.
A file can be shared in multiple conversations.

```sql
CREATE TABLE file_conversations (
    file_id TEXT NOT NULL,            -- Slack file ID
    conversation_id TEXT NOT NULL,    -- Conversation where file is shared

    -- When this association was cached
    cached_at TIMESTAMP NOT NULL,     -- When this association was observed

    PRIMARY KEY (file_id, conversation_id),
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE INDEX idx_file_conversations_file ON file_conversations(file_id);
CREATE INDEX idx_file_conversations_conversation ON file_conversations(conversation_id);
```

## Notes
- **Timestamps**: Use INTEGER for SQLite (Unix epoch seconds) or TEXT (ISO 8601)
- **Boolean**: SQLite uses INTEGER (0 = false, 1 = true)
- **Foreign Keys**: Must enable with `PRAGMA foreign_keys = ON;`
- **WAL Mode**: Enable with `PRAGMA journal_mode = WAL;` for better concurrent access
- **JSON Storage**: `full_object` column stores complete Slack API response for forward compatibility
```
