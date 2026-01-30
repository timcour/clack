use serde::{Deserialize, Serialize};

/// The outer event callback wrapper from Events API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EventCallback {
    /// Always "event_callback" for events
    #[serde(rename = "type")]
    pub callback_type: String,

    /// Workspace ID
    pub team_id: String,

    /// App ID
    pub api_app_id: String,

    /// The actual event payload
    pub event: Event,

    /// Unique event ID
    pub event_id: String,

    /// Unix timestamp of when event occurred
    pub event_time: i64,

    /// Event context (for newer events)
    pub event_context: Option<String>,

    /// Authorizations for this event
    pub authorizations: Option<Vec<Authorization>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Authorization {
    pub enterprise_id: Option<String>,
    pub team_id: Option<String>,
    pub user_id: Option<String>,
    pub is_bot: Option<bool>,
    pub is_enterprise_install: Option<bool>,
}

/// Event payload - currently only message type supported
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "message")]
    Message(MessageEvent),

    /// Catch-all for unsupported event types
    #[serde(other)]
    Unknown,
}

/// Message event payload
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MessageEvent {
    /// Channel ID where message was posted
    pub channel: String,

    /// User ID who sent the message (None for bot messages without user)
    pub user: Option<String>,

    /// Message text content
    pub text: Option<String>,

    /// Message timestamp (unique identifier)
    pub ts: String,

    /// Thread parent timestamp (if this is a reply)
    pub thread_ts: Option<String>,

    /// Message subtype (e.g., "bot_message", "channel_join")
    pub subtype: Option<String>,

    /// Bot ID if sent by a bot
    pub bot_id: Option<String>,

    /// App ID if sent by an app
    pub app_id: Option<String>,

    /// Channel type: "channel", "group", "im", "mpim"
    pub channel_type: Option<String>,

    /// Event timestamp
    pub event_ts: Option<String>,

    /// Blocks for rich message content
    pub blocks: Option<serde_json::Value>,

    /// Attachments
    pub attachments: Option<serde_json::Value>,

    /// Files attached to message
    pub files: Option<serde_json::Value>,

    /// If message was edited
    pub edited: Option<EditedInfo>,

    /// Hidden flag (for some message types)
    pub hidden: Option<bool>,

    /// For message_changed subtype - the actual message
    pub message: Option<Box<MessageEvent>>,

    /// For message_changed subtype - previous message
    pub previous_message: Option<Box<MessageEvent>>,

    /// For message_deleted subtype
    pub deleted_ts: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EditedInfo {
    pub user: String,
    pub ts: String,
}

impl MessageEvent {
    /// Get the effective user ID (handles bot messages)
    pub fn effective_user(&self) -> Option<&str> {
        self.user.as_deref().or(self.bot_id.as_deref())
    }

    /// Check if this is a bot message
    pub fn is_bot(&self) -> bool {
        self.bot_id.is_some() || self.subtype.as_deref() == Some("bot_message")
    }

    /// Check if this is a thread reply
    pub fn is_thread_reply(&self) -> bool {
        self.thread_ts.is_some() && self.thread_ts.as_deref() != Some(&self.ts)
    }

    /// Get display text (handles None case)
    pub fn display_text(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_event() {
        let json = r#"{
            "type": "message",
            "channel": "C123ABC456",
            "user": "U123ABC456",
            "text": "Hello world",
            "ts": "1355517523.000005",
            "event_ts": "1355517523.000005",
            "channel_type": "channel"
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Message(msg) => {
                assert_eq!(msg.channel, "C123ABC456");
                assert_eq!(msg.user, Some("U123ABC456".to_string()));
                assert_eq!(msg.text, Some("Hello world".to_string()));
                assert_eq!(msg.ts, "1355517523.000005");
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_parse_bot_message() {
        let json = r#"{
            "type": "message",
            "subtype": "bot_message",
            "channel": "C123ABC456",
            "bot_id": "B123",
            "text": "Bot says hello",
            "ts": "1355517523.000005"
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Message(msg) => {
                assert!(msg.is_bot());
                assert_eq!(msg.effective_user(), Some("B123"));
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_parse_thread_reply() {
        let json = r#"{
            "type": "message",
            "channel": "C123ABC456",
            "user": "U123ABC456",
            "text": "Reply in thread",
            "ts": "1355517524.000005",
            "thread_ts": "1355517523.000005"
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Message(msg) => {
                assert!(msg.is_thread_reply());
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_parse_event_callback() {
        let json = r#"{
            "type": "event_callback",
            "team_id": "T123",
            "api_app_id": "A123",
            "event": {
                "type": "message",
                "channel": "C123",
                "user": "U123",
                "text": "test",
                "ts": "123.456"
            },
            "event_id": "Ev123",
            "event_time": 1234567890
        }"#;

        let callback: EventCallback = serde_json::from_str(json).unwrap();
        assert_eq!(callback.team_id, "T123");
        assert_eq!(callback.event_id, "Ev123");
    }
}
