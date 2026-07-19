use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::value::Value;

#[test]
fn function_return_arithmetic_materializes_the_declared_integer_type() {
    let library = r#"
NAMESPACE Oscat
FUNCTION DIR_TO_DEG : INT
DIR_TO_DEG := (16 * 45 + 1) / 2;
END_FUNCTION
END_NAMESPACE
"#;

    let program = r#"
USING Oscat;
PROGRAM Main
VAR
    result : INT;
END_VAR
result := DIR_TO_DEG();
END_PROGRAM
"#;

    let mut harness =
        TestHarness::from_sources(&[library, program]).expect("compile namespaced runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("result"), Some(Value::Int(360)));
}

#[test]
fn for_loop_bounds_materialize_the_control_variable_type() {
    let source = r#"
PROGRAM Main
VAR
    index : SINT;
    iterations : INT;
END_VAR
FOR index := 0 TO 2 DO
    iterations := iterations + INT#1;
END_FOR;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("iterations"), Some(Value::Int(3)));
}

#[test]
fn call_arguments_materialize_formal_parameter_types() {
    let source = r#"
FUNCTION IdentityReal : REAL
VAR_INPUT
    value : REAL;
END_VAR
IdentityReal := value;
END_FUNCTION

FUNCTION_BLOCK RealLatch
VAR_INPUT
    value : REAL;
END_VAR
VAR_OUTPUT
    latched : REAL;
END_VAR
latched := value;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    block : RealLatch;
    function_named : REAL;
    function_positional : REAL;
    block_named : REAL;
    block_positional : REAL;
END_VAR
function_named := IdentityReal(value := 360.0);
function_positional := IdentityReal(45.0);
block(value := 1.0, latched => block_named);
block(value := 2.0, latched => block_positional);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("function_named"),
        Some(Value::Real(360.0))
    );
    assert_eq!(
        harness.get_output("function_positional"),
        Some(Value::Real(45.0))
    );
    assert_eq!(harness.get_output("block_named"), Some(Value::Real(1.0)));
    assert_eq!(
        harness.get_output("block_positional"),
        Some(Value::Real(2.0))
    );
}

#[test]
fn precision_losing_typed_integer_to_float_assignments_require_explicit_conversion() {
    let source = r#"
PROGRAM Main
VAR
    dint_value : DINT := DINT#16777217;
    lint_value : LINT := LINT#9007199254740993;
    real_value : REAL;
    lreal_value : LREAL;
END_VAR
real_value := dint_value;
lreal_value := lint_value;
END_PROGRAM
"#;

    let error = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("typed precision-losing integer-to-float assignments must not compile");
    assert_eq!(
        error.to_string().matches("error[E203]:").count(),
        2,
        "expected one incompatible-assignment diagnostic per lossy edge, got {error}"
    );
}

#[test]
fn accuracy_preserving_integer_to_float_assignments_materialize_target_tags() {
    let source = r#"
PROGRAM Main
VAR
    sint_value : SINT := SINT#-128;
    int_value : INT := INT#-32768;
    dint_value : DINT := DINT#16777217;
    as_real_from_sint : REAL;
    as_real_from_int : REAL;
    as_lreal_from_dint : LREAL;
    explicit_real : REAL;
END_VAR
as_real_from_sint := sint_value;
as_real_from_int := int_value;
as_lreal_from_dint := dint_value;
explicit_real := DINT_TO_REAL(dint_value);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("as_real_from_sint"),
        Some(Value::Real(-128.0))
    );
    assert_eq!(
        harness.get_output("as_real_from_int"),
        Some(Value::Real(-32768.0))
    );
    assert_eq!(
        harness.get_output("as_lreal_from_dint"),
        Some(Value::LReal(16777217.0))
    );
    assert_eq!(
        harness.get_output("explicit_real"),
        Some(Value::Real(16777216.0)),
        "the explicit conversion may round according to REAL representation"
    );
}

#[test]
fn subrange_accepts_inclusive_bounds_and_rejects_below_min_without_mutation() {
    let source = r#"
TYPE
    Limited : INT (0..10);
END_TYPE

PROGRAM Main
VAR
    source : INT := INT#-1;
    limited : Limited := 5;
END_VAR
limited := 0;
limited := 10;
limited := source;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert_eq!(
        cycle.errors.len(),
        1,
        "expected one below-minimum runtime error"
    );
    assert!(
        cycle.errors[0].to_string().contains("subrange"),
        "expected a visible subrange rejection, got {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("limited"),
        Some(Value::Int(10)),
        "the rejected write must preserve the last accepted inclusive-bound value"
    );
}
