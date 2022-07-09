// Tracks keyword karma.
// For example, `foo++` or `bar--`.

use regex::{Regex, RegexSet};
use rusqlite::params;

use crate::db::DB;
use crate::slack;
use crate::TAG1BOT_USER;

const REGEX_KARMA_WORD: &str = r#"^(?:@|#)??(\w{2,20})(?:\s)*(\+\+|\-\-)$"#;
const REGEX_KARMA_MENTION: &str = r#"^<@(\w{9})>(?:\s)*(\+\+|\-\-)$"#;

// Determine if Karma is being modified in this message. Returns `Some(thread id, message)` if karma
// is modified, returns `None` if not,
pub(crate) async fn process_message(message: &slack::Message) -> Option<(String, String)> {
    if message.user.id != TAG1BOT_USER {
        let trimmed_text = message.text.trim();
        let set = RegexSet::new(&[REGEX_KARMA_MENTION, REGEX_KARMA_WORD])
            .expect("failed to build RegexSet");
        if set.is_match(trimmed_text) {
            // Always reply in a thread: determine if reply is in a new thread or an existing thread.
            let reply_thread_ts = if let Some(thread_ts) = message.thread_ts.as_ref() {
                thread_ts.clone()
            } else {
                message.ts.clone()
            };
            let matches: Vec<_> = set.matches(trimmed_text).into_iter().collect();
            // Matched @MENTION, convert user_id to name (word).
            let (word, adjustment) = if matches[0] == 0 {
                let re =
                    Regex::new(REGEX_KARMA_MENTION).expect("failed to compile REGEX_KARMA_MENTION");
                let cap = re
                    .captures(trimmed_text)
                    .expect("failed to capture REGEX_KARMA_MENTION");
                let word = match slack::users_info(&cap[1]).await {
                    Ok(u) => u.name.to_lowercase(),
                    Err(e) => {
                        println!("unexpected error: {}", e);
                        return None;
                    }
                };
                let adjustment = cap[2].to_string();
                (word, adjustment)
            // Matched WORD.
            } else {
                let re = Regex::new(REGEX_KARMA_WORD).expect("failed to compile REGEX_KARMA_WORD");
                let cap = re
                    .captures(trimmed_text)
                    .expect("failed to capture REGEX_KARMA_WORD");
                let word = cap[1].to_lowercase();
                let adjustment = cap[2].to_string();
                (word, adjustment)
            };

            let reply_message = if adjustment == "++" {
                if message.user.name.to_lowercase() != word {
                    let karma = increment(&word);
                    format!("Karma for `{}` increased to {}.", word, karma)
                } else {
                    let karma = decrement(&word);
                    format!("Karma cannot be incremented for yourself, you have been penalized: Karma for `{}` decreased to {}.", word, karma)
                }
            } else {
                let karma = decrement(&word);
                format!("Karma for `{}` decreased to {}.", word, karma)
            };

            return Some((reply_thread_ts, reply_message));
        }
    }
    None
}

// Increment karma by 1 for given `text`.
pub(crate) fn increment(text: &str) -> i32 {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
    db.execute(
        "UPDATE karma SET counter = counter + 1 WHERE name = ?1",
        params![text],
    )
    .expect("failed to increment karma");
    db.execute(
        "INSERT INTO karma (name, counter) SELECT ?1, 1 WHERE (Select Changes() = 0)",
        params![text],
    )
    .expect("failed to increment karma");
    let mut statement = db
        .prepare("SELECT counter FROM karma WHERE name = :name")
        .expect("failed to prepare SELECT");
    let rows = statement
        .query_map(&[(":name", text)], |row| row.get(0))
        .expect("failed to SELECT");

    let mut values: Vec<i32> = Vec::new();
    for value_result in rows {
        values.push(value_result.expect("failed to extract result"));
    }

    values[0]
}

// Decrement karma by 1 for given `text`.
pub(crate) fn decrement(text: &str) -> i32 {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
    db.execute(
        "UPDATE karma SET counter = counter - 1 WHERE name = ?1",
        params![text],
    )
    .expect("failed to decrement karma");
    db.execute(
        "INSERT INTO karma (name, counter) SELECT ?1, -1 WHERE (Select Changes() = 0)",
        params![text],
    )
    .expect("failed to decrement karma");
    let mut statement = db
        .prepare("SELECT counter FROM karma WHERE name = :name")
        .expect("failed to prepare SELECT");
    let rows = statement
        .query_map(&[(":name", text)], |row| row.get(0))
        .expect("failed to SELECT");

    let mut values: Vec<i32> = Vec::new();
    for value_result in rows {
        values.push(value_result.expect("failed to extract result"));
    }

    values[0]
}
