//! The exit of a device's program before a program loaded in place replaces it (specification 44
//! §6.7; specification 10 §4.8): the loop leaves the program that ended first, as before a
//! download, the exit's fault does not stop the load, and the event records of the program loaded
//! in its place begin with that fault. Every expected value is worked out by hand from those rules.

use alloc::string::ToString;
use alloc::vec::Vec;

use serde_json::json;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value;

use crate::events::RuntimeEvent;
use crate::tests::runner_ports::{ms, CycleCall, Fixture, TestRunner};

/// The fault events the loop's records hold.
fn recorded_faults(runner: &TestRunner<'_>) -> Vec<RuntimeEvent> {
    runner
        .shell
        .records
        .read_events(|events| {
            events
                .iter()
                .filter(|event| matches!(event, RuntimeEvent::Fault { .. }))
                .cloned()
                .collect()
        })
        .expect("the records read")
}

/// A load in place on a device without a program loader: the program that ended leaves first,
/// its exit's division by zero is recorded at its time zero, and the load fails only for the
/// missing program. Catches a load that drops the program without its exit, and the exit's fault
/// lost or taken for the load's failure.
#[test]
fn a_load_in_place_leaves_the_program_that_ended_first() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.take_calls();
    fixture.script.borrow_mut().failing_leave = Some(RuntimeError::DivisionByZero);
    let error = runner
        .reload(RestartMode::Cold)
        .expect_err("the device has no program loader");
    assert!(
        error.to_string().contains("no installed program"),
        "{error}"
    );
    assert_eq!(fixture.take_calls(), [CycleCall::Leave]);
    assert_eq!(
        recorded_faults(&runner),
        [RuntimeEvent::fault(
            &RuntimeError::DivisionByZero,
            value::Duration::ZERO
        )]
    );
}

/// The program loaded in place begins its records, which start empty with it (specification 34
/// STATE-08), with the fault of the exit of the program it replaced, at its time zero; its first
/// scan at 0 ms adds no fault, and `faults` at 1 ms lists that one. Catches the fault recorded
/// before the records start again, and so lost.
#[test]
fn the_program_loaded_in_place_records_the_exit_fault_first() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    runner.left_fault = Some(RuntimeError::DivisionByZero);
    runner.begin_program(Vec::new());
    fixture.pass_at(&mut runner, 0);
    fixture.clock.set(ms(1));
    let faults = fixture.call(&mut runner, &json!({"id": 1, "type": "faults"}));
    assert_eq!(
        faults["result"]["faults"],
        json!([{"type": "fault", "error": "division by zero", "time_ns": 0}])
    );
}
