use serde::{Deserialize, Serialize};
use super::message::Message;

#[derive(Debug, Deserialize, Serialize)]
pub struct PinItem {
    pub channel: String,
    pub created: u64,
    pub created_by: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    pub message: Option<Message>,
}

#[derive(Debug, Deserialize)]
pub struct PinsListResponse {
    pub ok: bool,
    pub items: Vec<PinItem>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PinResponse {
    pub ok: bool,
    pub error: Option<String>,
}
