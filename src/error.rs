use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    
    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    
    #[error("Config error: {0}")]
    Config(#[from] config::ConfigError),
    
    #[error("Not a git repository")]
    NotGitRepository,
    
    #[error("No staged files")]
    NoStagedFiles,
    
    #[error("No API key configured")]
    NoApiKey,
    
    #[error("Too many tokens: {0}")]
    TooManyTokens(usize),
    
    #[error("Empty commit message")]
    EmptyCommitMessage,
    
    #[error("User cancelled")]
    UserCancelled,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Unsupported AI provider: {0}")]
    UnsupportedAiProvider(String),
    
    #[error("AI provider error: {0}")]
    AiProviderError(String),
    
    #[error("Commitlint error: {0}")]
    CommitlintError(String),
    
    #[error("Hook error: {0}")]
    HookError(String),
    
    #[error("{0}")]
    Generic(String),
}

pub type Result<T> = std::result::Result<T, Error>;