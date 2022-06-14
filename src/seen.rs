// Tracks when each user was last seen.
// For example, `seen nnewton?` or `seen peta`.

use regex::Regex;
use rusqlite::params;

use crate::db::DB;
use crate::slack;
use crate::util;

const REGEX_SEEN: &str = r"(?i)^seen (\w{1,42})(?:\?)?$";

// When a user was last seen, and what they said (if in a non-private channel).
#[derive(Debug)]
pub(crate) struct LastSeen {
    user: String,
    channel: String,
    last_said: String,
    last_seen: u32,
    //last_private: u32,
}

// Update last_seen for user posting message, reply if they're asking `seen displayname?`.
pub(crate) async fn process_message(message: &slack::Message) -> Option<(String, String)> {
    let trimmed_text = message.text.trim();

    // Check if someone is asking `seen <foo>?`.
    let re = Regex::new(REGEX_SEEN).expect("failed to compile REGEX_SEEN");
    let seen_request = if re.is_match(trimmed_text) {
        let cap = re
            .captures(trimmed_text)
            .expect("failed to capture REGEX_KARMA_PLUS");
        cap.get(1).map_or("", |m| m.as_str())
    } else {
        ""
    };

    // And if so, get the answer.
    let requested_user_last_seen = if seen_request.is_empty() {
        None
    } else {
        last_seen(seen_request)
    };

    // Either way, record that we're seeing a user message now.
    let current_user_last_seen = last_seen(&message.user.name);
    record_seen(
        message,
        message.channel.is_private,
        current_user_last_seen.is_some(),
    );

    // Prepare a reply, if someone asked `seen <foo>?`.
    let reply_message = if seen_request.is_empty() {
        // Do not send a reply.
        return None;
    } else if let Some(last_seen) = requested_user_last_seen {
        format!(
            "`{}` last seen in <#{}> saying `{}` {}.",
            last_seen.user,
            last_seen.channel,
            last_seen.last_said,
            util::time_ago(last_seen.last_seen as u64, false)
        )
    } else {
        format!("I've never seen `{}`.", seen_request)
    };

    // Always reply in a thread: determine if reply is in a new thread or an existing thread.
    let reply_thread_ts = if let Some(thread_ts) = message.thread_ts.as_ref() {
        thread_ts.to_string()
    } else {
        message.ts.to_string()
    };

    Some((reply_thread_ts, reply_message))
}

// Determine when a given user was last seen.
fn last_seen(user: &str) -> Option<LastSeen> {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
    let mut statement = db
        .prepare(
            //"SELECT user, channel, last_said, last_seen, last_private FROM seen WHERE user = :user",
            "SELECT user, channel, last_said, last_seen FROM seen WHERE user = :user",
        )
        .expect("failed to prepare SELECT");
    let mut seen_iter = statement
        .query_map(&[(":user", &user.to_lowercase())], |row| {
            Ok(LastSeen {
                user: row.get(0).expect("failed to get user"),
                channel: row.get(1).expect("failed to get channel"),
                last_said: row.get(2).expect("failed to get last_said"),
                last_seen: row.get(3).expect("failed to get last_seen"),
                //last_private: row.get(4).expect("failed to get last_private"),
            })
        })
        .expect("failed to select from seen table");

    // Return last_seen if exists.
    if let Some(seen) = seen_iter.next() {
        return Some(seen.unwrap());
    }
    None
}

// Create/update record for last_seen for current user.
fn record_seen(seen_message: &slack::Message, is_private: bool, previously_seen: bool) {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));

    match previously_seen {
        // The user has previously been seen, update their record with their latest message.
        true => {
            if is_private {
                // Only record timestamp if seeing user in a private channel.
                db.execute(
                    "UPDATE seen SET last_private = ?1 WHERE user = ?2",
                    params![util::timestamp_now(), seen_message.user.name.to_lowercase()],
                )
                .expect("failed to update seen");
            } else {
                // Record full information if seeing user in a public channel.
                db.execute(
                    "UPDATE seen SET channel = ?1, last_said = ?2, last_seen = ?3 WHERE user = ?4",
                    params![
                        seen_message.channel.id,
                        seen_message.text,
                        util::timestamp_now(),
                        seen_message.user.name.to_lowercase()
                    ],
                )
                .expect("failed to update seen");
            }
        }
        // The user has not been previously seen, create a new record with their first message.
        false => {
            if is_private {
                // Only record name and timestamp if seeing user in a private channel.
                db.execute(
                r#"INSERT INTO seen (user, last_said, channel, last_seen, last_private) VALUES(?1, "", "", 0, ?2)"#,
                params![
                    seen_message.user.name.to_lowercase(),
                    util::timestamp_now(),
                ],
            )
            .expect("failed to insert into seen");
            } else {
                // Record full information if seeing user in a public channel.
                db.execute(
                    "INSERT INTO seen (user, last_said, channel, last_seen) VALUES(?1, ?2, ?3, ?4)",
                    params![
                        seen_message.user.name.to_lowercase(),
                        seen_message.text,
                        seen_message.channel.id,
                        util::timestamp_now(),
                    ],
                )
                .expect("failed to insert into seen");
            }
        }
    }
}
