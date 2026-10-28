//! Fail-closed starting values and engineering writes: an interface variable starts as `NULL`, and a
//! queued write to a target the program does not hold is refused without faulting the scan.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

/// `Svc` is never assigned, so the block compares its starting value with `NULL`; an interface
/// variable that started as anything else would make `SvcIsNull` FALSE.
#[test]
fn interface_param_defaults_to_null_reference() {
    let source = r#"
INTERFACE I_Svc
END_INTERFACE

FUNCTION_BLOCK Consumer
VAR_INPUT
    Svc : I_Svc;
END_VAR
VAR_OUTPUT
    SvcIsNull : BOOL;
END_VAR
SvcIsNull := Svc = NULL;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    C : Consumer;
END_VAR
C();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("interface default should compile");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("Main.C.SvcIsNull"),
        Some(Value::Bool(true)),
        "an interface variable without an explicit initializer starts as NULL"
    );
}

/// HOST-09 rejects an invalid engineering request without faulting PLC execution: the queued write
/// names no symbol, so the cycle start refuses it as `UndefinedVariable` with a `WriteRejected`
/// event (specification 44 §13.3) and creates nothing.
#[test]
fn debug_queued_global_write_unknown_target_fails() {
    let mut harness =
        TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n").expect("an empty program compiles");
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    debug
        .try_enqueue_global_write("missing", Value::DInt(7))
        .expect("the queue has room for one write");

    let runtime = harness.runtime_mut();
    runtime
        .execute_cycle()
        .expect("the invalid request must not fault the scan");
    assert!(!runtime.faulted());
    let expected = RuntimeError::UndefinedVariable("missing".into()).to_string();
    assert!(debug
        .drain_runtime_events()
        .iter()
        .any(|event| matches!(event,
        RuntimeEvent::WriteRejected { target, error, .. }
            if target.as_str() == "missing" && error == &expected)));
    assert_eq!(
        runtime.read_variable("missing"),
        Err(RuntimeError::UndefinedVariable("missing".into()))
    );
}
