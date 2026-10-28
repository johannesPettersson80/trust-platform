//! The scenario editor's rules (specification 47 §6.2, §12; specification 46 §3.6, §20.2 of
//! specification 47): event times entered in seconds with millisecond precision, the checks of every
//! included event, the event entries a plant saves, the defaults of a new event and of a new action,
//! and the order that only equal-time events may change.

use serde_json::{json, Map, Value};

use super::issues::{Issue, Target};
use super::js;

/// The largest event time the runtime holds: its elapsed nanoseconds are an `i64` (events.rs).
const MAX_EVENT_MS: u64 = 9_223_372_036_854;

/// An event time entered as decimal seconds, in exact milliseconds: up to 16 whole digits and a
/// fraction of at most three significant digits (trailing zeros do not count), nothing rounded.
/// `None` for anything else, a finer fraction, or a time past the runtime's range.
pub(super) fn milliseconds_from_seconds(text: &str) -> Option<u64> {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty()
        || whole.len() > 16
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let fraction = fraction.trim_end_matches('0');
    if fraction.len() > 3 {
        return None;
    }
    let whole: u64 = whole.parse().ok()?;
    let fraction: u64 = format!("{fraction:0<3}").parse().ok()?;
    let milliseconds = whole.checked_mul(1000)?.checked_add(fraction)?;
    (milliseconds <= MAX_EVENT_MS).then_some(milliseconds)
}

/// An event's due time in milliseconds: from its time text when the user edited it, else its saved
/// `atMs`; `None` when that is not a nonnegative whole number of milliseconds the runtime holds.
pub(super) fn due(event: &Map<String, Value>) -> Option<u64> {
    if let Some(text) = event.get("timeSecondsText").and_then(Value::as_str) {
        return milliseconds_from_seconds(text);
    }
    js::field(event, "atMs")
        .safe_integer()
        .and_then(|value| u64::try_from(value).ok())
}

/// The plant facts the event checks need.
pub(super) struct EventContext {
    pub(super) enabled: bool,
    pub(super) states: usize,
    pub(super) outputs: usize,
    pub(super) scalar_actuator: bool,
    pub(super) parameters: Vec<String>,
}

/// The checks of the included events (specification 47 §12; specification 46 §3.6): the plant is
/// enabled; the time is a nonnegative whole number of milliseconds; a state or sensor event names a
/// state or output channel the model has and a finite value; a parameter event names a parameter of
/// the model and a finite value; an actuator event needs a plant with one command signal.
pub(super) fn checks(draft: &Map<String, Value>, context: &EventContext) -> Vec<Issue> {
    let mut issues = Vec::new();
    for (row, event) in js::rows(draft, "scenarioEvents").iter().enumerate() {
        if !js::flag(event, "enabled", false) {
            continue;
        }
        let mut add = |field: &'static str, message: String| {
            issues.push(
                Issue::new(
                    "invalid_scenario_event",
                    format!("scenario_events[{row}].{field}"),
                    message.clone(),
                    Target::Event(row, field),
                )
                .summarized(format!("Event {}: {message}", row + 1)),
            );
        };
        if !context.enabled {
            add(
                "kind",
                "Enable the plant or remove its scenario events.".into(),
            );
        }
        if due(event).is_none() {
            add(
                "at_ms",
                if event.contains_key("timeSecondsText") {
                    "Enter nonnegative decimal seconds with at most 0.001 s precision; the value must fit the supported time range."
                } else {
                    "Time must be a nonnegative whole number of milliseconds."
                }
                .into(),
            );
        }
        let channel = js::field(event, "channel").safe_integer();
        let finite_value = js::field(event, "value").finite().is_some();
        match js::text(event, "kind") {
            "set_state" | "add_state" => {
                if !channel
                    .is_some_and(|channel| channel >= 0 && (channel as usize) < context.states)
                {
                    add("channel", channel_range("State", context.states));
                }
                if !finite_value {
                    add("value", "Enter a finite event value.".into());
                }
            }
            "set_parameter" => {
                let parameter = js::text(event, "parameter");
                if parameter.is_empty() || !context.parameters.iter().any(|name| name == parameter)
                {
                    add(
                        "parameter",
                        "Choose a parameter owned by the selected plant model.".into(),
                    );
                }
                if !finite_value {
                    add("value", "Enter a finite event value.".into());
                }
            }
            "set_sensor_bias" => {
                if !channel
                    .is_some_and(|channel| channel >= 0 && (channel as usize) < context.outputs)
                {
                    add("channel", channel_range("Sensor", context.outputs));
                }
                if !finite_value {
                    add("value", "Enter a finite event value.".into());
                }
            }
            "stick_actuator" | "release_actuator" => {
                if !context.scalar_actuator {
                    add(
                        "kind",
                        "Actuator fault events require a plant with one command signal.".into(),
                    );
                }
            }
            "reset_plant" => {}
            _ => add("kind", "Choose a supported scenario action.".into()),
        }
    }
    issues
}

fn channel_range(name: &str, count: usize) -> String {
    if count > 0 {
        format!("{name} channel must be between 0 and {}.", count - 1)
    } else {
        format!("{name} channel is unavailable for this model.")
    }
}

/// The event entries a plant saves (specification 47 §20.2): the included events in their order,
/// each with its time, its action and only the channel, parameter and value that action takes.
pub(super) fn persisted(draft: &Map<String, Value>) -> Vec<Value> {
    js::rows(draft, "scenarioEvents")
        .into_iter()
        .filter(|event| js::flag(event, "enabled", false))
        .map(|event| {
            let kind = js::text(&event, "kind");
            let mut entry = Map::new();
            // An edited time is saved as its exact milliseconds; an untouched one as it was read.
            let at_ms = if event.contains_key("timeSecondsText") {
                Some(due(&event).map_or(Value::Null, Value::from))
            } else {
                event.get("atMs").cloned()
            };
            if let Some(at_ms) = at_ms {
                entry.insert("at_ms".into(), at_ms);
            }
            entry.insert("kind".into(), json!(kind));
            let fields: &[&str] = match kind {
                "set_state" | "add_state" | "set_sensor_bias" => &["channel", "value"],
                "set_parameter" => &["parameter", "value"],
                _ => &[],
            };
            for field in fields {
                if let Some(value) = event.get(*field) {
                    entry.insert((*field).to_string(), value.clone());
                }
            }
            Value::Object(entry)
        })
        .collect()
}

/// A new event: included, at 0 s, setting state 0 to 0.
pub(super) fn fresh() -> Map<String, Value> {
    match json!({"enabled": true, "atMs": 0, "kind": "set_state", "channel": 0, "value": 0}) {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// The event with a new action and the fields that action takes: a state or sensor action channel
/// 0 and value 0, a parameter action the model's first parameter and value 0; the time, its text and
/// the Include state are kept.
pub(super) fn with_kind(
    event: &Map<String, Value>,
    kind: &str,
    parameters: &[String],
) -> Map<String, Value> {
    let mut next = Map::new();
    for key in ["enabled", "atMs", "timeSecondsText"] {
        if let Some(value) = event.get(key) {
            next.insert(key.to_string(), value.clone());
        }
    }
    next.insert("kind".into(), json!(kind));
    match kind {
        "set_state" | "add_state" | "set_sensor_bias" => {
            next.insert("channel".into(), json!(0));
            next.insert("value".into(), json!(0));
        }
        "set_parameter" => {
            next.insert(
                "parameter".into(),
                json!(parameters.first().map_or("", String::as_str)),
            );
            next.insert("value".into(), json!(0));
        }
        _ => {}
    }
    next
}

/// Whether event `index` may change places with its neighbour `delta` away: both exist and are due
/// at the same time, since declaration order matters only between equal-time events.
pub(super) fn can_move(events: &[Map<String, Value>], index: usize, delta: isize) -> bool {
    let Some(target) = index.checked_add_signed(delta) else {
        return false;
    };
    match (events.get(index), events.get(target)) {
        (Some(event), Some(other)) => match (due(event), due(other)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        },
        _ => false,
    }
}
