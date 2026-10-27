//! Forces and queued writes across the debugger's lifecycle boundaries (specification 11 §6.9.2):
//! pause and resume and a debugger disconnect keep a force, a release writes no replacement value,
//! and a deliberate stop or a fault clears forces and queued writes.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

/// The bound of the variable write queue (specification 11).
const CAPACITY: usize = 1_024;

const SOURCE: &str = r#"
VAR_GLOBAL
    forced_value : DINT := DINT#1;
    queued_value : DINT := DINT#1;
END_VAR

PROGRAM Main
forced_value := forced_value + DINT#1;
queued_value := queued_value + DINT#1;
END_PROGRAM
"#;

#[test]
fn pause_resume_preserves_active_force() {
    let mut harness = harness();
    let debug = attach(&mut harness);
    force(&debug);
    cycle_clean(&mut harness, "pre-pause forced");

    let debugger = debug.compiled().expect("the compiled program's debugger");
    debugger.pause();
    debugger.resume();
    cycle_clean(&mut harness, "post-resume forced");

    assert_eq!(output_dint(&harness, "forced_value"), 50);
}

#[test]
fn nonterminating_debug_disconnect_preserves_active_force() {
    let mut harness = harness();
    let session = attach(&mut harness);
    force(&session);
    drop(session);

    cycle_clean(&mut harness, "post-disconnect forced");

    assert_eq!(output_dint(&harness, "forced_value"), 50);
}

#[test]
fn release_removes_force_without_replacement_write() {
    let mut harness = harness();
    let debug = attach(&mut harness);
    force(&debug);
    cycle_clean(&mut harness, "forced");
    assert_eq!(output_dint(&harness, "forced_value"), 50);

    debug.release_global("forced_value");
    cycle_clean(&mut harness, "released");

    assert_eq!(output_dint(&harness, "forced_value"), 51);
}

#[test]
fn deliberate_stop_boundary_clears_debug_mutations() {
    let mut harness = harness();
    let debug = attach(&mut harness);
    force(&debug);
    debug
        .try_enqueue_global_write("queued_value", Value::DInt(70))
        .expect("a new target is inside the bound");

    harness
        .runtime_mut()
        .apply_io_safe_state()
        .expect("deliberate stop safe-state boundary must succeed");
    cycle_clean(&mut harness, "post-stop");

    assert_eq!(output_dint(&harness, "forced_value"), 2);
    assert_eq!(output_dint(&harness, "queued_value"), 2);
}

#[test]
fn fault_boundary_clears_debug_mutations() {
    let mut harness = harness();
    let debug = attach(&mut harness);
    force(&debug);
    debug
        .try_enqueue_global_write("queued_value", Value::DInt(70))
        .expect("a new target is inside the bound");

    let _ = harness
        .runtime_mut()
        .simulation_fault("force lifecycle test fault");
    assert!(harness.runtime().faulted());

    assert_eq!(
        debug.forced_var_value(&ForcedVarTarget::Global("forced_value".into())),
        None,
        "the fault removes the force"
    );
    // The variable write queue takes a whole bound of new targets only when the fault removed the
    // queued write.
    for index in 0..CAPACITY {
        debug
            .try_enqueue_global_write(format!("fresh{index}"), Value::DInt(1))
            .expect("the fault removes the queued write");
    }
}

fn harness() -> TestHarness {
    TestHarness::from_source(SOURCE).expect("force lifecycle source must compile")
}

/// A debug control for the harness's program.
fn attach(harness: &mut TestHarness) -> DebugControl {
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    debug
}

/// Forces `forced_value` to 50.
fn force(debug: &DebugControl) {
    debug
        .try_force_global("forced_value", Value::DInt(50))
        .expect("a new force is inside the bound");
}

fn cycle_clean(harness: &mut TestHarness, phase: &str) {
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "{phase} cycle must succeed: {cycle:?}"
    );
}

fn output_dint(harness: &TestHarness, name: &str) -> i32 {
    match harness.try_get_output(name) {
        Ok(Value::DInt(value)) => value,
        Ok(value) => panic!("{name} has wrong type: {value:?}"),
        Err(error) => panic!("{name} read failed: {error}"),
    }
}
