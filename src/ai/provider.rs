use async_trait::async_trait;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct AIRequest {
    pub prompt: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct AIResponse {
    pub content: String,
    pub provider: String,
    pub model: String,
}

#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn send_request(&self, request: &AIRequest) -> Result<AIResponse, Box<dyn Error>>;
    fn name(&self) -> &str;
}