use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::numeric::to_i64;
use trust_runtime_core::value::Value;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_INTEGER_I64_NORMALIZATION_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_INTEGER_I64_NORMALIZATION_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:0b5068f16dc48ced664f4fa1fe7dbffc26b976db80bc4546be483691b359dfa4";

#[test]
fn integer_i64_normalization_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime-core must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = PureNumericProbe;
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_normalization_case)
        .expect("integer i64 normalization artifact must be written");
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
        "integer i64 normalization failures: {}",
        failed.join("; ")
    );
}

fn run_normalization_case(
    case: &CaseRecord,
    _probe: &mut PureNumericProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let checks = match scenario {
        "SIGNED_INTEGER_RUNTIME_VALUE" => signed_checks(),
        "UNSIGNED_REPRESENTABLE_RUNTIME_VALUE" => unsigned_checks(),
        "ULINT_ABOVE_I64_MAXIMUM" => overflow_checks(),
        "NON_INTEGER_RUNTIME_VALUE" => non_integer_checks(),
        other => return Err(format!("unreviewed integer normalization scenario {other}")),
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

type NormalizationCheck = (
    &'static str,
    Result<i64, RuntimeError>,
    Result<i64, RuntimeError>,
);

fn signed_checks() -> Vec<NormalizationCheck> {
    [
        ("sint_min", Value::SInt(i8::MIN), i64::from(i8::MIN)),
        ("sint_max", Value::SInt(i8::MAX), i64::from(i8::MAX)),
        ("int_min", Value::Int(i16::MIN), i64::from(i16::MIN)),
        ("int_max", Value::Int(i16::MAX), i64::from(i16::MAX)),
        ("dint_min", Value::DInt(i32::MIN), i64::from(i32::MIN)),
        ("dint_max", Value::DInt(i32::MAX), i64::from(i32::MAX)),
        ("lint_min", Value::LInt(i64::MIN), i64::MIN),
        ("lint_max", Value::LInt(i64::MAX), i64::MAX),
    ]
    .into_iter()
    .map(|(label, value, expected)| (label, to_i64(&value), Ok(expected)))
    .collect()
}

fn unsigned_checks() -> Vec<NormalizationCheck> {
    [
        ("usint_max", Value::USInt(u8::MAX), i64::from(u8::MAX)),
        ("uint_max", Value::UInt(u16::MAX), i64::from(u16::MAX)),
        ("udint_max", Value::UDInt(u32::MAX), i64::from(u32::MAX)),
        ("ulint_i64_max", Value::ULInt(i64::MAX as u64), i64::MAX),
    ]
    .into_iter()
    .map(|(label, value, expected)| (label, to_i64(&value), Ok(expected)))
    .collect()
}

fn overflow_checks() -> Vec<NormalizationCheck> {
    [
        ("ulint_i64_max_plus_one", i64::MAX as u64 + 1),
        ("ulint_max", u64::MAX),
    ]
    .into_iter()
    .map(|(label, value)| {
        (
            label,
            to_i64(&Value::ULInt(value)),
            Err(RuntimeError::Overflow),
        )
    })
    .collect()
}

fn non_integer_checks() -> Vec<NormalizationCheck> {
    [
        ("bool", Value::Bool(false)),
        ("real", Value::Real(0.0)),
        ("lreal", Value::LReal(0.0)),
        ("byte", Value::Byte(0)),
        ("word", Value::Word(0)),
        ("dword", Value::DWord(0)),
        ("lword", Value::LWord(0)),
        ("string", Value::String("0".into())),
        ("wstring", Value::WString("0".into())),
        ("char", Value::Char(b'0')),
        ("wchar", Value::WChar(u16::from(b'0'))),
        ("reference", Value::Reference(None)),
        ("null", Value::Null),
    ]
    .into_iter()
    .map(|(label, value)| (label, to_i64(&value), Err(RuntimeError::TypeMismatch)))
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
