use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub real_name: Option<String>,
    pub profile: UserProfile,
    pub deleted: bool,
    pub is_bot: bool,
    pub is_admin: Option<bool>,
    pub is_owner: Option<bool>,
    pub tz: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserProfile {
    pub email: Option<String>,
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
    pub display_name: Option<String>,
    pub image_72: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UsersListResponse {
    pub ok: bool,
    pub members: Vec<User>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserInfoResponse {
    pub ok: bool,
    pub user: User,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserProfileResponse {
    pub ok: bool,
    pub profile: UserProfile,
    pub error: Option<String>,
}
