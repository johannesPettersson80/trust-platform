use trust_runtime::boundary::BoundaryError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

fn boundary_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    arr : ARRAY[1..2] OF INT;
    fb : TON;
END_VAR
arr[1] := INT#10;
arr[2] := INT#20;
fb(IN := TRUE, PT := T#10MS);
END_PROGRAM
"#
}

#[test]
fn resolver_reads_indexed_and_dotted_program_paths() {
    let mut harness = TestHarness::from_source(boundary_program()).expect("compile harness");
    harness.cycle();

    assert_eq!(harness.try_get_output("arr[1]"), Ok(Value::Int(10)));
    assert_eq!(harness.try_get_output("arr[2]"), Ok(Value::Int(20)));
    assert_eq!(harness.try_get_output("fb.IN"), Ok(Value::Bool(true)));
}

#[test]
fn resolver_reports_unknown_path_without_null_fallback() {
    let harness = TestHarness::from_source(boundary_program()).expect("compile harness");

    let error = harness
        .try_get_output("arr[99]")
        .expect_err("out of bounds path must fail closed");
    assert_eq!(error.code(), "wrong_kind");

    let error = harness
        .try_get_output("missing")
        .expect_err("missing name must fail closed");
    assert!(matches!(error, BoundaryError::UnresolvedName { .. }));
}

#[test]
fn resolver_reports_ambiguous_unqualified_program_var() {
    let source = r#"
PROGRAM A
VAR
    shared : INT := INT#1;
END_VAR
END_PROGRAM

PROGRAM B
VAR
    shared : INT := INT#2;
END_VAR
END_PROGRAM
"#;
    let harness = TestHarness::from_source(source).expect("compile harness");

    let error = harness
        .try_get_output("shared")
        .expect_err("two program variables with same name must be ambiguous");
    let BoundaryError::AmbiguousName { candidates, .. } = error else {
        panic!("expected ambiguous name, got {error:?}");
    };
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().any(|candidate| candidate == "A.shared"));
    assert!(candidates.iter().any(|candidate| candidate == "B.shared"));
}
