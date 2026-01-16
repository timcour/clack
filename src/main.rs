mod api;
mod cache;
mod cli;
mod models;
mod output;

use anyhow::Result;
use clap::Parser;
use cli::{AuthType, Cli, Commands, SearchType};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client with verbose flag
    let mut client = api::client::SlackClient::new_verbose(cli.verbose).await?;

    // Initialize workspace context (fetches team_id)
    client.init_workspace().await?;

    // Execute command
    match cli.command {
        Commands::Users {
            limit,
            include_deleted,
        } => {
            let users = api::users::list_users(&client, limit, include_deleted).await?;

            match cli.format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&users)?),
                "yaml" => println!("{}", serde_yaml::to_string(&users)?),
                _ => {
                    let mut writer = output::color::ColorWriter::new(cli.no_color);
                    output::user_formatter::format_users_list(&users, &mut writer)?;
                }
            }
        }
        Commands::User { user_id } => {
            let user = api::users::get_user(&client, &user_id).await?;

            match cli.format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&user)?),
                "yaml" => println!("{}", serde_yaml::to_string(&user)?),
                _ => {
                    let mut writer = output::color::ColorWriter::new(cli.no_color);
                    output::user_formatter::format_user(&user, &mut writer)?;
                }
            }
        }
        Commands::Messages {
            channel,
            limit,
            latest,
            oldest,
        } => {
            // Resolve channel name to ID if needed
            let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

            let messages =
                api::messages::list_messages(&client, &channel_id, limit, latest, oldest).await?;

            match cli.format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&messages)?),
                "yaml" => println!("{}", serde_yaml::to_string(&messages)?),
                _ => {
                    // Fetch channel info for metadata
                    let channel_info = api::channels::get_channel(&client, &channel_id).await?;

                    // Fetch all users and build a lookup map
                    let all_users = api::users::list_users(&client, 200, false).await?;
                    let user_map: std::collections::HashMap<String, models::user::User> =
                        all_users.into_iter().map(|u| (u.id.clone(), u)).collect();

                    let mut writer = output::color::ColorWriter::new(cli.no_color);
                    output::message_formatter::format_messages(
                        &messages,
                        &channel_info,
                        &user_map,
                        &mut writer,
                    )?;
                }
            }
        }
        Commands::Thread {
            channel,
            message_ts,
        } => {
            // Resolve channel name to ID if needed
            let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

            let messages = api::messages::get_thread(&client, &channel_id, &message_ts).await?;

            match cli.format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&messages)?),
                "yaml" => println!("{}", serde_yaml::to_string(&messages)?),
                _ => {
                    // Fetch channel info for metadata
                    let channel_info = api::channels::get_channel(&client, &channel_id).await?;

                    // Fetch all users and build a lookup map
                    let all_users = api::users::list_users(&client, 200, false).await?;
                    let user_map: std::collections::HashMap<String, models::user::User> =
                        all_users.into_iter().map(|u| (u.id.clone(), u)).collect();

                    let mut writer = output::color::ColorWriter::new(cli.no_color);
                    output::thread_formatter::format_thread(
                        &messages,
                        &channel_info,
                        &user_map,
                        &mut writer,
                    )?;
                }
            }
        }
        Commands::Channels { include_archived, limit } => {
            let channels = api::channels::list_channels(&client, include_archived, limit).await?;

            match cli.format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&channels)?),
                "yaml" => println!("{}", serde_yaml::to_string(&channels)?),
                _ => {
                    let mut writer = output::color::ColorWriter::new(cli.no_color);
                    output::channel_formatter::format_channels_list(&channels, &mut writer)?;
                }
            }
        }
        Commands::Search { search_type } => match search_type {
            SearchType::Messages {
                query,
                from,
                channel,
                after,
                before,
                limit,
            } => {
                // Build search query with filters
                let search_query = api::search::build_search_query(
                    &query,
                    from.as_deref(),
                    channel.as_deref(),
                    after.as_deref(),
                    before.as_deref(),
                );

                let response = api::search::search_messages(&client, &search_query, Some(limit)).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&response)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&response)?),
                    _ => output::search_formatter::format_search_messages(&response, cli.no_color)?,
                }
            }
            SearchType::Files {
                query,
                from,
                channel,
                after,
                before,
                limit,
            } => {
                // Build search query with filters
                let search_query = api::search::build_search_query(
                    &query,
                    from.as_deref(),
                    channel.as_deref(),
                    after.as_deref(),
                    before.as_deref(),
                );

                let response = api::search::search_files(&client, &search_query, Some(limit)).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&response)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&response)?),
                    _ => output::search_formatter::format_search_files(&response, cli.no_color)?,
                }
            }
            SearchType::All {
                query,
                channel,
                limit,
            } => {
                // Build search query with filters
                let search_query = api::search::build_search_query(
                    &query,
                    None,
                    channel.as_deref(),
                    None,
                    None,
                );

                let response = api::search::search_all(&client, &search_query, Some(limit)).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&response)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&response)?),
                    _ => output::search_formatter::format_search_all(&response, cli.no_color)?,
                }
            }
            SearchType::Channels {
                query,
                include_archived,
            } => {
                let channels = api::channels::search_channels(&client, &query, include_archived).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&channels)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&channels)?),
                    _ => output::search_formatter::format_channel_search_results(&query, &channels, cli.no_color)?,
                }
            }
        },
        Commands::Auth { auth_type } => match auth_type {
            AuthType::Test => {
                let auth_response = api::auth::test_auth(&client).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&auth_response)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&auth_response)?),
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::auth_formatter::format_auth_test(&auth_response, &mut writer)?;
                    }
                }
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hello_world() {
        // Simple test that always passes
        assert_eq!(2 + 2, 4);
    }
}
