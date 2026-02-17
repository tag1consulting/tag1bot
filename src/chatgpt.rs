use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
        ChatCompletionRequestUserMessage, CreateChatCompletionRequestArgs,
    },
    Client,
};
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use std::env;

use crate::db::DB;
use crate::slack;

const REGEX_CHATGPT: &str = r#"(?i)^chatgpt (.*)$"#;

// All messages in a given ChatGPT conversation.
#[derive(Debug)]
pub(crate) struct ChatGPTContext {
    context: String,
}

// ID of ChatGPT thread.
#[derive(Debug)]
pub(crate) struct ChatGPTId {
    id: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct StoredMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ConversationHistory {
    pub history: Vec<StoredMessage>,
}

// Check if user is talking to chatgpt.
pub(crate) async fn process_message(message: &slack::Message) -> Option<(String, String)> {
    let trimmed_text = message.text.trim();

    // Check if someone is saying `chatgpt <foo>`.
    let re = Regex::new(REGEX_CHATGPT).expect("failed to compile REGEX_CHATGPT");
    let chatgpt_request = if re.is_match(trimmed_text) {
        let cap = re
            .captures(trimmed_text)
            .expect("failed to capture REGEX_CHATGPT");
        cap.get(1).map_or("", |m| m.as_str())
    } else {
        return None;
    };

    // Get required chatgpt api_key from environment variable.
    let api_key =
        env::var("CHATGPT_API_KEY").unwrap_or_else(|_| panic!("CHATGPT_API_KEY is not set."));

    // Always reply in a thread: determine if reply is in a new thread or an existing thread.
    let reply_thread_ts = if let Some(thread_ts) = message.thread_ts.as_ref() {
        thread_ts.to_string()
    } else {
        message.ts.to_string()
    };

    // Load context if this message is in a thread.
    let previous_history = if message.thread_ts.is_some() {
        match load_chatgpt_context(&reply_thread_ts).await {
            Some(context_str) => match serde_json::from_str::<ConversationHistory>(&context_str) {
                Ok(c) => c.history,
                Err(e) => {
                    println!("failed to deserialize conversation history: {}", e);
                    return None;
                }
            },
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };

    // Build messages from conversation history.
    let mut messages: Vec<ChatCompletionRequestMessage> = previous_history
        .iter()
        .map(|m| match m.role.as_str() {
            "assistant" => ChatCompletionRequestAssistantMessage::from(m.content.as_str()).into(),
            _ => ChatCompletionRequestUserMessage::from(m.content.as_str()).into(),
        })
        .collect();

    // Add the new user message.
    messages.push(ChatCompletionRequestUserMessage::from(chatgpt_request).into());

    // Create a new OpenAI client.
    let config = OpenAIConfig::new().with_api_key(api_key);
    let client = Client::with_config(config);

    // Build the chat completion request.
    let request = match CreateChatCompletionRequestArgs::default()
        .model("gpt-5.2")
        .messages(messages)
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            println!("failed to build chat completion request: {}", e);
            return None;
        }
    };

    // Send the request and get the response.
    let response = match client.chat().create(request).await {
        Ok(r) => r
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| "No response from model.".to_string()),
        Err(e) => {
            format!(
                "Sorry, something went wrong (complain to @jeremy please): {}",
                e
            )
        }
    };

    // Update conversation history with the new exchange.
    let mut updated_history = previous_history;
    updated_history.push(StoredMessage {
        role: "user".to_string(),
        content: chatgpt_request.to_string(),
    });
    updated_history.push(StoredMessage {
        role: "assistant".to_string(),
        content: response.clone(),
    });

    // Store the conversation context for possible future discussion in the
    // same thread.
    let conversation_history = ConversationHistory {
        history: updated_history,
    };
    store_chatgpt_context(&reply_thread_ts, conversation_history).await;

    println!("ChatGPT response: {}", response);

    Some((reply_thread_ts, response))
}

pub(crate) async fn load_chatgpt_context(thread: &str) -> Option<String> {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
    let mut statement = db
        .prepare("SELECT context FROM chatgpt_context WHERE thread = :thread")
        .expect("failed to prepare SELECT");

    let mut context_iter = statement
        .query_map(&[(":thread", thread)], |row| {
            Ok(ChatGPTContext {
                context: row.get(0).expect("failed to get context"),
            })
        })
        .expect("failed to select from seen table");

    // Return context if exists.
    if let Some(context) = context_iter.next() {
        match context {
            Ok(c) => return Some(c.context),
            Err(e) => {
                println!("failed to load thread from database: {}", e);
                return None;
            }
        }
    }
    None
}

pub(crate) async fn store_chatgpt_context(thread: &str, context: ConversationHistory) {
    // Convert context to String.
    let context_string = serde_json::to_string(&context).unwrap_or("".to_string());

    if !context_string.is_empty() {
        let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));

        let mut statement = db
            .prepare("SELECT id FROM chatgpt_context WHERE thread = :thread")
            .expect("failed to prepare SELECT");
        let mut chatgpt_id_iter = statement
            .query_map(&[(":thread", thread)], |row| {
                Ok(ChatGPTId {
                    id: row.get(0).expect("failed to get context"),
                })
            })
            .expect("failed to select from seen table");

        if let Some(id) = chatgpt_id_iter.next() {
            let id = match id {
                Ok(i) => i.id,
                Err(e) => {
                    println!("failed to load thread id from database: {}", e);
                    return;
                }
            };
            db.execute(
                r#"UPDATE chatgpt_context SET thread = ?1, context = ?2 WHERE id = ?3"#,
                params![thread, context_string, id],
            )
            .expect("failed to insert into chatgpt_context");
        } else {
            db.execute(
                r#"INSERT INTO chatgpt_context (thread, context) VALUES(?1, ?2)"#,
                params![thread, context_string,],
            )
            .expect("failed to insert into chatgpt_context");
        };
    }
}
