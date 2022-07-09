use async_std::task;
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::thread_rng;
use slack_rust::chat::post_message::{post_message, PostMessageRequest};
use slack_rust::event_api::event::{Event, EventCallbackType};
use slack_rust::http_client::{default_client, SlackWebAPIClient};
use slack_rust::socket::event::{EventsAPI, HelloEvent};
use slack_rust::socket::socket_mode::{ack, EventHandler, SocketMode, Stream};
use std::env;
use std::time::Duration;

mod convert;
mod db;
mod karma;
mod seen;
mod slack;
mod util;

#[macro_use]
extern crate lazy_static;

// Validation token to confirm command request was issued by Slack.
//const TAG1BOT_TOKEN: &str = "wSzWcxIK4FaiJjZ6CwA6zlu7";

// @TODO: Get this on the fly?
const TAG1BOT_USER: &str = "U03HT8ALNF4";

#[async_std::main]
async fn main() {
    env_logger::init();

    let slack_app_token = env::var("SLACK_APP_TOKEN")
        .unwrap_or_else(|_| panic!("slack app token is not set (starts with 'xapp')."));
    let slack_bot_token = env::var("SLACK_BOT_TOKEN")
        .unwrap_or_else(|_| panic!("slack bot token is not set (starts with 'xoxb')."));
    let slack_channel_id =
        env::var("SLACK_CHANNEL_ID").unwrap_or_else(|_| panic!("slack channel id is not set."));

    let enable_currency = if env::var("XE_ACCOUNT_ID").is_ok() && env::var("XE_API_KEY").is_ok() {
        true
    } else {
        log::warn!("XE_ACCOUNT_ID or XE_API_KEY not set, disabling currency conversion.");
        false
    };

    let api_client = default_client();

    // Be sure all required tables and indexes exist.
    db::setup();

    // If currency conversions is enabled, start the alert thread.
    if enable_currency {
        task::spawn(async {
            convert::alert_thread().await;
        });
    }

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
                    channel,
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
                        channel,
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
                    //event_ts,
                    channel,
                    text,
                    thread_ts,
                    ts,
                    user,
                    ..
                } => {
                    if let (Ok(user_object), Ok(channel_object)) = (
                        slack::users_info(&user).await,
                        slack::channels_info(&channel).await,
                    ) {
                        // The latest message received from Slack.
                        let message =
                            slack::Message::new(channel_object, user_object, text, thread_ts, ts);
                        //println!("{:#?}", message);
                        // Process the message for karma.
                        if let Some((reply_thread_ts, reply_message)) =
                            karma::process_message(&message).await
                        {
                            slack::reply_in_thread(
                                socket_mode,
                                &message,
                                reply_thread_ts,
                                reply_message,
                            )
                            .await;
                        }
                        // Process the message for seen.
                        if let Some((reply_thread_ts, reply_message)) =
                            seen::process_message(&message).await
                        {
                            slack::reply_in_thread(
                                socket_mode,
                                &message,
                                reply_thread_ts,
                                reply_message,
                            )
                            .await;
                        }
                        // If enabled, process the message for convert.
                        if env::var("XE_ACCOUNT_ID").is_ok() && env::var("XE_API_KEY").is_ok() {
                            if let Some((reply_thread_ts, reply_message)) =
                                convert::process_message(&message).await
                            {
                                slack::reply_in_thread(
                                    socket_mode,
                                    &message,
                                    reply_thread_ts,
                                    reply_message,
                                )
                                .await;
                            }
                        }
                    }
                }
                _ => {}
            },
        }
    }
}
