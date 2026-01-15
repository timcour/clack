mod api;
mod cli;
mod models;
mod output;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create API client
    let client = api::client::SlackClient::new()?;

    // Get appropriate formatter
    let formatter = output::formatter::get_formatter(&cli.format);

    // Execute command
    match cli.command {
        Commands::Users {
            limit,
            include_deleted,
        } => {
            let users = api::users::list_users(&client, limit, include_deleted).await?;
            let output = formatter.format(&users)?;
            println!("{}", output);
        }
        Commands::User { user_id } => {
            let user = api::users::get_user(&client, &user_id).await?;
            let output = formatter.format(&user)?;
            println!("{}", output);
        }
        Commands::Messages {
            channel,
            limit,
            latest,
            oldest,
        } => {
            let messages =
                api::messages::list_messages(&client, &channel, limit, latest, oldest).await?;
            let output = formatter.format(&messages)?;
            println!("{}", output);
        }
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
