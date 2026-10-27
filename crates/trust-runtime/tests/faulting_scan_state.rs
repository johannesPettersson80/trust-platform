#![cfg(feature = "full")]
//! The state a hosted controller keeps after a scan that faults while its program executes
//! (specification 11 section 6.6, "State after a faulting scan"; sections 6.9.2 and 6.7).
//!
//! The faulting scan applies a queued write and a force at its boundary, flips `toggle`, and then
//! divides by zero. Nothing it completed is undone, the failing statement stores nothing, the output
//! image keeps the value of the last completed scan, the cycle number does not advance, and the fault
//! removes the force without writing a replacement value. The statement before the failing one is what
//! the program of `stateful_lifecycle_sequences.rs` cannot show, because its first statement divides.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use trust_runtime::RestartMode;

const SOURCE: &str = r#"
VAR_GLOBAL
    toggle AT %QX0.0 : BOOL := FALSE;
    divisor : DINT := DINT#1;
    quotient : DINT := DINT#0;
    queued_target : DINT := DINT#0;
    forced_target : DINT := DINT#0;
END_VAR

PROGRAM Main
toggle := NOT toggle;
quotient := DINT#100 / divisor;
END_PROGRAM
"#;

/// A controller with one completed scan (`toggle` TRUE, `quotient` 100, image TRUE), a zero divisor, a
/// queued write to `queued_target` and a force on `forced_target`: the next scan faults.
fn controller_about_to_fault() -> (TestHarness, DebugControl) {
    let mut harness = TestHarness::from_source(SOURCE).expect("fixture compiles");
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    harness
        .runtime_mut()
        .execute_cycle()
        .expect("the first scan completes");
    assert_eq!(global(&harness, "toggle"), Value::Bool(true));
    assert_eq!(image_bit(&harness), Value::Bool(true));
    assert_eq!(global(&harness, "quotient"), Value::DInt(100));
    harness
        .runtime_mut()
        .write_variable("divisor", &Value::DInt(0))
        .expect("a checked write between scans is accepted");
    debug
        .try_enqueue_global_write("queued_target", Value::DInt(7))
        .expect("a new target is inside the bound");
    debug
        .try_force_global("forced_target", Value::DInt(5))
        .expect("a new force is inside the bound");
    (harness, debug)
}

fn global(harness: &TestHarness, name: &str) -> Value {
    harness
        .get_output(name)
        .unwrap_or_else(|| panic!("global {name} exists"))
}

/// The published output image of `toggle`.
fn image_bit(harness: &TestHarness) -> Value {
    harness
        .get_direct_output("%QX0.0")
        .expect("the output image reads")
}

/// Spec 11 6.6, state after a faulting scan: the variables keep every completed store (the boundary
/// writes and the statement before the failing one), the failing statement stores nothing, the
/// process image keeps the last completed scan's value, the cycle number does not advance, and the
/// fault removes the force.
#[test]
fn faulting_scan_keeps_what_it_completed_and_commits_no_output() {
    let (mut harness, debug) = controller_about_to_fault();

    assert_eq!(
        harness.runtime_mut().execute_cycle(),
        Err(RuntimeError::DivisionByZero)
    );

    assert_eq!(
        global(&harness, "queued_target"),
        Value::DInt(7),
        "the queued write was applied at the boundary and stays"
    );
    assert_eq!(
        global(&harness, "forced_target"),
        Value::DInt(5),
        "the forced value was applied at the boundary and stays"
    );
    assert_eq!(
        global(&harness, "toggle"),
        Value::Bool(false),
        "the statement before the failing one completed and stays"
    );
    assert_eq!(
        global(&harness, "quotient"),
        Value::DInt(100),
        "the failing statement stores nothing"
    );
    assert_eq!(
        image_bit(&harness),
        Value::Bool(true),
        "the scan reached no output phase, so the image keeps the last completed scan's value"
    );
    let runtime = harness.runtime();
    assert_eq!(
        runtime.cycle_counter(),
        1,
        "the faulting scan does not advance the cycle number"
    );
    assert!(runtime.faulted());
    assert_eq!(runtime.last_fault(), Some(RuntimeError::DivisionByZero));
    assert_eq!(
        debug.forced_var_value(&ForcedVarTarget::Global("forced_target".into())),
        None,
        "the fault is a clearing boundary: the force is gone and wrote no replacement value"
    );
}

/// Spec 11 6.6 and 6.9.2: the recovery of the faulted resource starts from the declared initial values
/// and does not replay the consumed write or the removed force; the output image is carried over.
#[test]
fn recovery_after_a_faulting_scan_does_not_replay_the_boundary_writes() {
    let (mut harness, debug) = controller_about_to_fault();
    assert_eq!(
        harness.runtime_mut().execute_cycle(),
        Err(RuntimeError::DivisionByZero)
    );

    harness
        .runtime_mut()
        .restart(RestartMode::Warm)
        .expect("a restart recovers the faulted resource");
    assert!(!harness.runtime().faulted());
    assert_eq!(harness.runtime().cycle_counter(), 0);
    assert_eq!(
        global(&harness, "toggle"),
        Value::Bool(false),
        "the restart gives the variable its initial value"
    );
    assert_eq!(
        image_bit(&harness),
        Value::Bool(true),
        "the restart carries the output image over: the next scan has not published yet"
    );

    harness
        .runtime_mut()
        .execute_cycle()
        .expect("the recovered resource scans");

    assert_eq!(global(&harness, "queued_target"), Value::DInt(0));
    assert_eq!(global(&harness, "forced_target"), Value::DInt(0));
    assert_eq!(global(&harness, "divisor"), Value::DInt(1));
    assert_eq!(global(&harness, "quotient"), Value::DInt(100));
    assert_eq!(global(&harness, "toggle"), Value::Bool(true));
    assert_eq!(image_bit(&harness), Value::Bool(true));
    assert_eq!(harness.runtime().cycle_counter(), 1);
    assert_eq!(
        debug.forced_var_value(&ForcedVarTarget::Global("forced_target".into())),
        None
    );
}
