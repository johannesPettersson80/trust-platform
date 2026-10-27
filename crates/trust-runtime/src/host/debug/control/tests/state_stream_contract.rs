use std::sync::mpsc::{channel, TryRecvError};

use super::super::*;
use crate::error::RuntimeError;
use crate::io::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::value::{Duration, Value};

fn fault_event(label: &str, millis: i64) -> RuntimeEvent {
    RuntimeEvent::fault(
        &RuntimeError::SimulationFault(label.into()),
        Duration::from_millis(millis),
    )
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
    assert!(control.snapshot().is_none(), "no program published a state");
    assert_eq!(control.hold_label(), None, "no debugger holds a scan");
    assert!(control.drain_runtime_events().is_empty());
    assert_eq!(control.runtime_events_dropped(), 0);
    let forced = control.forced_snapshot();
    assert!(forced.vars.is_empty() && forced.io.is_empty());
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
    assert!(control.lock().runtime_tx.is_none());
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
