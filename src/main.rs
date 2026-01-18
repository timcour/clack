mod api;
mod cache;
mod cli;
mod models;
mod output;

use anyhow::Result;
use clap::Parser;
use cli::{
    AuthType, ChatCommands, Cli, Commands, ConversationsCommands, FilesCommands, PinsCommands,
    ProfileCommands, ReactionsCommands, SearchType, UsersCommands,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client with verbose, debug_response, and refresh_cache flags
    let mut client = api::client::SlackClient::new(cli.verbose, cli.debug_response, cli.refresh_cache).await?;

    // Initialize workspace context (fetches team_id)
    client.init_workspace().await?;

    // Execute command
    match cli.command {
        Commands::Users { command } => match command {
            UsersCommands::List {
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
            UsersCommands::Info { user_id } => {
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
            UsersCommands::Profile { command } => match command {
                ProfileCommands::Get { user_id } => {
                    let profile = api::users::get_profile(&client, user_id.as_deref()).await?;

                    match cli.format.as_str() {
                        "json" => println!("{}", serde_json::to_string_pretty(&profile)?),
                        "yaml" => println!("{}", serde_yaml::to_string(&profile)?),
                        _ => {
                            let mut writer = output::color::ColorWriter::new(cli.no_color);
                            output::user_formatter::format_profile(&profile, &mut writer)?;
                        }
                    }
                }
            },
        },
        Commands::Conversations { command } => match command {
            ConversationsCommands::List { include_archived, limit } => {
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
            ConversationsCommands::Info { channel } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;
                let channel_info = api::channels::get_channel(&client, &channel_id).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&channel_info)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&channel_info)?),
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        // Reuse format_channels_list with a single-element vector
                        output::channel_formatter::format_channels_list(&vec![channel_info], &mut writer)?;
                    }
                }
            }
            ConversationsCommands::History {
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

                        // Build user lookup map - only fetch users mentioned in messages
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        for message in &messages {
                            if let Some(user_id) = &message.user {
                                if !user_map.contains_key(user_id) {
                                    // Fetch individual user (cache-first)
                                    if let Ok(user) = api::users::get_user(&client, user_id).await {
                                        user_map.insert(user.id.clone(), user);
                                    }
                                }
                            }
                        }

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
            ConversationsCommands::Replies {
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

                        // Build user lookup map - only fetch users mentioned in thread
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        for message in &messages {
                            if let Some(user_id) = &message.user {
                                if !user_map.contains_key(user_id) {
                                    // Fetch individual user (cache-first)
                                    if let Ok(user) = api::users::get_user(&client, user_id).await {
                                        user_map.insert(user.id.clone(), user);
                                    }
                                }
                            }
                        }

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
            ConversationsCommands::Members { channel, limit } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                let member_ids = api::channels::get_members(&client, &channel_id, limit).await?;

                // Fetch user details for each member
                let mut users = Vec::new();
                for user_id in &member_ids {
                    if let Ok(user) = api::users::get_user(&client, user_id).await {
                        users.push(user);
                    }
                }

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&users)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&users)?),
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::user_formatter::format_users_list(&users, &mut writer)?;
                    }
                }
            }
        },
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
        Commands::Files { command } => match command {
            FilesCommands::List { limit, user, channel } => {
                let files = api::files::list_files(&client, limit, user.as_deref(), channel.as_deref()).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&files)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&files)?),
                    _ => {
                        // Build user lookup map
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        for file in &files {
                            if !user_map.contains_key(&file.user) {
                                if let Ok(user) = api::users::get_user(&client, &file.user).await {
                                    user_map.insert(user.id.clone(), user);
                                }
                            }
                        }

                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::file_formatter::format_files_list(&files, &user_map, &mut writer)?;
                    }
                }
            }
            FilesCommands::Info { file_id } => {
                let file = api::files::get_file(&client, &file_id).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&file)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&file)?),
                    _ => {
                        // Build user lookup map for the single file uploader
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        if let Ok(user) = api::users::get_user(&client, &file.user).await {
                            user_map.insert(user.id.clone(), user);
                        }

                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::file_formatter::format_file(&file, &user_map, &mut writer)?;
                    }
                }
            }
        },
        Commands::Pins { command } => match command {
            PinsCommands::List { channel } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                let pins = api::pins::list_pins(&client, &channel_id).await?;

                match cli.format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&pins)?),
                    "yaml" => println!("{}", serde_yaml::to_string(&pins)?),
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::pin_formatter::format_pins_list(&pins, &mut writer)?;
                    }
                }
            }
            PinsCommands::Add { channel, message_ts } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::pins::add_pin(&client, &channel_id, &message_ts).await?;

                println!("✓ Message pinned successfully");
            }
            PinsCommands::Remove { channel, message_ts } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::pins::remove_pin(&client, &channel_id, &message_ts).await?;

                println!("✓ Message unpinned successfully");
            }
        },
        Commands::Reactions { command } => match command {
            ReactionsCommands::Add { channel, message_ts, emoji } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::reactions::add_reaction(&client, &channel_id, &message_ts, &emoji).await?;

                println!("✓ Reaction :{}: added successfully", emoji);
            }
            ReactionsCommands::Remove { channel, message_ts, emoji } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::reactions::remove_reaction(&client, &channel_id, &message_ts, &emoji).await?;

                println!("✓ Reaction :{}: removed successfully", emoji);
            }
        },
        Commands::Chat { command } => match command {
            ChatCommands::Post { channel, text, thread_ts } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                // Handle reading from stdin if text is "-"
                let message_text = if text == "-" {
                    use std::io::Read;
                    let mut buffer = String::new();
                    std::io::stdin().read_to_string(&mut buffer)?;
                    buffer
                } else {
                    text.clone()
                };

                let ts = api::chat::post_message(&client, &channel_id, &message_text, thread_ts.as_deref()).await?;

                println!("✓ Message posted successfully");
                println!("Message timestamp: {}", ts);
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
