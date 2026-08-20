use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::numeric::{signed_from_i128, NumericKind};
use trust_runtime_core::value::Value;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_SIGNED_INTEGER_MATERIALIZATION_TRACE_001";
const CASE_FILE: &str =
    "verification/cases/runtime_safety/RT_SIGNED_INTEGER_MATERIALIZATION_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:77f497d45102f17517ae0eba6e154b54e3861d5796082c1e2bacb0a423efc52d";

#[test]
fn signed_integer_materialization_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime-core must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = PureNumericProbe;
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_materialization_case)
        .expect("signed integer materialization artifact must be written");
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
        "signed integer materialization failures: {}",
        failed.join("; ")
    );
}

fn run_materialization_case(
    case: &CaseRecord,
    _probe: &mut PureNumericProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let checks = match scenario {
        "SIGNED_DESTINATION_REPRESENTABLE_VALUE" => representable_checks(),
        "SIGNED_DESTINATION_BELOW_MINIMUM" => below_minimum_checks(),
        "SIGNED_DESTINATION_ABOVE_MAXIMUM" => above_maximum_checks(),
        "NON_SIGNED_DESTINATION_CATEGORY" => non_signed_target_checks(),
        other => return Err(format!("unreviewed materialization scenario {other}")),
    };
    let failures = checks
        .into_iter()
        .filter_map(|(label, actual, expected)| {
            (actual != expected)
                .then(|| format!("{label}: expected {expected:?}, observed {actual:?}"))
        })
        .collect::<Vec<_>>();
    Ok(CaseExecution {
        result: if failures.is_empty() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!failures.is_empty()).then(|| failures.join("; ")),
        observed_status: Some(if failures.is_empty() {
            format!("{scenario}:contract_matched")
        } else {
            format!("{scenario}:contract_mismatch")
        }),
    })
}

type MaterializationCheck = (
    &'static str,
    Result<Value, RuntimeError>,
    Result<Value, RuntimeError>,
);

fn representable_checks() -> Vec<MaterializationCheck> {
    [
        (
            NumericKind::SInt,
            i128::from(i8::MIN),
            Value::SInt(i8::MIN),
            "sint_min",
        ),
        (NumericKind::SInt, 0, Value::SInt(0), "sint_zero"),
        (
            NumericKind::SInt,
            i128::from(i8::MAX),
            Value::SInt(i8::MAX),
            "sint_max",
        ),
        (
            NumericKind::Int,
            i128::from(i16::MIN),
            Value::Int(i16::MIN),
            "int_min",
        ),
        (NumericKind::Int, 0, Value::Int(0), "int_zero"),
        (
            NumericKind::Int,
            i128::from(i16::MAX),
            Value::Int(i16::MAX),
            "int_max",
        ),
        (
            NumericKind::DInt,
            i128::from(i32::MIN),
            Value::DInt(i32::MIN),
            "dint_min",
        ),
        (NumericKind::DInt, 0, Value::DInt(0), "dint_zero"),
        (
            NumericKind::DInt,
            i128::from(i32::MAX),
            Value::DInt(i32::MAX),
            "dint_max",
        ),
        (
            NumericKind::LInt,
            i128::from(i64::MIN),
            Value::LInt(i64::MIN),
            "lint_min",
        ),
        (NumericKind::LInt, 0, Value::LInt(0), "lint_zero"),
        (
            NumericKind::LInt,
            i128::from(i64::MAX),
            Value::LInt(i64::MAX),
            "lint_max",
        ),
    ]
    .into_iter()
    .map(|(target, value, expected, label)| (label, signed_from_i128(target, value), Ok(expected)))
    .collect()
}

fn below_minimum_checks() -> Vec<MaterializationCheck> {
    [
        (NumericKind::SInt, i128::from(i8::MIN) - 1, "sint_below_min"),
        (NumericKind::Int, i128::from(i16::MIN) - 1, "int_below_min"),
        (
            NumericKind::DInt,
            i128::from(i32::MIN) - 1,
            "dint_below_min",
        ),
        (
            NumericKind::LInt,
            i128::from(i64::MIN) - 1,
            "lint_below_min",
        ),
    ]
    .into_iter()
    .map(|(target, value, label)| {
        (
            label,
            signed_from_i128(target, value),
            Err(RuntimeError::Overflow),
        )
    })
    .collect()
}

fn above_maximum_checks() -> Vec<MaterializationCheck> {
    [
        (NumericKind::SInt, i128::from(i8::MAX) + 1, "sint_above_max"),
        (NumericKind::Int, i128::from(i16::MAX) + 1, "int_above_max"),
        (
            NumericKind::DInt,
            i128::from(i32::MAX) + 1,
            "dint_above_max",
        ),
        (
            NumericKind::LInt,
            i128::from(i64::MAX) + 1,
            "lint_above_max",
        ),
    ]
    .into_iter()
    .map(|(target, value, label)| {
        (
            label,
            signed_from_i128(target, value),
            Err(RuntimeError::Overflow),
        )
    })
    .collect()
}

fn non_signed_target_checks() -> Vec<MaterializationCheck> {
    [
        (NumericKind::USInt, "usint_target"),
        (NumericKind::UInt, "uint_target"),
        (NumericKind::UDInt, "udint_target"),
        (NumericKind::ULInt, "ulint_target"),
        (NumericKind::Real, "real_target"),
        (NumericKind::LReal, "lreal_target"),
    ]
    .into_iter()
    .map(|(target, label)| {
        (
            label,
            signed_from_i128(target, 0),
            Err(RuntimeError::TypeMismatch),
        )
    })
    .collect()
}

struct PureNumericProbe;

impl StateProbe for PureNumericProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: None,
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
