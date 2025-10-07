use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use super::provider::{AIProvider, AIRequest, AIResponse};

pub struct ChatGPTProvider {
    client: Client,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct ChatGPTRequest {
    model: String,
    messages: Vec<ChatGPTMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize, Deserialize)]
struct ChatGPTMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatGPTResponse {
    choices: Vec<ChatGPTChoice>,
}

#[derive(Deserialize)]
struct ChatGPTChoice {
    message: ChatGPTMessage,
}

impl ChatGPTProvider {
    pub fn new(api_key: String, model: Option<String>) -> Result<Self, Box<dyn Error>> {
        if api_key.is_empty() {
            return Err("ChatGPT API key cannot be empty".into());
        }

        Ok(Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
        })
    }
}

#[async_trait]
impl AIProvider for ChatGPTProvider {
    async fn send_request(&self, request: &AIRequest) -> Result<AIResponse, Box<dyn Error>> {
        let url = "https://api.openai.com/v1/chat/completions";

        let body = ChatGPTRequest {
            model: self.model.clone(),
            messages: vec![ChatGPTMessage {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
            max_tokens: request.max_tokens,
            temperature: request.temperature,
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                eprintln!("ChatGPT request failed: {}", e);
                e
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            eprintln!("ChatGPT API error ({}): {}", status, error_text);
            return Err(format!("ChatGPT API error ({}): {}", status, error_text).into());
        }

        let data: ChatGPTResponse = response.json().await.map_err(|e| {
            eprintln!("Failed to parse ChatGPT response: {}", e);
            e
        })?;

        if data.choices.is_empty() {
            eprintln!("ChatGPT response contains no choices");
            return Err("ChatGPT response contains no choices".into());
        }

        Ok(AIResponse {
            content: data.choices[0].message.content.clone(),
            provider: self.name().to_string(),
            model: self.model.clone(),
        })
    }

    fn name(&self) -> &str {
        "ChatGPT"
    }
}