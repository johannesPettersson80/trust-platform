//! A successful online change retires queued mutations.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.
use trust_runtime::debug::DebugControl;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// Spec 34 HOST-09 (lines 488-489): queued writes and forces are invalidated at the
/// successful schema boundary, so the next cycle applies neither of them.
#[test]
fn hosted_observation_successful_reload_drops_queued_writes_and_forces() {
    let source = "VAR_GLOBAL RETAIN count : DINT := DINT#3; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM";
    let mut harness = TestHarness::from_source(source).unwrap();
    harness
        .runtime_mut()
        .write_variable("count", &Value::DInt(12))
        .unwrap();
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    debug
        .try_enqueue_global_write("count", Value::DInt(77))
        .expect("a new target is inside the bound");
    debug
        .try_force_global("count", Value::DInt(44))
        .expect("a new force is inside the bound");

    harness
        .runtime_mut()
        .set_current_time(Duration::from_millis(35));
    harness
        .reload_source(&source.replace("count + DINT#1", "count + DINT#2"))
        .unwrap();
    harness.runtime_mut().execute_cycle().unwrap();

    // Retained 12 plus the replacement's increment; neither 77 nor 44 is applied.
    assert_eq!(
        harness.runtime().read_variable("count"),
        Ok(Value::DInt(14))
    );
}
