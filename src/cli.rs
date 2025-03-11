use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The command to execute
    #[command(subcommand)]
    pub command: Option<Commands>,
    
    /// Skip commit confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
    
    /// Use full GitMoji specification
    #[arg(long = "fgm")]
    pub fgm: bool,
    
    /// Additional user input context for the commit message
    #[arg(short, long)]
    pub context: Option<String>,
    
    /// Extra arguments passed to git commit
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configure OpenCommit
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    
    /// Manage git hooks
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    
    /// Configure commitlint integration
    Commitlint {
        #[command(subcommand)]
        action: CommitlintAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Get configuration value
    Get {
        /// Configuration keys to get
        keys: Vec<String>,
    },
    
    /// Set configuration value
    Set {
        /// Configuration key-value pairs to set (format: KEY=VALUE)
        key_values: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum HookAction {
    /// Set up OpenCommit as a git prepare-commit-msg hook
    Set,
    
    /// Remove OpenCommit as a git prepare-commit-msg hook
    Unset,
}

#[derive(Subcommand, Debug)]
pub enum CommitlintAction {
    /// Get commitlint configuration
    Get,
    
    /// Force update commitlint configuration
    Force,
}