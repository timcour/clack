use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub ts: String,
    pub user: Option<String>,
    pub text: String,
    pub thread_ts: Option<String>,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Debug, Deserialize, Serialize)]
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
