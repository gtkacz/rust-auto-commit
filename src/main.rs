use anyhow::Result;
use clap::Parser;
use log::error;
use opencommit::cli::Cli;
use opencommit::commands::{commit, config, githook, commitlint};
use opencommit::migrations::run_migrations;
use opencommit::utils::version::check_latest_version;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Run migrations for config if needed
    if let Err(e) = run_migrations().await {
        error!("Failed to run migrations: {}", e);
        return Err(e);
    }
    
    // Check if we're running the latest version
    if let Err(e) = check_latest_version().await {
        // Just log the error but continue
        error!("Failed to check latest version: {}", e);
    }
    
    // Execute the appropriate command
    match cli.command {
        Some(cmd) => match cmd {
            opencommit::cli::Commands::Config { action } => {
                config::handle_config_command(action).await
            }
            opencommit::cli::Commands::Hook { action } => {
                githook::handle_hook_command(action).await
            }
            opencommit::cli::Commands::Commitlint { action } => {
                commitlint::handle_commitlint_command(action).await
            }
        },
        None => {
            // Default command is commit
            commit::execute_commit(
                cli.extra_args, 
                cli.context, 
                false, 
                cli.fgm, 
                cli.yes
            ).await
        }
    }
}