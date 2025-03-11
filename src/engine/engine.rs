use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use crate::error::Result;

// Message struct for API requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }
    
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }
    
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

// Configuration for AI engines
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens_output: usize,
    pub max_tokens_input: usize,
    pub base_url: Option<String>,
}

// Trait for AI engines
#[async_trait]
pub trait AiEngine: Send + Sync {
    async fn generate_commit_message(&self, messages: Vec<Message>, diff: &str) -> Result<String>;
}