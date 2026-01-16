use chrono::NaiveDateTime;
use diesel::prelude::*;

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
            permalink: message.permalink.clone(),
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
