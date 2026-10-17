//! Exact event payloads, JSON projections and store bounds, as trust-runtime keeps them today.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use serde_json::json;
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

fn fault(error: &str) -> RuntimeEvent {
    RuntimeEvent::Fault {
        error: error.into(),
        time: Duration::from_millis(2),
    }
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
        RuntimeEvent::Fault {
            error: "fault".into(),
            time: Duration::from_millis(6),
        },
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
            RuntimeEvent::Fault {
                error: "division by zero".into(),
                time,
            },
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

#[test]
fn event_store_keeps_the_newest_1000_events_in_order() {
    let mut store = VecDeque::new();
    for index in 0..=EVENT_STORE_CAPACITY {
        store_event(&mut store, write_rejected(index));
    }
    let expected: Vec<_> = (1..=EVENT_STORE_CAPACITY).map(write_rejected).collect();
    assert_eq!(Vec::from(store), expected);
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
        fault("old fault"),
        cycle(2),
        RuntimeEvent::SafeStateFailed {
            root: "old fault".into(),
            error: "output write failed".into(),
            time: Duration::ZERO,
        },
        fault("new fault"),
        cycle(5),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        newest_faults(&store, 2),
        vec![fault("new fault"), fault("old fault")]
    );
    assert_eq!(newest_faults(&store, 1), vec![fault("new fault")]);
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
