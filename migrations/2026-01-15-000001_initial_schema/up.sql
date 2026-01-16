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
