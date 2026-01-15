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
