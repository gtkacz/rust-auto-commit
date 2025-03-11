use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::time::Duration;
use crate::error::{Error, Result};
use crate::engine::engine::{AiEngine, EngineConfig, Message};
use crate::utils::token_count::token_count;

#[derive(Debug, Clone)]
pub struct OpenAiEngine {
    config: EngineConfig,
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiChatCompletionRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    top_p: f32,
    max_tokens: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

impl OpenAiEngine {
    pub fn new(config: EngineConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            config,
            client,
        }
    }
    
    fn get_base_url(&self) -> String {
        self.config.base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".to_string())
    }
}

#[async_trait]
impl AiEngine for OpenAiEngine {
    async fn generate_commit_message(&self, messages: Vec<Message>, diff: &str) -> Result<String> {
        // Add diff to the last message
        let mut openai_messages: Vec<OpenAiMessage> = Vec::with_capacity(messages.len() + 1);
        
        // Add system and other messages
        for msg in messages.iter().filter(|m| m.role != "user") {
            openai_messages.push(OpenAiMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }
        
        // Add user message with diff
        openai_messages.push(OpenAiMessage {
            role: "user".to_string(),
            content: diff.to_string(),
        });
        
        // Calculate token count
        let request_tokens = openai_messages.iter()
            .map(|msg| token_count(&msg.content) + 4)
            .sum::<usize>();
            
        if request_tokens > self.config.max_tokens_input - self.config.max_tokens_output {
            return Err(Error::TooManyTokens(request_tokens));
        }
        
        // Prepare request
        let request = OpenAiChatCompletionRequest {
            model: self.config.model.clone(),
            messages: openai_messages,
            temperature: 0.0,
            top_p: 0.1,
            max_tokens: self.config.max_tokens_output,
        };
        
        // Send request
        let response = self.client.post(format!("{}/chat/completions", self.get_base_url()))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request)
            .send()
            .await?;
            
        // Handle errors
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(Error::AiProviderError(format!("OpenAI error: {}", error_text)));
        }
        
        // Parse response
        let response: OpenAiChatCompletionResponse = response.json().await?;
        
        // Get message content
        if response.choices.is_empty() {
            return Err(Error::EmptyCommitMessage);
        }
        
        let message = response.choices[0].message.content.clone();
        
        if message.is_empty() {
            return Err(Error::EmptyCommitMessage);
        }
        
        Ok(message)
    }
}