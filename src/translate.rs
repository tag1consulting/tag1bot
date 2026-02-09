use regex::Regex;
use std::collections::HashMap;
use crate::slack;
use lazy_static::lazy_static;
use crate::ai::{AIRequest, ChatGPTProvider, ClaudeProvider, OllamaProvider, AIProvider};

enum ProviderType {
    ChatGPT,
    Claude,
    Ollama,
}

fn create_provider(provider_type: ProviderType) -> Result<Box<dyn AIProvider>, Box<dyn std::error::Error>> {
    match provider_type {
        ProviderType::ChatGPT => {
            let api_key = std::env::var("OPENAI_API_KEY")?;
            let provider = ChatGPTProvider::new(api_key, None)?;
            Ok(Box::new(provider))
        },
        ProviderType::Claude => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")?;
            let provider = ClaudeProvider::new(api_key, None)?;
            Ok(Box::new(provider))
        },
        ProviderType::Ollama => {
            let provider = OllamaProvider::new(None, "llama2".to_string());
            Ok(Box::new(provider))
        },
    }
}

const REGEX_TRANSLATE: &str = r"(?i)^translate(?: to ([a-z]+))?(?: tone ([a-z]+))?\s+(.+)$";

const PROMPT_FILE: &str = "prompts/translate.md";

pub(crate) async fn process_message(message: &slack::Message) -> Option<(String, String)> {
    let trimmed_text = message.text.trim();

    // Check if this is a translate command
    let re = Regex::new(REGEX_TRANSLATE).expect("failed to compile REGEX_TRANSLATE");

    if !re.is_match(trimmed_text) {
        return None;
    }

    // Extract language, tone, and text
    let cap = re.captures(trimmed_text).expect("failed to capture");

    // Group 1 = language (optional)
    let target_language = cap.get(1).map_or("english", |m| m.as_str());

    // Group 2 = tone (optional)
    let tone = cap.get(2).map_or("neutral", |m| m.as_str());

    // Group 3 = message (required, always last)
    let text_to_translate = cap.get(3).map_or("", |m| m.as_str());

    // Load and process the prompt
    let prompt = match load_and_fill_prompt(text_to_translate, target_language, tone) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to load prompt: {}", e);
            return Some((
                message.ts.clone(),
                format!("Error loading translation prompt: {}", e)
            ));
        }
    };

    // Debug: log the filled prompt
    // log::debug!("Translation prompt:\n{}", prompt);

    let provider = create_provider(ProviderType::ChatGPT).ok()?;

    let request = AIRequest {
        prompt,
        max_tokens: Some(1000),
        temperature: Some(0.7),
    };

    let response = provider.send_request(&request).await.ok()?;

    // Determine reply thread
    let reply_thread_ts = if let Some(thread_ts) = message.thread_ts.as_ref() {
        thread_ts.clone()
    } else {
        message.ts.clone()
    };

    Some((reply_thread_ts, response.content))
}

lazy_static! {
    static ref PROMPT_TEMPLATE: String = {
        std::fs::read_to_string(PROMPT_FILE)
            .unwrap_or_else(|e| {
                log::error!("Failed to load {}: {}", PROMPT_FILE, e);
                panic!("Cannot start without prompt file");
            })
    };
}


/// Load the markdown prompt file and replace variables
fn load_and_fill_prompt(message: &str, language: &str, tone: &str) -> Result<String, String> {
    // Use the pre-loaded template
    let template = PROMPT_TEMPLATE.as_str();

    // Create replacements map
    let mut replacements = HashMap::new();
    replacements.insert("message", message);
    replacements.insert("language", language);
    replacements.insert("tone", tone);

    // Regex to match {{variable}} with optional spaces
    let re = Regex::new(r"\{\{\s*(\w+)\s*\}\}").expect("Invalid regex");

    // Replace all matches
    let filled = re.replace_all(&template, |caps: &regex::Captures| {
        let var_name = &caps[1];
        replacements.get(var_name).unwrap_or(&"").to_string()
    });

    Ok(filled.to_string())
}