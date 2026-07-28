use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_bytes_from_source, TestHarness};
use trust_runtime::retain::RetainStore;
use trust_runtime::value::Value;
use trust_runtime::{RestartMode, RetainSnapshot};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_RUNTIME_RELOAD_TRANSACTION_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_RELOAD_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:b09bbf3b482c1ca78865496ac9683c914caaf7550e8fead1f26223f218c3b79c";
const CASE_ID: &str = "RT_RELOAD_001_RETAIN_LOAD_FAILURE_PRESERVES_LIVE_RUNTIME";

const SOURCE_V1: &str = r#"
PROGRAM Main
VAR
    count : DINT := DINT#0;
END_VAR
count := count + DINT#1;
END_PROGRAM
"#;

const SOURCE_V2: &str = r#"
PROGRAM Main
VAR
    count : DINT := DINT#0;
END_VAR
count := count + DINT#10;
END_PROGRAM
"#;

struct FailingLoadStore;

impl RetainStore for FailingLoadStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Err(RuntimeError::RetainStore(
            "injected retain load failure".into(),
        ))
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[test]
fn reload_transaction_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("reload transaction runner must enter workspace root");

    let mut probe = ReloadTransactionProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_reload_case);

    env::set_current_dir(original_dir)
        .expect("reload transaction runner must restore current directory");
    let artifact = result.expect("reload transaction artifact must be written");
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
        "reload transaction trace failures: {}",
        failed.join("; ")
    );
}

fn run_reload_case(
    case: &CaseRecord,
    probe: &mut ReloadTransactionProbe,
) -> Result<CaseExecution, String> {
    if case.id != CASE_ID {
        return Err(format!("unreviewed reload transaction case {}", case.id));
    }
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 3 {
        return Err(format!(
            "{} must contain exactly three trace steps",
            case.id
        ));
    }

    let mut harness = vm_harness()?;
    let updated_bytes = bytecode_bytes_from_source(SOURCE_V2)
        .map_err(|error| format!("build updated bytecode: {error}"))?;
    let mut mismatches = Vec::new();
    let mut observations = Vec::new();
    for step in trace {
        let observation = execute_step(&mut harness, &updated_bytes, step)?;
        compare_step(step, &observation, &mut mismatches)?;
        observations.push(observation.clone());
    }
    probe.observed = Some(json!({ "trace": observations }));

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("reload_rejected_without_state_change".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("reload_failure_partially_applied".to_string()),
        })
    }
}

fn vm_harness() -> Result<TestHarness, String> {
    let mut harness = TestHarness::from_source(SOURCE_V1)
        .map_err(|error| format!("compile initial runtime: {error}"))?;
    let bytes = bytecode_bytes_from_source(SOURCE_V1)
        .map_err(|error| format!("build initial bytecode: {error}"))?;
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .map_err(|error| format!("apply initial bytecode: {error}"))?;
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .map_err(|error| format!("select VM backend: {error}"))?;
    harness
        .restart(RestartMode::Cold)
        .map_err(|error| format!("cold restart initial runtime: {error}"))?;
    Ok(harness)
}

fn execute_step(
    harness: &mut TestHarness,
    updated_bytes: &[u8],
    step: &TraceStep,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let action = trace_string(&step.stimulus, "action")?;
    let mut observation = BTreeMap::new();
    match action.as_str() {
        "establish_old_runtime_and_force" => {
            let cycle = harness.cycle();
            if !cycle.errors.is_empty() {
                return Err(format!("initial cycle failed: {cycle:?}"));
            }
            harness
                .runtime_mut()
                .enable_debug()
                .force_global("count", Value::DInt(50));
        }
        "attempt_reload_with_retain_load_failure" => {
            harness
                .runtime_mut()
                .set_retain_store(Some(Box::new(FailingLoadStore)), None);
            match harness
                .runtime_mut()
                .apply_online_change_bytes(updated_bytes, None)
            {
                Ok(_) => {
                    observation.insert("reload_result".to_string(), json!("accepted"));
                }
                Err(error) => {
                    observation.insert("reload_result".to_string(), json!("rejected"));
                    observation.insert("error".to_string(), json!(error.to_string()));
                }
            }
        }
        "release_force_and_run_old_program" => {
            harness
                .runtime()
                .debug_control()
                .ok_or_else(|| "debug control disappeared during failed reload".to_string())?
                .release_global("count");
            let cycle = harness.cycle();
            if !cycle.errors.is_empty() {
                return Err(format!("post-rejection cycle failed: {cycle:?}"));
            }
        }
        other => return Err(format!("unreviewed reload transaction action {other}")),
    }
    observation.insert("count".to_string(), json!(output_dint(harness, "count")?));
    observation.insert(
        "cycle_counter".to_string(),
        json!(harness.runtime().cycle_counter()),
    );
    Ok(observation)
}

fn compare_step(
    step: &TraceStep,
    actual: &BTreeMap<String, serde_json::Value>,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    for (field, expected) in &step.expected {
        if field == "error_contains" {
            let expected = expected
                .as_str()
                .ok_or_else(|| "error_contains must be text".to_string())?;
            let observed = actual
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("none");
            if !observed.contains(expected) {
                mismatches.push(format!(
                    "step {} error expected to contain {expected:?}, observed {observed:?}",
                    step.sequence
                ));
            }
            continue;
        }
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

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be text"))
}

fn toml_to_json(value: &toml::Value) -> Result<serde_json::Value, String> {
    match value {
        toml::Value::String(value) => Ok(json!(value)),
        toml::Value::Integer(value) => Ok(json!(value)),
        toml::Value::Boolean(value) => Ok(json!(value)),
        other => Err(format!("unsupported expected trace value {other:?}")),
    }
}

fn output_dint(harness: &TestHarness, name: &str) -> Result<i32, String> {
    match harness.try_get_output(name) {
        Ok(Value::DInt(value)) => Ok(value),
        Ok(value) => Err(format!("{name} has wrong type: {value:?}")),
        Err(error) => Err(format!("{name} read failed: {error}")),
    }
}

#[derive(Default)]
struct ReloadTransactionProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for ReloadTransactionProbe {
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
