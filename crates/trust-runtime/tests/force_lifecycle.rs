use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoSafeState};
use trust_runtime::value::Value;
use trust_runtime::RestartMode;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_RUNTIME_FORCE_LIFECYCLE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_FORCE_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:54aa127239e290a67249047cb771f7e46572c8af5ede8365f9b6513a76d96bc2";

const FORCE_LIFECYCLE_SOURCE: &str = r#"
VAR_GLOBAL
    forced_value : DINT := DINT#1;
    queued_value : DINT := DINT#1;
    output_bit AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
forced_value := forced_value + DINT#1;
queued_value := queued_value + DINT#1;
output_bit := FALSE;
END_PROGRAM
"#;

#[test]
fn force_lifecycle_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("force lifecycle runner must enter workspace root");

    let mut probe = ForceLifecycleProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_force_lifecycle_case);

    env::set_current_dir(original_dir)
        .expect("force lifecycle runner must restore current directory");
    let artifact = result.expect("force lifecycle case artifact must be written");
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
        "force lifecycle trace failures: {}",
        failed.join("; ")
    );
}

fn run_force_lifecycle_case(
    case: &CaseRecord,
    probe: &mut ForceLifecycleProbe,
) -> Result<CaseExecution, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 1 {
        return Err(format!("{} must contain exactly one trace step", case.id));
    }

    let mut harness = TestHarness::from_source(FORCE_LIFECYCLE_SOURCE)
        .map_err(|error| format!("{} failed to compile: {error}", case.id))?;
    let observation = match case.id.as_str() {
        "RT_SAFE_FORCE_001_RESTART_CLEARS_DEBUG_MUTATIONS" => {
            run_restart_case(&mut harness, &trace[0])?
        }
        "RT_SAFE_FORCE_001_SAFE_STATE_CLEARS_IO_FORCE" => {
            run_safe_state_case(&mut harness, &trace[0])?
        }
        other => return Err(format!("unreviewed force lifecycle trace case {other}")),
    };

    let mut mismatches = Vec::new();
    compare_expected(&trace[0], &observation, &mut mismatches)?;
    probe.observed = Some(json!({ "trace": [observation] }));
    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("debug_mutations_cleared".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("debug_mutations_persisted".to_string()),
        })
    }
}

fn run_restart_case(
    harness: &mut TestHarness,
    step: &TraceStep,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let action = trace_string(&step.stimulus, "action")?;
    if action != "warm_restart_then_cycle" {
        return Err(format!("unreviewed restart action {action}"));
    }
    let force_value = trace_i32(&step.stimulus, "force_value")?;
    let queued_write_value = trace_i32(&step.stimulus, "queued_write_value")?;
    let debug = harness.runtime_mut().enable_debug();
    debug.force_global("forced_value", Value::DInt(force_value));
    debug.enqueue_global_write("queued_value", Value::DInt(queued_write_value));
    harness
        .restart(RestartMode::Warm)
        .map_err(|error| format!("warm restart failed: {error}"))?;
    let cycle = harness.cycle();
    if !cycle.errors.is_empty() {
        return Err(format!("post-restart cycle failed: {cycle:?}"));
    }

    Ok(BTreeMap::from([
        (
            "forced_value".to_string(),
            json!(output_dint(harness, "forced_value")?),
        ),
        (
            "queued_value".to_string(),
            json!(output_dint(harness, "queued_value")?),
        ),
    ]))
}

fn run_safe_state_case(
    harness: &mut TestHarness,
    step: &TraceStep,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let action = trace_string(&step.stimulus, "action")?;
    if action != "apply_safe_state_then_cycle" {
        return Err(format!("unreviewed safe-state action {action}"));
    }
    let forced_output = trace_bool(&step.stimulus, "forced_output")?;
    let safe_output = trace_bool(&step.stimulus, "safe_output")?;
    let address = IoAddress::parse("%QX0.0").map_err(|error| error.to_string())?;
    let debug = harness.runtime_mut().enable_debug();
    debug.force_io(address.clone(), Value::Bool(forced_output));
    let forced_cycle = harness.cycle();
    if !forced_cycle.errors.is_empty() {
        return Err(format!("forced cycle failed: {forced_cycle:?}"));
    }

    let mut safe_state = IoSafeState::default();
    safe_state
        .outputs
        .push((address.clone(), Value::Bool(safe_output)));
    harness.runtime_mut().set_io_safe_state(safe_state);
    harness
        .runtime_mut()
        .apply_io_safe_state()
        .map_err(|error| format!("safe-state application failed: {error}"))?;
    let post_stop_cycle = harness.cycle();
    if !post_stop_cycle.errors.is_empty() {
        return Err(format!("post-stop cycle failed: {post_stop_cycle:?}"));
    }
    let output_bit = match harness.runtime().io().read(&address) {
        Ok(Value::Bool(value)) => value,
        Ok(value) => return Err(format!("output_bit has wrong type: {value:?}")),
        Err(error) => return Err(format!("output_bit read failed: {error}")),
    };
    Ok(BTreeMap::from([(
        "output_bit".to_string(),
        json!(output_bit),
    )]))
}

fn compare_expected(
    step: &TraceStep,
    actual: &BTreeMap<String, serde_json::Value>,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    for (field, expected) in &step.expected {
        let expected = toml_to_json(expected)?;
        let observed = actual.get(field);
        if observed != Some(&expected) {
            mismatches.push(format!(
                "step {} {field} expected {expected}, observed {}",
                step.sequence,
                observed.map_or_else(|| "missing".to_string(), ToString::to_string)
            ));
        }
    }
    Ok(())
}

fn toml_to_json(value: &toml::Value) -> Result<serde_json::Value, String> {
    match value {
        toml::Value::String(value) => Ok(json!(value)),
        toml::Value::Integer(value) => Ok(json!(value)),
        toml::Value::Boolean(value) => Ok(json!(value)),
        other => Err(format!("unsupported expected trace value {other:?}")),
    }
}

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be text"))
}

fn trace_i32(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i32, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("trace field {key} must be an integer"))
        .and_then(|value| {
            i32::try_from(value).map_err(|_| format!("trace field {key} is outside DINT range"))
        })
}

fn trace_bool(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<bool, String> {
    values
        .get(key)
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| format!("trace field {key} must be boolean"))
}

fn output_dint(harness: &TestHarness, name: &str) -> Result<i32, String> {
    match harness.try_get_output(name) {
        Ok(Value::DInt(value)) => Ok(value),
        Ok(value) => Err(format!("{name} has wrong type: {value:?}")),
        Err(error) => Err(format!("{name} read failed: {error}")),
    }
}

#[derive(Default)]
struct ForceLifecycleProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for ForceLifecycleProbe {
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
