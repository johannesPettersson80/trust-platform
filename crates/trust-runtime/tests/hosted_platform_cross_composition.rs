//! Spec 34 §4.3 HOST-07: the runtime-only and full compositions run the same saved
//! artifacts to the same values. This file has no feature gate, so the lead runs it
//! under both `--features full` (default) and `--no-default-features --features runtime-only`.

use trust_runtime::runtime_core::error::RuntimeError;
use trust_runtime::runtime_core::io_address::IoAddress;
use trust_runtime::runtime_core::value::{Duration, Value};
use trust_runtime::runtime_core::vm::PreparationLimits;
use trust_runtime::Runtime;

const PERIODIC: &[u8] = include_bytes!("fixtures/portability/stbc-2.0/program-v2.stbc");
const GPIO: &[u8] = include_bytes!("fixtures/portability/f401/gpio.stbc");

fn member(runtime: &Runtime, root: &str, name: &str) -> Value {
    let Some(Value::Instance(instance)) = runtime.storage().get_global(root) else {
        panic!("admitted root {root}")
    };
    runtime
        .storage()
        .get_instance_var(*instance, name)
        .unwrap()
        .clone()
}

/// Spec 34 §4.3 HOST-07 (line 357): the saved periodic artifact runs 40 activations over 0..=1000 ms with no fault in both compositions.
#[test]
fn both_compositions_run_the_saved_periodic_artifact_to_the_same_values() {
    let mut runtime = Runtime::from_bytes(PERIODIC, PreparationLimits::default()).unwrap();
    for ms in (0..=1000).step_by(10) {
        runtime.set_current_time(Duration::from_millis(ms));
        runtime.execute_cycle().unwrap();
    }
    assert_eq!(member(&runtime, "Plant", "activations"), Value::DInt(40));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3 HOST-07 (line 357): the saved GPIO artifact keeps checked writes, rejects a mistyped reference write and runs the same cycle in both compositions.
#[test]
fn both_compositions_keep_checked_writes_and_cycle_results_for_the_saved_gpio_artifact() {
    let mut gpio = Runtime::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    gpio.write_global("probe_depth", Value::Int(3)).unwrap();
    let reference = gpio.storage().ref_for_global("probe_depth").unwrap();
    assert_eq!(
        gpio.write_reference(&reference, Value::Bool(false)),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(gpio.read_reference(&reference).unwrap(), Value::Int(3));

    gpio.write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    gpio.advance_time(Duration::from_millis(10));
    gpio.execute_cycle().unwrap();
    assert_eq!(member(&gpio, "Plant", "depth_result"), Value::Int(3));
    assert!(!gpio.faulted());
}
