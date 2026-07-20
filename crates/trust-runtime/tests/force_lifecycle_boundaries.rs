use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

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
    let debug = harness.runtime_mut().enable_debug();
    debug.force_global("forced_value", Value::DInt(50));
    cycle_clean(&mut harness, "pre-pause forced");

    debug.pause();
    debug.continue_run();
    cycle_clean(&mut harness, "post-resume forced");

    assert_eq!(output_dint(&harness, "forced_value"), 50);
}

#[test]
fn nonterminating_debug_disconnect_preserves_active_force() {
    let mut harness = harness();
    let session = harness.runtime_mut().enable_debug();
    session.force_global("forced_value", Value::DInt(50));
    drop(session);

    cycle_clean(&mut harness, "post-disconnect forced");

    assert_eq!(output_dint(&harness, "forced_value"), 50);
}

#[test]
fn release_removes_force_without_replacement_write() {
    let mut harness = harness();
    let debug = harness.runtime_mut().enable_debug();
    debug.force_global("forced_value", Value::DInt(50));
    cycle_clean(&mut harness, "forced");
    assert_eq!(output_dint(&harness, "forced_value"), 50);

    debug.release_global("forced_value");
    cycle_clean(&mut harness, "released");

    assert_eq!(output_dint(&harness, "forced_value"), 51);
}

#[test]
fn deliberate_stop_boundary_clears_debug_mutations() {
    let mut harness = harness();
    let debug = harness.runtime_mut().enable_debug();
    debug.force_global("forced_value", Value::DInt(50));
    debug.enqueue_global_write("queued_value", Value::DInt(70));

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
    let debug = harness.runtime_mut().enable_debug();
    debug.force_global("forced_value", Value::DInt(50));
    debug.enqueue_global_write("queued_value", Value::DInt(70));

    let _ = harness
        .runtime_mut()
        .simulation_fault("force lifecycle test fault");
    assert!(harness.runtime().faulted());
    harness.runtime_mut().clear_fault();
    cycle_clean(&mut harness, "post-fault recovery");

    assert_eq!(output_dint(&harness, "forced_value"), 2);
    assert_eq!(output_dint(&harness, "queued_value"), 2);
}

fn harness() -> TestHarness {
    TestHarness::from_source(SOURCE).expect("force lifecycle source must compile")
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
