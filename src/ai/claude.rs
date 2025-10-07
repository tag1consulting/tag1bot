use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use super::provider::{AIProvider, AIRequest, AIResponse};

pub struct ClaudeProvider {
    client: Client,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    messages: Vec<ClaudeMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Deserialize)]
struct ClaudeContent {
    text: String,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: Option<String>) -> Result<Self, Box<dyn Error>> {
        if api_key.is_empty() {
            return Err("Claude API key cannot be empty".into());
        }

        Ok(Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        })
    }
}

#[async_trait]
impl AIProvider for ClaudeProvider {
    async fn send_request(&self, request: &AIRequest) -> Result<AIResponse, Box<dyn Error>> {
        let url = "https://api.anthropic.com/v1/messages";

        let body = ClaudeRequest {
            model: self.model.clone(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
            max_tokens: request.max_tokens.unwrap_or(1024),
            temperature: request.temperature,
        };

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Claude request failed: {}", e);
                e
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            eprintln!("Claude API error ({}): {}", status, error_text);
            return Err(format!("Claude API error ({}): {}", status, error_text).into());
        }

        let data: ClaudeResponse = response.json().await.map_err(|e| {
            eprintln!("Failed to parse Claude response: {}", e);
            e
        })?;

        if data.content.is_empty() {
            eprintln!("Claude response contains no content");
            return Err("Claude response contains no content".into());
        }

        Ok(AIResponse {
            content: data.content[0].text.clone(),
            provider: self.name().to_string(),
            model: self.model.clone(),
        })
    }

    fn name(&self) -> &str {
        "Claude"
    }
}
