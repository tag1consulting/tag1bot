use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use super::provider::{AIProvider, AIRequest, AIResponse};

pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl OllamaProvider {
    pub fn new(base_url: Option<String>, model: String) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            model,
        }
    }
}

#[async_trait]
impl AIProvider for OllamaProvider {
    async fn send_request(&self, request: &AIRequest) -> Result<AIResponse, Box<dyn Error>> {
        let url = format!("{}/api/generate", self.base_url);

        let options = if request.temperature.is_some() || request.max_tokens.is_some() {
            Some(OllamaOptions {
                temperature: request.temperature,
                num_predict: request.max_tokens,
            })
        } else {
            None
        };

        let body = OllamaRequest {
            model: self.model.clone(),
            prompt: request.prompt.clone(),
            stream: false,
            options,
        };

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?;

        let data: OllamaResponse = response.json().await?;

        Ok(AIResponse {
            content: data.response,
            provider: self.name().to_string(),
            model: self.model.clone(),
        })
    }

    fn name(&self) -> &str {
        "Ollama"
    }
}