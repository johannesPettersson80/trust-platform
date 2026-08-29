use std::sync::mpsc::{channel, TryRecvError};

use super::super::*;
use crate::io::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::memory::{FrameId, VariableStorage};
use crate::program_model::Expr;
use crate::value::{DateTimeProfile, Duration, Value};
use trust_hir::types::TypeRegistry;

fn fault_event(label: &str, millis: i64) -> RuntimeEvent {
    RuntimeEvent::Fault {
        error: label.to_string(),
        time: Duration::from_millis(millis),
    }
}

fn io_snapshot(scan: u64, value: i64) -> IoSnapshot {
    IoSnapshot {
        scan: Some(scan),
        outputs: vec![IoSnapshotEntry {
            name: Some("output".into()),
            address: IoAddress::parse("%QW0").expect("address"),
            value_type: None,
            value_type_name: None,
            value: IoSnapshotValue::Value(Value::LInt(value)),
            source: None,
        }],
        ..IoSnapshot::default()
    }
}

#[test]
fn initial_state_queries_report_only_declared_defaults() {
    let control = DebugControl::new();
    assert_eq!(control.mode(), DebugMode::Running);
    assert!(!control.is_paused());
    assert_eq!(control.last_location(), None);
    assert_eq!(control.last_call_depth(), 0);
    assert_eq!(control.current_thread(), Some(1));
    assert_eq!(control.target_thread(), None);
    assert_eq!(control.breakpoint_generation(1), None);
    assert_eq!(control.frame_location(FrameId(1)), None);
    assert!(control.frame_locations().is_empty());
    assert!(control.snapshot().is_none());
    assert!(control.last_stop().is_none());
    assert!(control.drain_logs().is_empty());
    assert!(control.drain_stops().is_empty());
    assert!(control.drain_runtime_events().is_empty());
}

#[test]
fn current_thread_setter_round_trips_some_and_none() {
    let control = DebugControl::new();
    control.set_current_thread(Some(42));
    assert_eq!(control.current_thread(), Some(42));
    control.set_current_thread(None);
    assert_eq!(control.current_thread(), None);
}

#[test]
fn breakpoint_query_returns_an_owned_copy() {
    let control = DebugControl::new();
    control.set_breakpoints_for_file(
        1,
        vec![DebugBreakpoint::new(SourceLocation::new(1, 10, 20))],
    );

    let mut copy = control.breakpoints();
    copy[0].hits = 99;
    copy.clear();

    let current = control.breakpoints();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].hits, 0);
}

#[test]
fn frame_location_queries_return_owned_exact_records() {
    let control = DebugControl::new();
    let first = SourceLocation::new(1, 10, 20);
    let second = SourceLocation::new(2, 30, 40);
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.frame_locations.insert(FrameId(7), first);
        state.frame_locations.insert(FrameId(8), second);
    }

    assert_eq!(control.frame_location(FrameId(7)), Some(first));
    assert_eq!(control.frame_location(FrameId(9)), None);

    let mut copy = control.frame_locations();
    copy.remove(&FrameId(7));
    assert_eq!(control.frame_locations().len(), 2);
}

#[test]
fn log_drain_is_ordered_and_atomic() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.logs.push(DebugLog {
            message: "first".into(),
            location: Some(SourceLocation::new(1, 1, 2)),
        });
        state.logs.push(DebugLog {
            message: "second".into(),
            location: None,
        });
    }

    let logs = control.drain_logs();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].message, "first");
    assert_eq!(logs[1].message, "second");
    assert!(control.drain_logs().is_empty());
}

#[test]
fn stop_drain_preserves_order_but_not_last_stop() {
    let control = DebugControl::new();
    let first = DebugStop {
        reason: DebugStopReason::Breakpoint,
        location: Some(SourceLocation::new(1, 1, 2)),
        thread_id: Some(1),
        breakpoint_generation: Some(3),
    };
    let second = DebugStop {
        reason: DebugStopReason::Pause,
        location: None,
        thread_id: Some(2),
        breakpoint_generation: None,
    };
    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state");
        state.stops.extend([first.clone(), second.clone()]);
        state.last_stop = Some(second);
    }

    let stops = control.drain_stops();
    assert_eq!(stops.len(), 2);
    assert_eq!(stops[0].reason, DebugStopReason::Breakpoint);
    assert_eq!(stops[1].reason, DebugStopReason::Pause);
    assert!(control.drain_stops().is_empty());
    assert_eq!(
        control.last_stop().expect("last stop").reason,
        DebugStopReason::Pause
    );
}

#[test]
fn last_stop_query_returns_an_owned_copy() {
    let control = DebugControl::new();
    {
        let (lock, _) = &*control.state;
        lock.lock().expect("debug state").last_stop = Some(DebugStop {
            reason: DebugStopReason::Entry,
            location: Some(SourceLocation::new(1, 1, 2)),
            thread_id: Some(1),
            breakpoint_generation: None,
        });
    }

    let mut copy = control.last_stop().expect("last stop");
    copy.reason = DebugStopReason::Pause;
    assert_eq!(copy.reason, DebugStopReason::Pause);
    assert_eq!(
        control.last_stop().expect("last stop").reason,
        DebugStopReason::Entry
    );
}

#[test]
fn runtime_event_drain_preserves_arrival_order() {
    let control = DebugControl::new();
    control.push_runtime_event(fault_event("first", 1));
    control.push_runtime_event(fault_event("second", 2));

    let events = control.drain_runtime_events();
    assert_eq!(
        events,
        vec![fault_event("first", 1), fault_event("second", 2)]
    );
    assert!(control.drain_runtime_events().is_empty());
}

#[test]
fn live_runtime_sender_receives_without_duplicate_buffering() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    control.set_runtime_sender(tx);

    let event = fault_event("live", 1);
    control.push_runtime_event(event.clone());

    assert_eq!(rx.recv().expect("streamed event"), event);
    assert!(control.drain_runtime_events().is_empty());
}

#[test]
fn replacing_runtime_sender_routes_only_to_the_new_receiver() {
    let control = DebugControl::new();
    let (first_tx, first_rx) = channel();
    let (second_tx, second_rx) = channel();
    control.set_runtime_sender(first_tx);
    control.set_runtime_sender(second_tx);

    let event = fault_event("replacement", 1);
    control.push_runtime_event(event.clone());

    assert!(matches!(
        first_rx.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
    assert_eq!(second_rx.recv().expect("new receiver"), event);
    assert!(control.drain_runtime_events().is_empty());
}

#[test]
fn closed_runtime_sender_falls_back_without_losing_triggering_event() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    control.set_runtime_sender(tx);
    drop(rx);

    let event = fault_event("closed", 1);
    control.push_runtime_event(event.clone());

    assert_eq!(control.drain_runtime_events(), vec![event]);
    let (lock, _) = &*control.state;
    assert!(lock.lock().expect("debug state").runtime_tx.is_none());
}

#[test]
fn clearing_runtime_sender_buffers_subsequent_events() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    control.set_runtime_sender(tx);
    control.clear_runtime_sender();

    let event = fault_event("buffered", 1);
    control.push_runtime_event(event.clone());

    assert!(matches!(rx.try_recv(), Err(TryRecvError::Disconnected)));
    assert_eq!(control.drain_runtime_events(), vec![event]);
}

#[test]
fn live_io_sender_receives_an_owned_coherent_snapshot() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    control.set_io_sender(tx);
    control.push_io_snapshot(io_snapshot(7, 42));

    let received = rx.recv().expect("I/O snapshot");
    assert_eq!(received.scan, Some(7));
    assert_eq!(received.outputs.len(), 1);
    assert!(matches!(
        received.outputs[0].value,
        IoSnapshotValue::Value(Value::LInt(42))
    ));
}

#[test]
fn replacing_io_sender_routes_only_to_the_new_receiver() {
    let control = DebugControl::new();
    let (first_tx, first_rx) = channel();
    let (second_tx, second_rx) = channel();
    control.set_io_sender(first_tx);
    control.set_io_sender(second_tx);

    control.push_io_snapshot(io_snapshot(8, 43));

    assert!(matches!(
        first_rx.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
    assert_eq!(second_rx.recv().expect("new receiver").scan, Some(8));
}

#[test]
fn clearing_io_sender_stops_delivery_without_synthetic_buffer() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    control.set_io_sender(tx);
    control.clear_io_sender();
    control.push_io_snapshot(io_snapshot(9, 44));

    assert!(matches!(rx.try_recv(), Err(TryRecvError::Disconnected)));
    assert!(control.drain_runtime_events().is_empty());
}

#[test]
fn closed_io_receiver_does_not_create_a_runtime_fault() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    control.set_io_sender(tx);
    drop(rx);

    control.push_io_snapshot(io_snapshot(10, 45));
    assert!(control.drain_runtime_events().is_empty());
}

#[test]
fn log_and_stop_sender_registration_replaces_and_clears_exact_slots() {
    let control = DebugControl::new();
    let (first_log_tx, _first_log_rx) = channel();
    let (second_log_tx, _second_log_rx) = channel();
    let (first_stop_tx, _first_stop_rx) = channel();
    let (second_stop_tx, _second_stop_rx) = channel();

    control.set_log_sender(first_log_tx);
    control.set_log_sender(second_log_tx);
    control.set_stop_sender(first_stop_tx);
    control.set_stop_sender(second_stop_tx);
    {
        let (lock, _) = &*control.state;
        let state = lock.lock().expect("debug state");
        assert!(state.log_tx.is_some());
        assert!(state.stop_tx.is_some());
    }

    control.clear_log_sender();
    control.clear_stop_sender();
    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state");
    assert!(state.log_tx.is_none());
    assert!(state.stop_tx.is_none());
}

#[test]
fn snapshot_query_returns_an_owned_copy() {
    let control = DebugControl::new();
    let mut storage = VariableStorage::new();
    storage.set_global("value", Value::DInt(7));
    control.refresh_snapshot_from_storage(&storage, Duration::from_millis(3));

    let mut first = control.snapshot().expect("snapshot");
    first.storage.set_global("value", Value::DInt(99));
    first.now = Duration::from_millis(4);

    let second = control.snapshot().expect("snapshot");
    assert_eq!(second.storage.get_global("value"), Some(&Value::DInt(7)));
    assert_eq!(second.now, Duration::from_millis(3));
}

#[test]
fn refresh_from_storage_captures_source_at_call_time() {
    let control = DebugControl::new();
    let mut storage = VariableStorage::new();
    storage.set_global("value", Value::DInt(7));

    control.refresh_snapshot_from_storage(&storage, Duration::from_millis(3));
    storage.set_global("value", Value::DInt(99));

    let snapshot = control.snapshot().expect("snapshot");
    assert_eq!(snapshot.storage.get_global("value"), Some(&Value::DInt(7)));
    assert_eq!(snapshot.now, Duration::from_millis(3));
}

#[test]
fn with_snapshot_does_not_invoke_callback_when_absent() {
    let control = DebugControl::new();
    let mut invoked = false;
    let result = control.with_snapshot(|_| {
        invoked = true;
        7
    });

    assert_eq!(result, None);
    assert!(!invoked);
}

#[test]
fn with_snapshot_mutates_only_the_stored_snapshot() {
    let control = DebugControl::new();
    let mut storage = VariableStorage::new();
    storage.set_global("value", Value::DInt(7));
    control.refresh_snapshot_from_storage(&storage, Duration::from_millis(3));

    assert_eq!(
        control.with_snapshot(|snapshot| {
            snapshot.storage.set_global("value", Value::DInt(8));
            snapshot.now = Duration::from_millis(4);
            "updated"
        }),
        Some("updated")
    );
    let snapshot = control.snapshot().expect("snapshot");
    assert_eq!(snapshot.storage.get_global("value"), Some(&Value::DInt(8)));
    assert_eq!(snapshot.now, Duration::from_millis(4));
    assert_eq!(storage.get_global("value"), Some(&Value::DInt(7)));
}

#[test]
fn first_watch_refresh_sets_edge_and_same_value_does_not_repeat_it() {
    let control = DebugControl::new();
    control.register_watch_expression(Expr::Literal(Value::DInt(7)));
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = DebugRuntimeContext {
        storage: &mut storage,
        registry: &registry,
        stdlib: None,
        profile: DateTimeProfile::default(),
        current_instance: None,
        now: Duration::from_millis(1),
    };

    control.refresh_snapshot(&mut ctx);
    assert!(control.take_watch_changed());
    assert!(!control.take_watch_changed());

    control.refresh_snapshot(&mut ctx);
    assert!(!control.take_watch_changed());
}

#[test]
fn watch_change_flag_accumulates_until_read() {
    let control = DebugControl::new();
    control.register_watch_expression(Expr::Name("watched".into()));
    let mut storage = VariableStorage::new();
    storage.set_global("watched", Value::DInt(1));
    let registry = TypeRegistry::new();

    {
        let mut ctx = DebugRuntimeContext {
            storage: &mut storage,
            registry: &registry,
            stdlib: None,
            profile: DateTimeProfile::default(),
            current_instance: None,
            now: Duration::from_millis(1),
        };
        control.refresh_snapshot(&mut ctx);
    }
    storage.set_global("watched", Value::DInt(2));
    {
        let mut ctx = DebugRuntimeContext {
            storage: &mut storage,
            registry: &registry,
            stdlib: None,
            profile: DateTimeProfile::default(),
            current_instance: None,
            now: Duration::from_millis(2),
        };
        control.refresh_snapshot(&mut ctx);
    }

    assert!(control.take_watch_changed());
    assert!(!control.take_watch_changed());
}

#[test]
fn watch_evaluation_failure_is_an_observable_value_transition() {
    let control = DebugControl::new();
    control.register_watch_expression(Expr::Name("watched".into()));
    let mut storage = VariableStorage::new();
    storage.set_global("watched", Value::DInt(1));
    let registry = TypeRegistry::new();

    {
        let mut ctx = DebugRuntimeContext {
            storage: &mut storage,
            registry: &registry,
            stdlib: None,
            profile: DateTimeProfile::default(),
            current_instance: None,
            now: Duration::ZERO,
        };
        control.refresh_snapshot(&mut ctx);
    }
    assert!(control.take_watch_changed());

    storage = VariableStorage::new();
    let mut ctx = DebugRuntimeContext {
        storage: &mut storage,
        registry: &registry,
        stdlib: None,
        profile: DateTimeProfile::default(),
        current_instance: None,
        now: Duration::from_millis(1),
    };
    control.refresh_snapshot(&mut ctx);
    assert!(control.take_watch_changed());
}

#[test]
fn clearing_watches_removes_entries_and_pending_edge() {
    let control = DebugControl::new();
    control.register_watch_expression(Expr::Literal(Value::DInt(7)));
    {
        let (lock, _) = &*control.state;
        lock.lock().expect("debug state").watch_changed = true;
    }

    control.clear_watch_expressions();

    let (lock, _) = &*control.state;
    assert!(lock.lock().expect("debug state").watches.is_empty());
    assert!(!control.take_watch_changed());
}
