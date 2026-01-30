mod api;
mod cache;
mod cli;
mod models;
mod output;
mod stream;

use anyhow::Result;
use clap::Parser;
use cli::{
    AuthType, ChatCommands, Cli, Commands, ConversationsCommands, FilesCommands, PinsCommands,
    ProfileCommands, ReactionsCommands, SearchType, StreamSearchType, StreamType, UsersCommands,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client with verbose, debug_response, and refresh_cache flags
    let mut client = api::client::SlackClient::new(cli.verbose, cli.debug_response, cli.refresh_cache).await?;

    // Initialize workspace context (fetches team_id)
    client.init_workspace().await?;

    // Will accumulate all output here
    let mut final_output = String::new();

    // Execute command
    match cli.command {
        Commands::Users { command } => match command {
            UsersCommands::List {
                limit,
                include_deleted,
            } => {
                let users = api::users::list_users(&client, limit, include_deleted).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&users)?,
                    "yaml" => serde_yaml::to_string(&users)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::user_formatter::format_users_list(&users, &mut writer)?;
                        writer.into_string()?
                    }
                };
            }
            UsersCommands::Info { user_id } => {
                let user = api::users::get_user(&client, &user_id).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&user)?,
                    "yaml" => serde_yaml::to_string(&user)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::user_formatter::format_user(&user, &mut writer)?;
                        writer.into_string()?
                    }
                };
            }
            UsersCommands::Profile { command } => match command {
                ProfileCommands::Get { user_id } => {
                    let profile = api::users::get_profile(&client, user_id.as_deref()).await?;

                    final_output = match cli.format.as_str() {
                        "json" => serde_json::to_string_pretty(&profile)?,
                        "yaml" => serde_yaml::to_string(&profile)?,
                        _ => {
                            let mut writer = output::color::ColorWriter::new(cli.no_color);
                            output::user_formatter::format_profile(&profile, &mut writer)?;
                            writer.into_string()?
                        }
                    }
                }
            },
        },
        Commands::Conversations { command } => match command {
            ConversationsCommands::List { include_archived, limit } => {
                let channels = api::channels::list_channels(&client, include_archived, limit).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&channels)?,
                    "yaml" => serde_yaml::to_string(&channels)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::channel_formatter::format_channels_list(&channels, &mut writer)?;
                        writer.into_string()?
                    }
                }
            }
            ConversationsCommands::Info { channel } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;
                let channel_info = api::channels::get_channel(&client, &channel_id).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&channel_info)?,
                    "yaml" => serde_yaml::to_string(&channel_info)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        // Reuse format_channels_list with a single-element vector
                        output::channel_formatter::format_channels_list(&vec![channel_info], &mut writer)?;
                        writer.into_string()?
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

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&messages)?,
                    "yaml" => serde_yaml::to_string(&messages)?,
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

                        // Build thread metadata map
                        let mut thread_info: std::collections::HashMap<String, (usize, Vec<String>)> =
                            std::collections::HashMap::new();

                        // Identify unique threads
                        let thread_timestamps: std::collections::HashSet<&String> = messages
                            .iter()
                            .filter_map(|m| m.thread_ts.as_ref())
                            .collect();

                        // Fetch metadata for each thread
                        for thread_ts in thread_timestamps {
                            if let Ok(thread_messages) = api::messages::get_thread(&client, &channel_id, thread_ts).await {
                                let (reply_count, participant_ids) = api::messages::get_thread_metadata(&thread_messages);
                                thread_info.insert(thread_ts.clone(), (reply_count, participant_ids.clone()));

                                // Also add participants to user_map
                                for user_id in &participant_ids {
                                    if !user_map.contains_key(user_id) {
                                        if let Ok(user) = api::users::get_user(&client, user_id).await {
                                            user_map.insert(user.id.clone(), user);
                                        }
                                    }
                                }
                            }
                        }

                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::message_formatter::format_messages_with_thread_info(
                            &messages,
                            &channel_info,
                            &user_map,
                            &thread_info,
                            &mut writer,
                        )?;
                        writer.into_string()?
                    }
                };
            }
            ConversationsCommands::Replies {
                channel,
                message_ts,
            } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                let messages = api::messages::get_thread(&client, &channel_id, &message_ts).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&messages)?,
                    "yaml" => serde_yaml::to_string(&messages)?,
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
                        writer.into_string()?
                    }
                };
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

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&users)?,
                    "yaml" => serde_yaml::to_string(&users)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::user_formatter::format_users_list(&users, &mut writer)?;
                        writer.into_string()?
                    }
                }
            }
        },
        Commands::Search { search_type } => match search_type {
            SearchType::Messages {
                query,
                from,
                to,
                channel,
                has,
                after,
                before,
                during,
                page,
                limit,
            } => {
                // Validate --during if provided
                if let Some(ref d) = during {
                    api::search::validate_during(d)?;
                }

                // Resolve user identifiers to IDs (format as <@USERID>)
                let resolved_from = if let Some(ref user) = from {
                    Some(format!("<@{}>", api::users::resolve_user_to_id(&client, user).await?))
                } else {
                    None
                };

                let resolved_to = if let Some(ref user) = to {
                    Some(format!("<@{}>", api::users::resolve_user_to_id(&client, user).await?))
                } else {
                    None
                };

                // Resolve channel identifier to ID (format as <#CHANNELID>)
                let resolved_channel = if let Some(ref ch) = channel {
                    Some(format!("<#{}>", api::channels::resolve_channel_id(&client, ch).await?))
                } else {
                    None
                };

                // Build search query with resolved filters
                let search_query = api::search::build_search_query_full(
                    &query,
                    resolved_from.as_deref(),
                    resolved_to.as_deref(),
                    resolved_channel.as_deref(),
                    has.as_deref(),
                    after.as_deref(),
                    before.as_deref(),
                    during.as_deref(),
                );

                let response = api::search::search_messages(&client, &search_query, Some(limit), Some(page)).await?;

                // Cache search result messages for offline access
                api::search::cache_search_messages(&client, &response.messages.matches).await;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&response)?,
                    "yaml" => final_output = serde_yaml::to_string(&response)?,
                    _ => {
                        // Build user lookup map from search results
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        for message in &response.messages.matches {
                            if let Some(user_id) = &message.user {
                                if !user_map.contains_key(user_id) {
                                    if let Ok(user) = api::users::get_user(&client, user_id).await {
                                        user_map.insert(user.id.clone(), user);
                                    }
                                }
                            }
                        }

                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::search_formatter::format_search_messages(&response, &user_map, &mut writer)?;
                        final_output = writer.into_string()?;
                    }
                }
            }
            SearchType::Files {
                query,
                from,
                channel,
                has,
                after,
                before,
                during,
                page,
                limit,
            } => {
                // Validate --during if provided
                if let Some(ref d) = during {
                    api::search::validate_during(d)?;
                }

                // Resolve user identifier to ID (format as <@USERID>)
                let resolved_from = if let Some(ref user) = from {
                    Some(format!("<@{}>", api::users::resolve_user_to_id(&client, user).await?))
                } else {
                    None
                };

                // Resolve channel identifier to ID (format as <#CHANNELID>)
                let resolved_channel = if let Some(ref ch) = channel {
                    Some(format!("<#{}>", api::channels::resolve_channel_id(&client, ch).await?))
                } else {
                    None
                };

                // Build search query with resolved filters
                let search_query = api::search::build_search_query_full(
                    &query,
                    resolved_from.as_deref(),
                    None, // files don't have 'to'
                    resolved_channel.as_deref(),
                    has.as_deref(),
                    after.as_deref(),
                    before.as_deref(),
                    during.as_deref(),
                );

                let response = api::search::search_files(&client, &search_query, Some(limit), Some(page)).await?;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&response)?,
                    "yaml" => final_output = serde_yaml::to_string(&response)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::search_formatter::format_search_files(&response, &mut writer)?;
                        final_output = writer.into_string()?;
                    }
                }
            }
            SearchType::All {
                query,
                channel,
                page,
                limit,
            } => {
                // Resolve channel identifier to ID (format as <#CHANNELID>)
                let resolved_channel = if let Some(ref ch) = channel {
                    Some(format!("<#{}>", api::channels::resolve_channel_id(&client, ch).await?))
                } else {
                    None
                };

                // Build search query with resolved filters
                let search_query = api::search::build_search_query(
                    &query,
                    None,
                    resolved_channel.as_deref(),
                    None,
                    None,
                );

                let response = api::search::search_all(&client, &search_query, Some(limit), Some(page)).await?;

                // Cache search result messages for offline access
                api::search::cache_search_messages(&client, &response.messages.matches).await;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&response)?,
                    "yaml" => final_output = serde_yaml::to_string(&response)?,
                    _ => {
                        // Build user lookup map from search results
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        for message in &response.messages.matches {
                            if let Some(user_id) = &message.user {
                                if !user_map.contains_key(user_id) {
                                    if let Ok(user) = api::users::get_user(&client, user_id).await {
                                        user_map.insert(user.id.clone(), user);
                                    }
                                }
                            }
                        }

                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::search_formatter::format_search_all(&response, &user_map, &mut writer)?;
                        final_output = writer.into_string()?;
                    }
                }
            }
            SearchType::Channels {
                query,
                include_archived,
            } => {
                let channels = api::channels::search_channels(&client, &query, include_archived).await?;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&channels)?,
                    "yaml" => final_output = serde_yaml::to_string(&channels)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::search_formatter::format_channel_search_results(&query, &channels, &mut writer)?;
                        final_output = writer.into_string()?;
                    }
                }
            }
        },
        Commands::Files { command } => match command {
            FilesCommands::List { limit, user, channel } => {
                let files = api::files::list_files(&client, limit, user.as_deref(), channel.as_deref()).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&files)?,
                    "yaml" => serde_yaml::to_string(&files)?,
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
                        writer.into_string()?
                    }
                }
            }
            FilesCommands::Info { file_id } => {
                let file = api::files::get_file(&client, &file_id).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&file)?,
                    "yaml" => serde_yaml::to_string(&file)?,
                    _ => {
                        // Build user lookup map for the single file uploader
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        if let Ok(user) = api::users::get_user(&client, &file.user).await {
                            user_map.insert(user.id.clone(), user);
                        }

                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::file_formatter::format_file(&file, &user_map, &mut writer)?;
                        writer.into_string()?
                    }
                }
            }
        },
        Commands::Pins { command } => match command {
            PinsCommands::List { channel } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                let pins = api::pins::list_pins(&client, &channel_id).await?;

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&pins)?,
                    "yaml" => serde_yaml::to_string(&pins)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::pin_formatter::format_pins_list(&pins, &mut writer)?;
                        writer.into_string()?
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

                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&auth_response)?,
                    "yaml" => serde_yaml::to_string(&auth_response)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::auth_formatter::format_auth_test(&auth_response, &mut writer)?;
                        writer.into_string()?
                    }
                }
            }
        },
        Commands::Stream {
            interval,
            stream_type,
        } => {
            // For streaming, use human-compact if default "human" format is specified
            let effective_format = if cli.format == "human" {
                "human-compact"
            } else {
                &cli.format
            };

            match stream_type {
                StreamType::Search { search_type } => match search_type {
                    StreamSearchType::Messages {
                        query,
                        from,
                        to,
                        channel,
                        has,
                    } => {
                        // Resolve user identifiers to IDs
                        let resolved_from = if let Some(ref user) = from {
                            Some(format!(
                                "<@{}>",
                                api::users::resolve_user_to_id(&client, user).await?
                            ))
                        } else {
                            None
                        };

                        let resolved_to = if let Some(ref user) = to {
                            Some(format!(
                                "<@{}>",
                                api::users::resolve_user_to_id(&client, user).await?
                            ))
                        } else {
                            None
                        };

                        let resolved_channel = if let Some(ref ch) = channel {
                            Some(format!(
                                "<#{}>",
                                api::channels::resolve_channel_id(&client, ch).await?
                            ))
                        } else {
                            None
                        };

                        // Build search query with resolved filters
                        let search_query = api::search::build_search_query_full(
                            &query,
                            resolved_from.as_deref(),
                            resolved_to.as_deref(),
                            resolved_channel.as_deref(),
                            has.as_deref(),
                            None,
                            None,
                            None,
                        );

                        // Run the streaming loop
                        stream::search::stream_search_messages(
                            &client,
                            &search_query,
                            interval,
                            effective_format,
                            cli.no_color,
                        )
                        .await?;
                    }
                },
            }
        }
    }

    // Output with pager if enabled
    if !final_output.is_empty() {
        let mut output_dest = output::pager::OutputDestination::new(cli.no_pager)?;
        output_dest.write_str(&final_output)?;
        output_dest.finish()?;
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
