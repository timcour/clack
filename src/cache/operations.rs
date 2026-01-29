use anyhow::Result;
use chrono::Utc;
use diesel::prelude::*;

use super::db::CacheConnection;
use super::models::{CachedConversation, CachedMessage, CachedUser};
use super::schema::{conversations, messages, users};
use crate::models::channel::Channel;
use crate::models::message::Message;
use crate::models::user::User;

// TTL constants (in seconds)
const USER_TTL_SECONDS: i64 = 3600 * 24 * 7; // 1 week
const CONVERSATION_TTL_SECONDS: i64 = 3600 * 24 * 7; // 1 week
const MESSAGE_TTL_SECONDS: i64 = 3600 * 24 * 7; // 1 week

/// Check if a cached item is fresh based on TTL
fn is_fresh(cached_at: chrono::NaiveDateTime, ttl_seconds: i64) -> bool {
    let cached_at_utc = chrono::DateTime::<Utc>::from_naive_utc_and_offset(cached_at, Utc);
    let age = Utc::now().signed_duration_since(cached_at_utc);
    age.num_seconds() < ttl_seconds
}

// User operations

/// Get a user from cache by ID.
///
/// # Arguments
/// * `ttl_override` - Optional TTL in seconds. If provided, overrides the default TTL.
///   Use `Some(i64::MAX)` to effectively ignore staleness and return any cached record.
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

    let ttl = ttl_override.unwrap_or(USER_TTL_SECONDS);

    match cached_user {
        Some(cached) => {
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

/// Get all users from cache for a workspace.
///
/// # Arguments
/// * `ttl_override` - Optional TTL in seconds. If provided, overrides the default TTL.
///   Use `Some(i64::MAX)` to effectively ignore staleness and return any cached records.
pub fn get_users(
    conn: &mut CacheConnection,
    ws_id: &str,
    verbose: bool,
    ttl_override: Option<i64>,
) -> Result<Option<Vec<User>>> {
    use super::schema::users::dsl::*;

    let cached_users: Vec<CachedUser> = users
        .filter(workspace_id.eq(ws_id))
        .filter(deleted_at.is_null())
        .load(conn)?;

    if cached_users.is_empty() {
        if verbose {
            eprintln!("[CACHE] Users - MISS (empty)");
        }
        return Ok(None);
    }

    let ttl = ttl_override.unwrap_or(USER_TTL_SECONDS);

    // Check if all users are fresh
    let all_fresh = cached_users.iter().all(|u| is_fresh(u.cached_at, ttl));

    if all_fresh {
        if verbose {
            eprintln!("[CACHE] Users - HIT ({} users)", cached_users.len());
        }
        let api_users: Result<Vec<User>> = cached_users.iter().map(|u| u.to_api_user()).collect();
        Ok(Some(api_users?))
    } else {
        if verbose {
            eprintln!("[CACHE] Users - MISS (some stale)");
        }
        Ok(None)
    }
}

pub fn upsert_user(
    conn: &mut CacheConnection,
    workspace_id: &str,
    user: &User,
    verbose: bool,
) -> Result<()> {
    let cached = CachedUser::from_api_user(user, workspace_id);

    diesel::replace_into(users::table)
        .values(&cached)
        .execute(conn)
        ?;

    if verbose {
        eprintln!("[CACHE] User {} - UPSERTED", user.id);
    }

    Ok(())
}

pub fn upsert_users(
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
            ?;
    }

    if verbose {
        eprintln!("[CACHE] Users - UPSERTED {} users", user_list.len());
    }

    Ok(())
}

// Conversation operations

/// Get a conversation from cache by ID.
///
/// # Arguments
/// * `ttl_override` - Optional TTL in seconds. If provided, overrides the default TTL.
///   Use `Some(i64::MAX)` to effectively ignore staleness and return any cached record.
pub fn get_conversation(
    conn: &mut CacheConnection,
    ws_id: &str,
    conversation_id: &str,
    verbose: bool,
    ttl_override: Option<i64>,
) -> Result<Option<Channel>> {
    use super::schema::conversations::dsl::*;

    let cached_conv: Option<CachedConversation> = conversations
        .filter(id.eq(conversation_id))
        .filter(workspace_id.eq(ws_id))
        .filter(deleted_at.is_null())
        .first(conn)
        .optional()?;

    let ttl = ttl_override.unwrap_or(CONVERSATION_TTL_SECONDS);

    match cached_conv {
        Some(cached) => {
            if is_fresh(cached.cached_at, ttl) {
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

/// Get all conversations from cache for a workspace.
///
/// # Arguments
/// * `ttl_override` - Optional TTL in seconds. If provided, overrides the default TTL.
///   Use `Some(i64::MAX)` to effectively ignore staleness and return any cached records.
pub fn get_conversations(
    conn: &mut CacheConnection,
    ws_id: &str,
    verbose: bool,
    ttl_override: Option<i64>,
) -> Result<Option<Vec<Channel>>> {
    use super::schema::conversations::dsl::*;

    let cached_convs: Vec<CachedConversation> = conversations
        .filter(workspace_id.eq(ws_id))
        .filter(deleted_at.is_null())
        .load(conn)?;

    if cached_convs.is_empty() {
        if verbose {
            eprintln!("[CACHE] Conversations - MISS (empty)");
        }
        return Ok(None);
    }

    let ttl = ttl_override.unwrap_or(CONVERSATION_TTL_SECONDS);

    let all_fresh = cached_convs.iter().all(|c| is_fresh(c.cached_at, ttl));

    if all_fresh {
        if verbose {
            eprintln!(
                "[CACHE] Conversations - HIT ({} conversations)",
                cached_convs.len()
            );
        }
        let api_channels: Result<Vec<Channel>> =
            cached_convs.iter().map(|c| c.to_api_channel()).collect();
        Ok(Some(api_channels?))
    } else {
        if verbose {
            eprintln!("[CACHE] Conversations - MISS (some stale)");
        }
        Ok(None)
    }
}

pub fn upsert_conversation(
    conn: &mut CacheConnection,
    workspace_id: &str,
    channel: &Channel,
    verbose: bool,
) -> Result<()> {
    let cached = CachedConversation::from_api_channel(channel, workspace_id);

    diesel::replace_into(conversations::table)
        .values(&cached)
        .execute(conn)
        ?;

    if verbose {
        eprintln!("[CACHE] Conversation #{} ({}) - UPSERTED", channel.name, channel.id);
    }

    Ok(())
}

pub fn upsert_conversations(
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
            ?;

        if verbose {
            eprintln!("[CACHE] Conversation #{} ({}) - UPSERTED", channel.name, channel.id);
        }
    }

    if verbose {
        eprintln!("[CACHE] Conversations - UPSERTED {} conversations total", channel_list.len());
    }

    Ok(())
}

// Message operations

pub fn get_messages(
    conn: &mut CacheConnection,
    ws_id: &str,
    conv_id: &str,
    verbose: bool,
) -> Result<Option<Vec<Message>>> {
    use super::schema::messages::dsl::*;

    let cached_msgs: Vec<CachedMessage> = messages
        .filter(conversation_id.eq(conv_id))
        .filter(workspace_id.eq(ws_id))
        .filter(deleted_at.is_null())
        .load(conn)
        ?;

    if cached_msgs.is_empty() {
        if verbose {
            eprintln!("[CACHE] Messages (conv {}) - MISS (empty)", conv_id);
        }
        return Ok(None);
    }

    let all_fresh = cached_msgs
        .iter()
        .all(|m| is_fresh(m.cached_at, MESSAGE_TTL_SECONDS));

    if all_fresh {
        if verbose {
            eprintln!("[CACHE] Messages (conv {}) - HIT ({} messages)", conv_id, cached_msgs.len());
        }
        let api_messages: Result<Vec<Message>> = cached_msgs
            .iter()
            .map(|m| m.to_api_message())
            .collect();
        Ok(Some(api_messages?))
    } else {
        if verbose {
            eprintln!("[CACHE] Messages (conv {}) - MISS (some stale)", conv_id);
        }
        Ok(None)
    }
}

pub fn upsert_messages(
    conn: &mut CacheConnection,
    workspace_id: &str,
    conv_id: &str,
    message_list: &[Message],
    verbose: bool,
) -> Result<()> {
    for message in message_list {
        let cached = CachedMessage::from_api_message(message, conv_id, workspace_id);
        diesel::replace_into(messages::table)
            .values(&cached)
            .execute(conn)
            ?;
    }

    if verbose {
        eprintln!("[CACHE] Messages (conv {}) - UPSERTED {} messages", conv_id, message_list.len());
    }

    Ok(())
}

// Cache clearing operations

pub fn clear_workspace_cache(
    conn: &mut CacheConnection,
    workspace_id: &str,
    verbose: bool,
) -> Result<()> {
    use super::schema::{conversations, messages, users};

    diesel::delete(messages::table.filter(messages::workspace_id.eq(workspace_id)))
        .execute(conn)
        ?;

    diesel::delete(conversations::table.filter(conversations::workspace_id.eq(workspace_id)))
        .execute(conn)
        ?;

    diesel::delete(users::table.filter(users::workspace_id.eq(workspace_id)))
        .execute(conn)
        ?;

    if verbose {
        eprintln!("[CACHE] Cleared all cache for workspace {}", workspace_id);
    }

    Ok(())
}

pub fn clear_all_cache(conn: &mut CacheConnection, verbose: bool) -> Result<()> {
    use super::schema::{conversations, messages, users};

    diesel::delete(messages::table).execute(conn)?;
    diesel::delete(conversations::table).execute(conn)?;
    diesel::delete(users::table).execute(conn)?;

    if verbose {
        eprintln!("[CACHE] Cleared all cache");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_fresh_with_default_ttl() {
        // Test that recently cached items are fresh
        let now = Utc::now().naive_utc();
        assert!(is_fresh(now, USER_TTL_SECONDS));

        // Test that old items are stale
        let old_time = now - chrono::Duration::seconds(USER_TTL_SECONDS + 1);
        assert!(!is_fresh(old_time, USER_TTL_SECONDS));
    }

    #[test]
    fn test_is_fresh_with_custom_ttl() {
        let now = Utc::now().naive_utc();

        // With a very short TTL (1 second), recent items should still be fresh
        assert!(is_fresh(now, 1));

        // With i64::MAX TTL, even very old items should be fresh
        let very_old = now - chrono::Duration::days(365 * 10); // 10 years ago
        assert!(is_fresh(very_old, i64::MAX));
    }

    #[test]
    fn test_ttl_override_ignores_staleness() {
        // This test verifies the concept: with i64::MAX as TTL override,
        // any cached record should be considered fresh regardless of age
        let ten_years_ago = Utc::now().naive_utc() - chrono::Duration::days(365 * 10);

        // With default TTL, should be stale
        assert!(!is_fresh(ten_years_ago, USER_TTL_SECONDS));

        // With i64::MAX TTL override, should be fresh
        assert!(is_fresh(ten_years_ago, i64::MAX));
    }
}
