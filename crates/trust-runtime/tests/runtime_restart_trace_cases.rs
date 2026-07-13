use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_RUNTIME_RESTART_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_RESTART_TIME_002.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:2e33873a2bbc84474e5bc403e836b773d31000bb584f702a644c8221b20aa1a2";

const RESTART_SOURCE: &str = r#"
PROGRAM Main
VAR RETAIN
    retained_value : INT := 1;
END_VAR
VAR NON_RETAIN
    non_retained_value : INT := 2;
END_VAR
VAR
    ordinary_value : INT := 3;
    timer : TON;
    timer_in : BOOL;
    pt : TIME := T#10ms;
    q : BOOL;
    et : TIME;
END_VAR
timer(IN := timer_in, PT := pt, Q => q, ET => et);
END_PROGRAM
"#;

#[test]
fn runtime_restart_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("restart trace runner must enter workspace root");

    let mut probe = RestartProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_trace_case);

    env::set_current_dir(original_dir)
        .expect("restart trace runner must restore current directory");
    let artifact = result.expect("restart trace case artifact must be written");
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
        "restart trace failures: {}",
        failed.join("; ")
    );
}

fn run_trace_case(case: &CaseRecord, probe: &mut RestartProbe) -> Result<CaseExecution, String> {
    match case.id.as_str() {
        "RT_SAFE_RESTART_TIME_002_WARM_PRESERVES_TIME_AND_REINITIALIZES_STATE"
        | "RT_SAFE_RESTART_TIME_002_COLD_PRESERVES_TIME_AND_REINITIALIZES_STATE" => {}
        other => return Err(format!("unreviewed restart trace case {other}")),
    }
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    let mut harness = TestHarness::from_source(RESTART_SOURCE)
        .map_err(|error| format!("{} failed to compile: {error}", case.id))?;
    let mut mismatches = Vec::new();

    for step in trace {
        execute_trace_step(&mut harness, step, probe, &mut mismatches)?;
    }

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("trace_passed".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("trace_mismatch".to_string()),
        })
    }
}

fn execute_trace_step(
    harness: &mut TestHarness,
    step: &TraceStep,
    probe: &mut RestartProbe,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    if let Some(mode) = optional_text(&step.stimulus, "restart")? {
        let mode = match mode {
            "warm" => RestartMode::Warm,
            "cold" => RestartMode::Cold,
            other => {
                return Err(format!(
                    "step {} unknown restart mode {other}",
                    step.sequence
                ))
            }
        };
        harness
            .restart(mode)
            .map_err(|error| format!("step {} restart failed: {error}", step.sequence))?;
    }

    set_optional_bool(harness, &step.stimulus, "input", "timer_in", step.sequence)?;
    set_optional_int(
        harness,
        &step.stimulus,
        "retain",
        "retained_value",
        step.sequence,
    )?;
    set_optional_int(
        harness,
        &step.stimulus,
        "non_retain",
        "non_retained_value",
        step.sequence,
    )?;
    set_optional_int(
        harness,
        &step.stimulus,
        "ordinary",
        "ordinary_value",
        step.sequence,
    )?;

    let advance_ms = required_non_negative_integer(&step.stimulus, "advance_ms")?;
    if advance_ms > 0 {
        harness.advance_time(Duration::from_millis(advance_ms));
    }
    let cycle = harness.cycle();
    if !cycle.errors.is_empty() {
        return Err(format!(
            "step {} runtime errors: {:?}",
            step.sequence, cycle.errors
        ));
    }

    let actual = ObservedState {
        current_time_ms: harness.current_time().as_millis(),
        et_ms: output_duration_ms(harness, "et", step.sequence)?,
        non_retain: output_int(harness, "non_retained_value", step.sequence)?,
        ordinary: output_int(harness, "ordinary_value", step.sequence)?,
        q: output_bool(harness, "q", step.sequence)?,
        retain: output_int(harness, "retained_value", step.sequence)?,
    };
    let expected = ObservedState {
        current_time_ms: required_non_negative_integer(&step.expected, "current_time_ms")?,
        et_ms: required_non_negative_integer(&step.expected, "et_ms")?,
        non_retain: required_i16(&step.expected, "non_retain")?,
        ordinary: required_i16(&step.expected, "ordinary")?,
        q: required_bool(&step.expected, "q")?,
        retain: required_i16(&step.expected, "retain")?,
    };

    probe.observed = Some(actual.as_json(step.sequence));
    if actual != expected {
        mismatches.push(format!(
            "step {} expected {expected:?}; observed {actual:?}",
            step.sequence
        ));
    }
    Ok(())
}

fn set_optional_bool(
    harness: &mut TestHarness,
    values: &BTreeMap<String, toml::Value>,
    key: &str,
    target: &str,
    sequence: u32,
) -> Result<(), String> {
    let Some(value) = values.get(key) else {
        return Ok(());
    };
    let value = value
        .as_bool()
        .ok_or_else(|| format!("trace field {key} must be BOOL"))?;
    harness
        .try_set_input(target, Value::Bool(value))
        .map_err(|error| format!("step {sequence} {target} write failed: {error}"))
}

fn set_optional_int(
    harness: &mut TestHarness,
    values: &BTreeMap<String, toml::Value>,
    key: &str,
    target: &str,
    sequence: u32,
) -> Result<(), String> {
    let Some(value) = values.get(key) else {
        return Ok(());
    };
    let value = i16::try_from(
        value
            .as_integer()
            .ok_or_else(|| format!("trace field {key} must be INT"))?,
    )
    .map_err(|_| format!("trace field {key} is outside INT range"))?;
    harness
        .try_set_input(target, Value::Int(value))
        .map_err(|error| format!("step {sequence} {target} write failed: {error}"))
}

fn optional_text<'a>(
    values: &'a BTreeMap<String, toml::Value>,
    key: &str,
) -> Result<Option<&'a str>, String> {
    values
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("trace field {key} must be text"))
        })
        .transpose()
}

fn required_bool(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<bool, String> {
    values
        .get(key)
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| format!("trace field {key} must be BOOL"))
}

fn required_i16(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i16, String> {
    i16::try_from(
        values
            .get(key)
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| format!("trace field {key} must be INT"))?,
    )
    .map_err(|_| format!("trace field {key} is outside INT range"))
}

fn required_non_negative_integer(
    values: &BTreeMap<String, toml::Value>,
    key: &str,
) -> Result<i64, String> {
    let value = values
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("trace field {key} must be non-negative integer"))?;
    if value < 0 {
        return Err(format!("trace field {key} must be non-negative integer"));
    }
    Ok(value)
}

fn output_bool(harness: &TestHarness, name: &str, sequence: u32) -> Result<bool, String> {
    match harness.try_get_output(name) {
        Ok(Value::Bool(value)) => Ok(value),
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

fn output_duration_ms(harness: &TestHarness, name: &str, sequence: u32) -> Result<i64, String> {
    match harness.try_get_output(name) {
        Ok(Value::Time(value)) | Ok(Value::LTime(value)) => Ok(value.as_millis()),
        Ok(value) => Err(format!("step {sequence} {name} has wrong type: {value:?}")),
        Err(error) => Err(format!("step {sequence} {name} read failed: {error}")),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ObservedState {
    current_time_ms: i64,
    et_ms: i64,
    non_retain: i16,
    ordinary: i16,
    q: bool,
    retain: i16,
}

impl ObservedState {
    fn as_json(&self, sequence: u32) -> serde_json::Value {
        json!({
            "current_time_ms": self.current_time_ms,
            "et_ms": self.et_ms,
            "non_retain": self.non_retain,
            "ordinary": self.ordinary,
            "q": self.q,
            "retain": self.retain,
            "sequence": sequence,
        })
    }
}

#[derive(Default)]
struct RestartProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for RestartProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_before {
            self.observed = None;
        }
        let target = self.observed.clone();
        self.next_snapshot_is_before = !self.next_snapshot_is_before;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target,
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
