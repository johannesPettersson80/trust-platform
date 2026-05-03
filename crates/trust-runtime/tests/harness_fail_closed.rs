use trust_runtime::harness::{HarnessAutomation, HarnessAutomationError, TestHarness};
use trust_runtime::value::Value;

fn drive_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    drive : BOOL;
    out : BOOL;
END_VAR
out := drive;
END_PROGRAM
"#
}

#[test]
fn set_input_typo_returns_boundary_error_not_silent_global_create() {
    let mut automation = HarnessAutomation::new();
    automation
        .load_sources(&[drive_program().to_string()])
        .expect("load harness");

    let error = automation
        .set_input("drrive", Value::Bool(true))
        .expect_err("typo must fail closed");
    let HarnessAutomationError::Boundary(boundary) = error else {
        panic!("expected boundary error");
    };
    assert_eq!(boundary.code(), "unresolved_name");

    let snapshot = automation
        .snapshot(&["drive".to_string(), "drrive".to_string()])
        .expect("snapshot");
    assert!(snapshot.values["drive"].is_ok());
    assert_eq!(
        snapshot.values["drrive"]
            .error
            .as_ref()
            .expect("error entry")
            .code(),
        "unresolved_name"
    );
}

#[test]
fn bind_direct_typo_returns_boundary_error_not_silent_binding() {
    let mut harness = TestHarness::from_source(drive_program()).expect("compile harness");

    let error = harness
        .bind_direct("drrive", "%IX0.0")
        .expect_err("typo must not bind silently");
    assert_eq!(error.code(), "undeclared_binding");
}

#[test]
fn declared_null_like_values_are_not_missing_name_errors() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    p_int : REF_TO INT;
END_VAR
END_PROGRAM
"#,
    )
    .expect("compile harness");
    harness.cycle();

    assert_eq!(
        harness.try_get_output("p_int"),
        Ok(Value::Reference(None)),
        "declared null references remain values, not missing-name sentinels"
    );
}
