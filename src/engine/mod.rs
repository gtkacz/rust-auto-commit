pub mod engine;
pub mod openai;
pub mod anthropic;
pub mod azure;
pub mod ollama;
pub mod gemini;
pub mod flowise;
pub mod groq;
pub mod mistral;
pub mod mlx;
pub mod deepseek;
pub mod test;

use crate::error::{Error, Result};
use crate::commands::config::{Config, AiProvider};
use crate::engine::engine::{AiEngine, EngineConfig, Message};

// Get the appropriate AI engine based on configuration
pub fn get_engine(config: &Config) -> Result<Box<dyn AiEngine>> {
    let provider = AiProvider::from_str(&config.ai_provider)?;
    
    let engine_config = EngineConfig {
        model: config.model.clone(),
        max_tokens_output: config.tokens_max_output,
        max_tokens_input: config.tokens_max_input,
        api_key: config.api_key.clone().unwrap_or_default(),
        base_url: config.api_url.clone(),
    };
    
    match provider {
        AiProvider::OpenAi => Ok(Box::new(openai::OpenAiEngine::new(engine_config))),
        AiProvider::Anthropic => Ok(Box::new(anthropic::AnthropicEngine::new(engine_config))),
        AiProvider::Azure => Ok(Box::new(azure::AzureEngine::new(engine_config))),
        AiProvider::Ollama => Ok(Box::new(ollama::OllamaEngine::new(engine_config))),
        AiProvider::Gemini => Ok(Box::new(gemini::GeminiEngine::new(engine_config))),
        AiProvider::Flowise => Ok(Box::new(flowise::FlowiseEngine::new(engine_config))),
        AiProvider::Groq => Ok(Box::new(groq::GroqEngine::new(engine_config))),
        AiProvider::Mistral => Ok(Box::new(mistral::MistralEngine::new(engine_config))),
        AiProvider::Mlx => Ok(Box::new(mlx::MlxEngine::new(engine_config))),
        AiProvider::Deepseek => Ok(Box::new(deepseek::DeepseekEngine::new(engine_config))),
        AiProvider::Test => Ok(Box::new(test::TestEngine::new())),
    }
}