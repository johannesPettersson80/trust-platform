use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoSafeState};
use trust_runtime::value::Value;
use trust_runtime::RestartMode;

const FORCE_LIFECYCLE_SOURCE: &str = r#"
VAR_GLOBAL
    forced_value : DINT := DINT#1;
    queued_value : DINT := DINT#1;
    output_bit AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
forced_value := forced_value + DINT#1;
queued_value := queued_value + DINT#1;
output_bit := FALSE;
END_PROGRAM
"#;

#[test]
fn restart_clears_active_force_and_queued_write() {
    let mut harness = TestHarness::from_source(FORCE_LIFECYCLE_SOURCE).expect("compile fixture");
    let debug = harness.runtime_mut().enable_debug();

    debug.force_global("forced_value", Value::DInt(50));
    debug.enqueue_global_write("queued_value", Value::DInt(70));

    harness
        .restart(RestartMode::Warm)
        .expect("warm restart must succeed");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "post-restart cycle: {cycle:?}");

    assert_eq!(harness.get_output("forced_value"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("queued_value"), Some(Value::DInt(2)));
}

#[test]
fn safe_state_boundary_clears_active_io_force() {
    let mut harness = TestHarness::from_source(FORCE_LIFECYCLE_SOURCE).expect("compile fixture");
    let debug = harness.runtime_mut().enable_debug();
    let address = IoAddress::parse("%QX0.0").expect("output address");

    debug.force_io(address.clone(), Value::Bool(true));
    let forced_cycle = harness.cycle();
    assert!(
        forced_cycle.errors.is_empty(),
        "forced cycle: {forced_cycle:?}"
    );
    assert_eq!(harness.runtime().io().read(&address), Ok(Value::Bool(true)));

    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address.clone(), Value::Bool(false)));
    harness.runtime_mut().set_io_safe_state(safe_state);
    harness
        .runtime_mut()
        .apply_io_safe_state()
        .expect("safe-state application");

    let post_stop_cycle = harness.cycle();
    assert!(
        post_stop_cycle.errors.is_empty(),
        "post-stop cycle: {post_stop_cycle:?}"
    );
    assert_eq!(
        harness.runtime().io().read(&address),
        Ok(Value::Bool(false))
    );
}
