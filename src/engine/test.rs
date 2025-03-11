use async_trait::async_trait;
use crate::error::Result;
use crate::engine::engine::{AiEngine, Message};

// Test engine for testing without making actual API calls
pub struct TestEngine;

impl TestEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AiEngine for TestEngine {
    async fn generate_commit_message(&self, _messages: Vec<Message>, diff: &str) -> Result<String> {
        // Check what's in the diff to generate an appropriate test message
        if diff.contains("PORT") {
            Ok("fix(server.ts): change port variable case from lowercase port to uppercase PORT".to_string())
        } else if diff.contains("feat") {
            Ok("feat: add new feature".to_string())
        } else {
            Ok("test(mock): test commit message".to_string())
        }
    }
}