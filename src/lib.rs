pub mod cli;
pub mod commands;
pub mod engine;
pub mod i18n;
pub mod modules;
pub mod utils;
pub mod prompts;
pub mod migrations;
pub mod error;

// Re-export key types for convenience
pub use cli::Cli;
pub use engine::Engine;
pub use error::Error;