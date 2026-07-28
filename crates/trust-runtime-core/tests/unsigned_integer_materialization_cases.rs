use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::numeric::{unsigned_from_u128, NumericKind};
use trust_runtime_core::value::Value;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_UNSIGNED_INTEGER_MATERIALIZATION_TRACE_001";
const CASE_FILE: &str =
    "verification/cases/runtime_safety/RT_UNSIGNED_INTEGER_MATERIALIZATION_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:a211f56c097808bde5b45d8c7e5db8f18139f85714016b4227cded296e3d827b";

#[test]
fn unsigned_integer_materialization_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime-core must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = PureNumericProbe;
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_materialization_case)
        .expect("unsigned integer materialization artifact must be written");
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
        "unsigned integer materialization failures: {}",
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
        "UNSIGNED_DESTINATION_REPRESENTABLE_VALUE" => representable_checks(),
        "UNSIGNED_DESTINATION_ABOVE_MAXIMUM" => above_maximum_checks(),
        "NON_UNSIGNED_DESTINATION_CATEGORY" => non_unsigned_target_checks(),
        "UNSIGNED_DESTINATION_BELOW_ZERO" => below_minimum_checks(),
        other => {
            return Err(format!(
                "unreviewed unsigned materialization scenario {other}"
            ))
        }
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
            NumericKind::USInt,
            u128::from(u8::MAX),
            Value::USInt(u8::MAX),
            "usint_zero",
            "usint_max",
        ),
        (
            NumericKind::UInt,
            u128::from(u16::MAX),
            Value::UInt(u16::MAX),
            "uint_zero",
            "uint_max",
        ),
        (
            NumericKind::UDInt,
            u128::from(u32::MAX),
            Value::UDInt(u32::MAX),
            "udint_zero",
            "udint_max",
        ),
        (
            NumericKind::ULInt,
            u128::from(u64::MAX),
            Value::ULInt(u64::MAX),
            "ulint_zero",
            "ulint_max",
        ),
    ]
    .into_iter()
    .flat_map(
        |(target, maximum, expected_maximum, zero_label, maximum_label)| {
            let expected_zero = match target {
                NumericKind::USInt => Value::USInt(0),
                NumericKind::UInt => Value::UInt(0),
                NumericKind::UDInt => Value::UDInt(0),
                NumericKind::ULInt => Value::ULInt(0),
                _ => unreachable!("representable matrix contains only unsigned targets"),
            };
            [
                (zero_label, unsigned_from_u128(target, 0), Ok(expected_zero)),
                (
                    maximum_label,
                    unsigned_from_u128(target, maximum),
                    Ok(expected_maximum),
                ),
            ]
        },
    )
    .collect()
}

fn above_maximum_checks() -> Vec<MaterializationCheck> {
    [
        (
            NumericKind::USInt,
            u128::from(u8::MAX) + 1,
            "usint_above_max",
        ),
        (
            NumericKind::UInt,
            u128::from(u16::MAX) + 1,
            "uint_above_max",
        ),
        (
            NumericKind::UDInt,
            u128::from(u32::MAX) + 1,
            "udint_above_max",
        ),
        (
            NumericKind::ULInt,
            u128::from(u64::MAX) + 1,
            "ulint_above_max",
        ),
    ]
    .into_iter()
    .map(|(target, value, label)| {
        (
            label,
            unsigned_from_u128(target, value),
            Err(RuntimeError::Overflow),
        )
    })
    .collect()
}

fn non_unsigned_target_checks() -> Vec<MaterializationCheck> {
    [
        (NumericKind::SInt, "sint_target"),
        (NumericKind::Int, "int_target"),
        (NumericKind::DInt, "dint_target"),
        (NumericKind::LInt, "lint_target"),
        (NumericKind::Real, "real_target"),
        (NumericKind::LReal, "lreal_target"),
    ]
    .into_iter()
    .map(|(target, label)| {
        (
            label,
            unsigned_from_u128(target, 0),
            Err(RuntimeError::TypeMismatch),
        )
    })
    .collect()
}

fn below_minimum_checks() -> Vec<MaterializationCheck> {
    vec![
        (
            "usint_below_zero",
            u8::try_from(-1_i64)
                .map(Value::USInt)
                .map_err(|_| RuntimeError::Overflow),
            Err(RuntimeError::Overflow),
        ),
        (
            "uint_below_zero",
            u16::try_from(-1_i64)
                .map(Value::UInt)
                .map_err(|_| RuntimeError::Overflow),
            Err(RuntimeError::Overflow),
        ),
        (
            "udint_below_zero",
            u32::try_from(-1_i64)
                .map(Value::UDInt)
                .map_err(|_| RuntimeError::Overflow),
            Err(RuntimeError::Overflow),
        ),
        (
            "ulint_below_zero",
            u64::try_from(-1_i64)
                .map(Value::ULInt)
                .map_err(|_| RuntimeError::Overflow),
            Err(RuntimeError::Overflow),
        ),
    ]
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
