#![cfg(feature = "full")]
//! A debugger pause and the STOP boundary keep the admitted image and the checked write path.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::{debug::DebugControl, error::RuntimeError, harness::TestHarness, value::Value};

const SOURCE: &str = r#"
VAR_GLOBAL
    level : DINT := DINT#1;
END_VAR
PROGRAM Main
level := level + DINT#1;
END_PROGRAM
"#;

/// Spec 34 §4.3.1 installed controller: a debugger pause and the STOP boundary leave the admitted
/// image in place, keep ordinary checked writes permitted, and keep type rejection unchanged. The
/// admitted program still runs after both: once resumed, one scan adds 1 to the 60 written last,
/// so `level` is 61.
#[test]
fn installed_write_pause_and_stop_keep_the_admitted_image_and_checked_writes() {
    let mut harness = TestHarness::from_source(SOURCE).expect("pause source must compile");

    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    let debugger = debug
        .compiled()
        .expect("the runtime sets its compiled debugger");
    debugger.pause();
    harness
        .runtime_mut()
        .write_variable("level", &Value::DInt(50))
        .expect("an ordinary checked write is permitted while paused");

    harness
        .runtime_mut()
        .apply_io_safe_state()
        .expect("the deliberate STOP boundary must succeed");
    harness
        .runtime_mut()
        .write_variable("level", &Value::DInt(60))
        .expect("an ordinary checked write is permitted after STOP");

    let rejected = harness
        .runtime_mut()
        .write_variable("level", &Value::Bool(true));
    assert_eq!(rejected, Err(RuntimeError::TypeMismatch));
    assert_eq!(
        harness.runtime().read_variable("level"),
        Ok(Value::DInt(60))
    );
    assert!(!harness.runtime().faulted());

    debugger.resume();
    harness.cycle();
    assert_eq!(
        harness.runtime().read_variable("level"),
        Ok(Value::DInt(61)),
        "pause and STOP must not replace the admitted executable"
    );
}
