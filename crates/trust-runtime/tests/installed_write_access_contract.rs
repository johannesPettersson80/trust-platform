#![cfg(feature = "full")]
//! Hosted VAR_ACCESS writes: exact contextual rejection, stable identity and no mutation.

use trust_runtime::{error::RuntimeError, harness::TestHarness, value::Value};

const ACCESS_SOURCE: &str = r#"
PROGRAM Main
VAR
    Value : INT := INT#5;
    Ceiling : INT := INT#7;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Observed : P.Value : INT READ_ONLY;
    Writable : P.Ceiling : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#;

const RETIRED_ACCESS_SOURCE: &str = r#"
PROGRAM Main
VAR
    Value : INT := INT#5;
    Ceiling : INT := INT#7;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
END_CONFIGURATION
"#;

/// Spec 34 §4.3.1 hosted read-only VAR_ACCESS: the hosted rejection adds the exact context `VAR_ACCESS '<name>' is read-only`.
#[test]
fn installed_write_read_only_alias_rejection_has_the_exact_context_message() {
    let mut harness =
        TestHarness::from_source(ACCESS_SOURCE).expect("read-only access must compile");
    let error = harness
        .set_access("Observed", Value::Int(9))
        .expect_err("a read-only alias must reject the write");
    assert_eq!(error.to_string(), "VAR_ACCESS 'Observed' is read-only");
    assert_eq!(harness.get_access("Observed"), Some(Value::Int(5)));
}

/// Spec 34 §4.3.1 hosted read-only VAR_ACCESS: the hosted adapter returns the contextual `ReadOnlyAlias` variant for an admitted read-only alias and does not latch a fault.
#[test]
fn installed_write_read_only_alias_rejection_returns_the_contextual_variant() {
    let mut harness =
        TestHarness::from_source(ACCESS_SOURCE).expect("read-only access must compile");
    let error = harness
        .set_access("Observed", Value::Int(9))
        .expect_err("a read-only alias must reject the write");
    assert_eq!(error, RuntimeError::ReadOnlyAlias("Observed".into()));
    assert!(!harness.runtime().faulted());
}

/// Spec 34 §4.3.1 hosted read-only VAR_ACCESS: a writable alias rejects a wrong-type value with the shared type error and no read-only context.
#[test]
fn installed_write_writable_alias_type_error_has_no_read_only_context() {
    let mut harness =
        TestHarness::from_source(ACCESS_SOURCE).expect("access declarations must compile");
    let error = harness
        .set_access("Writable", Value::Bool(true))
        .expect_err("a BOOL cannot be written through an INT alias");
    assert_eq!(error, RuntimeError::TypeMismatch);
    assert_eq!(harness.get_access("Writable"), Some(Value::Int(7)));
}

/// Spec 34 §4.3.1 hosted read-only VAR_ACCESS: an unknown alias keeps the shared `UndefinedVariable` identity and gains no read-only context.
#[test]
fn installed_write_unknown_alias_keeps_the_shared_undefined_variable_identity() {
    let mut harness =
        TestHarness::from_source(ACCESS_SOURCE).expect("access declarations must compile");
    let error = harness
        .set_access("Missing", Value::Int(1))
        .expect_err("an unknown alias must reject the write");
    assert_eq!(error, RuntimeError::UndefinedVariable("Missing".into()));
    assert_eq!(harness.get_access("Missing"), None);
    assert!(!harness.runtime().faulted());
}

/// Spec 34 §4.3.1 checked writes: a write through an alias retired by a source replacement is rejected before commit, and the replacement stays unfaulted.
#[test]
fn installed_write_alias_retired_by_source_replacement_is_rejected() {
    let mut harness =
        TestHarness::from_source(ACCESS_SOURCE).expect("access declarations must compile");
    harness
        .reload_source(RETIRED_ACCESS_SOURCE)
        .expect("the replacement without the alias must compile");
    let error = harness
        .set_access("Writable", Value::Int(9))
        .expect_err("a retired alias must not accept a write");
    assert_eq!(error, RuntimeError::UndefinedVariable("Writable".into()));
    assert_eq!(harness.get_access("Writable"), None);
    assert!(!harness.runtime().faulted());
}
