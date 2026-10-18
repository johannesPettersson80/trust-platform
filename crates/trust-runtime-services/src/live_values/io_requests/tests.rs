//! The `io.*` operation contract: the same parameters, refusals, error texts and JSON as
//! trust-runtime answers today (specification 11, "Live Values"; specification 34 §4.3.1).
//!
//! Every expected value is the stable answer the hosted handlers give today, moved here with the
//! handlers. The platform's queues are the shared `LiveValueQueues` behind the `IoQueuePort`, so a
//! moved test keeps its exact expectations.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::value::Value;

use super::*;
use crate::live_values::io_snapshot::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::live_values::queues::{LiveValueQueues, RequestQueueFull, REQUEST_QUEUE_CAPACITY};

/// A platform's Live Values queues over the shared `LiveValueQueues`, as `DebugControl` holds
/// them. The inherent helpers set up a test's starting state and read the queues back.
#[derive(Default)]
struct FakeQueues {
    queues: RefCell<LiveValueQueues<()>>,
}

impl FakeQueues {
    fn new() -> Self {
        Self::default()
    }

    fn push_write(&self, address: IoAddress, value: Value) {
        self.queues
            .borrow_mut()
            .try_enqueue_io_write(address, value)
            .expect("the test queue is not full");
    }

    fn push_force(&self, address: IoAddress, value: Value) {
        self.queues
            .borrow_mut()
            .try_upsert_forced_io(address, value)
            .expect("the test queue is not full");
    }

    fn drain_io_writes(&self) -> Vec<(IoAddress, Value)> {
        self.queues.borrow_mut().drain_io_writes()
    }

    fn forced_io(&self) -> Vec<(IoAddress, Value)> {
        self.queues.borrow().forced_io.clone()
    }
}

impl IoQueuePort for FakeQueues {
    fn io_snapshot(&self) -> IoSnapshotRead {
        IoSnapshotRead::Missing
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.queues
            .borrow_mut()
            .try_enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.queues
            .borrow_mut()
            .try_upsert_forced_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.queues.borrow_mut().release_io(address);
    }
}

fn result(response: ControlResponse) -> JsonValue {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn assert_invalid_params(response: &ControlResponse) {
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

fn assert_address_rejected(response: &ControlResponse) {
    assert!(!response.ok, "invalid address must reject");
    assert!(
        response.result.is_none(),
        "rejected address must not return a result"
    );
}

fn assert_unsupported_value(response: &ControlResponse) {
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

fn write_params(address: &str, value: &str) -> JsonValue {
    json!({"address": address, "value": value})
}

fn unforce_params(address: &str) -> JsonValue {
    json!({"address": address})
}

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("test address")
}

fn empty_forced() -> Vec<(IoAddress, Value)> {
    Vec::new()
}

#[test]
fn io_list_distinguishes_legitimate_missing_snapshot() {
    let response = io_list(1, IoSnapshotRead::Missing);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("no snapshot available"));
}

#[test]
fn io_read_exposes_legitimate_missing_snapshot_as_null() {
    assert_eq!(
        result(io_read(2, IoSnapshotRead::Missing)),
        json!({"snapshot": null})
    );
}

#[test]
fn io_list_and_io_read_report_an_unreadable_snapshot_slot() {
    // A platform whose snapshot slot cannot be read answers `I/O snapshot unavailable`, never the
    // missing-snapshot text and never an empty snapshot.
    let listed = io_list(9, IoSnapshotRead::Unavailable);
    assert!(!listed.ok);
    assert_eq!(listed.error.as_deref(), Some("I/O snapshot unavailable"));

    let read = io_read(10, IoSnapshotRead::Unavailable);
    assert!(!read.ok);
    assert_eq!(read.error.as_deref(), Some("I/O snapshot unavailable"));
}

#[test]
fn io_list_answers_the_published_snapshot_json() {
    let snapshot = IoSnapshot {
        scan: Some(7),
        forced: Vec::new(),
        inputs: Vec::new(),
        outputs: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("Lamp")),
            address: address("%QX0.0"),
            value_type_name: Some(SmolStr::new("BOOL")),
            value: IoSnapshotValue::Value(Value::Bool(true)),
            source: None,
        }],
        memory: Vec::new(),
    };
    assert_eq!(
        result(io_list(3, IoSnapshotRead::Published(snapshot))),
        json!({
            "scan": 7,
            "inputs": [],
            "outputs": [{
                "name": "Lamp",
                "address": "%QX0.0",
                "value": "Bool(true)",
                "forced": false,
                "valueType": "BOOL",
            }],
            "memory": [],
        })
    );
}

#[test]
fn io_read_wraps_the_published_snapshot() {
    let snapshot = IoSnapshot {
        scan: Some(8),
        forced: vec![address("%QW0")],
        inputs: Vec::new(),
        outputs: vec![IoSnapshotEntry {
            name: None,
            address: address("%QW0"),
            value_type_name: None,
            value: IoSnapshotValue::Value(Value::LInt(3)),
            source: None,
        }],
        memory: Vec::new(),
    };
    assert_eq!(
        result(io_read(4, IoSnapshotRead::Published(snapshot))),
        json!({
            "snapshot": {
                "scan": 8,
                "inputs": [],
                "outputs": [{
                    "name": null,
                    "address": "%QW0",
                    "value": "LInt(3)",
                    "forced": true,
                }],
                "memory": [],
            }
        })
    );
}

#[test]
fn io_write_requires_params() {
    let response = io_write(5, None, &FakeQueues::new());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

#[test]
fn io_force_requires_params() {
    let response = io_force(6, None, &FakeQueues::new());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

#[test]
fn io_unforce_requires_params() {
    let response = io_unforce(7, None, &FakeQueues::new());
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("missing params"));
}

macro_rules! write_invalid_params_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid_params(&io_write(100, Some($params), &FakeQueues::new()));
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
            assert_invalid_params(&io_force(200, Some($params), &FakeQueues::new()));
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
            assert_invalid_params(&io_unforce(300, Some($params), &FakeQueues::new()));
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
    let queues = FakeQueues::new();
    let response = io_write(8, Some(write_params(" \t%QX2.3\n", "TRUE")), &queues);
    assert_eq!(result(response), json!({"status": "queued"}));

    let writes = queues.drain_io_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, address("%QX2.3"));
    assert_eq!(writes[0].1, Value::Bool(true));
}

#[test]
fn io_write_accepts_each_process_image_area() {
    let queues = FakeQueues::new();
    for address in ["%IX0.0", "%QW2", "%MD4"] {
        let response = io_write(9, Some(write_params(address, "0")), &queues);
        assert!(response.ok, "{address} rejected: {:?}", response.error);
    }

    let writes = queues.drain_io_writes();
    assert_eq!(writes.len(), 3);
    assert_eq!(writes[0].0, address("%IX0.0"));
    assert_eq!(writes[1].0, address("%QW2"));
    assert_eq!(writes[2].0, address("%MD4"));
}

#[test]
fn io_write_preserves_hierarchical_address_segments() {
    let queues = FakeQueues::new();
    let response = io_write(10, Some(write_params("%IX1.2.3", "TRUE")), &queues);
    assert!(
        response.ok,
        "hierarchical address rejected: {:?}",
        response.error
    );

    let writes = queues.drain_io_writes();
    assert_eq!(writes[0].0.path, [1, 2]);
    assert_eq!(writes[0].0.bit, 3);
}

#[test]
fn io_write_accepts_last_complete_flat_long_word() {
    let queues = FakeQueues::new();
    let response = io_write(11, Some(write_params("%QL16777208", "0")), &queues);
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
            let queues = FakeQueues::new();
            assert_address_rejected(&io_write(400, Some(write_params($address, "0")), &queues));
            assert_eq!(queues.drain_io_writes(), Vec::new());
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
    let queues = FakeQueues::new();
    for address in ["%I*", "%QW16777215", "%mb0"] {
        assert_address_rejected(&io_force(12, Some(write_params(address, "0")), &queues));
    }
    assert_eq!(queues.forced_io(), empty_forced());
}

#[test]
fn io_unforce_uses_the_same_address_eligibility_as_write() {
    let queues = FakeQueues::new();
    for address in ["%M*", "%IL16777209", "%ix0.0"] {
        assert_address_rejected(&io_unforce(13, Some(unforce_params(address)), &queues));
    }
}

#[test]
fn attach_scalar_grammar_accepts_boolean_case_and_whitespace() {
    let queues = FakeQueues::new();
    for text in ["TRUE", "false", " \tTrUe\n"] {
        let response = io_write(14, Some(write_params("%QX0.0", text)), &queues);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = queues
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
    let queues = FakeQueues::new();
    for text in [
        "0",
        "+0007",
        "-0007",
        "9223372036854775807",
        "-9223372036854775808",
    ] {
        let response = io_write(15, Some(write_params("%QW0", text)), &queues);
        assert!(response.ok, "{text:?} rejected: {:?}", response.error);
    }

    let values = queues
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
            let queues = FakeQueues::new();
            assert_unsupported_value(&io_write(500, Some(write_params("%QW0", $text)), &queues));
            assert_eq!(queues.drain_io_writes(), Vec::new());
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
    let queues = FakeQueues::new();
    for value in ["1", "2", "3"] {
        assert!(
            io_write(16, Some(write_params("%QW0", value)), &queues).ok,
            "write rejected"
        );
    }

    let values = queues
        .drain_io_writes()
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    assert_eq!(values, [Value::LInt(1), Value::LInt(2), Value::LInt(3)]);
}

#[test]
fn io_force_replaces_same_address_without_duplication() {
    let queues = FakeQueues::new();
    assert!(
        io_force(17, Some(write_params("%QW0", "1")), &queues).ok,
        "first force rejected"
    );
    assert!(
        io_force(18, Some(write_params("%QW2", "2")), &queues).ok,
        "second force rejected"
    );
    assert!(
        io_force(19, Some(write_params("%QW0", "3")), &queues).ok,
        "replacement force rejected"
    );

    let forced = queues.forced_io();
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0].0, address("%QW0"));
    assert_eq!(forced[0].1, Value::LInt(3));
    assert_eq!(forced[1].0, address("%QW2"));
    assert_eq!(forced[1].1, Value::LInt(2));
}

#[test]
fn io_unforce_removes_only_exact_matching_force() {
    let queues = FakeQueues::new();
    assert!(io_force(20, Some(write_params("%QW0", "1")), &queues).ok);
    assert!(io_force(21, Some(write_params("%QW2", "2")), &queues).ok);

    assert_eq!(
        result(io_unforce(22, Some(unforce_params(" %QW0 ")), &queues)),
        json!({"status": "released"})
    );
    let forced = queues.forced_io();
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].0, address("%QW2"));
}

#[test]
fn io_unforce_is_idempotent_for_absent_force() {
    let queues = FakeQueues::new();
    assert_eq!(
        result(io_unforce(23, Some(unforce_params("%QX0.0")), &queues)),
        json!({"status": "released"})
    );
    assert_eq!(queues.forced_io(), empty_forced());
}

#[test]
fn rejected_write_preserves_existing_queue() {
    let queues = FakeQueues::new();
    queues.push_write(address("%QW0"), Value::LInt(7));

    assert_address_rejected(&io_write(24, Some(write_params("%Q*", "9")), &queues));
    let writes = queues.drain_io_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].1, Value::LInt(7));
}

#[test]
fn rejected_force_preserves_existing_force() {
    let queues = FakeQueues::new();
    queues.push_force(address("%QW0"), Value::LInt(7));

    assert_unsupported_value(&io_force(
        25,
        Some(write_params("%QW0", "not-a-value")),
        &queues,
    ));
    let forced = queues.forced_io();
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].1, Value::LInt(7));
}

#[test]
fn a_full_write_queue_refuses_the_request_with_its_stable_error() {
    let queues = FakeQueues::new();
    for _ in 0..REQUEST_QUEUE_CAPACITY {
        queues.push_write(address("%QW0"), Value::LInt(0));
    }

    let response = io_write(26, Some(write_params("%QW2", "1")), &queues);
    assert!(!response.ok, "a full queue must refuse a new address");
    assert_eq!(response.error.as_deref(), Some("request queue full"));
    assert_eq!(
        queues.drain_io_writes().len(),
        REQUEST_QUEUE_CAPACITY,
        "the refused write queues nothing"
    );
}

#[test]
fn a_full_force_queue_refuses_a_new_address_but_not_a_replacement() {
    let queues = FakeQueues::new();
    for index in 0..REQUEST_QUEUE_CAPACITY {
        queues.push_force(address(&format!("%QW{}", index * 2)), Value::LInt(0));
    }

    let refused = io_force(27, Some(write_params("%QW3000", "1")), &queues);
    assert_eq!(refused.error.as_deref(), Some("request queue full"));

    let replaced = io_force(28, Some(write_params("%QW0", "9")), &queues);
    assert_eq!(result(replaced), json!({"status": "forced"}));
    assert_eq!(queues.forced_io()[0].1, Value::LInt(9));
}
