use crate::error::{Error, Result};
use crate::cli::ConfigAction;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use dirs::home_dir;
use colored::Colorize;
use log::{info, error};

// Define configuration keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigKey {
    OcoApiKey,
    OcoTokensMaxInput,
    OcoTokensMaxOutput,
    OcoDescription,
    OcoEmoji,
    OcoModel,
    OcoLanguage,
    OcoMessageTemplateplaceholder,
    OcoPromptModule,
    OcoAiProvider,
    OcoOneLineCommit,
    OcoApiUrl,
    OcoGitpush,
    OcoWhy,
}

impl FromStr for ConfigKey {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "OCO_API_KEY" => Ok(ConfigKey::OcoApiKey),
            "OCO_TOKENS_MAX_INPUT" => Ok(ConfigKey::OcoTokensMaxInput),
            "OCO_TOKENS_MAX_OUTPUT" => Ok(ConfigKey::OcoTokensMaxOutput),
            "OCO_DESCRIPTION" => Ok(ConfigKey::OcoDescription),
            "OCO_EMOJI" => Ok(ConfigKey::OcoEmoji),
            "OCO_MODEL" => Ok(ConfigKey::OcoModel),
            "OCO_LANGUAGE" => Ok(ConfigKey::OcoLanguage),
            "OCO_MESSAGE_TEMPLATE_PLACEHOLDER" => Ok(ConfigKey::OcoMessageTemplateplaceholder),
            "OCO_PROMPT_MODULE" => Ok(ConfigKey::OcoPromptModule),
            "OCO_AI_PROVIDER" => Ok(ConfigKey::OcoAiProvider),
            "OCO_ONE_LINE_COMMIT" => Ok(ConfigKey::OcoOneLineCommit),
            "OCO_API_URL" => Ok(ConfigKey::OcoApiUrl),
            "OCO_GITPUSH" => Ok(ConfigKey::OcoGitpush),
            "OCO_WHY" => Ok(ConfigKey::OcoWhy),
            _ => Err(Error::InvalidConfiguration(format!("Unknown config key: {}", s))),
        }
    }
}

impl ToString for ConfigKey {
    fn to_string(&self) -> String {
        match self {
            ConfigKey::OcoApiKey => "OCO_API_KEY",
            ConfigKey::OcoTokensMaxInput => "OCO_TOKENS_MAX_INPUT",
            ConfigKey::OcoTokensMaxOutput => "OCO_TOKENS_MAX_OUTPUT",
            ConfigKey::OcoDescription => "OCO_DESCRIPTION",
            ConfigKey::OcoEmoji => "OCO_EMOJI",
            ConfigKey::OcoModel => "OCO_MODEL",
            ConfigKey::OcoLanguage => "OCO_LANGUAGE",
            ConfigKey::OcoMessageTemplateplaceholder => "OCO_MESSAGE_TEMPLATE_PLACEHOLDER",
            ConfigKey::OcoPromptModule => "OCO_PROMPT_MODULE",
            ConfigKey::OcoAiProvider => "OCO_AI_PROVIDER",
            ConfigKey::OcoOneLineCommit => "OCO_ONE_LINE_COMMIT",
            ConfigKey::OcoApiUrl => "OCO_API_URL",
            ConfigKey::OcoGitpush => "OCO_GITPUSH",
            ConfigKey::OcoWhy => "OCO_WHY",
        }.to_string()
    }
}

// Enum for AI providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiProvider {
    OpenAi,
    Anthropic,
    Azure,
    Ollama,
    Gemini,
    Flowise,
    Groq,
    Mistral,
    Mlx,
    Deepseek,
    Test,
}

impl FromStr for AiProvider {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(AiProvider::OpenAi),
            "anthropic" => Ok(AiProvider::Anthropic),
            "azure" => Ok(AiProvider::Azure),
            "ollama" => Ok(AiProvider::Ollama),
            "gemini" => Ok(AiProvider::Gemini),
            "flowise" => Ok(AiProvider::Flowise),
            "groq" => Ok(AiProvider::Groq),
            "mistral" => Ok(AiProvider::Mistral),
            "mlx" => Ok(AiProvider::Mlx),
            "deepseek" => Ok(AiProvider::Deepseek),
            "test" => Ok(AiProvider::Test),
            _ => Err(Error::UnsupportedAiProvider(s.to_string())),
        }
    }
}

impl ToString for AiProvider {
    fn to_string(&self) -> String {
        match self {
            AiProvider::OpenAi => "openai",
            AiProvider::Anthropic => "anthropic",
            AiProvider::Azure => "azure",
            AiProvider::Ollama => "ollama",
            AiProvider::Gemini => "gemini",
            AiProvider::Flowise => "flowise",
            AiProvider::Groq => "groq",
            AiProvider::Mistral => "mistral",
            AiProvider::Mlx => "mlx",
            AiProvider::Deepseek => "deepseek",
            AiProvider::Test => "test",
        }.to_string()
    }
}

// Enum for prompt modules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PromptModule {
    ConventionalCommit,
    Commitlint,
}

impl FromStr for PromptModule {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "conventional-commit" => Ok(PromptModule::ConventionalCommit),
            "@commitlint" => Ok(PromptModule::Commitlint),
            _ => Err(Error::InvalidConfiguration(format!("Invalid prompt module: {}", s))),
        }
    }
}

impl ToString for PromptModule {
    fn to_string(&self) -> String {
        match self {
            PromptModule::ConventionalCommit => "conventional-commit",
            PromptModule::Commitlint => "@commitlint",
        }.to_string()
    }
}

// Configuration struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "OCO_API_KEY")]
    pub api_key: Option<String>,
    
    #[serde(rename = "OCO_TOKENS_MAX_INPUT")]
    pub tokens_max_input: usize,
    
    #[serde(rename = "OCO_TOKENS_MAX_OUTPUT")]
    pub tokens_max_output: usize,
    
    #[serde(rename = "OCO_DESCRIPTION")]
    pub description: bool,
    
    #[serde(rename = "OCO_EMOJI")]
    pub emoji: bool,
    
    #[serde(rename = "OCO_MODEL")]
    pub model: String,
    
    #[serde(rename = "OCO_LANGUAGE")]
    pub language: String,
    
    #[serde(rename = "OCO_MESSAGE_TEMPLATE_PLACEHOLDER")]
    pub message_template_placeholder: String,
    
    #[serde(rename = "OCO_PROMPT_MODULE")]
    pub prompt_module: String,
    
    #[serde(rename = "OCO_AI_PROVIDER")]
    pub ai_provider: String,
    
    #[serde(rename = "OCO_ONE_LINE_COMMIT")]
    pub one_line_commit: bool,
    
    #[serde(rename = "OCO_API_URL")]
    pub api_url: Option<String>,
    
    #[serde(rename = "OCO_GITPUSH")]
    pub gitpush: bool,
    
    #[serde(rename = "OCO_WHY")]
    pub why: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: None,
            tokens_max_input: 40960,
            tokens_max_output: 4096,
            description: false,
            emoji: false,
            model: "gpt-4o-mini".to_string(),
            language: "en".to_string(),
            message_template_placeholder: "$msg".to_string(),
            prompt_module: "conventional-commit".to_string(),
            ai_provider: "openai".to_string(),
            one_line_commit: false,
            api_url: None,
            gitpush: true,
            why: false,
        }
    }
}

// Global config instance
static CONFIG: Lazy<Mutex<Option<Config>>> = Lazy::new(|| Mutex::new(None));

impl Config {
    pub fn global_config_path() -> PathBuf {
        home_dir().unwrap_or_default().join(".opencommit")
    }
    
    pub fn local_config_path() -> PathBuf {
        Path::new(".env").to_path_buf()
    }
    
    pub fn load() -> Result<Self> {
        // Try to get cached config
        if let Some(config) = CONFIG.lock().unwrap().clone() {
            return Ok(config);
        }
        
        // Load dotenv if exists
        let _ = dotenv::dotenv();
        
        // Start with default config
        let mut config = Config::default();
        
        // Load global config if exists
        let global_path = Self::global_config_path();
        if global_path.exists() {
            let content = fs::read_to_string(&global_path)?;
            match toml::from_str::<Config>(&content) {
                Ok(global_config) => {
                    // Merge global config into default
                    config = global_config;
                }
                Err(e) => {
                    // Log error but continue with defaults
                    error!("Failed to parse global config: {}", e);
                }
            }
        }
        
        // Override with environment variables (from .env or actual env)
        if let Ok(key) = std::env::var("OCO_API_KEY") {
            config.api_key = Some(key);
        }
        
        if let Ok(val) = std::env::var("OCO_TOKENS_MAX_INPUT") {
            if let Ok(num) = val.parse::<usize>() {
                config.tokens_max_input = num;
            }
        }
        
        if let Ok(val) = std::env::var("OCO_TOKENS_MAX_OUTPUT") {
            if let Ok(num) = val.parse::<usize>() {
                config.tokens_max_output = num;
            }
        }
        
        if let Ok(val) = std::env::var("OCO_DESCRIPTION") {
            if let Ok(b) = val.parse::<bool>() {
                config.description = b;
            }
        }
        
        if let Ok(val) = std::env::var("OCO_EMOJI") {
            if let Ok(b) = val.parse::<bool>() {
                config.emoji = b;
            }
        }
        
        if let Ok(val) = std::env::var("OCO_MODEL") {
            config.model = val;
        }
        
        if let Ok(val) = std::env::var("OCO_LANGUAGE") {
            config.language = val;
        }
        
        if let Ok(val) = std::env::var("OCO_MESSAGE_TEMPLATE_PLACEHOLDER") {
            config.message_template_placeholder = val;
        }
        
        if let Ok(val) = std::env::var("OCO_PROMPT_MODULE") {
            config.prompt_module = val;
        }
        
        if let Ok(val) = std::env::var("OCO_AI_PROVIDER") {
            config.ai_provider = val;
        }
        
        if let Ok(val) = std::env::var("OCO_ONE_LINE_COMMIT") {
            if let Ok(b) = val.parse::<bool>() {
                config.one_line_commit = b;
            }
        }
        
        if let Ok(val) = std::env::var("OCO_API_URL") {
            config.api_url = Some(val);
        }
        
        if let Ok(val) = std::env::var("OCO_GITPUSH") {
            if let Ok(b) = val.parse::<bool>() {
                config.gitpush = b;
            }
        }
        
        if let Ok(val) = std::env::var("OCO_WHY") {
            if let Ok(b) = val.parse::<bool>() {
                config.why = b;
            }
        }
        
        // Cache the config
        *CONFIG.lock().unwrap() = Some(config.clone());
        
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        let path = Self::global_config_path();
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Write config to file
        let content = toml::to_string(self)?;
        fs::write(&path, content)?;
        
        // Update cached config
        *CONFIG.lock().unwrap() = Some(self.clone());
        
        Ok(())
    }
    
    // Helper to get default model for a provider
    pub fn default_model_for_provider(provider: &str) -> String {
        match provider.to_lowercase().as_str() {
            "openai" => "gpt-4o-mini".to_string(),
            "anthropic" => "claude-3-5-sonnet-20240620".to_string(),
            "gemini" => "gemini-1.5-flash".to_string(),
            "groq" => "llama3-70b-8192".to_string(), 
            "mistral" => "mistral-small-latest".to_string(),
            "deepseek" => "deepseek-chat".to_string(),
            "ollama" => "mistral".to_string(),
            _ => "gpt-4o-mini".to_string(),
        }
    }
}

// Validators for config values
pub fn validate_config(key: &ConfigKey, value: &str) -> Result<String> {
    match key {
        ConfigKey::OcoApiKey => {
            if value.is_empty() {
                Err(Error::InvalidConfiguration("API key cannot be empty".to_string()))
            } else {
                Ok(value.to_string())
            }
        },
        ConfigKey::OcoTokensMaxInput => {
            match value.parse::<usize>() {
                Ok(_) => Ok(value.to_string()),
                Err(_) => Err(Error::InvalidConfiguration("Tokens max input must be a number".to_string())),
            }
        },
        ConfigKey::OcoTokensMaxOutput => {
            match value.parse::<usize>() {
                Ok(_) => Ok(value.to_string()),
                Err(_) => Err(Error::InvalidConfiguration("Tokens max output must be a number".to_string())),
            }
        },
        ConfigKey::OcoDescription => {
            match value.parse::<bool>() {
                Ok(_) => Ok(value.to_string()),
                Err(_) => Err(Error::InvalidConfiguration("Description must be a boolean".to_string())),
            }
        },
        ConfigKey::OcoEmoji => {
            match value.parse::<bool>() {
                Ok(_) => Ok(value.to_string()),
                Err(_) => Err(Error::InvalidConfiguration("Emoji must be a boolean".to_string())),
            }
        },
        ConfigKey::OcoModel => {
            // We don't validate model names to stay flexible
            Ok(value.to_string())
        },
        ConfigKey::OcoLanguage => {
            // Ideally would validate against available languages
            Ok(value.to_string())
        },
        ConfigKey::OcoMessageTemplateplaceholder => {
            if !value.starts_with('$') {
                Err(Error::InvalidConfiguration("Message template placeholder must start with $".to_string()))
            } else {
                Ok(value.to_string())
            }
        },
        ConfigKey::OcoPromptModule => {
            match value {
                "conventional-commit" | "@commitlint" => Ok(value.to_string()),
                _ => Err(Error::InvalidConfiguration("Prompt module must be 'conventional-commit' or '@commitlint'".to_string())),
            }
        },
        ConfigKey::OcoAiProvider => {
            // Validate provider
            match AiProvider::from_str(value) {
                Ok(_) => Ok(value.to_string()),
                Err(e) => Err(e),
            }
        },
        ConfigKey::OcoOneLineCommit => {
            match value.parse::<bool>() {
                Ok(_) => Ok(value.to_string()),
                Err(_) => Err(Error::InvalidConfiguration("One line commit must be a boolean".to_string())),
            }
        },
        ConfigKey::OcoApiUrl => {
            // Basic URL validation
            if !value.starts_with("http://") && !value.starts_with("https://") {
                Err(Error::InvalidConfiguration("API URL must start with http:// or https://".to_string()))
            } else {
                Ok(value.to_string())
            }
        },
        ConfigKey::OcoGitpush => {
            match value.parse::<bool>() {
                Ok(_) => Ok(value.to_string()),
                Err(_) => Err(Error::InvalidConfiguration("Gitpush must be a boolean".to_string())),
            }
        },
        ConfigKey::OcoWhy => {
            match value.parse::<bool>() {
                Ok(_) => Ok(value.to_string()),
                Err(_) => Err(Error::InvalidConfiguration("Why must be a boolean".to_string())),
            }
        },
    }
}

// Handler for config commands
pub async fn handle_config_command(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Get { keys } => {
            let config = Config::load()?;
            
            for key_str in keys {
                let key = ConfigKey::from_str(&key_str)?;
                let value = match key {
                    ConfigKey::OcoApiKey => config.api_key.unwrap_or_default(),
                    ConfigKey::OcoTokensMaxInput => config.tokens_max_input.to_string(),
                    ConfigKey::OcoTokensMaxOutput => config.tokens_max_output.to_string(),
                    ConfigKey::OcoDescription => config.description.to_string(),
                    ConfigKey::OcoEmoji => config.emoji.to_string(),
                    ConfigKey::OcoModel => config.model,
                    ConfigKey::OcoLanguage => config.language,
                    ConfigKey::OcoMessageTemplateplaceholder => config.message_template_placeholder,
                    ConfigKey::OcoPromptModule => config.prompt_module,
                    ConfigKey::OcoAiProvider => config.ai_provider,
                    ConfigKey::OcoOneLineCommit => config.one_line_commit.to_string(),
                    ConfigKey::OcoApiUrl => config.api_url.unwrap_or_default(),
                    ConfigKey::OcoGitpush => config.gitpush.to_string(),
                    ConfigKey::OcoWhy => config.why.to_string(),
                };
                
                println!("{}={}", key.to_string(), value);
            }
            
            Ok(())
        },
        ConfigAction::Set { key_values } => {
            let mut config = Config::load()?;
            
            for kv in key_values {
                let parts: Vec<&str> = kv.splitn(2, '=').collect();
                if parts.len() != 2 {
                    return Err(Error::InvalidConfiguration(format!("Invalid key-value pair: {}", kv)));
                }
                
                let key = ConfigKey::from_str(parts[0])?;
                let value = validate_config(&key, parts[1])?;
                
                // Update config
                match key {
                    ConfigKey::OcoApiKey => config.api_key = Some(value),
                    ConfigKey::OcoTokensMaxInput => config.tokens_max_input = value.parse().unwrap(),
                    ConfigKey::OcoTokensMaxOutput => config.tokens_max_output = value.parse().unwrap(),
                    ConfigKey::OcoDescription => config.description = value.parse().unwrap(),
                    ConfigKey::OcoEmoji => config.emoji = value.parse().unwrap(),
                    ConfigKey::OcoModel => config.model = value,
                    ConfigKey::OcoLanguage => config.language = value,
                    ConfigKey::OcoMessageTemplateplaceholder => config.message_template_placeholder = value,
                    ConfigKey::OcoPromptModule => config.prompt_module = value,
                    ConfigKey::OcoAiProvider => {
                        config.ai_provider = value;
                        // Update model if needed
                        if config.model.is_empty() || 
                           config.model == "gpt-4o-mini" ||
                           config.model == "claude-3-5-sonnet-20240620" {
                            config.model = Config::default_model_for_provider(&config.ai_provider);
                        }
                    },
                    ConfigKey::OcoOneLineCommit => config.one_line_commit = value.parse().unwrap(),
                    ConfigKey::OcoApiUrl => config.api_url = Some(value),
                    ConfigKey::OcoGitpush => config.gitpush = value.parse().unwrap(),
                    ConfigKey::OcoWhy => config.why = value.parse().unwrap(),
                }
            }
            
            // Save updated config
            config.save()?;
            
            println!("{}", "✓ Config successfully set".green());
            Ok(())
        }
    }
}