use crate::error::Result;
use crate::commands::config::{Config, AiProvider, ConfigKey};
use std::fs;
use std::path::Path;
use log::{info, error};
use colored::Colorize;

// Migration: Use single API key and URL
async fn migration_use_single_api_key_and_url() -> Result<()> {
    let config_path = Config::global_config_path();
    
    // If config doesn't exist, no need to migrate
    if !config_path.exists() {
        return Ok(());
    }
    
    let config = Config::load()?;
    
    // Get environment variables for different providers
    let mut api_key = None;
    let mut api_url = None;
    
    if config.ai_provider == "ollama" {
        api_key = std::env::var("OCO_OLLAMA_API_KEY").ok();
        api_url = std::env::var("OCO_OLLAMA_API_URL").ok();
    } else if config.ai_provider == "anthropic" {
        api_key = std::env::var("OCO_ANTHROPIC_API_KEY").ok();
        api_url = std::env::var("OCO_ANTHROPIC_BASE_PATH").ok();
    } else if config.ai_provider == "openai" {
        api_key = std::env::var("OCO_OPENAI_API_KEY").ok();
        api_url = std::env::var("OCO_OPENAI_BASE_PATH").ok();
    } else if config.ai_provider == "azure" {
        api_key = std::env::var("OCO_AZURE_API_KEY").ok();
        api_url = std::env::var("OCO_AZURE_ENDPOINT").ok();
    } else if config.ai_provider == "gemini" {
        api_key = std::env::var("OCO_GEMINI_API_KEY").ok();
        api_url = std::env::var("OCO_GEMINI_BASE_PATH").ok();
    } else if config.ai_provider == "flowise" {
        api_key = std::env::var("OCO_FLOWISE_API_KEY").ok();
        api_url = std::env::var("OCO_FLOWISE_ENDPOINT").ok();
    }
    
    // Update config with consolidated variables
    let mut updated_config = config.clone();
    
    if let Some(key) = api_key {
        updated_config.api_key = Some(key);
    }
    
    if let Some(url) = api_url {
        updated_config.api_url = Some(url);
    }
    
    // Save updated config
    updated_config.save()?;
    
    Ok(())
}

// Migration: set missing default values
async fn migration_set_missing_default_values() -> Result<()> {
    let config_path = Config::global_config_path();
    
    // If config doesn't exist, no need to migrate
    if !config_path.exists() {
        return Ok(());
    }
    
    let config = Config::load()?;
    let default_config = Config::default();
    
    // Fields to check and set if missing
    let mut updated_config = config.clone();
    
    if updated_config.tokens_max_input == 0 {
        updated_config.tokens_max_input = default_config.tokens_max_input;
    }
    
    if updated_config.tokens_max_output == 0 {
        updated_config.tokens_max_output = default_config.tokens_max_output;
    }
    
    if updated_config.model.is_empty() {
        updated_config.model = default_config.model;
    }
    
    if updated_config.language.is_empty() {
        updated_config.language = default_config.language;
    }
    
    if updated_config.message_template_placeholder.is_empty() {
        updated_config.message_template_placeholder = default_config.message_template_placeholder;
    }
    
    if updated_config.prompt_module.is_empty() {
        updated_config.prompt_module = default_config.prompt_module;
    }
    
    if updated_config.ai_provider.is_empty() {
        updated_config.ai_provider = default_config.ai_provider;
    }
    
    // Save updated config
    updated_config.save()?;
    
    Ok(())
}

// List of migrations to run
struct Migration {
    name: &'static str,
    function: fn() -> Result<()>,
}

// Get path to migrations record file
fn get_migrations_file_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".opencommit_migrations")
}

// Get completed migrations
fn get_completed_migrations() -> Result<Vec<String>> {
    let path = get_migrations_file_path();
    
    if !path.exists() {
        return Ok(Vec::new());
    }
    
    let content = fs::read_to_string(path)?;
    let migrations: Vec<String> = serde_json::from_str(&content)?;
    
    Ok(migrations)
}

// Save completed migration
fn save_completed_migration(migration_name: &str) -> Result<()> {
    let path = get_migrations_file_path();
    
    let mut migrations = get_completed_migrations()?;
    migrations.push(migration_name.to_string());
    
    let content = serde_json::to_string_pretty(&migrations)?;
    fs::write(path, content)?;
    
    Ok(())
}

// Run all migrations
pub async fn run_migrations() -> Result<()> {
    // If no config file, assume it's a new installation
    let config_path = Config::global_config_path();
    if !config_path.exists() {
        return Ok(());
    }
    
    // Skip migrations for test configuration
    let config = Config::load()?;
    if config.ai_provider == "test" {
        return Ok(());
    }
    
    // Define migrations
    let migrations = vec![
        Migration {
            name: "00_use_single_api_key_and_url",
            function: || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(migration_use_single_api_key_and_url())
            },
        },
        Migration {
            name: "01_set_missing_default_values",
            function: || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(migration_set_missing_default_values())
            },
        },
    ];
    
    // Get completed migrations
    let completed = get_completed_migrations()?;
    
    // Track if we ran any migrations
    let mut ran_migration = false;
    
    // Run migrations that haven't been completed
    for migration in migrations {
        if !completed.contains(&migration.name.to_string()) {
            info!("Applying migration: {}", migration.name);
            
            match (migration.function)() {
                Ok(_) => {
                    info!("Migration applied successfully: {}", migration.name);
                    save_completed_migration(migration.name)?;
                    ran_migration = true;
                }
                Err(e) => {
                    error!("Failed to apply migration {}: {}", migration.name, e);
                    return Err(e);
                }
            }
        }
    }
    
    // If we ran migrations, tell the user
    if ran_migration {
        println!("{}", "✓ Migrations to your config were applied successfully. Please rerun.".green());
        std::process::exit(0);
    }
    
    Ok(())
}