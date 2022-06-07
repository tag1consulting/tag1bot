use regex::Regex;
use std::env;

const REGEX_CONVERT: &str =
    r"(?i)^convert (from )?([0-9]*(\.[0-9]*)?( )?){1}([a-z]{3}) (to )?([a-z]{3})$";

const CURRENCY_API: &str = "https://api.currencyapi.com/v3/latest";

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
    let response = match match surf::get(format!(
        "{}?base_currency={}&currencies={}",
        CURRENCY_API, from_currency.to_uppercase(), to_currency.to_uppercase()
    ))
    .header(
        "apikey",
        env::var("CURRENCY_API_KEY").expect("CURRENCY_API_KEY vanished!?"),
    )
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
            ))
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
            ))
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
    let conversion_rate_json = &parsed_response["data"][to_currency.to_uppercase()]["value"];
    let conversion_rate: f32 = match conversion_rate_json.as_f32() {
        Some(c) => c,
        None => return Some((
            reply_thread_ts,
            format!("Sorry, I failed to convert the response ({}) from the ConversionAPI (`json::as_f32` error)", conversion_rate_json)
        )),
    };

    // The API seems to invert the conversion formula when converting to/from BTC.
    let is_btc = from_currency.to_uppercase() == "BTC" || to_currency.to_uppercase() == "BTC";

    // Perform the conversion.
    let rounded_value = if conversion_rate > 0.01 {
        // Round to the nearest 2 decimal points.
        let converted_value = if is_btc {
            amount / conversion_rate * 100.0
        } else {
            amount * conversion_rate * 100.0
        };
        converted_value.round() / 100.0
    } else if conversion_rate > 0.05 {
        // Round to the nearest 5 decimal points.
        let converted_value = if is_btc {
            amount / conversion_rate * 100000.0
        } else {
            amount * conversion_rate * 100000.0
        };
        converted_value.round() / 100000.0
    } else {
        // Don't round.
        if is_btc {
            amount / conversion_rate
        } else {
            amount * conversion_rate
        }
    };
    let reply_message = format!(
        "{} {} is currently {} {}.",
        amount, from_currency, rounded_value, to_currency
    );

    Some((reply_thread_ts, reply_message))
}
