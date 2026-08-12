use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::*;
use crate::historian::{
    HistorianConfig, HistorianSample, HistorianService, HistorianValue, RecordingMode,
};

const SOURCE: &str = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;

fn state() -> ControlState {
    crate::control::tests::hmi_test_state(SOURCE)
}

fn temp_history_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-status-contract-{name}-{}-{stamp}.jsonl",
        std::process::id()
    ))
}

fn historian_state(sample_count: usize) -> ControlState {
    let mut state = state();
    let path = temp_history_path("history");
    if sample_count > 0 {
        let body = (1..=sample_count)
            .map(|index| {
                serde_json::to_string(&HistorianSample {
                    timestamp_ms: index as u128,
                    source_time_ns: index as i64,
                    variable: "Main.run".to_string(),
                    value: HistorianValue::Bool(index % 2 == 0),
                })
                .expect("serialize sample")
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, format!("{body}\n")).expect("write historian fixture");
    }
    state.historian = Some(
        HistorianService::new(
            HistorianConfig {
                enabled: true,
                sample_interval_ms: 1,
                mode: RecordingMode::All,
                include: Vec::new(),
                history_path: path,
                max_entries: 10_000,
                prometheus_enabled: false,
                prometheus_path: "/metrics".into(),
                alerts: Vec::new(),
            },
            None,
        )
        .expect("historian service"),
    );
    state
}

fn result(response: ControlResponse) -> Value {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn assert_invalid(response: ControlResponse) {
    assert!(!response.ok, "invalid parameters must reject");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("invalid params")),
        "missing invalid-params diagnostic: {:?}",
        response.error
    );
}

fn item_timestamps(value: &Value) -> Vec<u128> {
    value["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| {
            item["timestamp_ms"]
                .as_u64()
                .map(u128::from)
                .expect("timestamp")
        })
        .collect()
}

#[test]
fn historian_query_reports_disabled_before_parsing() {
    let response = handle_historian_query(1, Some(json!({"unknown": true})), &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("historian disabled"));
}

#[test]
fn historian_alerts_reports_disabled_before_parsing() {
    let response = handle_historian_alerts(2, Some(json!({"unknown": true})), &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("historian disabled"));
}

#[test]
fn historian_query_default_limit_is_two_hundred_fifty() {
    let value = result(handle_historian_query(3, None, &historian_state(260)));
    let timestamps = item_timestamps(&value);
    assert_eq!(timestamps.len(), 250);
    assert_eq!(timestamps.first(), Some(&11));
    assert_eq!(timestamps.last(), Some(&260));
}

#[test]
fn historian_query_empty_object_uses_default_limit() {
    let value = result(handle_historian_query(
        4,
        Some(json!({})),
        &historian_state(260),
    ));
    assert_eq!(value["items"].as_array().expect("items").len(), 250);
}

#[test]
fn historian_alerts_default_shape_is_items_array() {
    assert_eq!(
        result(handle_historian_alerts(5, None, &historian_state(0))),
        json!({"items": []})
    );
}

#[test]
fn historian_query_accepts_minimum_limit() {
    let value = result(handle_historian_query(
        6,
        Some(json!({"limit": 1})),
        &historian_state(3),
    ));
    assert_eq!(item_timestamps(&value), [3]);
}

#[test]
fn historian_query_accepts_maximum_limit() {
    let value = result(handle_historian_query(
        7,
        Some(json!({"limit": 5_000})),
        &historian_state(260),
    ));
    assert_eq!(value["items"].as_array().expect("items").len(), 260);
}

#[test]
fn historian_alerts_accepts_minimum_limit() {
    let value = result(handle_historian_alerts(
        8,
        Some(json!({"limit": 1})),
        &historian_state(0),
    ));
    assert_eq!(value, json!({"items": []}));
}

#[test]
fn historian_alerts_accepts_maximum_limit() {
    let value = result(handle_historian_alerts(
        9,
        Some(json!({"limit": 1_000})),
        &historian_state(0),
    ));
    assert_eq!(value, json!({"items": []}));
}

#[test]
fn historian_query_matches_exact_variable_name() {
    let value = result(handle_historian_query(
        10,
        Some(json!({"variable": "Main.run", "limit": 5})),
        &historian_state(3),
    ));
    assert_eq!(item_timestamps(&value), [1, 2, 3]);
}

#[test]
fn historian_query_trims_variable_name_before_matching() {
    let value = result(handle_historian_query(
        11,
        Some(json!({"variable": " \tMain.run\n ", "limit": 5})),
        &historian_state(3),
    ));
    assert_eq!(item_timestamps(&value), [1, 2, 3]);
}

#[test]
fn historian_query_since_is_inclusive() {
    let value = result(handle_historian_query(
        12,
        Some(json!({"since_ms": 2, "limit": 5})),
        &historian_state(3),
    ));
    assert_eq!(item_timestamps(&value), [2, 3]);
}

#[test]
fn historian_query_combines_variable_since_and_limit() {
    let value = result(handle_historian_query(
        13,
        Some(json!({
            "variable": "Main.run",
            "since_ms": 2,
            "limit": 1,
        })),
        &historian_state(3),
    ));
    assert_eq!(item_timestamps(&value), [3]);
}

macro_rules! query_invalid_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid(handle_historian_query(
                100,
                Some($params),
                &historian_state(0),
            ));
        }
    };
}

query_invalid_case!(historian_query_rejects_null_params, Value::Null);
query_invalid_case!(historian_query_rejects_array_params, json!([]));
query_invalid_case!(historian_query_rejects_string_params, json!("query"));
query_invalid_case!(historian_query_rejects_unknown_field, json!({"limti": 10}));
query_invalid_case!(
    historian_query_rejects_limit_with_unknown_sibling,
    json!({"limit": 10, "extra": false})
);
query_invalid_case!(
    historian_query_rejects_null_variable,
    json!({"variable": null})
);
query_invalid_case!(
    historian_query_rejects_non_string_variable,
    json!({"variable": 12})
);
query_invalid_case!(
    historian_query_rejects_empty_variable,
    json!({"variable": ""})
);
query_invalid_case!(
    historian_query_rejects_whitespace_variable,
    json!({"variable": " \t\n "})
);
query_invalid_case!(
    historian_query_rejects_null_since,
    json!({"since_ms": null})
);
query_invalid_case!(
    historian_query_rejects_negative_since,
    json!({"since_ms": -1})
);
query_invalid_case!(
    historian_query_rejects_fractional_since,
    json!({"since_ms": 1.5})
);
query_invalid_case!(
    historian_query_rejects_string_since,
    json!({"since_ms": "1"})
);
query_invalid_case!(historian_query_rejects_null_limit, json!({"limit": null}));
query_invalid_case!(
    historian_query_rejects_boolean_limit,
    json!({"limit": true})
);
query_invalid_case!(
    historian_query_rejects_fractional_limit,
    json!({"limit": 1.5})
);
query_invalid_case!(historian_query_rejects_negative_limit, json!({"limit": -1}));
query_invalid_case!(historian_query_rejects_zero_limit, json!({"limit": 0}));
query_invalid_case!(
    historian_query_rejects_limit_above_maximum,
    json!({"limit": 5_001})
);

macro_rules! alerts_invalid_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid(handle_historian_alerts(
                200,
                Some($params),
                &historian_state(0),
            ));
        }
    };
}

alerts_invalid_case!(historian_alerts_rejects_null_params, Value::Null);
alerts_invalid_case!(historian_alerts_rejects_non_object_params, json!([]));
alerts_invalid_case!(historian_alerts_rejects_unknown_field, json!({"count": 5}));
alerts_invalid_case!(
    historian_alerts_rejects_limit_with_unknown_sibling,
    json!({"limit": 5, "extra": true})
);
alerts_invalid_case!(historian_alerts_rejects_null_limit, json!({"limit": null}));
alerts_invalid_case!(
    historian_alerts_rejects_boolean_limit,
    json!({"limit": false})
);
alerts_invalid_case!(historian_alerts_rejects_string_limit, json!({"limit": "5"}));
alerts_invalid_case!(
    historian_alerts_rejects_fractional_limit,
    json!({"limit": 2.5})
);
alerts_invalid_case!(
    historian_alerts_rejects_negative_limit,
    json!({"limit": -1})
);
alerts_invalid_case!(historian_alerts_rejects_zero_limit, json!({"limit": 0}));
alerts_invalid_case!(
    historian_alerts_rejects_limit_above_maximum,
    json!({"limit": 1_001})
);
