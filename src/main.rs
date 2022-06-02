use async_std::task;
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::{Regex, RegexSet};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use slack_rust::chat::post_message::{post_message, PostMessageRequest};
use slack_rust::event_api::event::{Event, EventCallbackType};
use slack_rust::http_client::{default_client, SlackWebAPIClient};
use slack_rust::socket::event::{EventsAPI, HelloEvent};
use slack_rust::socket::socket_mode::{ack, EventHandler, SocketMode, Stream};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[macro_use]
extern crate lazy_static;

// Validation token to confirm command request was issued by Slack.
//const TAG1BOT_TOKEN: &str = "wSzWcxIK4FaiJjZ6CwA6zlu7";

// @TODO: Get this on the fly?
const TAG1BOT_USER: &str = "U03HT8ALNF4";

const REGEX_KARMA_PLUS: &str = r"^(\w{1,20})\+\+$";
const REGEX_KARMA_MINUS: &str = r"^(\w{1,20})\-\-$";

const DATABASE_FILE: &str = "./state.sqlite3";

lazy_static! {
    static ref DB: Arc<Mutex<Connection>> = Arc::new(Mutex::new(
        Connection::open(DATABASE_FILE).expect(&format!("failed to open {}", DATABASE_FILE))
    ));
}

#[derive(Deserialize, Serialize)]
struct SlackMessage {
    text: String,
}

#[async_std::main]
async fn main() {
    env_logger::init();

    let slack_app_token = env::var("SLACK_APP_TOKEN")
        .unwrap_or_else(|_| panic!("slack app token is not set (starts with 'xapp')."));
    let slack_bot_token = env::var("SLACK_BOT_TOKEN")
        .unwrap_or_else(|_| panic!("slack bot token is not set (starts with 'xoxb')."));
    let slack_channel_id =
        env::var("SLACK_CHANNEL_ID").unwrap_or_else(|_| panic!("slack channel id is not set."));

    let api_client = default_client();

    // Open database.
    DB.lock()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS karma (
        id              INTEGER PRIMARY KEY,
        name            TEXT NOT NULL,
        counter         INTEGER
            )",
            [],
        )
        .expect("failed to create karma table");
    DB.lock()
        .unwrap()
        .execute("CREATE INDEX IF NOT EXISTS i_name ON karma (name)", [])
        .expect("failed to create index i_name");

    // Restart if the bot crashes.
    loop {
        match SocketMode::new(
            api_client.clone(),
            slack_app_token.clone(),
            slack_bot_token.clone(),
        )
        .option_parameter("SLACK_CHANNEL_ID".to_string(), slack_channel_id.clone())
        .run(&mut Handler)
        .await
        {
            Ok(_) => log::warn!("Socket mode completed"),
            Err(e) => log::warn!("Socket mode run error: {}", e),
        };

        // Wait a few seconds before reconnecting.
        task::sleep(Duration::from_secs(5)).await;
    }
}

pub struct Handler;

fn increment_karma(text: &str) -> i32 {
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

fn decrement_karma(text: &str) -> i32 {
    let db = DB.lock().unwrap();
    db.execute(
        "UPDATE karma SET counter = counter - 1 WHERE name = ?1",
        params![text],
    )
    .expect("failed to increment karma");
    db.execute(
        "INSERT INTO karma (name, counter) SELECT ?1, -1 WHERE (Select Changes() = 0)",
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

fn hello_text() -> String {
    let hellos = [
        "Hi.",
        "Hey.",
        "Hola.",
        "Hello.",
        "Salut.",
        "Eh oh.",
        "Niihau.",
        "Privet.",
        "Bonjour.",
        "Que tal.",
        "What's up?",
        "Ciao.",
        "Buongiorno.",
    ];
    let mut rng = thread_rng();

    hellos.choose(&mut rng).expect("random failure").to_string()
}

#[async_trait]
impl<S> EventHandler<S> for Handler
where
    S: SlackWebAPIClient,
{
    // Connect to Slack server.
    async fn on_connect(&mut self, _socket_mode: &SocketMode<S>) {
        log::warn!("Connecting to Slack in SocketMode...");
    }

    // Receive connections acknowledgement from Slack server.
    async fn on_hello(&mut self, _socket_mode: &SocketMode<S>, event: HelloEvent, _s: &mut Stream) {
        log::warn!("Connected: {:?}", event);
    }

    async fn on_events_api(&mut self, socket_mode: &SocketMode<S>, e: EventsAPI, s: &mut Stream) {
        log::info!("event: {:?}", e);
        ack(&e.envelope_id, s)
            .await
            .expect("socket mode ack error.");

        match e.payload {
            Event::EventCallback(event_callback) => match event_callback.event {
                EventCallbackType::AppMention {
                    //text,
                    //channel,
                    ts,
                    thread_ts,
                    ..
                } => {
                    let (reply_thread_ts, reply_text) = if let Some(thread_ts) = thread_ts {
                        (thread_ts, hello_text())
                    } else {
                        (ts, hello_text())
                    };

                    let request = PostMessageRequest {
                        channel: socket_mode
                            .option_parameter
                            .get("SLACK_CHANNEL_ID")
                            .unwrap()
                            .to_string(),
                        thread_ts: Some(reply_thread_ts),
                        text: Some(reply_text),
                        ..Default::default()
                    };
                    let response =
                        post_message(&socket_mode.api_client, &request, &socket_mode.bot_token)
                            .await
                            .expect("post message api error.");
                    log::info!("post message api response: {:?}", response);
                }
                EventCallbackType::Message {
                    //channel_type,
                    //channel,
                    //event_ts,
                    text,
                    thread_ts,
                    ts,
                    user,
                    ..
                } => {
                    if user != TAG1BOT_USER {
                        let trimmed_text = text.trim();
                        let set = RegexSet::new(&[REGEX_KARMA_PLUS, REGEX_KARMA_MINUS]).unwrap();
                        if set.is_match(trimmed_text) {
                            let matches: Vec<_> = set.matches(trimmed_text).into_iter().collect();
                            let set_match = matches[0];
                            let (reply_thread_ts, reply_text) = if let Some(thread_ts) = thread_ts {
                                if set_match == 0 {
                                    let re = Regex::new(REGEX_KARMA_PLUS).unwrap();
                                    let cap = re.captures(trimmed_text).unwrap();
                                    let text = cap.get(1).map_or("", |m| m.as_str());
                                    //Only run karma if user is not self-incrementing
                                    if user.to_lowercase() != text.to_lowercase() {
                                        let karma = increment_karma(&text.to_lowercase());
                                        (
                                            thread_ts,
                                            format!("Karma for `{}` increased to {}.", text, karma),
                                        )
                                    } else {
                                        let karma = decrement_karma(&text.to_lowercase());
                                        (
                                            thread_ts,
                                            format!("Karma cannot be incremented for yourself, you have been penalized: Karma for `{}` decreased to {}.", text, karma),
                                        )
                                    }
                                } else {
                                    let re = Regex::new(REGEX_KARMA_MINUS).unwrap();
                                    let cap = re.captures(trimmed_text).unwrap();
                                    let text = cap.get(1).map_or("", |m| m.as_str());
                                    let karma = decrement_karma(&text.to_lowercase());
                                    (
                                        thread_ts,
                                        format!("Karma for `{}` decreased to {}.", text, karma),
                                    )
                                }
                            } else {
                                if set_match == 0 {
                                    let re = Regex::new(REGEX_KARMA_PLUS).unwrap();
                                    let cap = re.captures(trimmed_text).unwrap();
                                    let text = cap.get(1).map_or("", |m| m.as_str());
                                    let karma = increment_karma(&text.to_lowercase());
                                    (ts, format!("Karma for `{}` increased to {}.", text, karma))
                                } else {
                                    let re = Regex::new(REGEX_KARMA_MINUS).unwrap();
                                    let cap = re.captures(trimmed_text).unwrap();
                                    let text = cap.get(1).map_or("", |m| m.as_str());
                                    let karma = decrement_karma(&text.to_lowercase());
                                    (ts, format!("Karma for `{}` decreased to {}.", text, karma))
                                }
                            };

                            let request = PostMessageRequest {
                                channel: socket_mode
                                    .option_parameter
                                    .get("SLACK_CHANNEL_ID")
                                    .unwrap()
                                    .to_string(),
                                thread_ts: Some(reply_thread_ts),
                                text: Some(reply_text),
                                ..Default::default()
                            };

                            let response = post_message(
                                &socket_mode.api_client,
                                &request,
                                &socket_mode.bot_token,
                            )
                            .await
                            .expect("post message api error.");
                            log::info!("post message api response: {:?}", response);
                        }
                    }
                }
                _ => {}
            },
        }
    }
}
