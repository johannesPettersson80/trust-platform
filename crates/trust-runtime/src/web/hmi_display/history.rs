//! A trend's history (specification 39, §5.7 item 1, §5.9 item 1): `GET /api/hmi/history` answers
//! the samples of each path over the trend's span: from the runtime's own trend history, on by
//! default, for a value a trend of the active bundle shows, else from the historian when it is on.
//! `GET /api/hmi/history/export` writes a trend's visible window as CSV from the same samples
//! (decided by the owner on 1 November 2026: the runtime writes the export, as TwinCAT and CODESYS
//! export from their recorders).

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
/// The most pens one export names.
const MAX_PENS: usize = 16;

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
    let mut samples = Map::new();
    for path in paths {
        let points = samples_since(control_state, &path, since_ms)
            .into_iter()
            .map(|(time, value)| json!([time, value]))
            .collect();
        samples.insert(path, Value::Array(points));
    }
    // History is on by default: the trends' values are always kept (§5.9 item 1).
    (200, json!({ "enabled": true, "samples": samples }))
}

/// The samples of `path` from `since_ms` on, oldest first: from the runtime's own trend history
/// for a value a trend of the active bundle shows, else from the historian when it is on.
fn samples_since(control_state: &ControlState, path: &str, since_ms: u64) -> Vec<(u64, f64)> {
    let exchange = crate::control::hmi_ports::hmi_exchange(control_state);
    if let Some(kept) = exchange.trend_samples(path, since_ms) {
        return kept;
    }
    let mut samples: Vec<(u64, f64)> = crate::control::hmi_ports::historian(control_state)
        .filter(|historian| historian.config().enabled)
        .map(|historian| {
            historian
                .query(Some(path), Some(u128::from(since_ms)), MAX_SAMPLES)
                .into_iter()
                .filter_map(|sample| {
                    let value = sample.value.as_f64().filter(|value| value.is_finite())?;
                    let time = u64::try_from(sample.timestamp_ms).unwrap_or(u64::MAX);
                    Some((time, value))
                })
                .collect()
        })
        .unwrap_or_default();
    samples.sort_by_key(|(time, _)| *time);
    samples
}

/// A trend's visible window as CSV (specification 39, §5.9 item 1):
/// `GET /api/hmi/history/export?object=<id>&from=<ms>&to=<ms>` and, for each pen the trend
/// shows, in order, `pen=<name>&path=<path>`. The answer is `{ "name", "csv" }`, or 400 with what
/// is wrong.
pub(super) fn export(url: &str, control_state: &ControlState, now_ms: u64) -> (u16, Value) {
    let object = query_value(url, "object").unwrap_or_default();
    if object.is_empty()
        || !object
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return (
            400,
            json!({ "error": "name the trend: /api/hmi/history/export?object=<id>&from=<ms>&to=<ms>&pen=<name>&path=<path>" }),
        );
    }
    let milliseconds =
        |key: &str| query_value(url, key).and_then(|text| text.trim().parse::<u64>().ok());
    let (Some(from), Some(to)) = (milliseconds("from"), milliseconds("to")) else {
        return (
            400,
            json!({ "error": "from and to are milliseconds since 1970" }),
        );
    };
    if to < from || to - from > MAX_SPAN_S * 1_000 {
        return (
            400,
            json!({ "error": "the window is 0 to 86400 seconds long" }),
        );
    }
    let names = query_values(url, "pen");
    let paths = query_values(url, "path");
    if names.is_empty() || names.len() != paths.len() || names.len() > MAX_PENS {
        return (
            400,
            json!({ "error": "name 1 to 16 pens, each with its path" }),
        );
    }
    // A pen holds its last value from before the window, which the history keeps for a day.
    let since = from.saturating_sub(MAX_SPAN_S * 1_000);
    let pens: Vec<(String, Vec<(u64, f64)>)> = names
        .into_iter()
        .zip(paths)
        .map(|(name, path)| {
            let samples = samples_since(control_state, &path, since);
            (name, samples)
        })
        .collect();
    (
        200,
        json!({ "name": export_name(&object, now_ms), "csv": export_csv(&pens, from, to) }),
    )
}

/// The file name of an export made at `now_ms`: `<object id>-<yyyymmdd>-<hhmmss>.csv` in UTC.
fn export_name(object: &str, now_ms: u64) -> String {
    let digits: String = utc_time(now_ms)
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    match (digits.get(0..8), digits.get(8..14)) {
        (Some(day), Some(time)) => format!("{object}-{day}-{time}.csv"),
        _ => format!("{object}.csv"),
    }
}

/// The CSV of `pens` over `from` to `to`: a `time` column in ISO 8601 UTC and a column per pen, in
/// order; a row for each time a pen has a sample in the window, each pen holding its last value at
/// or before that time, empty before its first.
fn export_csv(pens: &[(String, Vec<(u64, f64)>)], from: u64, to: u64) -> String {
    let mut times: Vec<u64> = pens
        .iter()
        .flat_map(|(_, samples)| samples.iter().map(|(time, _)| *time))
        .filter(|time| (from..=to).contains(time))
        .collect();
    times.sort_unstable();
    times.dedup();
    let mut text = String::from("time");
    for (name, _) in pens {
        text.push(',');
        text.push_str(&csv_field(name));
    }
    text.push('\n');
    for time in times {
        text.push_str(&utc_time(time));
        for (_, samples) in pens {
            text.push(',');
            if let Some((_, value)) = samples.iter().rev().find(|(at, _)| *at <= time) {
                text.push_str(&value.to_string());
            }
        }
        text.push('\n');
    }
    text
}

/// A CSV field: in quotes, its quotes doubled, when it holds a comma, a quote or a line break.
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// `milliseconds` since 1970 as ISO 8601 in UTC with milliseconds, as JavaScript writes it:
/// `2026-11-01T17:00:00.000Z`.
fn utc_time(milliseconds: u64) -> String {
    let Ok(at) =
        time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
    else {
        return String::new();
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
        at.millisecond()
    )
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
