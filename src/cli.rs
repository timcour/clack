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
    /// List all users
    Users {
        /// Maximum number of users to return
        #[arg(long)]
        limit: Option<u32>,

        /// Include deleted/deactivated users
        #[arg(long)]
        include_deleted: bool,
    },
    /// Get a specific user by ID
    User {
        /// Slack user ID (e.g., U1234ABCD)
        user_id: String,
    },
    /// List messages from a channel
    Messages {
        /// Channel ID or name
        channel: String,

        /// Number of messages to retrieve
        #[arg(long, default_value = "100")]
        limit: u32,

        /// End of time range (Unix timestamp)
        #[arg(long)]
        latest: Option<String>,

        /// Start of time range (Unix timestamp)
        #[arg(long)]
        oldest: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_users_command_parsing() {
        let cli = Cli::parse_from(["clack", "users"]);
        assert!(matches!(cli.command, Commands::Users { .. }));
        assert_eq!(cli.format, "human");
        assert!(!cli.no_color);
        assert!(!cli.verbose);
    }

    #[test]
    fn test_users_command_with_options() {
        let cli = Cli::parse_from(["clack", "users", "--limit", "50", "--include-deleted"]);
        match cli.command {
            Commands::Users {
                limit,
                include_deleted,
            } => {
                assert_eq!(limit, Some(50));
                assert!(include_deleted);
            }
            _ => panic!("Expected Users command"),
        }
    }

    #[test]
    fn test_user_command_with_id() {
        let cli = Cli::parse_from(["clack", "user", "U123"]);
        match cli.command {
            Commands::User { user_id } => assert_eq!(user_id, "U123"),
            _ => panic!("Expected User command"),
        }
    }

    #[test]
    fn test_messages_command_basic() {
        let cli = Cli::parse_from(["clack", "messages", "C123"]);
        match cli.command {
            Commands::Messages {
                channel,
                limit,
                latest,
                oldest,
            } => {
                assert_eq!(channel, "C123");
                assert_eq!(limit, 100); // default value
                assert_eq!(latest, None);
                assert_eq!(oldest, None);
            }
            _ => panic!("Expected Messages command"),
        }
    }

    #[test]
    fn test_messages_command_with_options() {
        let cli = Cli::parse_from([
            "clack",
            "messages",
            "C123",
            "--limit",
            "50",
            "--latest",
            "1234567890",
            "--oldest",
            "1234567800",
        ]);
        match cli.command {
            Commands::Messages {
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
            _ => panic!("Expected Messages command"),
        }
    }

    #[test]
    fn test_global_format_option() {
        let cli = Cli::parse_from(["clack", "--format", "json", "users"]);
        assert_eq!(cli.format, "json");
    }

    #[test]
    fn test_global_no_color_option() {
        let cli = Cli::parse_from(["clack", "--no-color", "users"]);
        assert!(cli.no_color);
    }

    #[test]
    fn test_global_verbose_option() {
        let cli = Cli::parse_from(["clack", "-v", "users"]);
        assert!(cli.verbose);
    }
}
