use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use serde_json::json;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::Runtime;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_DEADLINE_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:5a3415518942f973be3e62415a0fbee7937dbd00ab06d487096cc183d0c549ca";
const CASE_ID: &str = "RT_SAFE_DEADLINE_001_WATCHDOG_BEFORE_OUTPUT_COMMIT";

#[derive(Clone, Debug)]
struct StepClock {
    inner: Arc<Mutex<Duration>>,
}

impl StepClock {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }
}

impl Clock for StepClock {
    fn now(&self) -> Duration {
        *self.inner.lock().expect("step clock lock")
    }

    fn sleep_until(&self, deadline: Duration) {
        *self.inner.lock().expect("step clock lock") = deadline;
    }

    fn wake(&self) {}
}

#[derive(Debug)]
struct RecordingDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for RecordingDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .expect("driver writes lock")
            .push(outputs.to_vec());
        Ok(())
    }
}

#[test]
fn watchdog_deadline_before_output_commit_case() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("watchdog case runner must enter workspace root");

    let mut probe = WatchdogProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_watchdog_case);

    env::set_current_dir(original_dir)
        .expect("watchdog case runner must restore current directory");
    let artifact = result.expect("watchdog case artifact must be written");
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
        "watchdog output-commit failures: {}",
        failed.join("; ")
    );
}

fn run_watchdog_case(
    case: &CaseRecord,
    probe: &mut WatchdogProbe,
) -> Result<CaseExecution, String> {
    if case.id != CASE_ID {
        return Err(format!("unreviewed watchdog case {}", case.id));
    }
    let step = only_trace_step(case)?;
    let watchdog_enabled = trace_bool(&step.stimulus, "watchdog_enabled")?;
    let timeout_ns = trace_nonnegative_integer(&step.stimulus, "timeout_ns")?;
    let cycle_interval_ms = trace_nonnegative_integer(&step.stimulus, "cycle_interval_ms")?;
    let logical_output = trace_bool(&step.stimulus, "logical_output")?;
    let configured_safe_outputs =
        trace_nonnegative_integer(&step.stimulus, "configured_safe_outputs")?;
    if configured_safe_outputs != 0 {
        return Err("this case is restricted to an empty safe-state configuration".to_string());
    }

    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(0, 1, 0);
    runtime
        .storage_mut()
        .set_global("out", Value::Bool(logical_output));
    runtime.io_mut().bind(
        "out",
        IoAddress::parse("%QX0.0").map_err(|error| error.to_string())?,
    );
    runtime.add_io_driver(
        "watchdog-recorder",
        Box::new(RecordingDriver {
            writes: Arc::clone(&writes),
        }),
    );

    let clock = StepClock::new();
    let mut runner = ResourceRunner::new(runtime, clock, Duration::from_millis(cycle_interval_ms));
    runner.runtime_mut().set_watchdog_policy(WatchdogPolicy {
        enabled: watchdog_enabled,
        timeout: Duration::from_nanos(timeout_ns),
        action: WatchdogAction::Halt,
    });

    let mut handle = runner
        .spawn("watchdog-output-commit-case")
        .map_err(|error| format!("resource spawn failed: {error}"))?;
    let wait_deadline = Instant::now() + StdDuration::from_secs(2);
    while handle.state() != ResourceState::Faulted && Instant::now() < wait_deadline {
        std::thread::sleep(StdDuration::from_millis(1));
    }
    let state = handle.state();
    let error = handle.last_error();
    if state != ResourceState::Faulted {
        handle.stop();
    }
    let join_panicked = handle.join().is_err();
    let recorded_writes = writes.lock().expect("driver writes lock").clone();

    let observed_state = format!("{state:?}").to_ascii_lowercase();
    let observed_error = if matches!(error, Some(RuntimeError::WatchdogTimeout)) {
        "WatchdogTimeout".to_string()
    } else {
        error.map_or_else(|| "none".to_string(), |value| value.to_string())
    };
    let expected_state = trace_string(&step.expected, "resource_state")?;
    let expected_error = trace_string(&step.expected, "last_error")?;
    let expected_write_count =
        trace_nonnegative_integer(&step.expected, "physical_output_write_count")? as usize;

    probe.observed = Some(json!({
        "last_error": observed_error,
        "physical_output_write_count": recorded_writes.len(),
        "physical_output_writes": recorded_writes,
        "resource_state": observed_state,
    }));

    let mut mismatches = Vec::new();
    if observed_state != expected_state {
        mismatches.push(format!(
            "expected resource_state={expected_state}, observed {observed_state}"
        ));
    }
    if observed_error != expected_error {
        mismatches.push(format!(
            "expected last_error={expected_error}, observed {observed_error}"
        ));
    }
    if recorded_writes.len() != expected_write_count {
        mismatches.push(format!(
            "expected physical_output_write_count={expected_write_count}, observed {}",
            recorded_writes.len()
        ));
    }
    if join_panicked {
        mismatches.push("resource thread panicked".to_string());
    }

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("watchdog_contract_passed".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("watchdog_contract_mismatch".to_string()),
        })
    }
}

fn only_trace_step(case: &CaseRecord) -> Result<&TraceStep, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 1 {
        return Err(format!("{} must contain exactly one trace step", case.id));
    }
    Ok(&trace[0])
}

fn trace_bool(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<bool, String> {
    values
        .get(key)
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| format!("trace field {key} must be BOOL"))
}

fn trace_nonnegative_integer(
    values: &BTreeMap<String, toml::Value>,
    key: &str,
) -> Result<i64, String> {
    let value = values
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("trace field {key} must be an integer"))?;
    if value < 0 {
        return Err(format!("trace field {key} must be non-negative"));
    }
    Ok(value)
}

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be a string"))
}

#[derive(Default)]
struct WatchdogProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for WatchdogProbe {
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
