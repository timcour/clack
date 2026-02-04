mod api;
mod cache;
mod cli;
mod models;
mod output;
mod socket;
mod stream;
mod url_parser;

use anyhow::Result;
use clap::Parser;
use cli::{
    AuthType, CacheCommands, ChatCommands, Cli, Commands, ConversationsCommands, EventsCommands,
    FilesCommands, PinsCommands, ProfileCommands, ReactionsCommands, SearchType, StreamSearchType,
    StreamType, UsersCommands,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Check if first arg is a Slack URL and insert "open" subcommand if so
    let args: Vec<String> = std::env::args().collect();
    let cli = if args.len() >= 2
        && args[1].starts_with("https://")
        && args[1].contains(".slack.com/")
    {
        // Insert "open" subcommand before the URL
        let mut new_args = vec![args[0].clone(), "open".to_string()];
        new_args.extend(args[1..].iter().cloned());
        Cli::parse_from(new_args)
    } else {
        Cli::parse()
    };

    // Create API client with verbose, debug_response, and refresh_cache flags
    let mut client =
        api::client::SlackClient::new(cli.verbose, cli.debug_response, cli.refresh_cache).await?;

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

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&users)?,
                    "yaml" => final_output = serde_yaml::to_string(&users)?,
                    _ => {
                        // Progressive output: print directly to stdout
                        use std::io::Write;
                        let mut writer = output::color::ColorWriter::new(cli.no_color);

                        // Print header
                        writer.print_header(&format!("Users ({})", users.len()))?;
                        writer.print_separator()?;
                        print!("{}", writer.into_string()?);
                        std::io::stdout().flush()?;

                        // Print each user progressively
                        for (i, user) in users.iter().enumerate() {
                            let mut writer = output::color::ColorWriter::new(cli.no_color);
                            writer.write("@")?;
                            writer.print_bold(&user.name)?;
                            writer.write(" ")?;
                            writer.print_colored(
                                &format!("({})", user.id),
                                termcolor::Color::Yellow,
                            )?;
                            if let Some(real_name) = &user.real_name {
                                writer.write(&format!(" ({})", real_name))?;
                            }
                            if let Some(emoji) = &user.profile.status_emoji {
                                writer.write(&format!(" {}", emoji))?;
                            }
                            writer.writeln()?;
                            if let Some(email) = &user.profile.email {
                                writer.write("  ")?;
                                writer.print_colored("✉", termcolor::Color::Blue)?;
                                writer.write(&format!(" {}", email))?;
                                writer.writeln()?;
                            }
                            if i < users.len() - 1 {
                                writer.writeln()?;
                            }
                            print!("{}", writer.into_string()?);
                            std::io::stdout().flush()?;
                        }
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
            ConversationsCommands::List {
                include_archived,
                limit,
            } => {
                let channels =
                    api::channels::list_channels(&client, include_archived, limit).await?;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&channels)?,
                    "yaml" => final_output = serde_yaml::to_string(&channels)?,
                    _ => {
                        // Progressive output: print directly to stdout
                        use std::io::Write;

                        // Print header
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        writer.print_header(&format!("Channels ({})", channels.len()))?;
                        writer.print_separator()?;
                        print!("{}", writer.into_string()?);
                        std::io::stdout().flush()?;

                        // Sort channels by name
                        let mut sorted_channels = channels.to_vec();
                        sorted_channels.sort_by(|a, b| a.name.cmp(&b.name));

                        // Print each channel progressively
                        for (i, channel) in sorted_channels.iter().enumerate() {
                            let mut writer = output::color::ColorWriter::new(cli.no_color);
                            writer.print_colored(
                                &format!("#{}", channel.name),
                                termcolor::Color::Cyan,
                            )?;
                            writer.write(" ")?;
                            writer.print_colored(
                                &format!("({})", channel.id),
                                termcolor::Color::Yellow,
                            )?;
                            if channel.is_private == Some(true) {
                                writer.write(" ")?;
                                writer.print_colored("🔒 Private", termcolor::Color::Blue)?;
                            }
                            if channel.is_archived == Some(true) {
                                writer.write(" ")?;
                                writer.print_colored("📦 Archived", termcolor::Color::White)?;
                            }
                            writer.writeln()?;
                            if let Some(topic) = &channel.topic {
                                if !topic.value.is_empty() {
                                    writer.write("  ")?;
                                    writer.print_colored("Topic: ", termcolor::Color::Blue)?;
                                    writer.write(&topic.value)?;
                                    writer.writeln()?;
                                }
                            }
                            if let Some(num_members) = channel.num_members {
                                writer.write("  ")?;
                                writer.print_colored(
                                    &format!("{} members", num_members),
                                    termcolor::Color::Green,
                                )?;
                                writer.writeln()?;
                            }
                            if i < sorted_channels.len() - 1 {
                                writer.writeln()?;
                            }
                            print!("{}", writer.into_string()?);
                            std::io::stdout().flush()?;
                        }
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
                        output::channel_formatter::format_channels_list(
                            &vec![channel_info],
                            &mut writer,
                        )?;
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
                    api::messages::list_messages(&client, &channel_id, limit, latest, oldest)
                        .await?;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&messages)?,
                    "yaml" => final_output = serde_yaml::to_string(&messages)?,
                    _ => {
                        // Progressive output: print directly to stdout
                        use std::io::Write;

                        // Fetch channel info for metadata
                        let channel_info = api::channels::get_channel(&client, &channel_id).await?;

                        // Print channel header immediately
                        let mut header_writer = output::color::ColorWriter::new(cli.no_color);
                        output::message_formatter::format_channel_header(
                            &channel_info,
                            &mut header_writer,
                        )?;
                        print!("{}", header_writer.into_string()?);
                        println!("Messages ({})", messages.len());
                        println!("{}", "-".repeat(40));
                        std::io::stdout().flush()?;

                        // Build user lookup map progressively
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        // Build thread metadata map progressively
                        let mut thread_info: std::collections::HashMap<
                            String,
                            (usize, Vec<String>),
                        > = std::collections::HashMap::new();

                        // Process and print each message progressively
                        for (i, msg) in messages.iter().enumerate() {
                            // Fetch user if not already cached
                            if let Some(user_id) = &msg.user {
                                if !user_map.contains_key(user_id) {
                                    if let Ok(user) = api::users::get_user(&client, user_id).await {
                                        user_map.insert(user.id.clone(), user);
                                    }
                                }
                            }

                            // Fetch thread info if this message is part of a thread
                            if let Some(thread_ts) = &msg.thread_ts {
                                if !thread_info.contains_key(thread_ts) {
                                    if let Ok(thread_messages) =
                                        api::messages::get_thread(&client, &channel_id, thread_ts)
                                            .await
                                    {
                                        let (reply_count, participant_ids) =
                                            api::messages::get_thread_metadata(&thread_messages);

                                        // Fetch participants
                                        for user_id in &participant_ids {
                                            if !user_map.contains_key(user_id) {
                                                if let Ok(user) =
                                                    api::users::get_user(&client, user_id).await
                                                {
                                                    user_map.insert(user.id.clone(), user);
                                                }
                                            }
                                        }

                                        thread_info.insert(
                                            thread_ts.clone(),
                                            (reply_count, participant_ids),
                                        );
                                    }
                                }
                            }

                            // Format and print this message immediately
                            let mut msg_writer = output::color::ColorWriter::new(cli.no_color);
                            output::message_formatter::format_single_message(
                                msg,
                                &channel_info.name,
                                &channel_info.id,
                                &user_map,
                                &thread_info,
                                &mut msg_writer,
                            )?;
                            print!("{}", msg_writer.into_string()?);

                            // Add spacing between messages
                            if i < messages.len() - 1 {
                                println!();
                            }
                            std::io::stdout().flush()?;
                        }

                        // Skip pager for progressive output
                        return Ok(());
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

                // Resolve channel identifier to ID (format as <#CHANNELID>)
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
                    after.as_deref(),
                    before.as_deref(),
                    during.as_deref(),
                );

                let response =
                    api::search::search_messages(&client, &search_query, Some(limit), Some(page))
                        .await?;

                // Cache search result messages for offline access
                api::search::cache_search_messages(&client, &response.messages.matches).await;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&response)?,
                    "yaml" => final_output = serde_yaml::to_string(&response)?,
                    _ => {
                        // Progressive output: print directly to stdout
                        use std::io::Write;

                        // Print header immediately
                        let mut header_writer = output::color::ColorWriter::new(cli.no_color);
                        output::search_formatter::format_search_messages_header(
                            &response.query,
                            response.messages.total,
                            &mut header_writer,
                        )?;
                        print!("{}", header_writer.into_string()?);
                        std::io::stdout().flush()?;

                        // Build user lookup map progressively
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        // Process and print each message progressively
                        for (i, msg) in response.messages.matches.iter().enumerate() {
                            // Fetch user if not already cached
                            if let Some(user_id) = &msg.user {
                                if !user_map.contains_key(user_id) {
                                    if let Ok(user) = api::users::get_user(&client, user_id).await {
                                        user_map.insert(user.id.clone(), user);
                                    }
                                }
                            }

                            // Format and print this message immediately
                            let mut msg_writer = output::color::ColorWriter::new(cli.no_color);
                            output::search_formatter::format_search_message(
                                msg,
                                &user_map,
                                &mut msg_writer,
                            )?;
                            print!("{}", msg_writer.into_string()?);

                            // Add spacing between messages
                            if i < response.messages.matches.len() - 1 {
                                println!();
                            }
                            std::io::stdout().flush()?;
                        }

                        // Print pagination if available
                        if let Some(ref pagination) = response.messages.pagination {
                            let mut pag_writer = output::color::ColorWriter::new(cli.no_color);
                            output::search_formatter::format_search_pagination(
                                pagination,
                                &mut pag_writer,
                            )?;
                            print!("{}", pag_writer.into_string()?);
                            std::io::stdout().flush()?;
                        }

                        // Skip pager for progressive output
                        return Ok(());
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
                    Some(format!(
                        "<@{}>",
                        api::users::resolve_user_to_id(&client, user).await?
                    ))
                } else {
                    None
                };

                // Resolve channel identifier to ID (format as <#CHANNELID>)
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
                    None, // files don't have 'to'
                    resolved_channel.as_deref(),
                    has.as_deref(),
                    after.as_deref(),
                    before.as_deref(),
                    during.as_deref(),
                );

                let response =
                    api::search::search_files(&client, &search_query, Some(limit), Some(page))
                        .await?;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&response)?,
                    "yaml" => final_output = serde_yaml::to_string(&response)?,
                    _ => {
                        // Progressive output: print directly to stdout
                        use std::io::Write;

                        // Print header immediately
                        let mut header_writer = output::color::ColorWriter::new(cli.no_color);
                        output::search_formatter::format_search_files_header(
                            &response.query,
                            response.files.total,
                            &mut header_writer,
                        )?;
                        print!("{}", header_writer.into_string()?);
                        std::io::stdout().flush()?;

                        // Process and print each file progressively
                        for (i, file) in response.files.matches.iter().enumerate() {
                            let mut file_writer = output::color::ColorWriter::new(cli.no_color);
                            output::search_formatter::format_single_file(file, &mut file_writer)?;
                            print!("{}", file_writer.into_string()?);

                            // Add spacing between files
                            if i < response.files.matches.len() - 1 {
                                println!();
                            }
                            std::io::stdout().flush()?;
                        }

                        // Print pagination if available
                        if let Some(ref pagination) = response.files.pagination {
                            let mut pag_writer = output::color::ColorWriter::new(cli.no_color);
                            output::search_formatter::format_search_pagination(
                                pagination,
                                &mut pag_writer,
                            )?;
                            print!("{}", pag_writer.into_string()?);
                            std::io::stdout().flush()?;
                        }

                        // Skip pager for progressive output
                        return Ok(());
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
                    Some(format!(
                        "<#{}>",
                        api::channels::resolve_channel_id(&client, ch).await?
                    ))
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

                let response =
                    api::search::search_all(&client, &search_query, Some(limit), Some(page))
                        .await?;

                // Cache search result messages for offline access
                api::search::cache_search_messages(&client, &response.messages.matches).await;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&response)?,
                    "yaml" => final_output = serde_yaml::to_string(&response)?,
                    _ => {
                        // Progressive output: print directly to stdout
                        use std::io::Write;

                        // Print header immediately
                        let mut header_writer = output::color::ColorWriter::new(cli.no_color);
                        header_writer.print_header(&format!("Search results for '{}'", response.query))?;
                        header_writer.print_separator()?;
                        print!("{}", header_writer.into_string()?);
                        std::io::stdout().flush()?;

                        // Build user lookup map progressively
                        let mut user_map: std::collections::HashMap<String, models::user::User> =
                            std::collections::HashMap::new();

                        // Messages section
                        if response.messages.total > 0 {
                            println!("{} Message{}:", response.messages.total,
                                if response.messages.total == 1 { "" } else { "s" });
                            println!("{}", "-".repeat(40));
                            std::io::stdout().flush()?;

                            for (i, msg) in response.messages.matches.iter().enumerate() {
                                // Fetch user if not already cached
                                if let Some(user_id) = &msg.user {
                                    if !user_map.contains_key(user_id) {
                                        if let Ok(user) = api::users::get_user(&client, user_id).await {
                                            user_map.insert(user.id.clone(), user);
                                        }
                                    }
                                }

                                // Format and print this message immediately
                                let mut msg_writer = output::color::ColorWriter::new(cli.no_color);
                                output::search_formatter::format_search_message(
                                    msg,
                                    &user_map,
                                    &mut msg_writer,
                                )?;
                                print!("{}", msg_writer.into_string()?);

                                if i < response.messages.matches.len() - 1 {
                                    println!();
                                }
                                std::io::stdout().flush()?;
                            }

                            // Print pagination if available
                            if let Some(ref pagination) = response.messages.pagination {
                                let mut pag_writer = output::color::ColorWriter::new(cli.no_color);
                                output::search_formatter::format_search_pagination(
                                    pagination,
                                    &mut pag_writer,
                                )?;
                                print!("{}", pag_writer.into_string()?);
                                std::io::stdout().flush()?;
                            }
                        }

                        // Files section
                        if response.files.total > 0 {
                            if response.messages.total > 0 {
                                println!();
                                println!("{}", "-".repeat(40));
                            }
                            println!("{} File{}:", response.files.total,
                                if response.files.total == 1 { "" } else { "s" });
                            println!("{}", "-".repeat(40));
                            std::io::stdout().flush()?;

                            for (i, file) in response.files.matches.iter().enumerate() {
                                let mut file_writer = output::color::ColorWriter::new(cli.no_color);
                                output::search_formatter::format_single_file(file, &mut file_writer)?;
                                print!("{}", file_writer.into_string()?);

                                if i < response.files.matches.len() - 1 {
                                    println!();
                                }
                                std::io::stdout().flush()?;
                            }

                            // Print pagination if available
                            if let Some(ref pagination) = response.files.pagination {
                                let mut pag_writer = output::color::ColorWriter::new(cli.no_color);
                                output::search_formatter::format_search_pagination(
                                    pagination,
                                    &mut pag_writer,
                                )?;
                                print!("{}", pag_writer.into_string()?);
                                std::io::stdout().flush()?;
                            }
                        }

                        if response.messages.total == 0 && response.files.total == 0 {
                            println!();
                            println!("No results found.");
                        }

                        // Skip pager for progressive output
                        return Ok(());
                    }
                }
            }
            SearchType::Channels {
                query,
                include_archived,
            } => {
                let channels =
                    api::channels::search_channels(&client, &query, include_archived).await?;

                match cli.format.as_str() {
                    "json" => final_output = serde_json::to_string_pretty(&channels)?,
                    "yaml" => final_output = serde_yaml::to_string(&channels)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::search_formatter::format_channel_search_results(
                            &query,
                            &channels,
                            &mut writer,
                        )?;
                        final_output = writer.into_string()?;
                    }
                }
            }
        },
        Commands::Files { command } => match command {
            FilesCommands::List {
                limit,
                user,
                channel,
            } => {
                let files =
                    api::files::list_files(&client, limit, user.as_deref(), channel.as_deref())
                        .await?;

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
            PinsCommands::Add {
                channel,
                message_ts,
            } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::pins::add_pin(&client, &channel_id, &message_ts).await?;

                println!("✓ Message pinned successfully");
            }
            PinsCommands::Remove {
                channel,
                message_ts,
            } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::pins::remove_pin(&client, &channel_id, &message_ts).await?;

                println!("✓ Message unpinned successfully");
            }
        },
        Commands::Reactions { command } => match command {
            ReactionsCommands::Add {
                channel,
                message_ts,
                emoji,
            } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::reactions::add_reaction(&client, &channel_id, &message_ts, &emoji).await?;

                println!("✓ Reaction :{}: added successfully", emoji);
            }
            ReactionsCommands::Remove {
                channel,
                message_ts,
                emoji,
            } => {
                // Resolve channel name to ID if needed
                let channel_id = api::channels::resolve_channel_id(&client, &channel).await?;

                api::reactions::remove_reaction(&client, &channel_id, &message_ts, &emoji).await?;

                println!("✓ Reaction :{}: removed successfully", emoji);
            }
        },
        Commands::Chat { command } => match command {
            ChatCommands::Post {
                channel,
                text,
                thread_ts,
            } => {
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

                let ts = api::chat::post_message(
                    &client,
                    &channel_id,
                    &message_text,
                    thread_ts.as_deref(),
                )
                .await?;

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
        Commands::Events { command } => {
            match command {
                EventsCommands::Listen { channel, from } => {
                    // Get app token for Socket Mode
                    let app_token = api::client::SlackClient::get_app_token()?;

                    // Resolve channel names to IDs if needed
                    let mut channel_ids = Vec::new();
                    for ch in &channel {
                        let channel_id = api::channels::resolve_channel_id(&client, ch).await?;
                        channel_ids.push(channel_id);
                    }

                    // Resolve user names to IDs if needed
                    let mut user_ids = Vec::new();
                    for user in &from {
                        let user_id = api::users::resolve_user_to_id(&client, user).await?;
                        user_ids.push(user_id);
                    }

                    let filter = socket::events::EventFilter {
                        channels: channel_ids,
                        users: user_ids,
                    };

                    socket::events::listen_events(
                        &client,
                        &app_token,
                        filter,
                        &cli.format,
                        cli.no_color,
                        cli.verbose,
                    )
                    .await?;
                }
                EventsCommands::List {
                    channel,
                    from,
                    since,
                    limit,
                } => {
                    use anyhow::Context;

                    // Resolve channel name if provided
                    let channel_id = if let Some(ch) = &channel {
                        Some(api::channels::resolve_channel_id(&client, ch).await?)
                    } else {
                        None
                    };

                    // Resolve user name if provided
                    let user_id = if let Some(u) = &from {
                        Some(api::users::resolve_user_to_id(&client, u).await?)
                    } else {
                        None
                    };

                    // Parse since timestamp
                    let since_ts = if let Some(s) = &since {
                        // Try parsing as Unix timestamp first
                        if let Ok(ts) = s.parse::<i64>() {
                            Some(ts)
                        } else {
                            // Try parsing as date
                            use chrono::NaiveDate;
                            if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                                Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
                            } else {
                                anyhow::bail!("Invalid --since value. Use Unix timestamp or YYYY-MM-DD format.");
                            }
                        }
                    } else {
                        None
                    };

                    // Get cached events
                    let cached_events = if let Some(pool) = client.cache_pool() {
                        let mut conn = cache::db::get_connection(pool).await?;
                        let workspace_id = client.workspace_id().context("No workspace ID")?;
                        cache::operations::get_cached_events(
                            &mut conn,
                            workspace_id,
                            channel_id.as_deref(),
                            user_id.as_deref(),
                            since_ts,
                            limit,
                            cli.verbose,
                        )?
                    } else {
                        vec![]
                    };

                    // Output
                    if cached_events.is_empty() {
                        eprintln!("No cached events found.");
                    } else {
                        for cached in &cached_events {
                            if let Some(callback) = cached.to_event_callback() {
                                match cli.format.as_str() {
                                    "json" => println!("{}", serde_json::to_string(&callback)?),
                                    "yaml" => println!("{}", serde_yaml::to_string(&callback)?),
                                    _ => {
                                        // Human-readable format
                                        if let models::event::Event::Message(msg) = &callback.event
                                        {
                                            println!(
                                                "[{}] #{} {}: {}",
                                                chrono::DateTime::from_timestamp(
                                                    callback.event_time,
                                                    0
                                                )
                                                .map(|dt| dt
                                                    .format("%Y-%m-%d %H:%M:%S")
                                                    .to_string())
                                                .unwrap_or_else(|| callback.event_time.to_string()),
                                                msg.channel,
                                                msg.effective_user().unwrap_or("unknown"),
                                                msg.display_text()
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Commands::Cache { command } => {
            use anyhow::Context;

            let pool = client
                .cache_pool()
                .ok_or_else(|| anyhow::anyhow!("Cache not available"))?;
            let mut conn = cache::db::get_connection(pool).await?;
            let workspace_id = client.workspace_id().context("Workspace ID not initialized")?;

            match command {
                CacheCommands::List { table, limit } => {
                    match table.to_lowercase().as_str() {
                        "users" => {
                            let records = cache::operations::list_cached_users(
                                &mut conn,
                                workspace_id,
                                limit,
                                cli.verbose,
                            )?;

                            if records.is_empty() {
                                eprintln!("No cached users found.");
                            } else {
                                match cli.format.as_str() {
                                    "json" => {
                                        // Collect all users into an array
                                        let users: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        final_output = serde_json::to_string_pretty(&users)?;
                                    }
                                    "yaml" => {
                                        let users: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        final_output = serde_yaml::to_string(&users)?;
                                    }
                                    _ => {
                                        // Human format - deserialize and use formatter
                                        let users: Vec<models::user::User> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        writer.print_header(&format!("Cached Users ({} of {})", users.len(), records.len()))?;
                                        writer.print_separator()?;
                                        for (i, user) in users.iter().enumerate() {
                                            output::user_formatter::format_user(user, &mut writer)?;
                                            if i < users.len() - 1 {
                                                writer.writeln()?;
                                            }
                                        }
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                        }
                        "conversations" => {
                            let records = cache::operations::list_cached_conversations(
                                &mut conn,
                                workspace_id,
                                limit,
                                cli.verbose,
                            )?;

                            if records.is_empty() {
                                eprintln!("No cached conversations found.");
                            } else {
                                match cli.format.as_str() {
                                    "json" => {
                                        let channels: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        final_output = serde_json::to_string_pretty(&channels)?;
                                    }
                                    "yaml" => {
                                        let channels: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        final_output = serde_yaml::to_string(&channels)?;
                                    }
                                    _ => {
                                        // Human format - deserialize and use formatter
                                        let channels: Vec<models::channel::Channel> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        writer.print_header(&format!("Cached Conversations ({} of {})", channels.len(), records.len()))?;
                                        writer.print_separator()?;
                                        output::channel_formatter::format_channels_list(&channels, &mut writer)?;
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                        }
                        "messages" => {
                            let records = cache::operations::list_cached_messages(
                                &mut conn,
                                workspace_id,
                                limit,
                                cli.verbose,
                            )?;

                            if records.is_empty() {
                                eprintln!("No cached messages found.");
                            } else {
                                match cli.format.as_str() {
                                    "json" => {
                                        let messages: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        final_output = serde_json::to_string_pretty(&messages)?;
                                    }
                                    "yaml" => {
                                        let messages: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        final_output = serde_yaml::to_string(&messages)?;
                                    }
                                    _ => {
                                        // Human format - use compact message format
                                        let messages: Vec<models::message::Message> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_object).ok())
                                            .collect();
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        writer.print_header(&format!("Cached Messages ({} of {})", messages.len(), records.len()))?;
                                        writer.print_separator()?;
                                        let user_map = std::collections::HashMap::new();
                                        for (i, msg) in messages.iter().enumerate() {
                                            output::message_formatter::format_message_compact(msg, &user_map, &mut writer)?;
                                            if i < messages.len() - 1 {
                                                writer.writeln()?;
                                            }
                                        }
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                        }
                        "events" => {
                            let records = cache::operations::list_cached_events(
                                &mut conn,
                                workspace_id,
                                limit,
                                cli.verbose,
                            )?;

                            if records.is_empty() {
                                eprintln!("No cached events found.");
                            } else {
                                match cli.format.as_str() {
                                    "json" => {
                                        let events: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_payload).ok())
                                            .collect();
                                        final_output = serde_json::to_string_pretty(&events)?;
                                    }
                                    "yaml" => {
                                        let events: Vec<serde_json::Value> = records
                                            .iter()
                                            .filter_map(|r| serde_json::from_str(&r.full_payload).ok())
                                            .collect();
                                        final_output = serde_yaml::to_string(&events)?;
                                    }
                                    _ => {
                                        // Human format - show compact event info
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        writer.print_header(&format!("Cached Events ({})", records.len()))?;
                                        writer.print_separator()?;
                                        for (i, record) in records.iter().enumerate() {
                                            // Show event_id, type, channel, user, timestamp
                                            writer.print_colored(&record.event_id, termcolor::Color::Yellow)?;
                                            writer.write(" ")?;
                                            writer.print_colored(&record.event_type, termcolor::Color::Cyan)?;
                                            if let Some(ch) = &record.channel_id {
                                                writer.write(&format!(" #{}", ch))?;
                                            }
                                            if let Some(u) = &record.user_id {
                                                writer.write(&format!(" @{}", u))?;
                                            }
                                            writer.writeln()?;
                                            // Show text preview if available
                                            if let Some(text) = &record.message_text {
                                                let preview: String = text.chars().take(80).collect();
                                                writer.write("  ")?;
                                                writer.write(&preview)?;
                                                if text.len() > 80 {
                                                    writer.write("...")?;
                                                }
                                                writer.writeln()?;
                                            }
                                            if i < records.len() - 1 {
                                                writer.writeln()?;
                                            }
                                        }
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                        }
                        _ => {
                            anyhow::bail!(
                                "Unknown table '{}'. Valid tables: users, conversations, messages, events",
                                table
                            );
                        }
                    }
                }
                CacheCommands::Show { table, id } => match table.to_lowercase().as_str() {
                    "users" => {
                        let record = cache::operations::get_cached_user_by_id(
                            &mut conn,
                            workspace_id,
                            &id,
                        )?;

                        match record {
                            Some(r) => {
                                match cli.format.as_str() {
                                    "json" => {
                                        final_output = r.full_object;
                                    }
                                    "yaml" => {
                                        let user: serde_json::Value =
                                            serde_json::from_str(&r.full_object)?;
                                        final_output = serde_yaml::to_string(&user)?;
                                    }
                                    _ => {
                                        // Human format
                                        let user: models::user::User = serde_json::from_str(&r.full_object)?;
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        output::user_formatter::format_user(&user, &mut writer)?;
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                            None => eprintln!("User '{}' not found in cache.", id),
                        }
                    }
                    "conversations" => {
                        let record = cache::operations::get_cached_conversation_by_id(
                            &mut conn,
                            workspace_id,
                            &id,
                        )?;

                        match record {
                            Some(r) => {
                                match cli.format.as_str() {
                                    "json" => {
                                        final_output = r.full_object;
                                    }
                                    "yaml" => {
                                        let conv: serde_json::Value =
                                            serde_json::from_str(&r.full_object)?;
                                        final_output = serde_yaml::to_string(&conv)?;
                                    }
                                    _ => {
                                        // Human format
                                        let channel: models::channel::Channel = serde_json::from_str(&r.full_object)?;
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        output::channel_formatter::format_channels_list(&[channel], &mut writer)?;
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                            None => eprintln!("Conversation '{}' not found in cache.", id),
                        }
                    }
                    "messages" => {
                        // ID format: conversation_id:ts
                        let parts: Vec<&str> = id.splitn(2, ':').collect();
                        if parts.len() != 2 {
                            anyhow::bail!(
                                "Message ID must be in format 'CHANNEL_ID:TIMESTAMP' (e.g., 'C123:1234567890.123456')"
                            );
                        }

                        let record = cache::operations::get_cached_message_by_id(
                            &mut conn,
                            workspace_id,
                            parts[0],
                            parts[1],
                        )?;

                        match record {
                            Some(r) => {
                                match cli.format.as_str() {
                                    "json" => {
                                        final_output = r.full_object;
                                    }
                                    "yaml" => {
                                        let msg: serde_json::Value =
                                            serde_json::from_str(&r.full_object)?;
                                        final_output = serde_yaml::to_string(&msg)?;
                                    }
                                    _ => {
                                        // Human format - use compact message format
                                        let msg: models::message::Message = serde_json::from_str(&r.full_object)?;
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        let user_map = std::collections::HashMap::new();
                                        let thread_info = std::collections::HashMap::new();
                                        output::message_formatter::format_single_message(
                                            &msg,
                                            parts[0], // channel_id
                                            parts[0], // channel_name (use id as fallback)
                                            &user_map,
                                            &thread_info,
                                            &mut writer,
                                        )?;
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                            None => eprintln!("Message '{}' not found in cache.", id),
                        }
                    }
                    "events" => {
                        let record = cache::operations::get_cached_event_by_id(
                            &mut conn,
                            workspace_id,
                            &id,
                        )?;

                        match record {
                            Some(r) => {
                                match cli.format.as_str() {
                                    "json" => {
                                        final_output = r.full_payload;
                                    }
                                    "yaml" => {
                                        let event: serde_json::Value =
                                            serde_json::from_str(&r.full_payload)?;
                                        final_output = serde_yaml::to_string(&event)?;
                                    }
                                    _ => {
                                        // Human format - show event details
                                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                                        writer.print_header(&format!("Event: {}", r.event_id))?;
                                        writer.print_separator()?;
                                        writer.print_field("Type", &r.event_type)?;
                                        if let Some(ch) = &r.channel_id {
                                            writer.print_field("Channel", ch)?;
                                        }
                                        if let Some(u) = &r.user_id {
                                            writer.print_field("User", u)?;
                                        }
                                        writer.print_field("Event Time", &r.event_time.to_string())?;
                                        writer.print_field("Cached At", &r.cached_at.to_string())?;
                                        if let Some(text) = &r.message_text {
                                            writer.writeln()?;
                                            writer.print_field("Text", text)?;
                                        }
                                        final_output = writer.into_string()?;
                                    }
                                }
                            }
                            None => eprintln!("Event '{}' not found in cache.", id),
                        }
                    }
                    _ => {
                        anyhow::bail!(
                            "Unknown table '{}'. Valid tables: users, conversations, messages, events",
                            table
                        );
                    }
                },
            }
        }
        Commands::Open { url } => {
            let parsed = url_parser::SlackUrl::parse(&url)?;

            // Get channel info
            let channel = api::channels::get_channel(&client, &parsed.channel_id).await?;

            if let Some(message_ts) = &parsed.message_ts {
                // Fetch the specific message using inclusive timestamp range
                let messages = api::messages::list_messages(
                    &client,
                    &parsed.channel_id,
                    1,
                    Some(message_ts.clone()),
                    Some(message_ts.clone()),
                )
                .await?;

                if messages.is_empty() {
                    // Try fetching as thread parent
                    let thread_messages =
                        api::messages::get_thread(&client, &parsed.channel_id, message_ts).await?;

                    if thread_messages.is_empty() {
                        anyhow::bail!("Message not found: {}", message_ts);
                    }

                    // Render thread
                    final_output = match cli.format.as_str() {
                        "json" => serde_json::to_string_pretty(&thread_messages)?,
                        "yaml" => serde_yaml::to_string(&thread_messages)?,
                        _ => {
                            // Build user map
                            let mut user_map: std::collections::HashMap<
                                String,
                                models::user::User,
                            > = std::collections::HashMap::new();
                            for msg in &thread_messages {
                                if let Some(user_id) = &msg.user {
                                    if !user_map.contains_key(user_id) {
                                        if let Ok(user) =
                                            api::users::get_user(&client, user_id).await
                                        {
                                            user_map.insert(user.id.clone(), user);
                                        }
                                    }
                                }
                            }

                            let mut writer = output::color::ColorWriter::new(cli.no_color);
                            output::thread_formatter::format_thread(
                                &thread_messages,
                                &channel,
                                &user_map,
                                &mut writer,
                            )?;
                            writer.into_string()?
                        }
                    };
                } else {
                    // Render single message with context
                    let message = &messages[0];

                    final_output = match cli.format.as_str() {
                        "json" => serde_json::to_string_pretty(&message)?,
                        "yaml" => serde_yaml::to_string(&message)?,
                        _ => {
                            let mut user_map: std::collections::HashMap<
                                String,
                                models::user::User,
                            > = std::collections::HashMap::new();
                            if let Some(user_id) = &message.user {
                                if let Ok(user) = api::users::get_user(&client, user_id).await {
                                    user_map.insert(user.id.clone(), user);
                                }
                            }

                            let mut writer = output::color::ColorWriter::new(cli.no_color);

                            // Show channel info header
                            writer.print_header(&format!("#{} ({})", channel.name, channel.id))?;
                            if let Some(topic) = &channel.topic {
                                if !topic.value.is_empty() {
                                    writer.print_field("Topic", &topic.value)?;
                                }
                            }
                            writer.print_separator()?;

                            // Show the message
                            output::message_formatter::format_messages_with_thread_info(
                                &[message.clone()],
                                &channel,
                                &user_map,
                                &std::collections::HashMap::new(),
                                &mut writer,
                            )?;

                            writer.into_string()?
                        }
                    };
                }
            } else {
                // No message timestamp - show channel info
                final_output = match cli.format.as_str() {
                    "json" => serde_json::to_string_pretty(&channel)?,
                    "yaml" => serde_yaml::to_string(&channel)?,
                    _ => {
                        let mut writer = output::color::ColorWriter::new(cli.no_color);
                        output::channel_formatter::format_channels_list(&[channel], &mut writer)?;
                        writer.into_string()?
                    }
                };
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
