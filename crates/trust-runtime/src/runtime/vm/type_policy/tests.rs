use crate::bytecode::{TypeData, TypeEntry, TypeKind, TypeTable};
use crate::error::RuntimeError;
use crate::harness::{CompileSession, TestHarness};
use crate::value::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

use super::normalize_value_for_type_table;

fn primitive_table(prim_id: u16) -> TypeTable {
    primitive_table_with_max(prim_id, 0)
}

fn primitive_table_with_max(prim_id: u16, max_length: u16) -> TypeTable {
    TypeTable {
        offsets: Vec::new(),
        entries: vec![TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id,
                max_length,
            },
        }],
    }
}

#[test]
fn primitive_policy_rejects_incompatible_runtime_tags() {
    let real = primitive_table(14);
    let lreal = primitive_table(15);

    for error in [
        normalize_value_for_type_table(&real, 0, Value::DInt(16_777_217), 0),
        normalize_value_for_type_table(&lreal, 0, Value::LInt(9_007_199_254_740_993), 0),
        normalize_value_for_type_table(&real, 0, Value::Bool(true), 0),
    ] {
        let error = error.expect_err("incompatible primitive tag must reject");
        assert_eq!(error, RuntimeError::TypeMismatch);
        assert_eq!(error.stable_code().as_str(), "runtime_type_mismatch");
    }
}

#[test]
fn primitive_policy_materializes_only_accuracy_preserving_float_widening() {
    let real = primitive_table(14);
    let lreal = primitive_table(15);

    assert_eq!(
        normalize_value_for_type_table(&real, 0, Value::Int(i16::MIN), 0),
        Ok(Value::Real(f32::from(i16::MIN)))
    );
    assert_eq!(
        normalize_value_for_type_table(&lreal, 0, Value::DInt(16_777_217), 0),
        Ok(Value::LReal(16_777_217.0))
    );
}

#[test]
fn bounded_string_policy_truncates_by_scalar_and_rejects_wrong_family() {
    let string = primitive_table_with_max(24, 2);
    let wstring = primitive_table_with_max(25, 2);

    assert_eq!(
        normalize_value_for_type_table(&string, 0, Value::String("ÄB".into()), 0),
        Ok(Value::String("ÄB".into()))
    );
    assert_eq!(
        normalize_value_for_type_table(&wstring, 0, Value::WString("🙂Ω".into()), 0),
        Ok(Value::WString("🙂Ω".into()))
    );
    assert_eq!(
        normalize_value_for_type_table(&string, 0, Value::String("ÄBC".into()), 0),
        Ok(Value::String("ÄB".into()))
    );
    assert_eq!(
        normalize_value_for_type_table(&wstring, 0, Value::WString("🙂ΩX".into()), 0),
        Ok(Value::WString("🙂Ω".into()))
    );
    for error in [
        normalize_value_for_type_table(&string, 0, Value::WString("AB".into()), 0),
        normalize_value_for_type_table(&wstring, 0, Value::String("AB".into()), 0),
    ] {
        let error = error.expect_err("cross-family string tag must reject");
        assert_eq!(error, RuntimeError::TypeMismatch);
        assert_eq!(error.stable_code().as_str(), "runtime_type_mismatch");
    }
}

#[test]
fn subrange_policy_accepts_inclusive_bounds_and_rejects_other_values() {
    let table = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 7,
                    max_length: 0,
                },
            },
            TypeEntry {
                kind: TypeKind::Subrange,
                name_idx: None,
                data: TypeData::Subrange {
                    base_type_id: 0,
                    lower: -2,
                    upper: 2,
                },
            },
        ],
    };

    assert_eq!(
        normalize_value_for_type_table(&table, 1, Value::Int(-2), 0),
        Ok(Value::Int(-2))
    );
    assert_eq!(
        normalize_value_for_type_table(&table, 1, Value::Int(2), 0),
        Ok(Value::Int(2))
    );
    for error in [
        normalize_value_for_type_table(&table, 1, Value::Int(-3), 0),
        normalize_value_for_type_table(&table, 1, Value::Int(3), 0),
    ] {
        let error = error.expect_err("out-of-range subrange value must reject");
        assert!(matches!(error, RuntimeError::SubrangeViolation { .. }));
        assert_eq!(error.stable_code().as_str(), "runtime_subrange_violation");
    }

    let error = normalize_value_for_type_table(&table, 1, Value::Real(1.0), 0)
        .expect_err("wrong subrange base tag must reject");
    assert_eq!(error, RuntimeError::TypeMismatch);
    assert_eq!(error.stable_code().as_str(), "runtime_type_mismatch");
}

#[test]
fn declared_type_policy_trace_cases() {
    run_policy_cases(
        "TEST_VM_DECLARED_TYPE_TRACE_001",
        "verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml",
        "sha256:6a9ce7a1a4885787c6d4da925abdb01f1ca8c4279da3c4d68665fd014b7544cc",
        run_declared_type_case,
    );
}

#[test]
fn bounded_string_policy_trace_cases() {
    run_policy_cases(
        "TEST_VM_STRING_BOUND_TRACE_001",
        "verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml",
        "sha256:1824d7ca9494ef19a1d834cc7234f225492a7326255d10f96efc7429370ee63a",
        run_string_bound_case,
    );
}

#[test]
fn subrange_policy_trace_cases() {
    run_policy_cases(
        "TEST_VM_SUBRANGE_TRACE_001",
        "verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml",
        "sha256:b4fa0c17e297089eb1aca2c83e7a4b9b134a53865e37c2518340737dcf877466",
        run_subrange_case,
    );
}

fn run_policy_cases(
    test_id: &str,
    case_file: &str,
    case_file_digest: &str,
    runner: fn(&CaseRecord, &mut PolicyProbe) -> Result<CaseExecution, String>,
) {
    let workspace = workspace_root();
    let mut probe = PolicyProbe::default();
    let config = RunConfig::new(test_id, workspace.join(case_file), case_file_digest);
    let artifact = run_case_file(&config, &mut probe, runner)
        .unwrap_or_else(|error| panic!("{test_id} artifact must be written: {error}"));
    let failed = artifact
        .cases
        .iter()
        .filter(|case| case.result != CaseResult::Passed)
        .map(|case| {
            format!(
                "{}: {}",
                case.id,
                case.observed_error.as_deref().unwrap_or("not passed")
            )
        })
        .collect::<Vec<_>>();
    assert!(
        failed.is_empty(),
        "{test_id} failures: {}",
        failed.join("; ")
    );
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf()
}

fn run_declared_type_case(
    case: &CaseRecord,
    probe: &mut PolicyProbe,
) -> Result<CaseExecution, String> {
    let key = case_key(case)?;
    let outcome = match key {
        "INT_LITERAL_TO_REAL_SLOT" | "INT_VARIABLE_TO_REAL_SLOT" => {
            expect_normalized(14, Value::Int(1), Value::Real(1.0))
        }
        "INT_EXPRESSION_TO_DINT_SLOT" => expect_normalized(8, Value::Int(200), Value::DInt(200)),
        "COMPATIBLE_LITERAL_INITIALIZER_WIDENING" | "POU_OUTPUT_WIDENING" => {
            expect_normalized(9, Value::DInt(40_000), Value::LInt(40_000))
        }
        "COMPATIBLE_TYPED_ASSIGNMENT_WIDENING" | "FUNCTION_RESULT_WIDENING" => {
            expect_normalized(15, Value::Real(1.5), Value::LReal(1.5))
        }
        "ACCURACY_PRESERVING_INTEGER_TO_FLOAT" => {
            expect_normalized(14, Value::Int(i16::MIN), Value::Real(f32::from(i16::MIN)))
        }
        "INT_ACTUAL_TO_DINT_INOUT" => expect_inout_rejection(),
        "DINT_TO_INT_NARROWING_ASSIGNMENT" => expect_type_mismatch(7, Value::DInt(70_000)),
        "BOOL_TO_REAL_SLOT" => expect_type_mismatch(14, Value::Bool(true)),
        "DINT_TO_REAL_IMPLICIT" => expect_type_mismatch(14, Value::DInt(16_777_217)),
        "LINT_TO_LREAL_IMPLICIT" => expect_type_mismatch(15, Value::LInt(9_007_199_254_740_993)),
        "NONREPRESENTABLE_CONTEXT_INTEGER_TO_REAL" => {
            expect_type_mismatch(14, Value::DInt(16_777_217))
        }
        "CONTEXTUAL_UNTYPED_NUMERIC_LITERAL_COMMON_TYPE" => expect_contextual_literal_common_type(),
        "NO_ACCURACY_PRESERVING_COMMON_NUMERIC_TYPE" => {
            expect_no_accuracy_preserving_common_numeric_type()
        }
        other => return Err(format!("unreviewed declared-type scenario {other}")),
    }?;
    probe.observed = Some(serde_json::json!({"scenario": key, "outcome": outcome}));
    Ok(passed(outcome))
}

fn run_string_bound_case(
    case: &CaseRecord,
    probe: &mut PolicyProbe,
) -> Result<CaseExecution, String> {
    let key = case_key(case)?;
    let string = primitive_table_with_max(24, 5);
    let outcome = match key {
        "STRING_3_CONCAT_ASSIGNMENT_TRUNCATION" => expect_bounded_concat_assignment(),
        "FB_INPUT_COPY_IN_LENGTH_6" => expect_result(
            normalize_value_for_type_table(&string, 0, Value::String("abcdef".into()), 0),
            Value::String("abcde".into()),
        ),
        "WSTRING" => expect_error_code(
            normalize_value_for_type_table(&string, 0, Value::WString("wide".into()), 0),
            "runtime_type_mismatch",
        ),
        _ => {
            let value = case
                .input
                .get("string_value")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{} requires string_value", case.id))?;
            let expected = value.chars().take(5).collect::<String>();
            expect_result(
                normalize_value_for_type_table(
                    &string,
                    0,
                    Value::String(value.to_string().into()),
                    0,
                ),
                Value::String(expected.into()),
            )
        }
    }?;
    probe.observed = Some(serde_json::json!({"scenario": key, "outcome": outcome}));
    Ok(passed(outcome))
}

fn run_subrange_case(case: &CaseRecord, probe: &mut PolicyProbe) -> Result<CaseExecution, String> {
    let key = case_key(case)?;
    let table = subrange_table(0, 100);
    let outcome = if key == "REAL" {
        expect_error_code(
            normalize_value_for_type_table(&table, 1, Value::Real(1.0), 0),
            "runtime_type_mismatch",
        )?
    } else {
        let value = case
            .input
            .get("value")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| format!("{} requires integer value", case.id))?;
        let result = normalize_value_for_type_table(&table, 1, Value::Int(value as i16), 0);
        if (0..=100).contains(&value) {
            expect_result(result, Value::Int(value as i16))?
        } else {
            expect_error_code(result, "runtime_subrange_violation")?
        }
    };
    probe.observed = Some(serde_json::json!({"scenario": key, "outcome": outcome}));
    Ok(passed(outcome))
}

fn case_key(case: &CaseRecord) -> Result<&str, String> {
    case.input
        .get("scenario")
        .or_else(|| case.input.get("shape_descriptor"))
        .and_then(toml::Value::as_str)
        .or_else(|| {
            case.input
                .get("source_partition")
                .and_then(toml::Value::as_table)
                .and_then(|partition| partition.keys().next().map(String::as_str))
        })
        .ok_or_else(|| format!("{} has no reviewed scenario key", case.id))
}

fn expect_normalized(prim_id: u16, value: Value, expected: Value) -> Result<&'static str, String> {
    expect_result(
        normalize_value_for_type_table(&primitive_table(prim_id), 0, value, 0),
        expected,
    )
}

fn expect_type_mismatch(prim_id: u16, value: Value) -> Result<&'static str, String> {
    expect_error_code(
        normalize_value_for_type_table(&primitive_table(prim_id), 0, value, 0),
        "runtime_type_mismatch",
    )
}

fn expect_inout_rejection() -> Result<&'static str, String> {
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
    let error = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("INT VAR_IN_OUT passed to DINT must reject");
    if error.to_string().contains("error[E205]:") {
        Ok("compiler_type_mismatch")
    } else {
        Err(format!("expected E205 VAR_IN_OUT rejection, got {error}"))
    }
}

fn expect_contextual_literal_common_type() -> Result<&'static str, String> {
    let source = r#"
PROGRAM Main
VAR
    uint_value : UINT := UINT#2;
    arithmetic_left : UINT;
    arithmetic_right : UINT;
    equal_left : BOOL;
    equal_right : BOOL;
    maximum_left : UINT;
    maximum_right : UINT;
    minimum_left : UINT;
    minimum_right : UINT;
    real_value : REAL := REAL#2.0;
    real_arithmetic_left : REAL;
    real_arithmetic_right : REAL;
    real_equal_integer : BOOL;
    real_equal_left : BOOL;
    real_equal_right : BOOL;
    real_maximum_left : REAL;
    real_maximum_right : REAL;
END_VAR
arithmetic_left := uint_value + 1;
arithmetic_right := 1 + uint_value;
equal_left := uint_value = 1;
equal_right := 1 = uint_value;
maximum_left := MAX(uint_value, 1);
maximum_right := MAX(1, uint_value);
minimum_left := MIN(uint_value, 1);
minimum_right := MIN(1, uint_value);
real_arithmetic_left := real_value + 1.0;
real_arithmetic_right := 1.0 + real_value;
real_equal_integer := real_value = 1;
real_equal_left := real_value = 1.0;
real_equal_right := 1.0 = real_value;
real_maximum_left := MAX(real_value, 1.0);
real_maximum_right := MAX(1.0, real_value);
END_PROGRAM
"#;
    CompileSession::from_source(source)
        .build_runtime()
        .map(|_| "contextual_literal_common_type")
        .map_err(|error| format!("contextual literal fixture failed to compile: {error}"))
}

fn expect_no_accuracy_preserving_common_numeric_type() -> Result<&'static str, String> {
    let source = r#"
PROGRAM Main
VAR
    ulint_value : ULINT := ULINT#18446744073709551615;
    real_value : REAL := REAL#1.0;
    result : REAL;
END_VAR
result := ulint_value + real_value;
END_PROGRAM
"#;
    match CompileSession::from_source(source).build_runtime() {
        Err(error)
            if error
                .to_string()
                .contains("no accuracy-preserving common type") =>
        {
            Ok("no_common_numeric_type_rejected")
        }
        Err(error) => Err(format!(
            "expected no-common-type diagnostic, observed {error}"
        )),
        Ok(_) => Err("mixed ULINT and REAL unexpectedly compiled".to_string()),
    }
}

fn expect_bounded_concat_assignment() -> Result<&'static str, String> {
    let source = r#"
PROGRAM Main
VAR
    text : STRING[3] := 'A';
    source : STRING[6] := 'BCDE';
END_VAR
text := CONCAT(text, source);
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source)
        .map_err(|error| format!("bounded CONCAT fixture failed to compile: {error}"))?;
    let cycle = harness.cycle();
    if !cycle.errors.is_empty() {
        return Err(format!(
            "bounded CONCAT fixture faulted: {:?}",
            cycle.errors
        ));
    }
    if harness.get_output("text") == Some(Value::String("ABC".into())) {
        Ok("bounded_concat_truncated")
    } else {
        Err(format!(
            "bounded CONCAT expected STRING 'ABC', observed {:?}",
            harness.get_output("text")
        ))
    }
}

fn expect_result(
    result: Result<Value, RuntimeError>,
    expected: Value,
) -> Result<&'static str, String> {
    match result {
        Ok(actual) if actual == expected => Ok("accepted"),
        Ok(actual) => Err(format!("expected {expected:?}, observed {actual:?}")),
        Err(error) => Err(format!("expected {expected:?}, observed error {error:?}")),
    }
}

fn expect_error_code(
    result: Result<Value, RuntimeError>,
    expected_code: &str,
) -> Result<&'static str, String> {
    match result {
        Err(error) if error.stable_code().as_str() == expected_code => Ok("rejected"),
        Err(error) => Err(format!(
            "expected {expected_code}, observed {} ({error:?})",
            error.stable_code()
        )),
        Ok(value) => Err(format!(
            "expected {expected_code}, observed value {value:?}"
        )),
    }
}

fn subrange_table(lower: i64, upper: i64) -> TypeTable {
    TypeTable {
        offsets: Vec::new(),
        entries: vec![
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 7,
                    max_length: 0,
                },
            },
            TypeEntry {
                kind: TypeKind::Subrange,
                name_idx: None,
                data: TypeData::Subrange {
                    base_type_id: 0,
                    lower,
                    upper,
                },
            },
        ],
    }
}

fn passed(status: &str) -> CaseExecution {
    CaseExecution {
        result: CaseResult::Passed,
        observed_error: None,
        observed_status: Some(status.to_string()),
    }
}

#[derive(Default)]
struct PolicyProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for PolicyProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_before {
            self.observed = None;
        }
        self.next_snapshot_is_before = !self.next_snapshot_is_before;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.observed.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
