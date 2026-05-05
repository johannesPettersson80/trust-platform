use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::value::Value;

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn function_input_parameter_widening_executes_as_runtime_target_type() {
    let source = r#"
FUNCTION UseDint : DINT
VAR_INPUT
    x : DINT;
END_VAR
UseDint := x + DINT#1;
END_FUNCTION

PROGRAM Main
VAR
    result : DINT;
END_VAR
result := UseDint(x := 1);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    harness.cycle();
    harness.assert_eq("result", 2i32);
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn function_output_parameter_widening_executes_as_runtime_target_type() {
    let source = r#"
FUNCTION_BLOCK Producer
VAR_OUTPUT
    out_value : DINT;
END_VAR
out_value := 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Producer;
    result : DINT;
END_VAR
fb(out_value => result);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    harness.cycle();
    harness.assert_eq("result", 1i32);
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn assignment_widening_materializes_target_runtime_type() {
    let source = r#"
PROGRAM Main
VAR
    whole : LINT;
    real_value : LREAL;
END_VAR
whole := 1;
real_value := 1.5;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    harness.cycle();
    assert_eq!(harness.get_output("whole"), Some(Value::LInt(1)));
    assert_eq!(harness.get_output("real_value"), Some(Value::LReal(1.5)));
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn initializer_widening_materializes_target_runtime_type() {
    let source = r#"
PROGRAM Main
VAR
    whole : LINT := 1;
    real_value : LREAL := 1.5;
END_VAR
END_PROGRAM
"#;

    let harness = TestHarness::from_source(source).expect("compile runtime");
    assert_eq!(harness.get_output("whole"), Some(Value::LInt(1)));
    assert_eq!(harness.get_output("real_value"), Some(Value::LReal(1.5)));
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn return_value_widening_executes_as_runtime_target_type() {
    let source = r#"
FUNCTION GiveLreal : LREAL
GiveLreal := 1.5;
END_FUNCTION

PROGRAM Main
VAR
    result : LREAL;
END_VAR
result := GiveLreal();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    harness.cycle();
    assert_eq!(harness.get_output("result"), Some(Value::LReal(1.5)));
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn inout_narrowing_is_rejected_instead_of_silent_writeback_loss() {
    let source = r#"
FUNCTION_BLOCK Mutate
VAR_IN_OUT
    value : DINT;
END_VAR
value := value + DINT#1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Mutate;
    small : INT := 1;
END_VAR
fb(value := small);
END_PROGRAM
"#;

    let err = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("INT VAR_IN_OUT passed to DINT must not compile silently");
    assert!(
        err.to_string().contains("type")
            || err.to_string().contains("assign")
            || err.to_string().contains("conversion")
            || err.to_string().contains("E205")
            || err.to_string().contains("expects"),
        "expected type/conversion rejection, got {err}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn narrowing_assignment_is_rejected_instead_of_silent_truncation() {
    let source = r#"
PROGRAM Main
VAR
    small : INT;
END_VAR
small := DINT#70000;
END_PROGRAM
"#;

    let err = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("narrowing assignment must not compile silently");
    assert!(
        err.to_string().contains("type")
            || err.to_string().contains("assign")
            || err.to_string().contains("conversion"),
        "expected type/conversion rejection, got {err}"
    );
}
