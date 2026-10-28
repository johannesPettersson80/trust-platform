#![cfg(feature = "full")]
//! Checked engineering writes keep the existing conversion rules and the instance-field boundary.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::{error::RuntimeError, harness::TestHarness, value::Value};

const GLOBALS_SOURCE: &str = r#"
VAR_GLOBAL
    narrow : INT := INT#1;
    wide : DINT := DINT#2;
END_VAR
PROGRAM Main
END_PROGRAM
"#;

const INSTANCE_SOURCE: &str = r#"
FUNCTION_BLOCK Counter
VAR
    count : DINT := DINT#0;
END_VAR
END_FUNCTION_BLOCK
VAR_GLOBAL
    plant : Counter;
END_VAR
PROGRAM Main
END_PROGRAM
"#;

/// Spec 34 §4.3.1 checked writes: existing conversion rules are kept, so a widening write is stored in the destination type.
#[test]
fn installed_write_widening_is_stored_in_the_destination_type() {
    let mut harness = TestHarness::from_source(GLOBALS_SOURCE).expect("globals must compile");
    harness
        .runtime_mut()
        .write_variable("wide", &Value::Int(7))
        .expect("an INT value widens into a DINT destination");
    assert_eq!(harness.runtime().read_variable("wide"), Ok(Value::DInt(7)));
}

/// Spec 34 §4.3.1 checked writes: the boundary adds no coercion, so a narrowing write is refused and the destination keeps its value.
#[test]
fn installed_write_narrowing_is_refused_and_keeps_the_destination() {
    let mut harness = TestHarness::from_source(GLOBALS_SOURCE).expect("globals must compile");
    let error = harness
        .runtime_mut()
        .write_variable("narrow", &Value::DInt(9))
        .expect_err("a DINT value must not narrow into an INT destination");
    assert_eq!(error, RuntimeError::TypeMismatch);
    assert_eq!(harness.runtime().read_variable("narrow"), Ok(Value::Int(1)));
}

/// Spec 34 §4.3.1 checked writes: a BOOL is refused rather than converted into an INT destination, which keeps its value.
#[test]
fn installed_write_bool_is_refused_rather_than_converted_into_an_int() {
    let mut harness = TestHarness::from_source(GLOBALS_SOURCE).expect("globals must compile");
    let error = harness
        .runtime_mut()
        .write_variable("narrow", &Value::Bool(true))
        .expect_err("a BOOL value must not convert into an INT destination");
    assert_eq!(error, RuntimeError::TypeMismatch);
    assert_eq!(harness.runtime().read_variable("narrow"), Ok(Value::Int(1)));
}

/// Spec 34 §4.3.1 checked writes: an instance write cannot add a field the admitted type does not
/// declare, and the declared field keeps its value. A write by name reaches a variable through the
/// symbol table (specification 44 §13.3), which has no symbol for the undeclared member, so the
/// write is refused as `UndefinedVariable` and the member still names nothing afterwards.
#[test]
fn installed_write_instance_write_cannot_add_an_undeclared_field() {
    let mut harness = TestHarness::from_source(INSTANCE_SOURCE).expect("instance must compile");
    let error = harness
        .runtime_mut()
        .write_variable("plant.undeclared", &Value::DInt(1))
        .expect_err("an undeclared field is not an admitted destination");
    assert_eq!(
        error,
        RuntimeError::UndefinedVariable("plant.undeclared".into())
    );
    assert_eq!(
        harness.runtime().read_variable("plant.undeclared"),
        Err(RuntimeError::UndefinedVariable("plant.undeclared".into()))
    );
    assert_eq!(
        harness.runtime().read_variable("plant.count"),
        Ok(Value::DInt(0))
    );
}
