use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration as WallDuration, Instant};

use serde_json::json;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState};
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::FaultPolicy;
use trust_runtime::Runtime;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_RUNTIME_PANIC_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_PANIC_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:b3d0a395b84d31729cf2a16720a8dcf8063ac9b842418a75775ea47912c82fe9";

#[test]
fn runtime_panic_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("panic trace runner must enter workspace root");

    let mut probe = PanicProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_panic_case);

    env::set_current_dir(original_dir).expect("panic trace runner must restore current directory");
    let artifact = result.expect("panic case artifact must be written");
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
        "panic containment trace failures: {}",
        failed.join("; ")
    );
}

fn run_panic_case(case: &CaseRecord, probe: &mut PanicProbe) -> Result<CaseExecution, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 1 {
        return Err(format!("{} must contain exactly one trace step", case.id));
    }
    let step = &trace[0];
    let scenario = trace_string(&step.stimulus, "scenario")?;
    let observed = match scenario.as_str() {
        "input_driver_panic" => input_driver_panic_observation(false)?,
        "panic_safe_output" => safe_output_panic_observation()?,
        "outer_resource_thread_panic" => outer_thread_panic_observation()?,
        "panic_with_restart_policy" => input_driver_panic_observation(true)?,
        other => return Err(format!("unreviewed panic scenario {other}")),
    };

    let mut mismatches = Vec::new();
    compare_expected(step, &observed, &mut mismatches)?;
    probe.observed = Some(json!({ "trace": [observed] }));
    Ok(CaseExecution {
        result: if mismatches.is_empty() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!mismatches.is_empty()).then(|| mismatches.join("; ")),
        observed_status: Some(if mismatches.is_empty() {
            "panic_contract_matched".to_string()
        } else {
            "panic_contract_mismatch".to_string()
        }),
    })
}

fn input_driver_panic_observation(
    restart_policy: bool,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(1, 0, 0);
    runtime.add_io_driver("panic-input", Box::new(PanickingReadDriver));
    if restart_policy {
        runtime.set_fault_policy(FaultPolicy::Restart);
    }
    let runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    let mut handle = runner
        .spawn("panic-input-proof")
        .map_err(|error| error.to_string())?;
    wait_for_fault(&handle)?;
    let state = handle.state();
    let resource_panic = matches!(handle.last_error(), Some(RuntimeError::ResourcePanic(_)));
    let join_ok = handle.join().is_ok();
    Ok(BTreeMap::from([
        ("state".to_string(), json!(resource_state_name(state))),
        ("resource_panic".to_string(), json!(resource_panic)),
        ("join_ok".to_string(), json!(join_ok)),
    ]))
}

fn safe_output_panic_observation() -> Result<BTreeMap<String, serde_json::Value>, String> {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(0, 1, 0);
    runtime.storage_mut().set_global("out", Value::Bool(true));
    let address = IoAddress::parse("%QX0.0").map_err(|error| error.to_string())?;
    runtime.io_mut().bind("out", address.clone());
    runtime.add_io_driver(
        "panic-recorder",
        Box::new(PanickingRecordingDriver {
            writes: Arc::clone(&writes),
        }),
    );
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe_state);

    let runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    let mut handle = runner
        .spawn("panic-safe-output-proof")
        .map_err(|error| error.to_string())?;
    wait_for_fault(&handle)?;
    let state = handle.state();
    let resource_panic = matches!(handle.last_error(), Some(RuntimeError::ResourcePanic(_)));
    handle.join().map_err(|error| error.to_string())?;
    let safe_output = writes
        .lock()
        .map_err(|_| "panic writes lock poisoned".to_string())?
        .last()
        .and_then(|bytes| bytes.first())
        .map(|byte| *byte != 0)
        .ok_or_else(|| "safe output was not written".to_string())?;
    Ok(BTreeMap::from([
        ("state".to_string(), json!(resource_state_name(state))),
        ("resource_panic".to_string(), json!(resource_panic)),
        ("safe_output".to_string(), json!(safe_output)),
    ]))
}

fn outer_thread_panic_observation() -> Result<BTreeMap<String, serde_json::Value>, String> {
    let runner = ResourceRunner::new(
        Runtime::new(),
        PanicOnFirstSleepClock::new(),
        Duration::from_millis(1),
    );
    let mut handle = runner
        .spawn("panic-outer-thread-proof")
        .map_err(|error| error.to_string())?;
    wait_for_fault(&handle)?;
    let state = handle.state();
    let resource_panic = matches!(handle.last_error(), Some(RuntimeError::ResourcePanic(_)));
    let join_ok = handle.join().is_ok();
    Ok(BTreeMap::from([
        ("state".to_string(), json!(resource_state_name(state))),
        ("resource_panic".to_string(), json!(resource_panic)),
        ("join_ok".to_string(), json!(join_ok)),
    ]))
}

fn wait_for_fault<C>(handle: &trust_runtime::scheduler::ResourceHandle<C>) -> Result<(), String>
where
    C: Clock + Clone,
{
    let deadline = Instant::now() + WallDuration::from_secs(2);
    while handle.state() != ResourceState::Faulted && Instant::now() < deadline {
        std::thread::sleep(WallDuration::from_millis(1));
    }
    if handle.state() == ResourceState::Faulted {
        Ok(())
    } else {
        Err(format!(
            "resource did not fault; state={:?}, error={:?}",
            handle.state(),
            handle.last_error()
        ))
    }
}

fn resource_state_name(state: ResourceState) -> &'static str {
    match state {
        ResourceState::Boot => "boot",
        ResourceState::Ready => "ready",
        ResourceState::Running => "running",
        ResourceState::Paused => "paused",
        ResourceState::Faulted => "faulted",
        ResourceState::Stopped => "stopped",
    }
}

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

#[derive(Clone, Debug)]
struct PanicOnFirstSleepClock {
    sleeps: Arc<AtomicUsize>,
}

impl PanicOnFirstSleepClock {
    fn new() -> Self {
        Self {
            sleeps: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Clock for PanicOnFirstSleepClock {
    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn sleep_until(&self, _deadline: Duration) {
        if self.sleeps.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("intentional scheduler sleep panic after running");
        }
    }

    fn wake(&self) {}
}

#[derive(Debug)]
struct PanickingReadDriver;

impl IoDriver for PanickingReadDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        panic!("intentional runtime safety panic from input driver");
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[derive(Debug)]
struct PanickingRecordingDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for PanickingRecordingDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        panic!("intentional runtime safety panic before output commit");
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .expect("driver writes lock")
            .push(outputs.to_vec());
        Ok(())
    }
}

fn compare_expected(
    step: &TraceStep,
    observed: &BTreeMap<String, serde_json::Value>,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    for (field, expected) in &step.expected {
        let expected = toml_to_json(expected)?;
        if observed.get(field) != Some(&expected) {
            mismatches.push(format!(
                "step {} {field} expected {expected}, observed {}",
                step.sequence,
                observed
                    .get(field)
                    .map_or_else(|| "missing".to_string(), ToString::to_string)
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

#[derive(Default)]
struct PanicProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for PanicProbe {
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
