use std::sync::{Arc, Mutex};

use serde_json::{json, Value as JsonValue};

use super::*;
use crate::value::Value;

const SOURCE: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

fn state() -> ControlState {
    crate::control::tests::hmi_test_state(SOURCE)
}

fn result(response: ControlResponse) -> JsonValue {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn assert_invalid_params(response: ControlResponse) {
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

fn assert_address_rejected(response: ControlResponse) {
    assert!(!response.ok, "invalid address must reject");
    assert!(
        response.result.is_none(),
        "rejected address must not return a result"
    );
}

fn assert_unsupported_value(response: ControlResponse) {
    assert!(!response.ok, "unsupported value must reject");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unsupported value")),
        "missing unsupported-value diagnostic: {:?}",
        response.error
    );
}

fn poison<T>(mutex: &Arc<Mutex<T>>) {
    let mutex = Arc::clone(mutex);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = mutex.lock().expect("lock before poison");
        panic!("intentional contract-test poison");
    }));
    assert!(result.is_err(), "poison setup must panic");
}

fn write_params(address: &str, value: &str) -> Option<JsonValue> {
    Some(json!({"address": address, "value": value}))
}

fn unforce_params(address: &str) -> Option<JsonValue> {
    Some(json!({"address": address}))
}

#[test]
fn io_list_distinguishes_legitimate_missing_snapshot() {
    let response = handle_io_list(1, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("no snapshot available"));
}

#[test]
fn io_read_exposes_legitimate_missing_snapshot_as_null() {
    assert_eq!(
        result(handle_io_read(2, &state())),
        json!({"snapshot": null})
    );
}

#[test]
fn io_list_fails_closed_when_snapshot_lock_is_unavailable() {
    let state = state();
    poison(&state.io_snapshot);

    let response = handle_io_list(3, &state);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("I/O snapshot unavailable"));
}

#[test]
fn io_read_fails_closed_when_snapshot_lock_is_unavailable() {
    let state = state();
    poison(&state.io_snapshot);

    let response = handle_io_read(4, &state);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("I/O snapshot unavailable"));
}

#[test]
fn io_write_requires_params() {
    let response = handle_io_write(5, None, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

#[test]
fn io_force_requires_params() {
    let response = handle_io_force(6, None, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

#[test]
fn io_unforce_requires_params() {
    let response = handle_io_unforce(7, None, &state());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! write_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(handle_io_write(100, Some($params), &state()));
        }
    };
}

write_invalid_params_case!(io_write_rejects_null_params, JsonValue::Null);
write_invalid_params_case!(io_write_rejects_array_params, json!([]));
write_invalid_params_case!(io_write_rejects_string_params, json!("write"));
write_invalid_params_case!(io_write_rejects_missing_address, json!({"value": "TRUE"}));
write_invalid_params_case!(io_write_rejects_missing_value, json!({"address": "%QX0.0"}));
write_invalid_params_case!(
    io_write_rejects_unknown_field,
    json!({"address": "%QX0.0", "value": "TRUE", "mode": "once"})
);
write_invalid_params_case!(
    io_write_rejects_null_address,
    json!({"address": null, "value": "TRUE"})
);
write_invalid_params_case!(
    io_write_rejects_numeric_address,
    json!({"address": 1, "value": "TRUE"})
);
write_invalid_params_case!(
    io_write_rejects_null_value,
    json!({"address": "%QX0.0", "value": null})
);
write_invalid_params_case!(
    io_write_rejects_boolean_value,
    json!({"address": "%QX0.0", "value": true})
);

macro_rules! force_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(handle_io_force(200, Some($params), &state()));
        }
    };
}

force_invalid_params_case!(io_force_rejects_null_params, JsonValue::Null);
force_invalid_params_case!(io_force_rejects_array_params, json!([]));
force_invalid_params_case!(io_force_rejects_string_params, json!("force"));
force_invalid_params_case!(io_force_rejects_missing_address, json!({"value": "TRUE"}));
force_invalid_params_case!(io_force_rejects_missing_value, json!({"address": "%QX0.0"}));
force_invalid_params_case!(
    io_force_rejects_unknown_field,
    json!({"address": "%QX0.0", "value": "TRUE", "sticky": true})
);
force_invalid_params_case!(
    io_force_rejects_null_address,
    json!({"address": null, "value": "TRUE"})
);
force_invalid_params_case!(
    io_force_rejects_object_address,
    json!({"address": {}, "value": "TRUE"})
);
force_invalid_params_case!(
    io_force_rejects_null_value,
    json!({"address": "%QX0.0", "value": null})
);
force_invalid_params_case!(
    io_force_rejects_numeric_value,
    json!({"address": "%QX0.0", "value": 1})
);

macro_rules! unforce_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(handle_io_unforce(300, Some($params), &state()));
        }
    };
}

unforce_invalid_params_case!(io_unforce_rejects_null_params, JsonValue::Null);
unforce_invalid_params_case!(io_unforce_rejects_array_params, json!([]));
unforce_invalid_params_case!(io_unforce_rejects_string_params, json!("release"));
unforce_invalid_params_case!(io_unforce_rejects_missing_address, json!({}));
unforce_invalid_params_case!(
    io_unforce_rejects_unknown_field,
    json!({"address": "%QX0.0", "all": true})
);
unforce_invalid_params_case!(io_unforce_rejects_null_address, json!({"address": null}));
unforce_invalid_params_case!(
    io_unforce_rejects_boolean_address,
    json!({"address": false})
);

#[test]
fn io_write_trims_canonical_address() {
    let state = state();
    let response = handle_io_write(8, write_params(" \t%QX2.3\n", "TRUE"), &state);
    assert_eq!(result(response), json!({"status": "queued"}));

    let writes = state.debug.drain_io_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0].0,
        IoAddress::parse("%QX2.3").expect("canonical address")
    );
    assert_eq!(writes[0].1, Value::Bool(true));
}

#[test]
fn io_write_accepts_each_process_image_area() {
    let state = state();
    for address in ["%IX0.0", "%QW2", "%MD4"] {
        let response = handle_io_write(9, write_params(address, "0"), &state);
        assert!(response.ok, "{address} rejected: {:?}", response.error);
    }

    let writes = state.debug.drain_io_writes();
    assert_eq!(writes.len(), 3);
    assert_eq!(writes[0].0, IoAddress::parse("%IX0.0").unwrap());
    assert_eq!(writes[1].0, IoAddress::parse("%QW2").unwrap());
    assert_eq!(writes[2].0, IoAddress::parse("%MD4").unwrap());
}

#[test]
fn io_write_preserves_hierarchical_address_segments() {
    let state = state();
    let response = handle_io_write(10, write_params("%IX1.2.3", "TRUE"), &state);
    assert!(
        response.ok,
        "hierarchical address rejected: {:?}",
        response.error
    );

    let writes = state.debug.drain_io_writes();
    assert_eq!(writes[0].0.path, [1, 2]);
    assert_eq!(writes[0].0.bit, 3);
}

#[test]
fn io_write_accepts_last_complete_flat_long_word() {
    let state = state();
    let response = handle_io_write(11, write_params("%QL16777208", "0"), &state);
    assert!(
        response.ok,
        "last complete LWORD rejected: {:?}",
        response.error
    );
}

macro_rules! write_invalid_address_case {
    ($name:ident, $address:expr) => {
        #[test]
        fn $name() {
            let state = state();
            assert_address_rejected(handle_io_write(400, write_params($address, "0"), &state));
            assert!(state.debug.drain_io_writes().is_empty());
        }
    };
}

write_invalid_address_case!(io_write_rejects_empty_address, "");
write_invalid_address_case!(io_write_rejects_missing_percent, "QX0.0");
write_invalid_address_case!(io_write_rejects_lowercase_area, "%qx0.0");
write_invalid_address_case!(io_write_rejects_unknown_area, "%RX0.0");
write_invalid_address_case!(io_write_rejects_bit_above_seven, "%QX0.8");
write_invalid_address_case!(io_write_rejects_wildcard, "%Q*");
write_invalid_address_case!(io_write_rejects_byte_above_area, "%QB16777216");
write_invalid_address_case!(io_write_rejects_word_crossing_area, "%QW16777215");
write_invalid_address_case!(io_write_rejects_dword_crossing_area, "%QD16777213");
write_invalid_address_case!(io_write_rejects_lword_crossing_area, "%QL16777209");

#[test]
fn io_force_uses_the_same_address_eligibility_as_write() {
    let state = state();
    for address in ["%I*", "%QW16777215", "%mb0"] {
        assert_address_rejected(handle_io_force(12, write_params(address, "0"), &state));
    }
    assert!(state.debug.forced_snapshot().io.is_empty());
}

#[test]
fn io_unforce_uses_the_same_address_eligibility_as_write() {
    let state = state();
    for address in ["%M*", "%IL16777209", "%ix0.0"] {
        assert_address_rejected(handle_io_unforce(13, unforce_params(address), &state));
    }
}

#[test]
fn attach_scalar_grammar_accepts_boolean_case_and_whitespace() {
    let state = state();
    for text in ["TRUE", "false", " \tTrUe\n"] {
        let response = handle_io_write(14, write_params("%QX0.0", text), &state);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = state
        .debug
        .drain_io_writes()
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [Value::Bool(true), Value::Bool(false), Value::Bool(true)]
    );
}

#[test]
fn attach_scalar_grammar_accepts_signed_i64_boundaries() {
    let state = state();
    for text in [
        "0",
        "+0007",
        "-0007",
        "9223372036854775807",
        "-9223372036854775808",
    ] {
        let response = handle_io_write(15, write_params("%QW0", text), &state);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = state
        .debug
        .drain_io_writes()
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [
            Value::LInt(0),
            Value::LInt(7),
            Value::LInt(-7),
            Value::LInt(i64::MAX),
            Value::LInt(i64::MIN),
        ]
    );
}

macro_rules! invalid_scalar_case {
    ($name:ident, $text:expr) => {
        #[test]
        fn $name() {
            let state = state();
            assert_unsupported_value(handle_io_write(500, write_params("%QW0", $text), &state));
            assert!(state.debug.drain_io_writes().is_empty());
        }
    };
}

invalid_scalar_case!(attach_scalar_rejects_empty_text, "");
invalid_scalar_case!(attach_scalar_rejects_whitespace_text, " \t\n");
invalid_scalar_case!(attach_scalar_rejects_integer_separator, "1_000");
invalid_scalar_case!(attach_scalar_rejects_hex_prefix, "16#FF");
invalid_scalar_case!(attach_scalar_rejects_decimal_point, "1.0");
invalid_scalar_case!(attach_scalar_rejects_exponent, "1e3");
invalid_scalar_case!(attach_scalar_rejects_quoted_text, "\"TRUE\"");
invalid_scalar_case!(
    attach_scalar_rejects_positive_i64_overflow,
    "9223372036854775808"
);
invalid_scalar_case!(
    attach_scalar_rejects_negative_i64_overflow,
    "-9223372036854775809"
);

#[test]
fn io_write_preserves_duplicate_requests_in_arrival_order() {
    let state = state();
    for value in ["1", "2", "3"] {
        assert!(
            handle_io_write(16, write_params("%QW0", value), &state).ok,
            "write rejected"
        );
    }

    let values = state
        .debug
        .drain_io_writes()
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    assert_eq!(values, [Value::LInt(1), Value::LInt(2), Value::LInt(3)]);
}

#[test]
fn io_force_replaces_same_address_without_duplication() {
    let state = state();
    assert!(
        handle_io_force(17, write_params("%QW0", "1"), &state).ok,
        "first force rejected"
    );
    assert!(
        handle_io_force(18, write_params("%QW2", "2"), &state).ok,
        "second force rejected"
    );
    assert!(
        handle_io_force(19, write_params("%QW0", "3"), &state).ok,
        "replacement force rejected"
    );

    let forced = state.debug.forced_snapshot().io;
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0].0, IoAddress::parse("%QW0").unwrap());
    assert_eq!(forced[0].1, Value::LInt(3));
    assert_eq!(forced[1].0, IoAddress::parse("%QW2").unwrap());
    assert_eq!(forced[1].1, Value::LInt(2));
}

#[test]
fn io_unforce_removes_only_exact_matching_force() {
    let state = state();
    assert!(handle_io_force(20, write_params("%QW0", "1"), &state).ok);
    assert!(handle_io_force(21, write_params("%QW2", "2"), &state).ok);

    assert_eq!(
        result(handle_io_unforce(22, unforce_params(" %QW0 "), &state)),
        json!({"status": "released"})
    );
    let forced = state.debug.forced_snapshot().io;
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].0, IoAddress::parse("%QW2").unwrap());
}

#[test]
fn io_unforce_is_idempotent_for_absent_force() {
    let state = state();
    assert_eq!(
        result(handle_io_unforce(23, unforce_params("%QX0.0"), &state)),
        json!({"status": "released"})
    );
    assert!(state.debug.forced_snapshot().io.is_empty());
}

#[test]
fn rejected_write_preserves_existing_queue() {
    let state = state();
    state
        .debug
        .enqueue_io_write(IoAddress::parse("%QW0").unwrap(), Value::LInt(7));

    assert_address_rejected(handle_io_write(24, write_params("%Q*", "9"), &state));
    let writes = state.debug.drain_io_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].1, Value::LInt(7));
}

#[test]
fn rejected_force_preserves_existing_force() {
    let state = state();
    state
        .debug
        .force_io(IoAddress::parse("%QW0").unwrap(), Value::LInt(7));

    assert_unsupported_value(handle_io_force(
        25,
        write_params("%QW0", "not-a-value"),
        &state,
    ));
    let forced = state.debug.forced_snapshot().io;
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].1, Value::LInt(7));
}
