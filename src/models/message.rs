use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    pub ts: String,
    pub user: Option<String>,
    pub text: String,
    pub thread_ts: Option<String>,
    pub reactions: Option<Vec<Reaction>>,
    // Channel can be either a string (conversations.history) or object (search)
    pub channel: Option<MessageChannel>,
    pub permalink: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum MessageChannel {
    String(String),
    Object { id: String, name: Option<String> },
}

impl MessageChannel {
    pub fn id(&self) -> &str {
        match self {
            MessageChannel::String(s) => s,
            MessageChannel::Object { id, .. } => id,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            MessageChannel::String(_) => None,
            MessageChannel::Object { name, .. } => name.as_deref(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Reaction {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Deserialize)]
pub struct MessagesResponse {
    pub ok: bool,
    pub messages: Vec<Message>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_channel_deserialize_string() {
        // Test deserialization of channel as string (conversations.history format)
        let json = r#""C0880B46V4J""#;
        let channel: MessageChannel = serde_json::from_str(json).unwrap();

        assert_eq!(channel.id(), "C0880B46V4J");
        assert_eq!(channel.name(), None);

        // Verify it's the String variant
        match channel {
            MessageChannel::String(s) => assert_eq!(s, "C0880B46V4J"),
            _ => panic!("Expected String variant"),
        }
    }

    #[test]
    fn test_message_channel_deserialize_object() {
        // Test deserialization of channel as object (search.messages format)
        let json = r#"{"id": "C123", "name": "general"}"#;
        let channel: MessageChannel = serde_json::from_str(json).unwrap();

        assert_eq!(channel.id(), "C123");
        assert_eq!(channel.name(), Some("general"));

        // Verify it's the Object variant
        match channel {
            MessageChannel::Object { id, name } => {
                assert_eq!(id, "C123");
                assert_eq!(name, Some("general".to_string()));
            }
            _ => panic!("Expected Object variant"),
        }
    }

    #[test]
    fn test_message_channel_deserialize_object_no_name() {
        // Test deserialization of channel object without name
        let json = r#"{"id": "C123"}"#;
        let channel: MessageChannel = serde_json::from_str(json).unwrap();

        assert_eq!(channel.id(), "C123");
        assert_eq!(channel.name(), None);
    }

    #[test]
    fn test_message_deserialize_with_string_channel() {
        // Test full Message deserialization with channel as string
        let json = r#"{
            "ts": "1768596285.399169",
            "user": "U04UD3CHNSJ",
            "text": "test message",
            "channel": "C0880B46V4J"
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();

        assert_eq!(message.ts, "1768596285.399169");
        assert_eq!(message.user, Some("U04UD3CHNSJ".to_string()));
        assert_eq!(message.text, "test message");
        assert!(message.channel.is_some());

        let channel = message.channel.unwrap();
        assert_eq!(channel.id(), "C0880B46V4J");
        assert_eq!(channel.name(), None);
    }

    #[test]
    fn test_message_deserialize_with_object_channel() {
        // Test full Message deserialization with channel as object
        let json = r#"{
            "ts": "1768596285.399169",
            "user": "U04UD3CHNSJ",
            "text": "test message",
            "channel": {"id": "C123", "name": "general"}
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();

        assert_eq!(message.ts, "1768596285.399169");
        assert!(message.channel.is_some());

        let channel = message.channel.unwrap();
        assert_eq!(channel.id(), "C123");
        assert_eq!(channel.name(), Some("general"));
    }

    #[test]
    fn test_message_deserialize_without_channel() {
        // Test Message deserialization without channel field
        let json = r#"{
            "ts": "1768596285.399169",
            "user": "U04UD3CHNSJ",
            "text": "test message"
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();

        assert_eq!(message.ts, "1768596285.399169");
        assert!(message.channel.is_none());
    }

    #[test]
    fn test_message_deserialize_with_reactions() {
        // Test Message deserialization with reactions
        let json = r#"{
            "ts": "1768596285.399169",
            "user": "U04UD3CHNSJ",
            "text": "test message",
            "channel": "C123",
            "reactions": [
                {"name": "thumbsup", "count": 5},
                {"name": "heart", "count": 3}
            ]
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();

        assert!(message.reactions.is_some());
        let reactions = message.reactions.unwrap();
        assert_eq!(reactions.len(), 2);
        assert_eq!(reactions[0].name, "thumbsup");
        assert_eq!(reactions[0].count, 5);
        assert_eq!(reactions[1].name, "heart");
        assert_eq!(reactions[1].count, 3);
    }

    #[test]
    fn test_messages_response_deserialize_mixed_channels() {
        // Test MessagesResponse with a mix of string and object channels
        // This simulates what could happen if we mix data from different endpoints
        let json = r#"{
            "ok": true,
            "messages": [
                {
                    "ts": "1768596285.399169",
                    "user": "U123",
                    "text": "message 1",
                    "channel": "C123"
                },
                {
                    "ts": "1768596286.399169",
                    "user": "U456",
                    "text": "message 2",
                    "channel": {"id": "C456", "name": "random"}
                }
            ]
        }"#;

        let response: MessagesResponse = serde_json::from_str(json).unwrap();

        assert!(response.ok);
        assert_eq!(response.messages.len(), 2);

        // First message has string channel
        assert_eq!(response.messages[0].channel.as_ref().unwrap().id(), "C123");
        assert_eq!(response.messages[0].channel.as_ref().unwrap().name(), None);

        // Second message has object channel
        assert_eq!(response.messages[1].channel.as_ref().unwrap().id(), "C456");
        assert_eq!(response.messages[1].channel.as_ref().unwrap().name(), Some("random"));
    }
}
