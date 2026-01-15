use super::client::SlackClient;
use crate::models::user::{User, UserInfoResponse, UsersListResponse};
use anyhow::Result;

pub async fn list_users(
    client: &SlackClient,
    limit: Option<u32>,
    include_deleted: bool,
) -> Result<Vec<User>> {
    let mut query = vec![];

    if let Some(limit) = limit {
        query.push(("limit", limit.to_string()));
    }

    let response: UsersListResponse = client.get("users.list", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    let mut users = response.members;
    if !include_deleted {
        users.retain(|u| !u.deleted);
    }

    Ok(users)
}

pub async fn get_user(client: &SlackClient, user_id: &str) -> Result<User> {
    let query = vec![("user", user_id.to_string())];
    let response: UserInfoResponse = client.get("users.info", &query).await?;

    if !response.ok {
        anyhow::bail!("Slack API error: {}", response.error.unwrap_or_default());
    }

    Ok(response.user)
}
