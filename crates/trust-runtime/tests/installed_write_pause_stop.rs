#![cfg(feature = "full")]
//! A debugger pause and the STOP boundary keep the admitted image and the checked write path.

use trust_runtime::{error::RuntimeError, harness::TestHarness, value::Value};

const SOURCE: &str = r#"
VAR_GLOBAL
    level : DINT := DINT#1;
END_VAR
PROGRAM Main
level := level + DINT#1;
END_PROGRAM
"#;

/// Spec 34 §4.3.1 installed controller: a debugger pause and the STOP boundary leave the admitted image in place, keep ordinary checked writes permitted, and keep type rejection unchanged.
#[test]
fn installed_write_pause_and_stop_keep_the_admitted_image_and_checked_writes() {
    let mut harness = TestHarness::from_source(SOURCE).expect("pause source must compile");
    let admitted = std::ptr::from_ref(harness.runtime().prepared());

    let debug = harness.runtime_mut().enable_debug();
    debug.pause();
    harness
        .runtime_mut()
        .write_global("level", Value::DInt(50))
        .expect("an ordinary checked write is permitted while paused");

    harness
        .runtime_mut()
        .apply_io_safe_state()
        .expect("the deliberate STOP boundary must succeed");
    harness
        .runtime_mut()
        .write_global("level", Value::DInt(60))
        .expect("an ordinary checked write is permitted after STOP");

    let rejected = harness
        .runtime_mut()
        .write_global("level", Value::Bool(true));
    assert_eq!(rejected, Err(RuntimeError::TypeMismatch));
    assert_eq!(
        harness.runtime().storage().get_global("level"),
        Some(&Value::DInt(60))
    );
    assert!(
        std::ptr::eq(admitted, harness.runtime().prepared()),
        "pause and STOP must not replace the admitted executable"
    );
    assert!(!harness.runtime().faulted());
}
