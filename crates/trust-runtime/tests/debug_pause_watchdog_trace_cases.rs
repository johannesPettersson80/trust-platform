use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use serde_json::json;
use trust_runtime::debug::{DebugStopReason, RuntimeEvent};
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ResourceRunner, ResourceState, StdClock};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_DEBUG_PAUSE_TRACE_001";
const CASE_FILE: &str = "verification/cases/editor_safety/DEBUG_PAUSE_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:55b162ceb84c960ea196be182d8ad5d5f740f9fc2f6bf47a012b81ab776efe9d";

const COUNTER_SOURCE: &str = r#"
PROGRAM Main
VAR
    counter : DINT := 0;
END_VAR
counter := counter + DINT#1;
END_PROGRAM
"#;

#[test]
fn debug_pause_watchdog_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("pause trace runner must enter workspace root");

    let mut probe = PauseProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_pause_case);

    env::set_current_dir(original_dir).expect("pause trace runner must restore current directory");
    let artifact = result.expect("pause case artifact must be written");
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
        "pause/watchdog trace failures: {}",
        failed.join("; ")
    );
}

fn run_pause_case(case: &CaseRecord, probe: &mut PauseProbe) -> Result<CaseExecution, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 1 {
        return Err(format!("{} must contain exactly one trace step", case.id));
    }
    let step = &trace[0];
    let scenario = trace_string(&step.stimulus, "scenario")?;
    let watchdog_ms = trace_millis(&step.stimulus, "watchdog_ms")?;
    let dwell_ms = trace_millis(&step.stimulus, "dwell_ms")?;
    let observed = match scenario.as_str() {
        "statement_pause_longer_than_watchdog" => {
            statement_pause_observation(watchdog_ms, dwell_ms)?
        }
        "resource_pause_between_cycles" => resource_pause_observation(watchdog_ms, dwell_ms)?,
        other => return Err(format!("unreviewed pause scenario {other}")),
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
            "pause_contract_matched".to_string()
        } else {
            "pause_contract_mismatch".to_string()
        }),
    })
}

fn statement_pause_observation(
    watchdog_ms: i64,
    dwell_ms: i64,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let (runtime, debug) = counter_runtime()?;
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    debug.set_stop_sender(stop_tx);
    debug.pause();

    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    runner
        .runtime_mut()
        .set_watchdog_policy(watchdog_policy(watchdog_ms));
    let mut handle = runner
        .spawn("debug-pause-proof")
        .map_err(|error| error.to_string())?;
    let stop = stop_rx
        .recv_timeout(WallDuration::from_secs(2))
        .map_err(|error| format!("runtime did not stop: {error}"))?;
    if stop.reason != DebugStopReason::Pause {
        return Err(format!("unexpected stop reason: {:?}", stop.reason));
    }

    thread::sleep(WallDuration::from_millis(dwell_ms as u64));
    debug.continue_run();
    thread::sleep(WallDuration::from_millis(60));
    let observed = BTreeMap::from([
        (
            "state".to_string(),
            json!(resource_state_name(handle.state())),
        ),
        (
            "has_error".to_string(),
            json!(handle.last_error().is_some()),
        ),
    ]);
    handle.stop();
    debug.continue_run();
    handle
        .join()
        .map_err(|_| "statement-pause resource thread panicked".to_string())?;
    Ok(observed)
}

fn resource_pause_observation(
    watchdog_ms: i64,
    dwell_ms: i64,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let (runtime, debug) = counter_runtime()?;
    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    runner
        .runtime_mut()
        .set_watchdog_policy(watchdog_policy(watchdog_ms));
    let mut handle = runner
        .spawn("resource-pause-proof")
        .map_err(|error| error.to_string())?;
    let control = handle.control();

    wait_for_state(&handle, ResourceState::Running)?;
    control.pause().map_err(|error| error.to_string())?;
    wait_for_state(&handle, ResourceState::Paused)?;
    let _ = debug.drain_runtime_events();
    thread::sleep(WallDuration::from_millis(dwell_ms as u64));
    let cycle_events_during_pause = debug
        .drain_runtime_events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                RuntimeEvent::CycleStart { .. } | RuntimeEvent::CycleEnd { .. }
            )
        })
        .count();

    control.resume().map_err(|error| error.to_string())?;
    wait_for_state(&handle, ResourceState::Running)?;
    thread::sleep(WallDuration::from_millis(40));
    let observed = BTreeMap::from([
        (
            "state".to_string(),
            json!(resource_state_name(handle.state())),
        ),
        (
            "has_error".to_string(),
            json!(handle.last_error().is_some()),
        ),
        (
            "cycle_events_during_pause".to_string(),
            json!(cycle_events_during_pause),
        ),
    ]);
    handle.stop();
    handle
        .join()
        .map_err(|_| "between-cycle resource thread panicked".to_string())?;
    Ok(observed)
}

fn counter_runtime() -> Result<(trust_runtime::Runtime, trust_runtime::debug::DebugControl), String>
{
    let mut harness =
        TestHarness::from_source(COUNTER_SOURCE).map_err(|error| error.to_string())?;
    let debug = harness.runtime_mut().enable_debug();
    Ok((harness.into_runtime(), debug))
}

fn watchdog_policy(timeout_ms: i64) -> WatchdogPolicy {
    WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_millis(timeout_ms),
        action: WatchdogAction::Halt,
    }
}

fn wait_for_state(
    handle: &trust_runtime::scheduler::ResourceHandle<StdClock>,
    expected: ResourceState,
) -> Result<(), String> {
    let deadline = Instant::now() + WallDuration::from_secs(2);
    while Instant::now() < deadline {
        if handle.state() == expected {
            return Ok(());
        }
        thread::sleep(WallDuration::from_millis(2));
    }
    Err(format!(
        "resource did not reach {expected:?}; current={:?}, error={:?}",
        handle.state(),
        handle.last_error()
    ))
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

fn trace_millis(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i64, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .filter(|value| *value >= 0)
        .ok_or_else(|| format!("trace field {key} must be non-negative integer milliseconds"))
}

#[derive(Default)]
struct PauseProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for PauseProbe {
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
