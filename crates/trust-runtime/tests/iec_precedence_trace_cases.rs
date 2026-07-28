use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_IEC_PRECEDENCE_TRACE_001";
const CASE_FILE: &str = "verification/cases/compiler_iec/IEC_PREC_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:019b4d3ab7f122061c1b6af27b568e5cb0b8b9d5c15db4ca180c72ee24905710";

#[test]
fn expression_precedence_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = PrecedenceProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_precedence_case);

    let artifact = result.expect("precedence case artifact must be written");
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
        "precedence case failures: {}",
        failed.join("; ")
    );
}

fn run_precedence_case(
    case: &CaseRecord,
    probe: &mut PrecedenceProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let (expression, expected) = match scenario {
        "MULTIPLICATIVE_BEFORE_ADDITIVE" => ("1 + 2 * 3", Value::DInt(7)),
        "PARENTHESES_OVERRIDE_PRECEDENCE" => ("(1 + 2) * 3", Value::DInt(9)),
        "EXPONENTIATION_LEFT_ASSOCIATIVE" => ("2 ** 3 ** 2", Value::DInt(64)),
        "UNARY_BEFORE_EXPONENTIATION" => ("-2 ** 2", Value::DInt(4)),
        "ADDITIVE_LEFT_ASSOCIATIVE" => ("20 - 5 - 3", Value::DInt(12)),
        "MULTIPLICATIVE_LEFT_ASSOCIATIVE" => ("100 / 10 / 2", Value::DInt(5)),
        "COMPARISON_BEFORE_BOOLEAN_AND" => ("1 < 2 AND 2 < 3", Value::Bool(true)),
        "MULTIPLICATIVE_BEFORE_COMPARISON" => ("2 * 3 = 6", Value::Bool(true)),
        "AND_BEFORE_XOR_BEFORE_OR" => (
            "(TRUE XOR TRUE AND FALSE) AND (TRUE OR TRUE XOR TRUE)",
            Value::Bool(true),
        ),
        other => return Err(format!("unreviewed precedence scenario {other}")),
    };
    let result_type = if matches!(expected, Value::Bool(_)) {
        "BOOL"
    } else {
        "DINT"
    };
    let source = format!(
        "PROGRAM Test\nVAR\n    result : {result_type};\nEND_VAR\nresult := {expression};\nEND_PROGRAM\n"
    );
    let mut harness = TestHarness::from_source(&source)
        .map_err(|error| format!("{} failed to compile: {error}", case.id))?;
    let cycle = harness.cycle();
    if !cycle.errors.is_empty() {
        return Err(format!("{} runtime errors: {:?}", case.id, cycle.errors));
    }
    let actual = harness
        .try_get_output("result")
        .map_err(|error| format!("{} result read failed: {error}", case.id))?;
    probe.observed = Some(serde_json::json!({
        "expression": expression,
        "result": format!("{actual:?}"),
    }));

    if actual == expected {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("precedence_match".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(format!(
                "{expression} expected {expected:?}, observed {actual:?}"
            )),
            observed_status: Some("precedence_mismatch".to_string()),
        })
    }
}

#[derive(Default)]
struct PrecedenceProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for PrecedenceProbe {
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
