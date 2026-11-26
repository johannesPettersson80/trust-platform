//! Exact event payloads, JSON projections and store bounds, as trust-runtime keeps them today.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use serde_json::json;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::fault::FaultSite;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::value::Duration;

use super::*;

fn write_rejected(index: usize) -> RuntimeEvent {
    RuntimeEvent::WriteRejected {
        target: format!("target{index}").into(),
        error: "out of range".into(),
        time: Duration::ZERO,
    }
}

fn cycle(cycle: u64) -> RuntimeEvent {
    RuntimeEvent::CycleStart {
        cycle,
        time: Duration::from_millis(1),
    }
}

/// A fault event whose fixed record and hosted message render `error`'s today text.
fn fault(error: &RuntimeError) -> RuntimeEvent {
    fault_at(error, Duration::from_millis(2))
}

/// A fault event at `time`, both in the record's logical time and the event's own time.
fn fault_at(error: &RuntimeError, time: Duration) -> RuntimeEvent {
    RuntimeEvent::fault(error, time)
}

#[test]
fn runtime_event_variants_preserve_exact_payloads_and_identity() {
    let events = vec![
        RuntimeEvent::CycleStart {
            cycle: 1,
            time: Duration::from_millis(1),
        },
        RuntimeEvent::CycleEnd {
            cycle: 1,
            time: Duration::from_millis(2),
        },
        RuntimeEvent::TaskStart {
            name: "Fast".into(),
            priority: 0,
            time: Duration::from_millis(3),
        },
        RuntimeEvent::TaskEnd {
            name: "Fast".into(),
            priority: 0,
            time: Duration::from_millis(4),
        },
        RuntimeEvent::TaskOverrun {
            name: "Fast".into(),
            missed: u64::MAX,
            time: Duration::from_millis(5),
        },
        fault_at(
            &RuntimeError::SimulationFault("fault".into()),
            Duration::from_millis(6),
        ),
        RuntimeEvent::SafeStateFailed {
            root: "fault".into(),
            error: "safe state".into(),
            time: Duration::from_millis(7),
        },
        RuntimeEvent::RetainOrphanDropped {
            name: "old".into(),
            time: Duration::from_millis(8),
        },
        RuntimeEvent::RetainMigrationApplied {
            name: "kept".into(),
            detail: "DINT to LINT".into(),
            time: Duration::from_millis(9),
        },
        RuntimeEvent::AuditDropped {
            request_id: u64::MAX,
            request_type: "set".into(),
            error: "offline".into(),
            time: Some(Duration::from_millis(10)),
        },
        RuntimeEvent::FeatureDisabled {
            feature: "opcua".into(),
            request_type: Some("opcua.status".into()),
            time: Some(Duration::from_millis(11)),
        },
    ];

    assert_eq!(events, events.clone());
    assert_ne!(events[0], events[1]);
    assert!(matches!(
        &events[4],
        RuntimeEvent::TaskOverrun {
            missed: u64::MAX,
            ..
        }
    ));
    assert!(matches!(
        &events[9],
        RuntimeEvent::AuditDropped {
            request_id: u64::MAX,
            request_type,
            ..
        } if request_type == "set"
    ));
}

#[test]
fn cycle_and_task_events_project_to_their_exact_json() {
    let time = Duration::from_millis(3);
    for (event, expected) in [
        (
            RuntimeEvent::CycleStart { cycle: 7, time },
            json!({"type": "cycle_start", "cycle": 7, "time_ns": 3_000_000}),
        ),
        (
            RuntimeEvent::CycleEnd { cycle: 7, time },
            json!({"type": "cycle_end", "cycle": 7, "time_ns": 3_000_000}),
        ),
        (
            RuntimeEvent::TaskStart {
                name: "Fast".into(),
                priority: 2,
                time,
            },
            json!({"type": "task_start", "name": "Fast", "priority": 2, "time_ns": 3_000_000}),
        ),
        (
            RuntimeEvent::TaskEnd {
                name: "Fast".into(),
                priority: 2,
                time,
            },
            json!({"type": "task_end", "name": "Fast", "priority": 2, "time_ns": 3_000_000}),
        ),
        (
            RuntimeEvent::TaskOverrun {
                name: "Fast".into(),
                missed: 4,
                time,
            },
            json!({"type": "task_overrun", "name": "Fast", "missed": 4, "time_ns": 3_000_000}),
        ),
        (
            // Specification 11, "Rust POU budgets": the call's time is `call_ns`, as `time_ns` is
            // every event's scan time.
            RuntimeEvent::RustBudgetExceeded {
                name: "SPIN".into(),
                call_ns: 52_000,
                budget_ns: 1_000,
                time,
            },
            json!({"type": "rust_budget_exceeded", "name": "SPIN", "call_ns": 52_000, "budget_ns": 1_000, "time_ns": 3_000_000}),
        ),
    ] {
        assert_eq!(runtime_event_to_json(event), expected);
    }
}

#[test]
fn diagnostic_events_project_to_their_exact_json() {
    let time = Duration::from_nanos(-5);
    for (event, expected) in [
        (
            RuntimeEvent::WriteRejected {
                target: "%QX0.0".into(),
                error: "out of range".into(),
                time,
            },
            json!({"type": "write_rejected", "target": "%QX0.0", "error": "out of range", "time_ns": -5}),
        ),
        (
            fault_at(&RuntimeError::DivisionByZero, time),
            json!({"type": "fault", "error": "division by zero", "time_ns": -5}),
        ),
        (
            RuntimeEvent::SafeStateFailed {
                root: "division by zero".into(),
                error: "safe output rejected".into(),
                time,
            },
            json!({
                "type": "safe_state_failed",
                "root": "division by zero",
                "error": "safe output rejected",
                "time_ns": -5,
            }),
        ),
        (
            RuntimeEvent::RetainOrphanDropped {
                name: "old".into(),
                time,
            },
            json!({"type": "retain_orphan_dropped", "name": "old", "time_ns": -5}),
        ),
        (
            RuntimeEvent::RetainMigrationApplied {
                name: "kept".into(),
                detail: "DINT to LINT".into(),
                time,
            },
            json!({
                "type": "retain_migration_applied",
                "name": "kept",
                "detail": "DINT to LINT",
                "time_ns": -5,
            }),
        ),
        (
            RuntimeEvent::AuditDropped {
                request_id: 9,
                request_type: "set".into(),
                error: "offline".into(),
                time: Some(time),
            },
            json!({
                "type": "audit_dropped",
                "request_id": 9,
                "request_type": "set",
                "error": "offline",
                "time_ns": -5,
            }),
        ),
        (
            RuntimeEvent::FeatureDisabled {
                feature: "debug".into(),
                request_type: Some("debug.state".into()),
                time: Some(time),
            },
            json!({
                "type": "feature_disabled",
                "feature": "debug",
                "request_type": "debug.state",
                "time_ns": -5,
            }),
        ),
        (
            RuntimeEvent::FeatureDisabled {
                feature: "debug".into(),
                request_type: None,
                time: Some(time),
            },
            json!({"type": "feature_disabled", "feature": "debug", "request_type": null, "time_ns": -5}),
        ),
    ] {
        assert_eq!(runtime_event_to_json(event), expected);
    }
}

/// Spec 11 (`faults` and `events.tail`) and spec 34 MEM-05: both replies render a fault event's
/// text from its fixed record, byte for byte as trust-runtime answers today. It covers one fault
/// of each kind the record carries: division by zero, an index out of range, a null reference, an
/// overflow, the work limit and the call depth. It would catch a reply that renders the stable
/// code instead of the text, one that drops a bounded detail, and one whose `error` field moved.
#[test]
fn an_online_change_value_event_projects_to_its_exact_json() {
    // Specification 11 §6.7.1: each variable an online change did not give its running value as it
    // was is an `online_change_value` event with the variable's path as `name` and what happened as
    // `detail`; the event that counts the variables beyond the bound has a null `name`. Wrong
    // behaviour caught: another type or member name than the specification gives, and a count event
    // that names a variable.
    let at = Duration::from_millis(90);
    let cases = [
        (
            CarriedValueEvent::New {
                name: "Plant.motor.speed".into(),
            },
            json!("Plant.motor.speed"),
            "new variable: declared initial value",
        ),
        (
            CarriedValueEvent::Converted { name: "a".into() },
            json!("a"),
            "declaration changed: the running value converted to it",
        ),
        (
            CarriedValueEvent::NotKept {
                name: "P1.lim".into(),
            },
            json!("P1.lim"),
            "declaration changed: the running value does not fit, declared initial value",
        ),
        (
            CarriedValueEvent::Cleared { name: "q".into() },
            json!("q"),
            "its target is gone or has another type: NULL",
        ),
        (
            CarriedValueEvent::More { count: 8 },
            serde_json::Value::Null,
            "8 more variables did not keep their running values",
        ),
    ];
    for (event, name, detail) in cases {
        assert_eq!(
            runtime_event_to_json(RuntimeEvent::OnlineChangeValue { event, time: at }),
            json!({
                "type": "online_change_value",
                "name": name,
                "detail": detail,
                "time_ns": 90_000_000,
            })
        );
    }
}

#[test]
fn a_fault_event_replies_as_before() {
    use crate::status::{events_tail, faults};
    use crate::tests::fake_host::FakeHost;

    let time = Duration::from_millis(4_036);
    let cases = [
        (RuntimeError::DivisionByZero, "division by zero"),
        (
            RuntimeError::IndexOutOfBounds {
                index: 9,
                lower: 1,
                upper: 4,
            },
            "array index 9 out of bounds [1..4]",
        ),
        (RuntimeError::NullReference, "null reference dereference"),
        (RuntimeError::Overflow, "arithmetic overflow"),
        (
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::AllocationBytes,
                used: 40,
                requested: 24,
                limit: 60,
            },
            "execution timed out: allocation bytes limit 60, used 40, requested 24",
        ),
        (RuntimeError::CallDepthExceeded, "call depth exceeded"),
    ];

    for (error, text) in cases {
        let event = fault_at(&error, time);
        // Both replies project the event through the same JSON form.
        assert_eq!(
            runtime_event_to_json(event.clone()),
            json!({"type": "fault", "error": text, "time_ns": 4_036_000_000_i64}),
            "the event's JSON for {text}"
        );

        let host = FakeHost::running();
        host.events.borrow_mut().push_back(event);
        assert_eq!(
            faults(1, None, &host).result.expect("faults result"),
            json!({"faults": [{"type": "fault", "error": text, "time_ns": 4_036_000_000_i64}]}),
            "the faults reply for {text}"
        );
        assert_eq!(
            events_tail(2, None, &host)
                .result
                .expect("events tail result"),
            json!({"events": [{"type": "fault", "error": text, "time_ns": 4_036_000_000_i64}]}),
            "the events.tail reply for {text}"
        );
    }
}

/// Spec 34 MEM-05 and spec 48 Appendix A: a Rust POU's own fault keeps the number the POU chose and
/// its two numbers in the fixed record, three bounded numbers as an index fault's are, and every
/// reply names them as the error does. A sequence's `.within` that waited 2,010 ms at its third
/// wait point, whose limit is 2,000 ms, gives the numbers 3, 2,000 and 2,010 (RU-72), and reads
/// `Rust fault 3 (2000, 2010)`. It would catch a record that keeps only the stable code, whose text
/// `runtime_rust_fault` names neither the wait point nor its times, and one that loses or swaps a
/// number.
#[test]
fn a_rust_fault_replies_with_its_number_and_its_two_numbers() {
    use crate::status::{events_tail, faults};
    use crate::tests::fake_host::FakeHost;

    let time = Duration::from_millis(2_320);
    let error = RuntimeError::RustFault {
        number: 3,
        first: 2_000,
        second: 2_010,
    };
    let text = "Rust fault 3 (2000, 2010)";
    let event = fault_at(&error, time);
    assert_eq!(
        runtime_event_to_json(event.clone()),
        json!({"type": "fault", "error": text, "time_ns": 2_320_000_000_i64}),
        "the event's JSON"
    );

    let host = FakeHost::running();
    host.events.borrow_mut().push_back(event);
    assert_eq!(
        faults(1, None, &host).result.expect("faults result"),
        json!({"faults": [{"type": "fault", "error": text, "time_ns": 2_320_000_000_i64}]}),
        "the faults reply"
    );
    assert_eq!(
        events_tail(1, None, &host)
            .result
            .expect("events tail result"),
        json!({"events": [{"type": "fault", "error": text, "time_ns": 2_320_000_000_i64}]}),
        "the events.tail reply"
    );
}

/// Spec 11 (`faults` and `events.tail`) and spec 34 MEM-05: a fault whose fixed record does not
/// hold a hosted name or message renders today's text from the message the error carried, byte for
/// byte as trust-runtime answers today. It covers one fault of each kind that carries a message:
/// simulation, resource panic, the four I/O kinds, the three retain kinds, invalid config, a
/// read-only alias, a retained subrange name and bytecode. It would catch a reply that renders the
/// record's stable code instead of the message, one that drops the hosted payload, and one whose
/// `error` field moved.
#[test]
fn a_message_fault_replies_as_before() {
    use crate::status::{events_tail, faults};
    use crate::tests::fake_host::FakeHost;

    let time = Duration::from_millis(4_036);
    let cases = [
        (
            RuntimeError::SimulationFault("slot 3".into()),
            "simulation fault 'slot 3'",
        ),
        (
            RuntimeError::ResourcePanic("out of memory".into()),
            "resource panic 'out of memory'",
        ),
        (
            RuntimeError::IoDriver("adapter stopped".into()),
            "i/o driver error 'adapter stopped'",
        ),
        (
            RuntimeError::IoTransport("session closed".into()),
            "i/o transport error 'session closed'",
        ),
        (
            RuntimeError::IoAddress("%IX9.9".into()),
            "i/o address error '%IX9.9'",
        ),
        (
            RuntimeError::IoFreshness("%IX0.0".into()),
            "i/o freshness error '%IX0.0'",
        ),
        (
            RuntimeError::RetainStore("write failed".into()),
            "retain store error 'write failed'",
        ),
        (
            RuntimeError::RetainCorruption("checksum mismatch".into()),
            "retain corruption 'checksum mismatch'",
        ),
        (
            RuntimeError::RetainMigration("DINT to LINT".into()),
            "retain migration error 'DINT to LINT'",
        ),
        (
            RuntimeError::InvalidConfig("missing endpoint".into()),
            "invalid config 'missing endpoint'",
        ),
        (
            RuntimeError::ReadOnlyAlias("Observed".into()),
            "VAR_ACCESS 'Observed' is read-only",
        ),
        (
            RuntimeError::RetainSubrangeViolation {
                name: "Speed".into(),
                value: 5_000,
                lower: 0,
                upper: 1_000,
            },
            "retain migration error 'invalid retained value for 'Speed': value 5000 outside declared subrange 0..1000'",
        ),
    ];

    for (error, text) in cases {
        let event = fault_at(&error, time);
        assert_eq!(
            runtime_event_to_json(event.clone()),
            json!({"type": "fault", "error": text, "time_ns": 4_036_000_000_i64}),
            "the event's JSON for {text}"
        );

        let host = FakeHost::running();
        host.events.borrow_mut().push_back(event);
        assert_eq!(
            faults(1, None, &host).result.expect("faults result"),
            json!({"faults": [{"type": "fault", "error": text, "time_ns": 4_036_000_000_i64}]}),
            "the faults reply for {text}"
        );
        assert_eq!(
            events_tail(2, None, &host)
                .result
                .expect("events tail result"),
            json!({"events": [{"type": "fault", "error": text, "time_ns": 4_036_000_000_i64}]}),
            "the events.tail reply for {text}"
        );
    }
}

/// Spec 11 (event store) and D-C3-2: the store keeps the newest 1,000 events on every device. It
/// would catch a capacity that moved off the committed 1,000.
#[test]
fn the_event_store_keeps_1000_events() {
    assert_eq!(EVENT_STORE_CAPACITY, 1_000);
}

#[test]
fn event_store_keeps_the_newest_1000_events_in_order() {
    let mut store = VecDeque::new();
    for index in 0..=EVENT_STORE_CAPACITY {
        store_event(&mut store, write_rejected(index), EVENT_STORE_CAPACITY);
    }
    let expected: Vec<_> = (1..=EVENT_STORE_CAPACITY).map(write_rejected).collect();
    assert_eq!(Vec::from(store), expected);
}

#[test]
fn the_store_keeps_exactly_its_capacity_and_drops_the_oldest() {
    // Specification 11, the event store: the store keeps the newest `capacity` events and a newer
    // event displaces the oldest. Wrong behaviour caught: a store that keeps every event it is
    // given, or one that drops the newest instead of the oldest.
    let mut store = VecDeque::new();
    for index in 0..5 {
        store_event(&mut store, write_rejected(index), 3);
    }
    let expected: Vec<_> = (2..5).map(write_rejected).collect();
    assert_eq!(Vec::from(store), expected);
}

#[test]
fn a_store_above_its_capacity_keeps_only_the_newest_at_the_next_event() {
    // Specification 11, the event store: the store keeps the newest `capacity` events. A store that
    // holds five events when its capacity becomes 3 keeps events 3, 4 and 5 once event 5 is
    // stored. Wrong behaviour caught: an eviction that drops one event per new event, which leaves
    // the store above its bound after its capacity shrinks.
    let mut store = VecDeque::new();
    for index in 0..5 {
        store_event(&mut store, write_rejected(index), 10);
    }
    store_event(&mut store, write_rejected(5), 3);
    let expected: Vec<_> = (3..6).map(write_rejected).collect();
    assert_eq!(Vec::from(store), expected);
}

#[test]
fn the_store_keeps_no_event_that_every_scan_makes() {
    // Specification 11, the event store: the store keeps every event but the four that every scan
    // makes, so they never displace the events that matter. Worked by hand: a store of capacity 2
    // given a fault, then ten scans' cycle start, task start, task end and cycle end, then an
    // overrun, holds the fault and the overrun. Catches a store that keeps the scans' events, where
    // the first scan's events would have displaced the fault.
    let mut store = VecDeque::new();
    let fault = fault(&RuntimeError::SimulationFault("fault".into()));
    store_event(&mut store, fault.clone(), 2);
    let time = Duration::from_millis(10);
    for cycle in 0..10 {
        for event in [
            RuntimeEvent::CycleStart { cycle, time },
            RuntimeEvent::TaskStart {
                name: "Fast".into(),
                priority: 0,
                time,
            },
            RuntimeEvent::TaskEnd {
                name: "Fast".into(),
                priority: 0,
                time,
            },
            RuntimeEvent::CycleEnd { cycle, time },
        ] {
            assert!(event.is_per_scan(), "{event:?}");
            store_event(&mut store, event, 2);
        }
    }
    let overrun = RuntimeEvent::TaskOverrun {
        name: "Fast".into(),
        missed: 1,
        time,
    };
    assert!(!overrun.is_per_scan());
    store_event(&mut store, overrun.clone(), 2);
    assert_eq!(Vec::from(store), vec![fault, overrun]);
}

#[test]
fn debug_buffer_keeps_newest_1024_events_and_counts_the_dropped_oldest() {
    let mut buffer = VecDeque::new();
    let mut dropped = 0_u64;
    for index in 0..=DEBUG_EVENT_BUFFER_CAPACITY {
        buffer_event(&mut buffer, &mut dropped, write_rejected(index));
    }
    assert_eq!(dropped, 1);
    let expected: Vec<_> = (1..=DEBUG_EVENT_BUFFER_CAPACITY)
        .map(write_rejected)
        .collect();
    assert_eq!(Vec::from(buffer.clone()), expected);

    buffer.clear();
    for index in 0..3 {
        buffer_event(&mut buffer, &mut dropped, write_rejected(index));
    }
    assert_eq!(buffer.len(), 3);
    assert_eq!(dropped, 1, "a buffer below its bound drops nothing");

    let mut saturated = u64::MAX;
    let mut full: VecDeque<_> = (0..DEBUG_EVENT_BUFFER_CAPACITY)
        .map(write_rejected)
        .collect();
    buffer_event(&mut full, &mut saturated, write_rejected(0));
    assert_eq!(saturated, u64::MAX, "the drop count saturates");
}

#[test]
fn newest_events_are_returned_newest_first_up_to_the_limit() {
    let store: VecDeque<_> = (1..=5).map(cycle).collect();
    assert_eq!(newest_events(&store, 3), vec![cycle(5), cycle(4), cycle(3)]);
    assert_eq!(newest_events(&store, 10).len(), 5);
    assert_eq!(
        newest_events(&VecDeque::new(), 3),
        Vec::<RuntimeEvent>::new()
    );
}

#[test]
fn newest_faults_filter_before_applying_the_limit() {
    let store: VecDeque<_> = [
        fault(&RuntimeError::SimulationFault("old fault".into())),
        cycle(2),
        RuntimeEvent::SafeStateFailed {
            root: "old fault".into(),
            error: "output write failed".into(),
            time: Duration::ZERO,
        },
        fault(&RuntimeError::SimulationFault("new fault".into())),
        cycle(5),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        newest_faults(&store, 2),
        vec![
            fault(&RuntimeError::SimulationFault("new fault".into())),
            fault(&RuntimeError::SimulationFault("old fault".into()))
        ]
    );
    assert_eq!(
        newest_faults(&store, 1),
        vec![fault(&RuntimeError::SimulationFault("new fault".into()))]
    );
}

#[test]
fn wall_clock_readings_become_event_times_and_an_unset_clock_stays_unset() {
    assert_eq!(wall_time(None), None);
    assert_eq!(wall_time(Some(1_500)), Some(Duration::from_millis(1_500)));
    assert_eq!(
        wall_time(Some(u64::MAX)),
        Some(Duration::from_millis(i64::MAX)),
        "a reading beyond the event time range saturates"
    );
    // TARGET-06: an event that carries the wall-clock time reports `null` while it is not set.
    for (event, expected) in [
        (
            RuntimeEvent::AuditDropped {
                request_id: 9,
                request_type: "set".into(),
                error: "offline".into(),
                time: None,
            },
            json!({
                "type": "audit_dropped",
                "request_id": 9,
                "request_type": "set",
                "error": "offline",
                "time_ns": null,
            }),
        ),
        (
            RuntimeEvent::FeatureDisabled {
                feature: "debug".into(),
                request_type: None,
                time: None,
            },
            json!({"type": "feature_disabled", "feature": "debug", "request_type": null, "time_ns": null}),
        ),
    ] {
        assert_eq!(runtime_event_to_json(event), expected);
    }
}

#[test]
fn retained_value_changes_become_their_runtime_events_with_the_host_text() {
    use alloc::sync::Arc;
    use smol_str::SmolStr;
    use trust_runtime_core::collections::OrderedMap;
    use trust_runtime_core::value::StructValue;

    let structure = |fields: &[&str]| {
        let mut map = OrderedMap::default();
        for field in fields {
            map.insert(SmolStr::new(field), Value::Int(0));
        }
        Value::Struct(Arc::new(StructValue::from_untyped_parts(
            "State".into(),
            map,
        )))
    };
    let time = Duration::from_millis(4);
    for (event, expected) in [
        (
            RetainApplyEvent::Orphan { name: "old".into() },
            RuntimeEvent::RetainOrphanDropped {
                name: "old".into(),
                time,
            },
        ),
        (
            RetainApplyEvent::Migrated {
                name: "count".into(),
                before: Value::DInt(1),
                after: Value::LInt(1),
            },
            RuntimeEvent::RetainMigrationApplied {
                name: "count".into(),
                detail: "migrated retained value from DINT to LINT".into(),
                time,
            },
        ),
        (
            RetainApplyEvent::Migrated {
                name: "state".into(),
                before: structure(&["Old", "Kept"]),
                after: structure(&["kept", "New"]),
            },
            RuntimeEvent::RetainMigrationApplied {
                name: "state".into(),
                detail: "dropped field Old, added field New".into(),
                time,
            },
        ),
        (
            RetainApplyEvent::Migrated {
                name: "state".into(),
                before: structure(&["Speed"]),
                after: structure(&["SPEED"]),
            },
            RuntimeEvent::RetainMigrationApplied {
                name: "state".into(),
                detail: "migrated retained value from STRUCT to STRUCT".into(),
                time,
            },
        ),
    ] {
        assert_eq!(retain_apply_event(event, time), expected);
    }
}

/// The event of a runtime that runs on without its endpoint file (spec 11, section 6.8.0a).
#[test]
fn an_unavailable_endpoint_file_event_names_the_endpoint_and_the_reason() {
    let event = RuntimeEvent::ControlEndpointFileUnavailable {
        endpoint: SmolStr::new("unix:///run/user/1000/trust-0123456789abcdef.sock"),
        error: "write /plc/.trust-runtime/control.json: Read-only file system".into(),
        time: Duration::from_millis(5),
    };

    assert_eq!(
        runtime_event_to_json(event),
        json!({
            "type": "control_endpoint_file_unavailable",
            "endpoint": "unix:///run/user/1000/trust-0123456789abcdef.sock",
            "error": "write /plc/.trust-runtime/control.json: Read-only file system",
            "time_ns": 5_000_000,
        })
    );
}

/// The event of a resource that starts without its flight recorder (specification 11 §6.8.1,
/// "Cycle traces"): it names the bytes the flight recorder needs and the bytes free, or none when
/// the runtime does not know them, as a computer does not. Catches the needed bytes left out, an
/// unknown free amount written as 0, and another type.
#[test]
fn a_flight_recorder_without_memory_names_the_bytes_it_needs() {
    let computer = RuntimeEvent::TraceMemory {
        needed: 10_485_760,
        free: None,
        time: Duration::ZERO,
    };
    assert_eq!(
        runtime_event_to_json(computer),
        json!({"type": "runtime_trace_memory", "needed": 10_485_760, "free": null, "time_ns": 0})
    );
    let chip = RuntimeEvent::TraceMemory {
        needed: 98_304,
        free: Some(40_960),
        time: Duration::from_millis(10),
    };
    assert_eq!(
        runtime_event_to_json(chip),
        json!({"type": "runtime_trace_memory", "needed": 98_304, "free": 40_960,
            "time_ns": 10_000_000})
    );
}

/// The event of a fault's trace that the runtime could not write into the folder of its files
/// (specification 11 §6.8.1, "Cycle traces"): the file and the reason. Catches the file or the
/// reason left out, and another type.
#[test]
fn an_unwritten_fault_trace_names_its_file_and_the_reason() {
    let event = RuntimeEvent::TraceFileUnavailable {
        path: "/plc/traces/fault-20261124T013000Z.trusttrace".into(),
        error: "Read-only file system".into(),
        time: Duration::from_millis(5),
    };
    assert_eq!(
        runtime_event_to_json(event),
        json!({"type": "runtime_trace_file_unavailable",
            "path": "/plc/traces/fault-20261124T013000Z.trusttrace",
            "error": "Read-only file system", "time_ns": 5_000_000})
    );
}

#[test]
fn a_program_fault_names_its_place_only_while_its_program_runs() {
    // Specification 11, the fault entry, and specification 34 MEM-05: the record of a fault that
    // the program of generation 7 raised at the statement 5 of the POU 2 keeps that site, and the
    // reply names the place the runtime gives for it. Worked by hand: with the place Main,
    // src/Main.st, line 17, the entry is {"type": "fault", "error": "division by zero", "pou":
    // "Main", "file": "src/Main.st", "line": 17, "time_ns": 5000000000}; once generation 8 runs, a
    // restart's or an online change's, the runtime names no place for the site and the entry has
    // none; a fault from outside the program, without a site, names none whatever the lookup would
    // give. Catches a record that drops the site or its generation, and a reply that names a place
    // for a fault without a site.
    let site = FaultSite {
        pou: 2,
        statement: 5,
        generation: Generation(7),
    };
    let time = Duration::from_millis(5_000);
    let event = RuntimeEvent::program_fault(&RuntimeError::DivisionByZero, site, time);
    let RuntimeEvent::Fault { fault, .. } = &event else {
        panic!("a fault event: {event:?}");
    };
    assert_eq!(fault.site, site);

    let main = || NamedPlace {
        pou: "Main".into(),
        file: "src/Main.st".into(),
        line: 17,
    };
    let running = |generation: Generation| {
        move |asked: FaultSite| (asked.generation == generation).then(main)
    };
    assert_eq!(
        runtime_event_to_json_placed(event.clone(), &running(Generation(7))),
        json!({"type": "fault", "error": "division by zero", "pou": "Main",
            "file": "src/Main.st", "line": 17, "time_ns": 5_000_000_000_i64})
    );
    assert_eq!(
        runtime_event_to_json_placed(event, &running(Generation(8))),
        json!({"type": "fault", "error": "division by zero", "time_ns": 5_000_000_000_i64})
    );
    let outside = RuntimeEvent::fault(&RuntimeError::WatchdogTimeout, time);
    assert_eq!(
        runtime_event_to_json_placed(outside, &|_| Some(main()))["pou"],
        serde_json::Value::Null
    );
}
