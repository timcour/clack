use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub is_channel: Option<bool>,
    pub is_group: Option<bool>,
    pub is_im: Option<bool>,
    pub is_mpim: Option<bool>,
    pub is_private: Option<bool>,
    pub is_archived: Option<bool>,
    pub topic: Option<ChannelTopic>,
    pub purpose: Option<ChannelPurpose>,
    pub num_members: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChannelTopic {
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChannelPurpose {
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct ChannelInfoResponse {
    pub ok: bool,
    pub channel: Channel,
    pub error: Option<String>,
}
