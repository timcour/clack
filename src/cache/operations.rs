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

pub fn get_user(
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

pub fn get_users(
    conn: &mut CacheConnection,
    workspace_id: &str,
    verbose: bool,
) -> Result<Option<Vec<User>>> {
    use super::schema::users::dsl::*;

    let cached_users: Vec<CachedUser> = users
        .filter(workspace_id.eq(workspace_id))
        .filter(deleted_at.is_null())
        .load(conn)
        ?;

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

pub fn get_conversation(
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

pub fn get_conversations(
    conn: &mut CacheConnection,
    workspace_id: &str,
    verbose: bool,
) -> Result<Option<Vec<Channel>>> {
    use super::schema::conversations::dsl::*;

    let cached_convs: Vec<CachedConversation> = conversations
        .filter(workspace_id.eq(workspace_id))
        .filter(deleted_at.is_null())
        .load(conn)
        ?;

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
        eprintln!("[CACHE] Conversation {} - UPSERTED", channel.id);
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
    }

    if verbose {
        eprintln!("[CACHE] Conversations - UPSERTED {} conversations", channel_list.len());
    }

    Ok(())
}

// Message operations

pub fn get_messages(
    conn: &mut CacheConnection,
    workspace_id: &str,
    conv_id: &str,
    verbose: bool,
) -> Result<Option<Vec<Message>>> {
    use super::schema::messages::dsl::*;

    let cached_msgs: Vec<CachedMessage> = messages
        .filter(conversation_id.eq(conv_id))
        .filter(workspace_id.eq(workspace_id))
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

pub fn clear_all_cache(
    conn: &mut CacheConnection,
    verbose: bool,
) -> Result<()> {
    use super::schema::{conversations, messages, users};

    diesel::delete(messages::table).execute(conn)?;
    diesel::delete(conversations::table).execute(conn)?;
    diesel::delete(users::table).execute(conn)?;

    if verbose {
        eprintln!("[CACHE] Cleared all cache");
    }

    Ok(())
}
