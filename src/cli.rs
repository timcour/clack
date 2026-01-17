use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clack")]
#[command(about = "A Slack API CLI tool", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Disable colorized output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Output format (human, json, yaml)
    #[arg(long, global = true, default_value = "human")]
    pub format: String,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// User-related commands
    Users {
        #[command(subcommand)]
        command: UsersCommands,
    },
    /// Conversation-related commands (channels, messages, threads)
    Conversations {
        #[command(subcommand)]
        command: ConversationsCommands,
    },
    /// File-related commands
    Files {
        #[command(subcommand)]
        command: FilesCommands,
    },
    /// Pin-related commands
    Pins {
        #[command(subcommand)]
        command: PinsCommands,
    },
    /// Reaction-related commands
    Reactions {
        #[command(subcommand)]
        command: ReactionsCommands,
    },
    /// Chat/message posting commands
    Chat {
        #[command(subcommand)]
        command: ChatCommands,
    },
    /// Search for messages, files, or channels
    Search {
        #[command(subcommand)]
        search_type: SearchType,
    },
    /// Authentication commands
    Auth {
        #[command(subcommand)]
        auth_type: AuthType,
    },
}

#[derive(Subcommand)]
pub enum UsersCommands {
    /// List all users
    List {
        /// Maximum number of users to return
        #[arg(long, default_value = "200")]
        limit: u32,

        /// Include deleted/deactivated users
        #[arg(long)]
        include_deleted: bool,
    },
    /// Get information about a specific user
    Info {
        /// Slack user ID (e.g., U1234ABCD)
        user_id: String,
    },
    /// Get user profile information
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
}

#[derive(Subcommand)]
pub enum ProfileCommands {
    /// Get user profile
    Get {
        /// Slack user ID (optional, defaults to authenticated user)
        user_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConversationsCommands {
    /// List all channels the bot has access to
    List {
        /// Include archived channels
        #[arg(long)]
        include_archived: bool,

        /// Maximum number of channels to retrieve per page (default: 200, max: 1000)
        #[arg(long, default_value = "200")]
        limit: u32,
    },
    /// Get information about a specific channel
    Info {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,
    },
    /// Get message history from a channel
    History {
        /// Channel ID or name
        channel: String,

        /// Number of messages to retrieve
        #[arg(long, default_value = "200")]
        limit: u32,

        /// End of time range (Unix timestamp)
        #[arg(long)]
        latest: Option<String>,

        /// Start of time range (Unix timestamp)
        #[arg(long)]
        oldest: Option<String>,
    },
    /// Get all replies in a conversation thread
    Replies {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Message timestamp/ID (e.g., 1234567890.123456)
        message_ts: String,
    },
    /// Get list of members in a conversation
    Members {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Maximum number of members to retrieve
        #[arg(long, default_value = "200")]
        limit: u32,
    },
}

#[derive(Subcommand)]
pub enum SearchType {
    /// Search messages
    Messages {
        /// Search query
        query: String,

        /// Filter by user (user ID, @username, or display name)
        #[arg(long)]
        from: Option<String>,

        /// Filter by channel (channel ID, #name, or name)
        #[arg(long, alias = "in")]
        channel: Option<String>,

        /// Filter messages after date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        after: Option<String>,

        /// Filter messages before date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        before: Option<String>,

        /// Maximum number of results
        #[arg(long, default_value = "200")]
        limit: u32,
    },
    /// Search files
    Files {
        /// Search query
        query: String,

        /// Filter by user (user ID, @username, or display name)
        #[arg(long)]
        from: Option<String>,

        /// Filter by channel (channel ID, #name, or name)
        #[arg(long, alias = "in")]
        channel: Option<String>,

        /// Filter files after date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        after: Option<String>,

        /// Filter files before date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        before: Option<String>,

        /// Maximum number of results
        #[arg(long, default_value = "200")]
        limit: u32,
    },
    /// Search all (messages and files)
    All {
        /// Search query
        query: String,

        /// Filter by channel (channel ID, #name, or name)
        #[arg(long, alias = "in")]
        channel: Option<String>,

        /// Maximum number of results
        #[arg(long, default_value = "200")]
        limit: u32,
    },
    /// Search channels by name
    Channels {
        /// Search query (channel name substring)
        query: String,

        /// Include archived channels
        #[arg(long)]
        include_archived: bool,
    },
}

#[derive(Subcommand)]
pub enum FilesCommands {
    /// List files in the workspace
    List {
        /// Maximum number of files to return
        #[arg(long, default_value = "200")]
        limit: u32,

        /// Filter by user (user ID)
        #[arg(long)]
        user: Option<String>,

        /// Filter by channel (channel ID or name)
        #[arg(long)]
        channel: Option<String>,
    },
    /// Get information about a specific file
    Info {
        /// File ID (e.g., F1234ABCD)
        file_id: String,
    },
}

#[derive(Subcommand)]
pub enum PinsCommands {
    /// List pinned items in a channel
    List {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,
    },
    /// Pin a message to a channel
    Add {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Message timestamp to pin (e.g., 1234567890.123456)
        message_ts: String,
    },
    /// Remove a pin from a channel
    Remove {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Message timestamp to unpin (e.g., 1234567890.123456)
        message_ts: String,
    },
}

#[derive(Subcommand)]
pub enum ReactionsCommands {
    /// Add a reaction to a message
    Add {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Message timestamp (e.g., 1234567890.123456)
        message_ts: String,

        /// Emoji name (without colons, e.g., thumbsup, heart, rocket)
        emoji: String,
    },
    /// Remove a reaction from a message
    Remove {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Message timestamp (e.g., 1234567890.123456)
        message_ts: String,

        /// Emoji name (without colons, e.g., thumbsup, heart, rocket)
        emoji: String,
    },
}

#[derive(Subcommand)]
pub enum ChatCommands {
    /// Post a message to a channel
    Post {
        /// Channel ID or name (e.g., C1234ABCD, #general, or general)
        channel: String,

        /// Message text (use - to read from stdin)
        text: String,

        /// Thread timestamp to reply to (makes this a thread reply)
        #[arg(long)]
        thread_ts: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum AuthType {
    /// Test authentication and display workspace metadata
    Test,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_users_list_command_parsing() {
        let cli = Cli::parse_from(["clack", "users", "list"]);
        assert!(matches!(cli.command, Commands::Users { .. }));
        assert_eq!(cli.format, "human");
        assert!(!cli.no_color);
        assert!(!cli.verbose);
    }

    #[test]
    fn test_users_list_command_with_options() {
        let cli = Cli::parse_from(["clack", "users", "list", "--limit", "50", "--include-deleted"]);
        match cli.command {
            Commands::Users { command } => match command {
                UsersCommands::List {
                    limit,
                    include_deleted,
                } => {
                    assert_eq!(limit, 50);
                    assert!(include_deleted);
                }
                _ => panic!("Expected Users List command"),
            },
            _ => panic!("Expected Users command"),
        }
    }

    #[test]
    fn test_users_info_command_with_id() {
        let cli = Cli::parse_from(["clack", "users", "info", "U123"]);
        match cli.command {
            Commands::Users { command } => match command {
                UsersCommands::Info { user_id } => assert_eq!(user_id, "U123"),
                _ => panic!("Expected Users Info command"),
            },
            _ => panic!("Expected Users command"),
        }
    }

    #[test]
    fn test_conversations_history_command_basic() {
        let cli = Cli::parse_from(["clack", "conversations", "history", "C123"]);
        match cli.command {
            Commands::Conversations { command } => match command {
                ConversationsCommands::History {
                    channel,
                    limit,
                    latest,
                    oldest,
                } => {
                    assert_eq!(channel, "C123");
                    assert_eq!(limit, 200); // default value
                    assert_eq!(latest, None);
                    assert_eq!(oldest, None);
                }
                _ => panic!("Expected Conversations History command"),
            },
            _ => panic!("Expected Conversations command"),
        }
    }

    #[test]
    fn test_conversations_history_command_with_options() {
        let cli = Cli::parse_from([
            "clack",
            "conversations",
            "history",
            "C123",
            "--limit",
            "50",
            "--latest",
            "1234567890",
            "--oldest",
            "1234567800",
        ]);
        match cli.command {
            Commands::Conversations { command } => match command {
                ConversationsCommands::History {
                    channel,
                    limit,
                    latest,
                    oldest,
                } => {
                    assert_eq!(channel, "C123");
                    assert_eq!(limit, 50);
                    assert_eq!(latest, Some("1234567890".to_string()));
                    assert_eq!(oldest, Some("1234567800".to_string()));
                }
                _ => panic!("Expected Conversations History command"),
            },
            _ => panic!("Expected Conversations command"),
        }
    }

    #[test]
    fn test_global_format_option() {
        let cli = Cli::parse_from(["clack", "--format", "json", "users", "list"]);
        assert_eq!(cli.format, "json");
    }

    #[test]
    fn test_global_no_color_option() {
        let cli = Cli::parse_from(["clack", "--no-color", "users", "list"]);
        assert!(cli.no_color);
    }

    #[test]
    fn test_global_verbose_option() {
        let cli = Cli::parse_from(["clack", "-v", "users", "list"]);
        assert!(cli.verbose);
    }

    #[test]
    fn test_conversations_replies_command() {
        let cli = Cli::parse_from(["clack", "conversations", "replies", "C123", "1234567890.123456"]);
        match cli.command {
            Commands::Conversations { command } => match command {
                ConversationsCommands::Replies {
                    channel,
                    message_ts,
                } => {
                    assert_eq!(channel, "C123");
                    assert_eq!(message_ts, "1234567890.123456");
                }
                _ => panic!("Expected Conversations Replies command"),
            },
            _ => panic!("Expected Conversations command"),
        }
    }

    #[test]
    fn test_conversations_replies_command_with_channel_name() {
        let cli = Cli::parse_from(["clack", "conversations", "replies", "#general", "1234567890.123456"]);
        match cli.command {
            Commands::Conversations { command } => match command {
                ConversationsCommands::Replies {
                    channel,
                    message_ts,
                } => {
                    assert_eq!(channel, "#general");
                    assert_eq!(message_ts, "1234567890.123456");
                }
                _ => panic!("Expected Conversations Replies command"),
            },
            _ => panic!("Expected Conversations command"),
        }
    }

    #[test]
    fn test_conversations_list_command() {
        let cli = Cli::parse_from(["clack", "conversations", "list"]);
        match cli.command {
            Commands::Conversations { command } => match command {
                ConversationsCommands::List { include_archived, limit } => {
                    assert!(!include_archived);
                    assert_eq!(limit, 200); // default value
                }
                _ => panic!("Expected Conversations List command"),
            },
            _ => panic!("Expected Conversations command"),
        }
    }

    #[test]
    fn test_conversations_list_command_with_archived() {
        let cli = Cli::parse_from(["clack", "conversations", "list", "--include-archived"]);
        match cli.command {
            Commands::Conversations { command } => match command {
                ConversationsCommands::List { include_archived, limit } => {
                    assert!(include_archived);
                    assert_eq!(limit, 200); // default value
                }
                _ => panic!("Expected Conversations List command"),
            },
            _ => panic!("Expected Conversations command"),
        }
    }

    #[test]
    fn test_conversations_info_command() {
        let cli = Cli::parse_from(["clack", "conversations", "info", "C123"]);
        match cli.command {
            Commands::Conversations { command } => match command {
                ConversationsCommands::Info { channel } => {
                    assert_eq!(channel, "C123");
                }
                _ => panic!("Expected Conversations Info command"),
            },
            _ => panic!("Expected Conversations command"),
        }
    }

    #[test]
    fn test_search_messages_basic() {
        let cli = Cli::parse_from(["clack", "search", "messages", "hello world"]);
        match cli.command {
            Commands::Search { search_type } => match search_type {
                SearchType::Messages {
                    query,
                    from,
                    channel,
                    after,
                    before,
                    limit,
                } => {
                    assert_eq!(query, "hello world");
                    assert_eq!(from, None);
                    assert_eq!(channel, None);
                    assert_eq!(after, None);
                    assert_eq!(before, None);
                    assert_eq!(limit, 200);
                }
                _ => panic!("Expected Messages search type"),
            },
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_search_messages_with_filters() {
        let cli = Cli::parse_from([
            "clack",
            "search",
            "messages",
            "deploy",
            "--from",
            "alice",
            "--channel",
            "engineering",
            "--after",
            "2024-01-01",
            "--before",
            "2024-12-31",
            "--limit",
            "50",
        ]);
        match cli.command {
            Commands::Search { search_type } => match search_type {
                SearchType::Messages {
                    query,
                    from,
                    channel,
                    after,
                    before,
                    limit,
                } => {
                    assert_eq!(query, "deploy");
                    assert_eq!(from, Some("alice".to_string()));
                    assert_eq!(channel, Some("engineering".to_string()));
                    assert_eq!(after, Some("2024-01-01".to_string()));
                    assert_eq!(before, Some("2024-12-31".to_string()));
                    assert_eq!(limit, 50);
                }
                _ => panic!("Expected Messages search type"),
            },
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_search_files_basic() {
        let cli = Cli::parse_from(["clack", "search", "files", "*.pdf"]);
        match cli.command {
            Commands::Search { search_type } => match search_type {
                SearchType::Files { query, .. } => {
                    assert_eq!(query, "*.pdf");
                }
                _ => panic!("Expected Files search type"),
            },
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_search_all() {
        let cli = Cli::parse_from(["clack", "search", "all", "budget 2024"]);
        match cli.command {
            Commands::Search { search_type } => match search_type {
                SearchType::All {
                    query,
                    channel,
                    limit,
                } => {
                    assert_eq!(query, "budget 2024");
                    assert_eq!(channel, None);
                    assert_eq!(limit, 200);
                }
                _ => panic!("Expected All search type"),
            },
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_search_channels() {
        let cli = Cli::parse_from(["clack", "search", "channels", "engineering"]);
        match cli.command {
            Commands::Search { search_type } => match search_type {
                SearchType::Channels {
                    query,
                    include_archived,
                } => {
                    assert_eq!(query, "engineering");
                    assert!(!include_archived);
                }
                _ => panic!("Expected Channels search type"),
            },
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_search_channels_with_archived() {
        let cli = Cli::parse_from(["clack", "search", "channels", "old-project", "--include-archived"]);
        match cli.command {
            Commands::Search { search_type } => match search_type {
                SearchType::Channels {
                    query,
                    include_archived,
                } => {
                    assert_eq!(query, "old-project");
                    assert!(include_archived);
                }
                _ => panic!("Expected Channels search type"),
            },
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_auth_test_command() {
        let cli = Cli::parse_from(["clack", "auth", "test"]);
        match cli.command {
            Commands::Auth { auth_type } => match auth_type {
                AuthType::Test => {
                    // Success - command parsed correctly
                }
            },
            _ => panic!("Expected Auth command"),
        }
    }
}
