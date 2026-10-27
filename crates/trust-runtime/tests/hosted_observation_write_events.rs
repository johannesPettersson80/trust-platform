//! Rejected management writes are reported with their cycle time and never fault the PLC.
//!
//! A compiled program reaches a variable by its symbol (specification 44 §13.3), so a write to a
//! name that names no variable is refused as an undefined variable.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.
use trust_runtime::{
    debug::{DebugControl, RuntimeEvent},
    error::RuntimeError,
    harness::TestHarness,
    value::{Duration, Value},
    CompiledRuntime,
};

/// A program that copies the button at `%IX0.0` to the lamp at `%QX0.0` every 10 ms.
const GPIO: &str = r#"
PROGRAM Gpio
VAR
    button AT %IX0.0 : BOOL;
    led AT %QX0.0 : BOOL;
END_VAR
led := button;
END_PROGRAM

CONFIGURATION BoardProbe
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Gpio;
END_RESOURCE
END_CONFIGURATION
"#;

/// The program of [`GPIO`], loaded.
fn gpio() -> CompiledRuntime {
    TestHarness::from_source(GPIO)
        .expect("the GPIO program compiles")
        .into_runtime()
}

/// The program of [`GPIO`] with a debug control.
fn debugged() -> (CompiledRuntime, DebugControl) {
    let mut runtime = gpio();
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    (runtime, debug)
}

/// The error text of a write to `missing`, which names no variable.
fn undefined() -> String {
    RuntimeError::UndefinedVariable("missing".into()).to_string()
}

/// Spec 34 HOST-09 (lines 503-506): a rejected queued write reports its destination,
/// error and the cycle time at which it was attempted.
#[test]
fn hosted_observation_rejected_queued_write_reports_the_cycle_time() {
    let (mut runtime, debug) = debugged();
    debug
        .try_enqueue_global_write("missing", Value::DInt(7))
        .expect("a new target is inside the bound");
    runtime.set_current_time(Duration::from_millis(35));

    runtime
        .execute_cycle()
        .expect("an invalid engineering request is not a PLC fault");
    assert!(!runtime.faulted());
    let rejected: Vec<bool> = debug
        .drain_runtime_events()
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::WriteRejected {
                target,
                error,
                time,
            } => Some(
                target.as_str() == "missing"
                    && *error == undefined()
                    && *time == Duration::from_millis(35),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        rejected,
        vec![true],
        "exactly one rejection, stamped with 35 ms"
    );
}

/// Spec 34 HOST-09 (lines 503-506): a rejected forced write reports its destination,
/// error and cycle time, and the force does not fault the PLC.
#[test]
fn hosted_observation_rejected_forced_write_reports_the_cycle_time() {
    let (mut runtime, debug) = debugged();
    debug
        .try_force_global("missing", Value::DInt(7))
        .expect("a new force is inside the bound");
    runtime.set_current_time(Duration::from_millis(10));

    runtime
        .execute_cycle()
        .expect("a rejected force is not a PLC fault");
    assert!(!runtime.faulted());
    assert!(runtime.read_variable("missing").is_err());
    let events = debug.drain_runtime_events();
    assert!(
        events.iter().any(|event| matches!(event,
            RuntimeEvent::WriteRejected { target, error, time }
                if target.as_str() == "missing"
                    && *error == undefined()
                    && *time == Duration::from_millis(10))),
        "expected a forced-write rejection stamped with 10 ms, got {events:?}"
    );
}
