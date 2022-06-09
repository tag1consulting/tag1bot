use async_std::task;
use regex::{Regex, RegexSet};
use rusqlite::params;
use std::{collections::HashMap, env, time::Duration};

use crate::db::DB;
use crate::slack;
use crate::util;

const REGEX_CONVERT: &str =
    r"(?i)^convert (from )?([0-9]*(\.[0-9]*)?( )?){1}([a-z]{3,4}) (to )?([a-z]{3,4})$";
const REGEX_ALERT_GREATER: &str = r"(?i)^alert(?:\s)*(me|all|everyone)?(?:\s)*(?:when|if)?(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})(?:\s)*(?:is)?(?:\s)*(?:greater|greater than|greater then|gt|>|more|more than|more then)(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})$";
const REGEX_ALERT_LESSER: &str = r"(?i)^alert(?:\s)*(me|all|everyone)?(?:\s)*(?:when|if)?(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})(?:\s)*(?:is)?(?:\s)*(?:lesser|less|lesser than|less than|lesser then|less than|lt|<)(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})$";

const CURRENCY_API: &str = "https://xecdapi.xe.com/v1/convert_from.json/";

// Details needed to determine if a message modifies karma and to build a reply.
pub(crate) struct ConvertMessage {
    pub(crate) channel_id: String,
    pub(crate) username: String,
    pub(crate) text: String,
    pub(crate) thread_ts: Option<String>,
    pub(crate) ts: String,
}

#[derive(Debug)]
struct CurrencyAlert {
    id: u32,
    channel: String,
    user: String,
    from_currency: String,
    from_amount: f32,
    comparison: String,
    to_currency: String,
    to_amount: f32,
}

// Check if user is asking for currency conversion.
pub(crate) async fn process_message(message: &ConvertMessage) -> Option<(String, String)> {
    let trimmed_text = message.text.trim();

    // First test if this is a request to convert currency.
    let response_string = currency_convert(trimmed_text).await;

    // If response_string is set, do nothing more.
    let response_string = if response_string.is_some() {
        response_string
    // Otherwise, test if this is a request to set an alert.
    } else {
        currency_alert(message, trimmed_text).await
    };

    // If we have a response thread, return thread and message.
    if let Some(response_string) = response_string {
        let reply_thread_ts = if let Some(thread_ts) = message.thread_ts.as_ref() {
            thread_ts.clone()
        } else {
            message.ts.clone()
        };
        Some((reply_thread_ts, response_string))
    } else {
        None
    }
}

// Determine if this is a request to convert currency.
pub(crate) async fn currency_convert(trimmed_text: &str) -> Option<String> {
    // Check if someone is asking `convert from # FOO to BAR?`.
    let re = Regex::new(REGEX_CONVERT).expect("failed to compile REGEX_CONVERT");
    let (amount, from_currency, to_currency) = if re.is_match(trimmed_text) {
        let cap = re
            .captures(trimmed_text)
            .expect("failed to capture REGEX_CONVERT");
        (
            cap.get(2).map_or("", |m| m.as_str()),
            cap.get(5).map_or("", |m| m.as_str()),
            cap.get(7).map_or("", |m| m.as_str()),
        )
    } else {
        // No conversion command, exit now.
        return None;
    };

    // Convert number string to f32, defaulting to 1.0 if empty or invalid.
    let amount = amount.trim().parse::<f32>().unwrap_or(1.0);

    // Perform the remote currency quote request.
    let value = get_currency_quote(from_currency, to_currency, amount).await;

    if let Ok(value) = value {
        Some(format!(
            "{} {} is currently {} {}.",
            amount,
            from_currency.to_uppercase(),
            value,
            to_currency.to_uppercase()
        ))
    } else if let Err(message) = value {
        // Something went wrong with currency conversion, pass along the message.
        Some(message)
    } else {
        None
    }
}

// Determine if this is a request to set a ccurrency conversion alert.
pub(crate) async fn currency_alert(message: &ConvertMessage, trimmed_text: &str) -> Option<String> {
    let set = RegexSet::new(&[REGEX_ALERT_GREATER, REGEX_ALERT_LESSER])
        .expect("failed to build RegexSet");
    if set.is_match(trimmed_text) {
        let matches: Vec<_> = set.matches(trimmed_text).into_iter().collect();
        let set_match = matches[0];
        let cap = if set_match == 0 {
            let re =
                Regex::new(REGEX_ALERT_GREATER).expect("failed to compile REGEX_ALERT_GREATER");
            re.captures(trimmed_text)
                .expect("failed to capture REGEX_ALERT_GREATER")
        } else {
            let re = Regex::new(REGEX_ALERT_LESSER).expect("failed to compile REGEX_ALERT_LESSER");
            re.captures(trimmed_text)
                .expect("failed to capture REGEX_ALERT_LESSER")
        };
        let who = cap.get(1).map_or("", |m| m.as_str());
        let from_amount = cap.get(2).map_or("", |m| m.as_str());
        let from_currency = cap.get(3).map_or("", |m| m.as_str());
        let to_amount = cap.get(4).map_or("", |m| m.as_str());
        let to_currency = cap.get(5).map_or("", |m| m.as_str());

        let from_amount = from_amount.trim().parse::<f32>().unwrap_or(1.0);
        let to_amount = to_amount.trim().parse::<f32>().unwrap_or(1.0);

        let who = if who.is_empty() || who == "me" {
            " you"
        } else {
            ""
        };

        let comparison = if set_match == 0 { "more" } else { "less" };

        // Before we set an alert, be sure the request isn't already rue.
        let value = get_currency_quote(from_currency, to_currency, from_amount).await;

        // If currency conversion failed, pass the error along and exit.
        if let Err(e) = value {
            return Some(e);
        }

        // Be sure the alert isn't already true.
        if let Ok(value) = value {
            if (comparison == "more" && value > to_amount)
                || (comparison == "less" && value < to_amount)
            {
                {
                    return Some(format!(
                        "Silly, {} {} is already worth {} than {} {} -- it's currently worth {} {}.",
                        from_amount, from_currency, comparison, to_amount, to_currency, value, to_currency
                    ));
                }
            }
        }

        // Add alert to the database.
        let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
        db.execute(
            r#"INSERT INTO currency_alert (channel, user, from_currency, from_amount, comparison, to_currency, to_amount)  VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![message.channel_id, message.username, from_currency, from_amount, comparison, to_currency, to_amount],
        )
        .expect("failed to increment karma");

        Some(format!(
            "I will alert{} when {} {} is worth {} than {} {}.",
            who, from_amount, from_currency, comparison, to_amount, to_currency
        ))
    } else {
        None
    }
}

// Determine if this is a request to set a ccurrency conversion alert.
pub(crate) async fn get_currency_quote(
    from_currency: &str,
    to_currency: &str,
    amount: f32,
) -> Result<f32, String> {
    // Get XE API secrets from the envinroment.
    let id = env::var("XE_ACCOUNT_ID").unwrap_or_else(|_| panic!("XE_ACCOUNT_ID is not set."));
    let key = env::var("XE_API_KEY").unwrap_or_else(|_| panic!("XE_API_KEY is not set."));
    // Make the remote request.
    let response = match match surf::get(format!(
        "{}?from={}&to={}&amount={}&crypto=true",
        CURRENCY_API,
        from_currency.to_uppercase(),
        to_currency.to_uppercase(),
        amount
    ))
    .header("Authorization", util::generate_basic_auth(&id, &key))
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(format!(
                "Sorry, my request to the ConversionAPI failed (`surf::get()`): {}",
                e
            ));
        }
    }
    .body_string()
    .await
    {
        Ok(s) => s,
        Err(e) => {
            return Err(format!(
                "Sorry, my request to the ConversionAPI failed (`surf::body_string()`): {}",
                e
            ));
        }
    };

    // Parse the CurrencyAPI response.
    let parsed_response = match json::parse(&response) {
        Ok(j) => j,
        Err(e) => {
            return Err(format!(
                "Sorry, the response from the ConversionAPI was invalid (`json::parse` error): {}",
                e
            ))
        }
    };

    // Extract the conversion rate from the parsed JSON.
    let converted_json = &parsed_response["to"][0]["mid"];
    let converted: f32 = match converted_json.as_f32() {
        Some(c) => c,
        None => {
            return Err(format!(
                "{} and/or {} unknown, failed to convert.",
                from_currency.to_uppercase(),
                to_currency.to_uppercase()
            ))
        }
    };

    // For values greater than 100.0, round to two decimals.
    if converted > 100.0 {
        let to_round = converted * 100.0;
        Ok(to_round.round() / 100.0)
    // For values greater than 0.1, round to three decimals.
    } else if converted > 0.1 {
        let to_round = converted * 1000.0;
        Ok(to_round.round() / 1000.0)
    // For values greater than 0.000001, round to six decimals.
    } else if converted > 0.000001 {
        let to_round = converted * 1000000.0;
        Ok(to_round.round() / 1000000.0)
    // For very small values, don't round.
    } else {
        Ok(converted)
    }
}

// Wake regularly and process alerts.
pub(crate) async fn alert_thread() {
    loop {
        // Rebuild currency_map each time around to work with the latest quotes.
        let mut currency_map = HashMap::new();
        let alerts = load_alerts();
        for alert in alerts {
            let conversion_pair = format!("{}-{}", alert.from_currency, alert.to_currency);
            if !currency_map.contains_key(&conversion_pair) {
                // Look up the conversion of 1 from_currency to to_currency, using this to locally calculate all alerts for
                // this currency pair with a single lookup.
                let value = get_currency_quote(&alert.from_currency, &alert.to_currency, 1.0).await;
                // If currency conversion failed, throw and error and move on.
                if let Err(e) = value {
                    log::error!("currency lookup error: {}", e);
                // Otherwise store the result to avoid duplicate API requests while processing alerts.
                } else if let Ok(value) = value {
                    currency_map.insert(conversion_pair.clone(), value);
                }
            }

            // This can fail if the lookup failed above.
            match currency_map.get(&conversion_pair) {
                Some(rate) => {
                    let value = rate * alert.from_amount;
                    if (alert.comparison == "more" && value > alert.to_amount)
                        || (alert.comparison == "less" && value < alert.to_amount)
                    {
                        let text = format!(
                            "{} CURRENCY ALERT: {} {} is now worth {} than {} {} -- it's currently worth {} {}.",
                            alert.user,
                            alert.from_amount,
                            alert.from_currency,
                            alert.comparison,
                            alert.to_amount,
                            alert.to_currency,
                            value,
                            alert.to_currency
                        );
                        slack::post_text(&alert.channel, &text).await;
                        delete_alert(alert.id);
                    }
                }
                None => log::error!("failed to process alert: {:#?}", alert),
            }
        }
        let alert_pairs = currency_map.len();
        let sleep_seconds = if alert_pairs <= 5 {
            // Check hourly if there are 5 or fewer API calls to make.
            60 * 60
        } else if alert_pairs <= 10 {
            // Check every other hour if there are 10 or fewer API calls to make.
            60 * 60 * 2
        } else if alert_pairs <= 20 {
            // Check every four hours if there are 20 or fewer API calls to make.
            60 * 60 * 4
        } else if alert_pairs <= 50 {
            // Check every eight hours if there are 50 or fewer API calls to make.
            60 * 60 * 8
        } else if alert_pairs <= 100 {
            // Check twice a day if there are 100 or fewer API calls to make.
            60 * 60 * 12
        } else {
            // Check daily if there more API calls to make, and hope we don't run out.
            60 * 60 * 24
        };
        log::info!("currency alert thread sleeping {} seconds", sleep_seconds);
        task::sleep(Duration::from_secs(sleep_seconds)).await;
    }
}

// Load all alerts from the database.
fn load_alerts() -> Vec<CurrencyAlert> {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
    let mut statement = db
        .prepare(
            "SELECT id, channel, user, from_currency, from_amount, comparison, to_currency, to_amount FROM currency_alert",
        )
        .expect("failed to prepare SELECT");
    let currency_alert_iterator = statement
        .query_map([], |row| {
            Ok(CurrencyAlert {
                id: row.get(0).expect("failed to get id"),
                channel: row.get(1).expect("failed to get channel"),
                user: row.get(2).expect("failed to get user"),
                from_currency: row.get(3).expect("failed to get user"),
                from_amount: row.get(4).expect("failed to get user"),
                comparison: row.get(5).expect("failed to get user"),
                to_currency: row.get(6).expect("failed to get user"),
                to_amount: row.get(7).expect("failed to get user"),
            })
        })
        .expect("failed to select from seen table");

    let mut currency_alerts = Vec::new();
    for currency_alert in currency_alert_iterator {
        currency_alerts.push(currency_alert.expect("failed to load row from currency_alert"));
    }
    currency_alerts
}

// Delete an alert once it has triggered.
fn delete_alert(alert_id: u32) {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
    db.execute(
        r#"DELETE FROM currency_alert WHERE id = ?1"#,
        params![alert_id],
    )
    .expect("failed to delete currency alert");
}
