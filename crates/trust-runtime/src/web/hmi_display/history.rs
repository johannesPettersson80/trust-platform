//! A trend's history (specification 39, §5.7 item 1, §5.9 item 1): `GET /api/hmi/history` answers
//! the samples of each path over the trend's span: from the runtime's own trend history, on by
//! default, for a value a trend of the active bundle shows, else from the historian when it is on.

use serde_json::{json, Map, Value};

use super::super::web_helpers::{query_value, query_values};
use crate::control::ControlState;

/// The most paths one request names.
const MAX_PATHS: usize = 16;
/// The longest span, a day, and the default.
const MAX_SPAN_S: u64 = 86_400;
const DEFAULT_SPAN_S: u64 = 300;
/// The most samples per path.
const MAX_SAMPLES: usize = 5_000;

/// The answer and its status: 200 with the samples, or 400 with what is wrong.
pub(super) fn history(url: &str, control_state: &ControlState, now_ms: u64) -> (u16, Value) {
    let paths = query_values(url, "path");
    if paths.is_empty() || paths.len() > MAX_PATHS {
        return (
            400,
            json!({ "error": "name 1 to 16 paths: /api/hmi/history?path=<path>&span=<seconds>" }),
        );
    }
    let span = match query_value(url, "span") {
        None => DEFAULT_SPAN_S,
        Some(text) => match text.trim().parse::<u64>() {
            Ok(seconds) if (1..=MAX_SPAN_S).contains(&seconds) => seconds,
            _ => return (400, json!({ "error": "the span is 1 to 86400 seconds" })),
        },
    };
    let since_ms = now_ms.saturating_sub(span * 1_000);
    let since = u128::from(since_ms);
    let exchange = control_state.resource.hmi_exchange();
    let historian = control_state
        .historian
        .as_ref()
        .filter(|historian| historian.config().enabled);
    let mut samples = Map::new();
    for path in paths {
        if let Some(kept) = exchange.trend_samples(&path, since_ms) {
            let points = kept
                .into_iter()
                .map(|(time, value)| json!([time, value]))
                .collect();
            samples.insert(path, Value::Array(points));
            continue;
        }
        let points: Vec<Value> = historian
            .map(|historian| {
                historian
                    .query(Some(path.as_str()), Some(since), MAX_SAMPLES)
                    .into_iter()
                    .filter_map(|sample| {
                        let value = sample.value.as_f64().filter(|value| value.is_finite())?;
                        let time = u64::try_from(sample.timestamp_ms).unwrap_or(u64::MAX);
                        Some(json!([time, value]))
                    })
                    .collect()
            })
            .unwrap_or_default();
        samples.insert(path, Value::Array(points));
    }
    // History is on by default: the trends' values are always kept (§5.9 item 1).
    (200, json!({ "enabled": true, "samples": samples }))
}
