use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct File {
    pub id: String,
    pub created: u64,
    pub timestamp: u64,
    pub name: String,
    pub title: String,
    pub mimetype: String,
    pub filetype: String,
    pub pretty_type: String,
    pub user: String,
    pub size: u64,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub permalink: Option<String>,
    pub permalink_public: Option<String>,
    pub is_external: Option<bool>,
    pub is_public: Option<bool>,
    pub channels: Option<Vec<String>>,
    pub groups: Option<Vec<String>>,
    pub ims: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct FilesListResponse {
    pub ok: bool,
    pub files: Vec<File>,
    pub paging: Option<Paging>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileInfoResponse {
    pub ok: bool,
    pub file: File,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Paging {
    pub count: u32,
    pub total: u32,
    pub page: u32,
    pub pages: u32,
}
