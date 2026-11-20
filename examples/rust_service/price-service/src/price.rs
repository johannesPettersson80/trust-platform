//! The electricity price, and the heater's setpoint for it.

use std::time::Duration;

/// Below this price, in EUR/MWh, electricity is cheap and the heater stores heat.
pub const CHEAP_BELOW: f64 = 50.0;

/// The setpoint while electricity is cheap, in °C: the tank stores heat for later.
pub const STORE: f32 = 65.0;

/// The setpoint while it is not, in °C.
pub const NORMAL: f32 = 50.0;

/// How long a fetch waits for the source's answer.
const TIMEOUT: Duration = Duration::from_secs(2);

/// The heater's setpoint at `price` EUR/MWh.
#[must_use]
pub fn setpoint_for(price: f64) -> f32 {
    if price < CHEAP_BELOW {
        STORE
    } else {
        NORMAL
    }
}

/// The price in a source's answer, `{"price": 38.2, "unit": "EUR/MWh"}`.
///
/// # Errors
///
/// An answer that is no JSON object with a number `price`.
pub fn parse(body: &str) -> Result<f64, String> {
    let answer: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("the answer is no JSON: {error}"))?;
    answer
        .get("price")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| format!("the answer has no number \"price\": {body}"))
}

/// Fetches the price from `url`.
///
/// # Errors
///
/// A source that does not answer within 2 s, answers with an error, or gives no price.
pub fn fetch(url: &str) -> Result<f64, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();
    let body = agent
        .get(url)
        .call()
        .map_err(|error| error.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    parse(&body)
}

#[cfg(test)]
mod tests {
    use super::{parse, setpoint_for, NORMAL, STORE};

    #[test]
    fn a_price_below_fifty_stores_heat_and_fifty_or_more_does_not() {
        // The README's rule: below 50 EUR/MWh the setpoint is 65 °C, at 50 and above 50 °C.
        // Catches a rule that takes 50 itself as cheap, or that swaps the two setpoints.
        assert_eq!(setpoint_for(38.2), STORE);
        assert_eq!(setpoint_for(49.9), STORE);
        assert_eq!(setpoint_for(50.0), NORMAL);
        assert_eq!(setpoint_for(120.0), NORMAL);
        assert_eq!((STORE, NORMAL), (65.0, 50.0));
    }

    #[test]
    fn the_price_is_the_number_of_the_answers_price() {
        // The stand-in's answer, a whole number, and answers without a price. Catches a parser
        // that takes another field, or that gives a price for an answer without one.
        assert_eq!(parse(r#"{"price": 38.2, "unit": "EUR/MWh"}"#), Ok(38.2));
        assert_eq!(parse(r#"{"price": 120}"#), Ok(120.0));
        assert!(parse(r#"{"unit": "EUR/MWh"}"#).is_err());
        assert!(parse(r#"{"price": "cheap"}"#).is_err());
        assert!(parse("Service Unavailable").is_err());
    }
}
