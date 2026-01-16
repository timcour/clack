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
