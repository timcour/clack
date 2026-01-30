-- Events table for caching received Socket Mode events
CREATE TABLE IF NOT EXISTS events (
    -- Composite primary key
    event_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,

    -- Event metadata
    event_type TEXT NOT NULL,
    event_time INTEGER NOT NULL,
    api_app_id TEXT NOT NULL,

    -- Message event fields (nullable for other event types)
    channel_id TEXT,
    user_id TEXT,
    message_ts TEXT,
    message_text TEXT,
    thread_ts TEXT,
    subtype TEXT,

    -- Full event payload as JSON
    full_payload TEXT NOT NULL,

    -- Cache metadata
    cached_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (event_id, workspace_id)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_events_workspace_time ON events(workspace_id, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_channel ON events(workspace_id, channel_id, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_user ON events(workspace_id, user_id, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_cached_at ON events(cached_at);
