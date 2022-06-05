use regex::{Regex, RegexSet};
use rusqlite::params;

use crate::db::DB;
use crate::slack::User;
use crate::TAG1BOT_USER;

const REGEX_KARMA_PLUS: &str = r"^(\w{1,20})\+\+$";
const REGEX_KARMA_MINUS: &str = r"^(\w{1,20})\-\-$";

// Details needed to determine if a message modifies karma and to build a reply.
pub(crate) struct KarmaMessage {
    pub(crate) user: User,
    pub(crate) text: String,
    pub(crate) thread_ts: Option<String>,
    pub(crate) ts: String,
}

// Increment karma by 1 for given `text`.
pub(crate) fn increment(text: &str) -> i32 {
    let db = DB.lock().unwrap();
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
    let db = DB.lock().unwrap();
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

// Determine if Karma is being modified in this message. Returns `Some(thread id, message)` if karma
// is modified, returns `None` if not,
pub(crate) async fn process_message(message: KarmaMessage) -> Option<(String, String)> {
    if message.user.id != TAG1BOT_USER {
        let trimmed_text = message.text.trim();
        let set = RegexSet::new(&[REGEX_KARMA_PLUS, REGEX_KARMA_MINUS]).unwrap();
        if set.is_match(trimmed_text) {
            // Always reply in a thread: determine if reply is in a new thread or an existing thread.
            let reply_thread_ts = if let Some(thread_ts) = message.thread_ts {
                thread_ts
            } else {
                message.ts
            };
            let matches: Vec<_> = set.matches(trimmed_text).into_iter().collect();
            let set_match = matches[0];
            let reply_message = if set_match == 0 {
                let re = Regex::new(REGEX_KARMA_PLUS).unwrap();
                let cap = re.captures(trimmed_text).unwrap();
                let item = cap.get(1).map_or("", |m| m.as_str());
                // Only run karma if user is not self-incrementing.
                if message.user.name.to_lowercase() != item.to_lowercase() {
                    let karma = increment(&item.to_lowercase());
                    format!("Karma for `{}` increased to {}.", item, karma)
                } else {
                    let karma = decrement(&item.to_lowercase());
                    format!("Karma cannot be incremented for yourself, you have been penalized: Karma for `{}` decreased to {}.", item, karma)
                }
            } else {
                let re = Regex::new(REGEX_KARMA_MINUS).unwrap();
                let cap = re.captures(trimmed_text).unwrap();
                let item = cap.get(1).map_or("", |m| m.as_str());
                let karma = decrement(&item.to_lowercase());
                format!("Karma for `{}` decreased to {}.", item, karma)
            };
            return Some((reply_thread_ts, reply_message));
        }
    }
    None
}
