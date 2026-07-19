use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use trust_runtime::RetainSnapshot;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_RETAIN_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:9739265a8c6c8b5a6eeb787dca2acdffc7235dc8bd25f0b317519d2aed6e84da";
const CASE_ID: &str = "RT_SAFE_RETAIN_001_SNAPSHOT_REJECTS_ATOMICALLY_ON_LATE_INCOMPATIBLE_VALUE";

const RETAIN_SOURCE: &str = r#"
VAR_GLOBAL RETAIN
    accepted_first : DINT := DINT#1;
    rejected_later : INT(0..10) := INT#2;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

#[test]
fn retain_failure_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("retain trace runner must enter workspace root");

    let mut probe = RetainProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_retain_case);

    env::set_current_dir(original_dir).expect("retain trace runner must restore current directory");
    let artifact = result.expect("retain trace case artifact must be written");
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
        "retain trace failures: {}",
        failed.join("; ")
    );
}

fn run_retain_case(case: &CaseRecord, probe: &mut RetainProbe) -> Result<CaseExecution, String> {
    if case.id != CASE_ID {
        return Err(format!("unreviewed retain trace case {}", case.id));
    }
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 2 {
        return Err(format!("{} must contain exactly two trace steps", case.id));
    }

    let mut harness = TestHarness::from_source(RETAIN_SOURCE)
        .map_err(|error| format!("{} failed to compile: {error}", case.id))?;
    let mut mismatches = Vec::new();
    let mut observations = Vec::new();
    for step in trace {
        let observation = execute_step(&mut harness, step)?;
        compare_step(step, &observation, &mut mismatches)?;
        observations.push(observation.as_json(step.sequence));
    }
    probe.observed = Some(json!({ "trace": observations }));

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("retain_snapshot_rejected_atomically".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("retain_snapshot_partial_apply".to_string()),
        })
    }
}

fn execute_step(harness: &mut TestHarness, step: &TraceStep) -> Result<Observation, String> {
    let action = trace_string(&step.stimulus, "action")?;
    let requested_first = trace_i32(&step.stimulus, "accepted_first")?;
    let requested_later = trace_i16(&step.stimulus, "rejected_later")?;
    let (apply_result, error) = match action.as_str() {
        "set_current" => {
            harness.set_input("accepted_first", Value::DInt(requested_first));
            harness.set_input("rejected_later", Value::Int(requested_later));
            ("not_attempted", None)
        }
        "apply_snapshot" => {
            let mut snapshot = RetainSnapshot::default();
            snapshot.insert("accepted_first", Value::DInt(requested_first));
            snapshot.insert("rejected_later", Value::Int(requested_later));
            match harness.runtime_mut().apply_retain_snapshot(&snapshot) {
                Ok(()) => ("accepted", None),
                Err(error) => ("rejected", Some(error.to_string())),
            }
        }
        other => return Err(format!("unreviewed retain trace action {other}")),
    };

    Ok(Observation {
        accepted_first: output_dint(harness, "accepted_first", step.sequence)?,
        apply_result,
        error,
        rejected_later: output_int(harness, "rejected_later", step.sequence)?,
    })
}

fn compare_step(
    step: &TraceStep,
    actual: &Observation,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    let expected_first = trace_i32(&step.expected, "accepted_first")?;
    let expected_later = trace_i16(&step.expected, "rejected_later")?;
    let expected_result = trace_string(&step.expected, "apply_result")?;
    compare_value(
        step.sequence,
        "accepted_first",
        actual.accepted_first,
        expected_first,
        mismatches,
    );
    compare_value(
        step.sequence,
        "rejected_later",
        actual.rejected_later,
        expected_later,
        mismatches,
    );
    compare_value(
        step.sequence,
        "apply_result",
        actual.apply_result,
        expected_result.as_str(),
        mismatches,
    );
    if let Some(expected_error) = step.expected.get("error_contains") {
        let expected_error = expected_error
            .as_str()
            .ok_or_else(|| "trace field error_contains must be text".to_string())?;
        let actual_error = actual.error.as_deref().unwrap_or("none");
        if !actual_error.contains(expected_error) {
            mismatches.push(format!(
                "step {} error expected to contain {expected_error:?}, observed {actual_error:?}",
                step.sequence
            ));
        }
    }
    Ok(())
}

fn compare_value<T: std::fmt::Debug + PartialEq>(
    sequence: u32,
    field: &str,
    actual: T,
    expected: T,
    mismatches: &mut Vec<String>,
) {
    if actual != expected {
        mismatches.push(format!(
            "step {sequence} {field} expected {expected:?}, observed {actual:?}"
        ));
    }
}

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be text"))
}

fn trace_i64(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i64, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("trace field {key} must be an integer"))
}

fn trace_i32(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i32, String> {
    i32::try_from(trace_i64(values, key)?)
        .map_err(|_| format!("trace field {key} is outside DINT range"))
}

fn trace_i16(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i16, String> {
    i16::try_from(trace_i64(values, key)?)
        .map_err(|_| format!("trace field {key} is outside INT range"))
}

fn output_dint(harness: &TestHarness, name: &str, sequence: u32) -> Result<i32, String> {
    match harness.try_get_output(name) {
        Ok(Value::DInt(value)) => Ok(value),
        Ok(value) => Err(format!("step {sequence} {name} has wrong type: {value:?}")),
        Err(error) => Err(format!("step {sequence} {name} read failed: {error}")),
    }
}

fn output_int(harness: &TestHarness, name: &str, sequence: u32) -> Result<i16, String> {
    match harness.try_get_output(name) {
        Ok(Value::Int(value)) => Ok(value),
        Ok(value) => Err(format!("step {sequence} {name} has wrong type: {value:?}")),
        Err(error) => Err(format!("step {sequence} {name} read failed: {error}")),
    }
}

struct Observation {
    accepted_first: i32,
    apply_result: &'static str,
    error: Option<String>,
    rejected_later: i16,
}

impl Observation {
    fn as_json(&self, sequence: u32) -> serde_json::Value {
        json!({
            "accepted_first": self.accepted_first,
            "apply_result": self.apply_result,
            "error": self.error,
            "rejected_later": self.rejected_later,
            "sequence": sequence,
        })
    }
}

#[derive(Default)]
struct RetainProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for RetainProbe {
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
