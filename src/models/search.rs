use serde::{Deserialize, Serialize};
use super::message::Message;

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchMessagesResponse {
    pub ok: bool,
    pub query: String,
    pub messages: SearchMessagesMatches,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchMessagesMatches {
    pub total: u32,
    pub matches: Vec<Message>,
    pub pagination: Option<SearchPagination>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchPagination {
    pub total_count: u32,
    pub page: u32,
    pub per_page: u32,
    pub page_count: u32,
    pub first: u32,
    pub last: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchFilesResponse {
    pub ok: bool,
    pub query: String,
    pub files: SearchFilesMatches,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchFilesMatches {
    pub total: u32,
    pub matches: Vec<FileResult>,
    pub pagination: Option<SearchPagination>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileResult {
    pub id: String,
    pub created: u64,
    pub timestamp: u64,
    pub name: String,
    pub title: String,
    pub mimetype: String,
    pub filetype: String,
    pub pretty_type: String,
    pub user: String,
    pub size: u32,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub permalink: Option<String>,
    pub permalink_public: Option<String>,
    pub channels: Option<Vec<String>>,
    pub groups: Option<Vec<String>>,
    pub ims: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchAllResponse {
    pub ok: bool,
    pub query: String,
    pub messages: SearchMessagesMatches,
    pub files: SearchFilesMatches,
    pub error: Option<String>,
}
