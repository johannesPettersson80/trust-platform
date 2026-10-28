//! The temperature teaching example (specification 47 §7.1): a first-order-with-dead-time model
//! shown in heater percent and degrees Celsius. The physical values are an exact, reversible view of
//! the model parameters: resting temperature = output bias − gain × input bias, full-power rise =
//! gain × 100, response time = time constant, delay = dead time.

use serde_json::{json, Map, Value};

use super::js::{self, Num};
use super::tables;

/// The physical values the example shows, from the draft's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Values {
    pub(super) resting_temperature: f64,
    pub(super) full_power_rise: f64,
    pub(super) time_constant_s: f64,
    pub(super) dead_time_s: f64,
}

/// A parameter as `Number(value)` reads it: absent `NaN`, a non-number `NaN`.
fn number(parameters: &Map<String, Value>, key: &str) -> f64 {
    js::field(parameters, key).finite().unwrap_or(f64::NAN)
}

/// A parameter as `Number(value ?? fallback)` reads it: absent is the fallback.
fn number_or(parameters: &Map<String, Value>, key: &str, fallback: f64) -> f64 {
    match js::field(parameters, key) {
        Num::Absent => fallback,
        Num::NotFinite => f64::NAN,
        Num::Finite(value) => value,
    }
}

pub(super) fn values(draft: &Map<String, Value>) -> Values {
    let parameters = js::object(draft, "parameters");
    let gain = number(parameters, "gain");
    Values {
        resting_temperature: number_or(parameters, "output_bias", 0.0)
            - gain * number_or(parameters, "input_bias", 0.0),
        full_power_rise: gain * 100.0,
        time_constant_s: number(parameters, "time_constant_s"),
        dead_time_s: number(parameters, "dead_time_s"),
    }
}

/// The draft with the example applied (after the user confirmed the listed replacements): the
/// example's model, parameters, starting state and heater limits, the identity conversion for each
/// existing port and an initial input of 0; identity, bindings, effects and events are kept.
pub(super) fn apply(draft: &Map<String, Value>) -> Map<String, Value> {
    let (model, parameters, initial_state, actuator) =
        tables::proposal(tables::TEMPERATURE_EXAMPLE)
            .expect("the temperature example is a proposal");
    let commands = js::strings(draft, "commandSources").len();
    let feedbacks = js::strings(draft, "feedbackTargets").len();
    let filled = |count: usize, value: i64| Value::Array(vec![json!(value); count]);
    let mut next = draft.clone();
    next.insert("model".into(), json!(model));
    next.insert("parameters".into(), Value::Object(parameters));
    next.insert("initialState".into(), Value::Array(initial_state));
    next.insert(
        "actuator".into(),
        Value::Object(actuator.unwrap_or_default()),
    );
    next.insert("startingPoint".into(), json!(tables::TEMPERATURE_EXAMPLE));
    next.remove(UNITS_CONFIRMED);
    next.insert("modelValuesTouched".into(), json!(true));
    next.insert("dirty".into(), json!(true));
    next.insert("initialInput".into(), filled(commands, 0));
    next.insert("inputScale".into(), filled(commands, 1));
    next.insert("inputOffset".into(), filled(commands, 0));
    next.insert("outputScale".into(), filled(feedbacks, 1));
    next.insert("outputOffset".into(), filled(feedbacks, 0));
    next
}

/// The physical values the example shows, each with its label and help.
pub(super) const FIELDS: [(&str, &str, &str); 4] = [
    (
        "resting_temperature",
        "Resting temperature (°C)",
        "The temperature approached with heater power at 0%.",
    ),
    (
        "full_power_rise",
        "Temperature rise at full power (°C)",
        "The change above the resting temperature at 100% power. A negative value describes cooling.",
    ),
    (
        "time_constant_s",
        "Response time (s)",
        "After the delay, about 63% of the remaining temperature change occurs in this time.",
    ),
    (
        "dead_time_s",
        "Delay before response (s)",
        "How long a new heater command takes to begin affecting temperature.",
    ),
];

/// The value the user typed for a physical field, or why it cannot be applied: the text must be a
/// finite number, the response time positive and the delay not negative (the rise may be negative:
/// the example then cools).
pub(super) fn entered(field: &str, text: &str) -> Result<f64, String> {
    let label = FIELDS
        .iter()
        .find(|(key, _, _)| *key == field)
        .map_or(field, |(_, label, _)| label);
    let value = js::number_from_text(text);
    if text.trim().is_empty() || !value.is_finite() {
        return Err(format!("{label} must be a finite number."));
    }
    match field {
        "time_constant_s" if value <= 0.0 => Err("Response time must be greater than 0 s.".into()),
        "dead_time_s" if value < 0.0 => Err("Delay must be 0 s or greater.".into()),
        _ => Ok(value),
    }
}

/// The draft with one physical value changed through the exact inverse mapping: a new resting
/// temperature moves the output bias, a new full-power rise sets the gain to rise / 100 and keeps
/// the resting temperature, and the response time and delay are the time constant and dead time.
pub(super) fn with_value(
    draft: &Map<String, Value>,
    field: &str,
    value: f64,
) -> Map<String, Value> {
    let current = js::object(draft, "parameters");
    let mut parameters = current.clone();
    let input_bias = number_or(current, "input_bias", 0.0);
    match field {
        "resting_temperature" => {
            let output_bias = value + number(current, "gain") * input_bias;
            parameters.insert("output_bias".into(), js::js_number(output_bias));
        }
        "full_power_rise" => {
            let resting = values(draft).resting_temperature;
            let gain = value / 100.0;
            parameters.insert("gain".into(), js::js_number(gain));
            parameters.insert(
                "output_bias".into(),
                js::js_number(resting + gain * input_bias),
            );
        }
        "time_constant_s" | "dead_time_s" => {
            parameters.insert(field.into(), js::js_number(value));
        }
        _ => return draft.clone(),
    }
    let mut next = draft.clone();
    next.insert("parameters".into(), Value::Object(parameters));
    next.insert("modelValuesTouched".into(), json!(true));
    next.insert("dirty".into(), json!(true));
    next
}

/// The draft member that holds the user's confirmation of the example's units: the bindings and
/// conversions it was given for.
const UNITS_CONFIRMED: &str = "temperatureUnitsConfirmed";

/// What the units confirmation covers: the PLC signals and their conversions.
fn confirmed_signals(draft: &Map<String, Value>) -> Value {
    let member = |key: &str| draft.get(key).cloned().unwrap_or(Value::Null);
    json!([
        member("commandSources"),
        member("feedbackTargets"),
        member("inputScale"),
        member("inputOffset"),
        member("outputScale"),
        member("outputOffset"),
    ])
}

/// Whether the user confirmed the example's units for the draft's current signals and conversions
/// (specification 47 §7.1): changing a binding or a conversion asks for the confirmation again.
pub(super) fn units_confirmed(draft: &Map<String, Value>) -> bool {
    draft.get(UNITS_CONFIRMED) == Some(&confirmed_signals(draft))
}

/// The draft with the units confirmation given for its current signals, or withdrawn.
pub(super) fn with_units_confirmed(
    draft: &Map<String, Value>,
    confirmed: bool,
) -> Map<String, Value> {
    let mut next = draft.clone();
    if confirmed {
        next.insert(UNITS_CONFIRMED.into(), confirmed_signals(draft));
    } else {
        next.remove(UNITS_CONFIRMED);
    }
    next
}

/// A number as JavaScript's `String(value)` shows it in the example's sentences.
pub(super) fn shown(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else {
        format!("{value}")
    }
}
