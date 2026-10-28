#![cfg(feature = "full")]
//! Live Values on retained variables by their retain-map keys (specification 44 §13.3, §5.10): a
//! global's key is its name, a program variable's is `@program/<instance>/<name>`.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
use trust_runtime::{
    debug::{DebugControl, RuntimeEvent},
    harness::TestHarness,
    value::Value,
};
const SOURCE: &str = "VAR_GLOBAL RETAIN kept : INT := 1; END_VAR VAR_GLOBAL plain : INT := 2; END_VAR PROGRAM Main VAR RETAIN member : INT := 3; END_VAR END_PROGRAM";

#[test]
fn retained_targets_rebind_actual_globals_and_program_members_without_a_bank() {
    let mut runtime = TestHarness::from_source(SOURCE).unwrap().into_runtime();
    let control = DebugControl::new();
    runtime.set_debug_control(control.clone());
    control
        .try_enqueue_retain_write("kept", Value::Int(7))
        .unwrap();
    control
        .try_enqueue_retain_write("@program/Main/member", Value::Int(8))
        .unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.read_variable("kept"), Ok(Value::Int(7)));
    assert_eq!(runtime.read_variable("Main.member"), Ok(Value::Int(8)));
    control.try_force_retain("kept", Value::Int(9)).unwrap();
    control
        .try_force_retain("@program/Main/member", Value::Int(10))
        .unwrap();
    runtime.execute_cycle().unwrap();
    runtime.write_variable("kept", &Value::Int(11)).unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.read_variable("kept"), Ok(Value::Int(9)));
    assert_eq!(runtime.read_variable("Main.member"), Ok(Value::Int(10)));
    control.release_retain("kept");
    control.release_retain("@program/Main/member");
    runtime.write_variable("kept", &Value::Int(12)).unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.read_variable("kept"), Ok(Value::Int(12)));
    assert!(control
        .drain_runtime_events()
        .iter()
        .all(|event| !matches!(event, RuntimeEvent::WriteRejected { .. })));
}
