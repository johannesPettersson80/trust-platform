//! What a connection's runtime reports about it (specification 25 §5.5 and §5.13): the connector
//! rows `Connection`, `Health`, `Verification` and `Signals`, and, when reported, its live values,
//! the last time it was seen and its response time. The simulator's own I/O values are the live
//! values of its simulated and loopback I/O, the first twelve as before.

use serde_json::Value;

use super::fields::{scalar, text_of};
use super::row;
use crate::request::IoValue;
use crate::texts::connections as texts;
use crate::texts::graph;
use crate::topology::{ConnectorStatus, Endpoint};
use crate::view::health;
use crate::view::runtimes::format_ms;
use crate::view::types::Row;

/// The most live values a connection shows.
const MAX_VALUES: usize = 12;

/// The connector rows (specification 25 §5.5): the product words of the connector's state,
/// health and discovery confidence, and its signals.
pub(super) fn connector_rows(connector: Option<&ConnectorStatus>) -> Vec<Row> {
    let Some(connector) = connector else {
        return Vec::new();
    };
    let counts = &connector.point_counts;
    vec![
        row(
            texts::ROW_CONNECTION,
            health::connector_state_word(connector.state),
        ),
        row(
            texts::ROW_HEALTH,
            health::connector_health_word(connector.health),
        ),
        row(
            texts::ROW_VERIFICATION,
            health::confidence_word(connector.confidence),
        ),
        row(
            texts::ROW_SIGNALS,
            texts::signals(
                counts.good,
                counts.degraded.saturating_add(counts.unavailable),
            ),
        ),
    ]
}

/// What a connection's runtime reports live.
#[derive(Debug, Clone, Default)]
pub(super) struct Live {
    /// The live values, each with the name of its signal.
    values: Vec<(String, String)>,
    /// How long ago it was last seen.
    pub(super) last_seen: Option<String>,
    /// Its response time.
    pub(super) response_time: Option<String>,
}

impl Live {
    /// What `endpoint` reports, with the simulator's I/O values `io_values` when they are its
    /// own, at the time `now_ms`.
    pub(super) fn of(endpoint: &Endpoint, io_values: &[IoValue], now_ms: u64) -> Self {
        let live = endpoint.live.as_ref();
        let mut values: Vec<(String, String)> = io_values
            .iter()
            .map(|value| {
                let name = value.name.trim();
                let signal = if name.is_empty() {
                    value.address.trim()
                } else {
                    name
                };
                (signal.to_owned(), value.value.trim().to_owned())
            })
            .collect();
        if values.is_empty() {
            values = reported_values(live);
        }
        values.truncate(MAX_VALUES);
        Self {
            values,
            last_seen: last_seen(live, now_ms),
            response_time: response_time(live),
        }
    }

    /// The live rows: each value under the name of its signal, then the last time seen and the
    /// response time.
    pub(super) fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .values
            .iter()
            .map(|(signal, value)| row(signal, value.as_str()))
            .collect();
        rows.extend(
            self.last_seen
                .as_deref()
                .map(|seen| row(texts::ROW_LAST_SEEN, seen)),
        );
        rows.extend(
            self.response_time
                .as_deref()
                .map(|time| row(texts::ROW_RESPONSE_TIME, time)),
        );
        rows
    }

    /// The live values in one line, for the list.
    pub(super) fn values_cell(&self) -> Option<String> {
        (!self.values.is_empty()).then(|| {
            self.values
                .iter()
                .map(|(signal, value)| format!("{signal}: {value}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
    }
}

/// How long ago live evidence was last seen, at the time `now_ms`.
pub(crate) fn last_seen(live: Option<&Value>, now_ms: u64) -> Option<String> {
    live.and_then(|live| live.get("last_seen_ms"))
        .and_then(Value::as_u64)
        .map(|seen| graph::ago(now_ms.saturating_sub(seen) / 1_000))
}

/// The response time in live evidence.
pub(crate) fn response_time(live: Option<&Value>) -> Option<String> {
    live.and_then(|live| live.get("rtt_ms"))
        .and_then(Value::as_f64)
        .filter(|milliseconds| milliseconds.is_finite() && *milliseconds >= 0.0)
        .map(|milliseconds| graph::milliseconds(&format_ms(milliseconds)))
}

/// The values in a connection's live evidence: a single value, the sample of its I/O image, or
/// the values of the points of its client connections.
fn reported_values(live: Option<&Value>) -> Vec<(String, String)> {
    match live.and_then(|live| live.get("value")) {
        Some(Value::Object(object)) => {
            if let Some(sample) = object.get("sample").and_then(Value::as_array) {
                sample.iter().filter_map(sample_value).collect()
            } else if let Some(connections) = object.get("connections").and_then(Value::as_array) {
                connections.iter().flat_map(point_values).collect()
            } else {
                Vec::new()
            }
        }
        Some(Value::String(text)) => vec![(texts::ROW_VALUE.to_owned(), text.trim().to_owned())],
        Some(value @ (Value::Bool(_) | Value::Number(_))) => {
            vec![(texts::ROW_VALUE.to_owned(), text_of(Some(value)))]
        }
        Some(Value::Array(_) | Value::Null) | None => Vec::new(),
    }
}

/// A value of an I/O image's sample: its variable or address and its value or error.
fn sample_value(entry: &Value) -> Option<(String, String)> {
    let name = scalar(entry.get("name"));
    let signal = if name.is_empty() {
        scalar(entry.get("address"))
    } else {
        name
    };
    let value = match entry.get("value") {
        Some(Value::Object(error)) => scalar(error.get("error")),
        other => scalar(other),
    };
    (!signal.is_empty() && !value.is_empty()).then_some((signal, value))
}

/// The values that the points of a client connection report.
fn point_values(connection: &Value) -> Vec<(String, String)> {
    connection
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|point| {
            let name = scalar(point.get("var"));
            let signal = if name.is_empty() {
                scalar(point.get("node_id"))
            } else {
                name
            };
            let value = scalar(point.get("value"));
            (!signal.is_empty() && !value.is_empty()).then_some((signal, value))
        })
        .collect()
}
