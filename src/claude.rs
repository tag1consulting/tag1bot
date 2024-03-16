use clust::messages::ClaudeModel;
use clust::messages::MaxTokens;
use clust::messages::Message;
use clust::messages::MessagesRequestBody;
use clust::messages::SystemPrompt;
use clust::Client;

use regex::Regex;
use serde::{Deserialize, Serialize};

use std::env;

use crate::db::DB;
use crate::slack;

const REGEX_CLAUDE: &str = r#"(?i)^claude (.*)$"#;

// All messages in a given Claude conversation.
#[derive(Debug)]
pub(crate) struct ClaudeContext {
    context: String,
}

// ID of Claude thread.
#[derive(Debug)]
pub(crate) struct ClaudeId {
    id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ConversationHistory {
    pub history: Vec<ChatMessage>,
}

// Check if user is talking to Claude.
pub(crate) async fn process_message(message: &slack::Message) -> Option<(String, String)> {
    let trimmed_text = message.text.trim();

    // Check if someone is saying `claude <foo>`.
    let re = Regex::new(REGEX_CLAUDE).expect("failed to compile REGEX_CLAUDE");
    let claude_request = if re.is_match(trimmed_text) {
        let cap = re
            .captures(trimmed_text)
            .expect("failed to capture REGEX_CLAUDE");
        cap.get(1).map_or("", |m| m.as_str())
    } else {
        return None;
    };

    // Get required chatgpt api_key from environment variable.
    let api_key =
        env::var("CLAUDE_API_KEY").unwrap_or_else(|_| panic!("CLAUDE_API_KEY is not set."));

    // Always reply in a thread: determine if reply is in a new thread or an existing thread.
    let reply_thread_ts = if let Some(thread_ts) = message.thread_ts.as_ref() {
        thread_ts.to_string()
    } else {
        message.ts.to_string()
    };

    // Load context if this message is in a thread.
    let chatgpt_context = if message.thread_ts.is_some() {
        load_claude_context(&reply_thread_ts).await
    } else {
        None
    };

    // Create a new Claude client.
    let client = Client::from_api_key(api_key);

    // Use conversation if existing, or start a new conversation.
    let mut conversation = if let Some(context) = claude_context {
        let conversation_history: ConversationHistory = match serde_json::from_str(&context) {
            Ok(c) => c,
            Err(e) => {
                println!("failed to deserialize converation history: {}", e);
                return None;
            }
        };
        // @TODO: CONVERT TO CLAUDE
        Conversation::new_with_history(client, conversation_history.history)
    } else {
        client.new_conversation()
    };

    // Sending a message and getting the response.
    let response = match conversation.send_message(claude_request).await {
        Ok(r) => r.message().content.to_string(),
        Err(e) => {
            format!(
                "Sorry, something went wrong (complain to @jeremy please): {}",
                e
            )
        }
    };

    // Store the conversation context for possible future discussion in the
    // same thread.
    let conversation_history = ConversationHistory {
        history: conversation.history,
    };
    store_claude_context(&reply_thread_ts, conversation_history).await;

    println!("Claude response: {}", response);

    Some((reply_thread_ts, response))
}

pub(crate) async fn load_claude_context(thread: &str) -> Option<String> {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
    let mut statement = db
        .prepare("SELECT context FROM claude_context WHERE thread = :thread")
        .expect("failed to prepare SELECT");

    let mut context_iter = statement
        .query_map(&[(":thread", thread)], |row| {
            Ok(ClaudeContext {
                context: row.get(0).expect("failed to get context"),
            })
        })
        .expect("failed to select from claude_context table");

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

pub(crate) async fn store_claude_context(thread: &str, context: ConversationHistory) {
    // Convert context to String.
    let context_string = serde_json::to_string(&context).unwrap_or("".to_string());

    if !context_string.is_empty() {
        let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));

        let mut statement = db
            .prepare("SELECT id FROM claude_context WHERE thread = :thread")
            .expect("failed to prepare SELECT");
        let mut claude_id_iter = statement
            .query_map(&[(":thread", thread)], |row| {
                Ok(ClaudeId {
                    id: row.get(0).expect("failed to get context"),
                })
            })
            .expect("failed to select from claude_context table");

        if let Some(id) = claude_id_iter.next() {
            let id = match id {
                Ok(i) => i.id,
                Err(e) => {
                    println!("failed to load thread id from database: {}", e);
                    return;
                }
            };
            db.execute(
                r#"UPDATE claude_context SET thread = ?1, context = ?2 WHERE id = ?3"#,
                params![thread, context_string, id],
            )
            .expect("failed to insert into claude_context");
        } else {
            db.execute(
                r#"INSERT INTO claude_context (thread, context) VALUES(?1, ?2)"#,
                params![thread, context_string,],
            )
            .expect("failed to insert into claude_context");
        };
    }
}
