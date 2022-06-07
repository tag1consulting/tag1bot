use regex::Regex;
use std::env;

use crate::util;

const REGEX_CONVERT: &str =
    r"(?i)^convert (from )?([0-9]*(\.[0-9]*)?( )?){1}([a-z]{3,4}) (to )?([a-z]{3,4})$";

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

    // Always reply in a thread: determine if reply is in a new thread or an existing thread.
    let reply_thread_ts = if let Some(thread_ts) = message.thread_ts.as_ref() {
        thread_ts.clone()
    } else {
        message.ts.clone()
    };

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
            return Some((
                reply_thread_ts,
                format!(
                    "Sorry, my request to the ConversionAPI failed (`surf::get()`): {}",
                    e
                ),
            ));
        }
    }
    .body_string()
    .await
    {
        Ok(s) => s,
        Err(e) => {
            return Some((
                reply_thread_ts,
                format!(
                    "Sorry, my request to the ConversionAPI failed (`surf::body_string()`): {}",
                    e
                ),
            ));
        }
    };

    // Parse the CurrencyAPI response.
    let parsed_response = match json::parse(&response) {
        Ok(j) => j,
        Err(e) => {
            return Some((
                reply_thread_ts,
                format!(
                "Sorry, the response from the ConversionAPI was invalid (`json::parse` error): {}",
                e
            ),
            ))
        }
    };

    // Extract the conversion rate from the parsed JSON.
    let converted_json = &parsed_response["to"][0]["mid"];
    let converted: f32 = match converted_json.as_f32() {
        Some(c) => c,
        None => {
            return Some((
                reply_thread_ts,
                format!(
                    "{} and/or {} unknown, failed to convert.",
                    from_currency.to_uppercase(),
                    to_currency.to_uppercase()
                ),
            ))
        }
    };

    let rounded = if converted > 100.0 {
        // For values greater than 100.0, round to two decimals.
        let to_round = converted * 100.0;
        to_round.round() / 100.0
    } else if converted > 0.1 {
        // For values greater than 0.1, round to three decimals.
        let to_round = converted * 1000.0;
        to_round.round() / 1000.0
    } else if converted > 0.000001 {
        // For values greater than 0.000001, round to six decimals.
        let to_round = converted * 1000000.0;
        to_round.round() / 1000000.0
    } else {
        // For very small values, don't round.
        converted
    };

    let reply_message = format!(
        "{} {} is currently {} {}.",
        amount,
        from_currency.to_uppercase(),
        rounded,
        to_currency.to_uppercase()
    );

    Some((reply_thread_ts, reply_message))
}
