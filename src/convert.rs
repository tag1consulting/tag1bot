use regex::{Regex, RegexSet};
use rusqlite::params;
use std::env;

use crate::db::DB;
use crate::util;

const REGEX_CONVERT: &str =
    r"(?i)^convert (from )?([0-9]*(\.[0-9]*)?( )?){1}([a-z]{3,4}) (to )?([a-z]{3,4})$";
const REGEX_ALERT_GREATER: &str = r"(?i)^alert(?:\s)*(me|all|everyone)?(?:\s)*(?:when|if)?(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})(?:\s)*(?:is)?(?:\s)*(?:greater|greater than|greater then|gt|>|more|more than|more then)(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})$";
const REGEX_ALERT_LESSER: &str = r"(?i)^alert(?:\s)*(me|all|everyone)?(?:\s)*(?:when|if)?(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})(?:\s)*(?:is)?(?:\s)*(?:lesser|less|lesser than|less than|lesser then|less than|lt|<)(?:\s)*([0-9]*(?:\.[0-9]*)?){1}(?:\s)*([a-z]{3,4})$";

const CURRENCY_API: &str = "https://xecdapi.xe.com/v1/convert_from.json/";

// Details needed to determine if a message modifies karma and to build a reply.
pub(crate) struct ConvertMessage {
    pub(crate) text: String,
    pub(crate) thread_ts: Option<String>,
    pub(crate) ts: String,
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

    // Use CurrencyAPI to perform actual conversion.
    let id = env::var("XE_ACCOUNT_ID").unwrap_or_else(|_| panic!("XE_ACCOUNT_ID is not set."));
    let key = env::var("XE_API_KEY").unwrap_or_else(|_| panic!("XE_API_KEY is not set."));
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
            return Some(format!(
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
            return Some(format!(
                "Sorry, my request to the ConversionAPI failed (`surf::body_string()`): {}",
                e
            ));
        }
    };

    // Parse the CurrencyAPI response.
    let parsed_response = match json::parse(&response) {
        Ok(j) => j,
        Err(e) => {
            return Some(format!(
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
            return Some(format!(
                "{} and/or {} unknown, failed to convert.",
                from_currency.to_uppercase(),
                to_currency.to_uppercase()
            ))
        }
    };

    // For values greater than 100.0, round to two decimals.
    let rounded = if converted > 100.0 {
        let to_round = converted * 100.0;
        to_round.round() / 100.0
    // For values greater than 0.1, round to three decimals.
    } else if converted > 0.1 {
        let to_round = converted * 1000.0;
        to_round.round() / 1000.0
    // For values greater than 0.000001, round to six decimals.
    } else if converted > 0.000001 {
        let to_round = converted * 1000000.0;
        to_round.round() / 1000000.0
    // For very small values, don't round.
    } else {
        converted
    };

    Some(format!(
        "{} {} is currently {} {}.",
        amount,
        from_currency.to_uppercase(),
        rounded,
        to_currency.to_uppercase()
    ))
}

// Determine if this is a request to set a ccurrency conversion alert.
pub(crate) async fn currency_alert(
    _message: &ConvertMessage,
    trimmed_text: &str,
) -> Option<String> {
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
        println!("cap: {:?}", cap);
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

        // @TODO: Perform conversion now, don't set alert if it's already true.

        // Add alert to the database.
        let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));
        db.execute(
            r#"INSERT INTO currency_alert (channel, user, from_currency, from_amount, comparison, to_currency, to_amount)  VALUES("", "", ?1, ?2, ?3, ?4, ?5)"#,
            params![from_currency, from_amount, comparison, to_currency, to_amount],
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
