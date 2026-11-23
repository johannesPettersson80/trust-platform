//! A tool's method call on a chip's loop (specification 11 §6.8.1, "Calling a method";
//! specification 44 §13.3): the loop lends its program to the request between two scans, the
//! kernel makes the call at the logical time of the last completed scan, and a fault of the method
//! faults the resource as a faulted scan does, alike on every runtime (specification 34 STATE-08).
//! The program is the board probe, whose scans run every 10 ms and halt on a fault; the test
//! scripts what its methods answer, and a call without a script is the kernel's.

use alloc::vec::Vec;
use core::time::Duration;

use serde_json::{json, Value as JsonValue};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::tool::MethodCallError;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use crate::tests::runner_ports::{CycleCall, Fixture, TestRunner};

/// `method.call` of `method` of `ISequence` of `instance`, with a set point of 21.5.
fn call(id: u64, instance: &str, method: &str) -> JsonValue {
    json!({"id": id, "type": "method.call", "params": {
        "instance": instance, "interface": "ISequence", "method": method,
        "inputs": {"Setpoint": "21.5"}
    }})
}

/// The record of a call of `method` of `ISequence` of `instance` at `at` milliseconds.
fn method(instance: &str, method: &str, at: u64) -> CycleCall {
    CycleCall::Method {
        instance: instance.into(),
        interface: "ISequence".into(),
        method: method.into(),
        now: Duration::from_millis(at),
    }
}

/// Runs the probe's scans at 0 and 10 ms and forgets what the loop asked of it.
fn two_scans(fixture: &Fixture, runner: &mut TestRunner<'_>) {
    fixture.pass_at(runner, 0);
    fixture.pass_at(runner, 10);
    let _ = fixture.take_calls();
}

#[test]
fn a_running_program_answers_a_call_made_at_the_time_of_its_last_scan() {
    // The probe's scans run at 0 and 10 ms. A call at 15 ms is made between that scan and the next,
    // at the logical time of the last completed scan, 10 ms, and answers the method's result. A
    // call the kernel refuses, for an instance the probe does not have, answers the kernel's
    // reason, and the program runs on: its next scan is at 20 ms. Catches a call made at the wall
    // time, a result not answered, a call that never reaches the kernel, and a refusal that stops
    // the program.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    two_scans(&fixture, &mut runner);

    fixture
        .script
        .borrow_mut()
        .method_answers
        .push_back(Ok(Value::Bool(true)));
    let answered = fixture.call_at(&mut runner, 15, &call(1, "Plant", "Start"));
    assert_eq!(answered["ok"], json!(true), "{answered}");
    assert_eq!(answered["result"], json!({"value": true}));
    assert_eq!(fixture.take_calls(), [method("Plant", "Start", 10)]);

    let refused = fixture.call(&mut runner, &call(2, "Missing", "Start"));
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(refused["error"], json!("undefined variable 'Missing'"));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    let calls = fixture.take_calls();
    assert_eq!(calls.first(), Some(&method("Missing", "Start", 10)));
    assert!(
        calls.iter().any(|call| matches!(
            call,
            CycleCall::Scan { now, .. } if *now == Duration::from_millis(20)
        )),
        "{calls:?}"
    );
}

#[test]
fn a_method_that_faults_faults_the_resource_as_a_faulted_scan_does() {
    // Specification 44 §13.3: a fault of the method is the program's, and the resource faults as
    // at a faulted scan. The probe halts on a fault: a scan that divides by zero at 20 ms leaves
    // the fault latched and the loop's fault handling after it, which a method that divides by zero
    // at 15 ms leaves alike, after its call. The request is refused with the fault, the resource is
    // Faulted, no scan runs at 20 ms, and a further call is refused as a pause is
    // (`resource_not_running`). Catches a fault answered but not latched, a resource left running,
    // a fault handled otherwise than a scan's, and a call made into a faulted program.
    let scanned = Fixture::board_probe();
    let mut scanning = scanned.runner();
    two_scans(&scanned, &mut scanning);
    scanned
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    scanned.pass_at(&mut scanning, 20);
    assert_eq!(scanning.resource_state(), ResourceState::Faulted);
    let scan_fault = scanned.take_calls();
    assert!(
        matches!(scan_fault.first(), Some(CycleCall::Scan { .. })),
        "{scan_fault:?}"
    );

    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    two_scans(&fixture, &mut runner);
    fixture
        .script
        .borrow_mut()
        .method_answers
        .push_back(Err(MethodCallError::Fault(RuntimeError::DivisionByZero)));
    let failed = fixture.call_at(&mut runner, 15, &call(1, "Plant", "Fail"));
    assert_eq!(failed["ok"], json!(false), "{failed}");
    assert_eq!(
        failed["error"],
        json!("the method faulted: division by zero")
    );
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    let method_fault = fixture.take_calls();
    assert_eq!(method_fault.first(), Some(&method("Plant", "Fail", 10)));
    assert!(
        method_fault.contains(&CycleCall::Latch("division by zero".into())),
        "{method_fault:?}"
    );
    assert_eq!(method_fault[1..], scan_fault[1..]);

    fixture.pass_at(&mut runner, 20);
    let after = fixture.call(&mut runner, &call(2, "Plant", "Start"));
    assert_eq!(after["ok"], json!(false), "{after}");
    assert_eq!(
        after["error"],
        json!("resource is faulted; restart required")
    );
    assert_eq!(after["error_code"], json!("resource_not_running"));
    assert_eq!(fixture.take_calls(), Vec::<CycleCall>::new());
}

#[test]
fn a_stopped_program_takes_no_call() {
    // A stopped resource has no scans to fault: it refuses the call as it refuses a pause, and the
    // program is not asked. Catches a call made into a stopped program.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.call_at(&mut runner, 5, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let _ = fixture.take_calls();

    let reply = fixture.call(&mut runner, &call(2, "Plant", "Start"));
    assert_eq!(reply["ok"], json!(false), "{reply}");
    assert_eq!(reply["error"], json!("resource is stopped"));
    assert_eq!(reply["error_code"], json!("resource_not_running"));
    assert_eq!(fixture.take_calls(), Vec::<CycleCall>::new());
}
