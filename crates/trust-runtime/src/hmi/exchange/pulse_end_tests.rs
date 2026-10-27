//! Specification 39, §12.6 and §21.5 item 1: the exchange owns a pulse's end. It is scheduled by
//! the call that applies the pulse, written at the first call after the pulse's duration whether
//! or not the requester is still there, tried again at every call until it is written, and told
//! to the requester's report.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::{HmiExchange, PulseEnd, PulseEndReport};
use crate::harness::TestHarness;
use crate::hmi::commands::write::{WriteAction, Written};
use crate::hmi::member_paths::{member_value, MemberPath};
use crate::value::Value;

fn plant() -> TestHarness {
    let mut plant =
        TestHarness::from_source("PROGRAM Main VAR Pulse : BOOL; END_VAR END_PROGRAM").unwrap();
    plant.cycle();
    plant
}

fn pulse_path() -> MemberPath {
    MemberPath::parse("Main.Pulse").unwrap()
}

fn value(plant: &TestHarness, path: &MemberPath) -> Option<Value> {
    plant
        .runtime()
        .with_program_view(|program| member_value(path, program))
}

/// One call of `Hmi.Display` serving the exchange from the plant's program.
fn serve(exchange: &HmiExchange, plant: &mut TestHarness) {
    plant
        .runtime_mut()
        .with_program(|program| exchange.serve(program, 0, &[]));
}

/// A report that keeps what it is told.
fn recorder() -> (PulseEndReport, Arc<Mutex<Vec<PulseEnd>>>) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let keep = told.clone();
    (Box::new(move |end| keep.lock().unwrap().push(end)), told)
}

/// Queues a pulse as the web command does and goes away, as a browser that disconnects does.
fn pulse_and_leave(exchange: &HmiExchange, duration: Duration, report: PulseEndReport) {
    let (_, reply) = exchange.queue_write_with_id(
        pulse_path(),
        WriteAction::Set(json!(true)),
        Some(duration),
        Some(report),
    );
    drop(reply);
}

#[test]
fn a_pulse_end_is_scheduled_as_the_pulse_starts_and_written_after_its_duration() {
    let mut plant = plant();
    let exchange = HmiExchange::new();
    let (report, told) = recorder();
    pulse_and_leave(&exchange, Duration::from_millis(80), report);
    serve(&exchange, &mut plant);
    assert_eq!(value(&plant, &pulse_path()), Some(Value::Bool(true)));
    assert_eq!(
        exchange.state().pulse_ends.len(),
        1,
        "scheduled with the start"
    );
    serve(&exchange, &mut plant);
    assert_eq!(
        value(&plant, &pulse_path()),
        Some(Value::Bool(true)),
        "not before its duration"
    );
    assert!(told.lock().unwrap().is_empty());
    std::thread::sleep(Duration::from_millis(100));
    serve(&exchange, &mut plant);
    assert_eq!(value(&plant, &pulse_path()), Some(Value::Bool(false)));
    assert_eq!(exchange.state().pulse_ends.len(), 0);
    assert_eq!(
        *told.lock().unwrap(),
        vec![PulseEnd::Written {
            written: Written {
                old: Value::Bool(true),
                readback: Value::Bool(false),
            },
            attempts: 1,
        }]
    );
}

#[test]
fn a_pulse_end_due_while_no_call_runs_is_written_at_the_next_call() {
    let mut plant = plant();
    let exchange = HmiExchange::new();
    let (report, told) = recorder();
    pulse_and_leave(&exchange, Duration::from_millis(10), report);
    serve(&exchange, &mut plant);
    // The program does not call Hmi.Display for longer than any request would have waited.
    let paused = Instant::now();
    std::thread::sleep(Duration::from_millis(50));
    assert!(paused.elapsed() >= Duration::from_millis(50));
    assert_eq!(value(&plant, &pulse_path()), Some(Value::Bool(true)));
    serve(&exchange, &mut plant);
    assert_eq!(value(&plant, &pulse_path()), Some(Value::Bool(false)));
    assert!(matches!(
        told.lock().unwrap().as_slice(),
        [PulseEnd::Written { attempts: 1, .. }]
    ));
}

#[test]
fn an_end_that_cannot_be_written_stays_due_and_is_tried_at_every_call() {
    let mut plant = plant();
    let exchange = HmiExchange::new();
    let (report, told) = recorder();
    let gone = MemberPath::parse("Main.Gone").unwrap();
    exchange
        .state()
        .pulse_ends
        .schedule(gone, Instant::now(), Duration::ZERO, Some(report));
    for _ in 0..3 {
        serve(&exchange, &mut plant);
        assert_eq!(
            exchange.state().pulse_ends.len(),
            1,
            "kept for the next call"
        );
    }
    let told = told.lock().unwrap();
    assert_eq!(told.len(), 1, "the failure is told once: {told:?}");
    assert!(
        matches!(&told[0], PulseEnd::Retrying { reason } if reason.contains("Main.Gone")),
        "{told:?}"
    );
}

#[test]
fn a_later_command_to_the_point_replaces_the_waiting_end_and_tells_it() {
    let mut plant = plant();
    let exchange = HmiExchange::new();
    let (report, told) = recorder();
    pulse_and_leave(&exchange, Duration::from_secs(60), report);
    serve(&exchange, &mut plant);
    assert_eq!(value(&plant, &pulse_path()), Some(Value::Bool(true)));
    let reply = exchange.queue_write(pulse_path(), WriteAction::Set(json!(false)));
    serve(&exchange, &mut plant);
    assert!(reply.try_recv().unwrap().is_ok());
    assert_eq!(value(&plant, &pulse_path()), Some(Value::Bool(false)));
    assert_eq!(exchange.state().pulse_ends.len(), 0);
    assert_eq!(
        *told.lock().unwrap(),
        vec![PulseEnd::Replaced { attempts: 0 }]
    );
}

#[test]
fn a_pulse_that_is_not_applied_schedules_no_end() {
    let mut plant = plant();
    let exchange = HmiExchange::new();
    let (report, told) = recorder();
    let (_, reply) = exchange.queue_write_with_id(
        MemberPath::parse("Main.Gone").unwrap(),
        WriteAction::Set(json!(true)),
        Some(Duration::ZERO),
        Some(report),
    );
    serve(&exchange, &mut plant);
    assert!(reply.try_recv().unwrap().is_err());
    assert_eq!(exchange.state().pulse_ends.len(), 0);
    assert!(told.lock().unwrap().is_empty());
}
