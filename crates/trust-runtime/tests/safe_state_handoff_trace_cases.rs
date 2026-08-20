use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use serde_json::json;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoAddress, IoDriver, IoDriverHealth, IoSafeState};
use trust_runtime::scheduler::{ResourceControl, ResourceRunner, ResourceState, StdClock};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_RUNTIME_SAFE_STATE_HANDOFF_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_STOP_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:5e3004dc8ff7a4aead90899d3a995f0dbd38bf6ce335c0d637e4a30cd9e636bb";

#[test]
fn safe_state_handoff_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("safe-state trace runner must enter workspace root");

    let mut probe = SafeStateProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_safe_state_case);

    env::set_current_dir(original_dir)
        .expect("safe-state trace runner must restore current directory");
    let artifact = result.expect("safe-state trace case artifact must be written");
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
        "safe-state trace failures: {}",
        failed.join("; ")
    );
}

fn run_safe_state_case(
    case: &CaseRecord,
    probe: &mut SafeStateProbe,
) -> Result<CaseExecution, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 1 {
        return Err(format!("{} must contain exactly one trace step", case.id));
    }
    let observation = execute_step(&trace[0])?;
    let mismatches = compare_step(&trace[0], &observation)?;
    probe.observed = Some(observation.as_json(trace[0].sequence));

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some(observation.resource_state.clone()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some(observation.resource_state.clone()),
        })
    }
}

fn execute_step(step: &TraceStep) -> Result<Observation, String> {
    let first_health = DriverHealth::parse(&trace_string(&step.stimulus, "first_health")?)?;
    let second_health = trace_string(&step.stimulus, "second_health")?;
    let first = Arc::new(Mutex::new(Vec::new()));
    let second = Arc::new(Mutex::new(Vec::new()));

    let mut runtime = Runtime::new();
    runtime.io_mut().resize(0, 1, 0);
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((
        IoAddress::parse("%QX0.0").map_err(|error| error.to_string())?,
        Value::Bool(false),
    ));
    runtime.set_io_safe_state(safe_state);
    runtime.add_io_driver(
        "first-safe-state",
        Box::new(ObservedDriver::new(first_health, Arc::clone(&first))),
    );
    if second_health != "absent" {
        runtime.add_io_driver(
            "second-safe-state",
            Box::new(ObservedDriver::new(
                DriverHealth::parse(&second_health)?,
                Arc::clone(&second),
            )),
        );
    }

    let runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    let mut handle = runner
        .spawn("safe-state-trace")
        .map_err(|error| error.to_string())?;
    let control = handle.control();
    control.pause().map_err(|error| error.to_string())?;
    wait_for_state(&control, ResourceState::Paused)?;
    first
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
    second
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
    handle.stop();
    handle
        .join()
        .map_err(|_| "safe-state trace resource thread panicked".to_string())?;

    let first_writes = first.lock().unwrap_or_else(|error| error.into_inner());
    let second_writes = second.lock().unwrap_or_else(|error| error.into_inner());
    Ok(Observation {
        first_output: first_writes.last().and_then(|value| value.first()).copied(),
        first_writes: first_writes.len(),
        last_error: handle.last_error().map(|error| error.to_string()),
        resource_state: state_name(handle.state()).to_string(),
        second_output: second_writes
            .last()
            .and_then(|value| value.first())
            .copied(),
        second_writes: second_writes.len(),
    })
}

fn wait_for_state(
    control: &ResourceControl<StdClock>,
    expected: ResourceState,
) -> Result<(), String> {
    let deadline = Instant::now() + StdDuration::from_secs(1);
    while control.state() != expected {
        if Instant::now() >= deadline {
            return Err(format!(
                "resource did not reach {expected:?}; observed {:?}",
                control.state()
            ));
        }
        thread::sleep(StdDuration::from_millis(1));
    }
    Ok(())
}

fn compare_step(step: &TraceStep, actual: &Observation) -> Result<Vec<String>, String> {
    let mut mismatches = Vec::new();
    compare_value(
        step.sequence,
        "resource_state",
        actual.resource_state.as_str(),
        trace_string(&step.expected, "resource_state")?.as_str(),
        &mut mismatches,
    );
    compare_value(
        step.sequence,
        "first_writes",
        actual.first_writes,
        trace_usize(&step.expected, "first_writes")?,
        &mut mismatches,
    );
    compare_value(
        step.sequence,
        "second_writes",
        actual.second_writes,
        trace_usize(&step.expected, "second_writes")?,
        &mut mismatches,
    );
    compare_value(
        step.sequence,
        "first_output",
        actual.first_output,
        Some(trace_u8(&step.expected, "first_output")?),
        &mut mismatches,
    );
    if let Some(expected) = step.expected.get("second_output") {
        let expected = expected
            .as_integer()
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| "trace field second_output must be a byte".to_string())?;
        compare_value(
            step.sequence,
            "second_output",
            actual.second_output,
            Some(expected),
            &mut mismatches,
        );
    }
    let expected_error = trace_string(&step.expected, "last_error_contains")?;
    let observed_error = actual.last_error.as_deref().unwrap_or("none");
    if !observed_error.contains(&expected_error) {
        mismatches.push(format!(
            "step {} last_error expected to contain {expected_error:?}, observed {observed_error:?}",
            step.sequence
        ));
    }
    Ok(mismatches)
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

fn trace_usize(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<usize, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("trace field {key} must be a nonnegative integer"))
}

fn trace_u8(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<u8, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| format!("trace field {key} must be a byte"))
}

fn state_name(state: ResourceState) -> &'static str {
    match state {
        ResourceState::Boot => "boot",
        ResourceState::Ready => "ready",
        ResourceState::Running => "running",
        ResourceState::Paused => "paused",
        ResourceState::Stopped => "stopped",
        ResourceState::Faulted => "faulted",
    }
}

#[derive(Clone, Copy)]
enum DriverHealth {
    Ok,
    Degraded,
    Faulted,
}

impl DriverHealth {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "ok" => Ok(Self::Ok),
            "degraded" => Ok(Self::Degraded),
            "faulted" => Ok(Self::Faulted),
            other => Err(format!("unreviewed driver health {other}")),
        }
    }

    fn observed(self) -> IoDriverHealth {
        match self {
            Self::Ok => IoDriverHealth::Ok,
            Self::Degraded => IoDriverHealth::Degraded {
                error: "handoff pending".into(),
            },
            Self::Faulted => IoDriverHealth::Faulted {
                error: "device down".into(),
            },
        }
    }
}

struct ObservedDriver {
    health: DriverHealth,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ObservedDriver {
    fn new(health: DriverHealth, writes: Arc<Mutex<Vec<Vec<u8>>>>) -> Self {
        Self { health, writes }
    }
}

impl IoDriver for ObservedDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(outputs.to_vec());
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.health.observed()
    }
}

struct Observation {
    first_output: Option<u8>,
    first_writes: usize,
    last_error: Option<String>,
    resource_state: String,
    second_output: Option<u8>,
    second_writes: usize,
}

impl Observation {
    fn as_json(&self, sequence: u32) -> serde_json::Value {
        json!({
            "first_output": self.first_output,
            "first_writes": self.first_writes,
            "last_error": self.last_error,
            "resource_state": self.resource_state,
            "second_output": self.second_output,
            "second_writes": self.second_writes,
            "sequence": sequence,
        })
    }
}

#[derive(Default)]
struct SafeStateProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for SafeStateProbe {
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
