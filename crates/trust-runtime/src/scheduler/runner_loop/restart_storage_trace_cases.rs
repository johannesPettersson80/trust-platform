use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::harness::TestHarness;
use crate::value::Value;
use crate::RestartMode;
use verification_cases::{
    check_case_expectation, check_trace_fields, run_case_file, CaseExecution, CaseRecord,
    CaseResult, ObservedOperation, RunConfig, StateProbe, StateSnapshot,
};

use super::*;

const TEST_ID: &str = "TEST_RUNTIME_RESTART_STORAGE_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_RESTART_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:8c3201f09c0797283e78f0c1c7e7bf7e132f4ccf7752f7168308358cef675e3b";
const AUTOMATIC_SCENARIO: &str = "AUTOMATIC_FAULT_RESTART_STORAGE";
/// Trace fields this runner compares. Every scenario observes its resource state.
const READ_FIELDS: &[&str] = &[
    "last_error",
    "non_retain",
    "ordinary",
    "resource_state",
    "retain",
];

const SOURCE: &str = r#"
VAR_GLOBAL RETAIN
    retained_value : DINT := DINT#1;
END_VAR
VAR_GLOBAL NON_RETAIN
    transient_value : DINT := DINT#2;
END_VAR
VAR_GLOBAL
    ordinary_value : DINT := DINT#3;
END_VAR
PROGRAM Main
END_PROGRAM
"#;

#[derive(Clone, Debug)]
struct TraceClock;

impl Clock for TraceClock {
    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn sleep_until(&self, _deadline: Duration) {}

    fn wake(&self) {}
}

#[test]
fn runtime_restart_storage_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = RestartStorageProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_restart_case)
        .expect("restart storage case artifact must be written");
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
        "restart storage failures: {}",
        failed.join("; ")
    );
}

fn run_restart_case(
    case: &CaseRecord,
    probe: &mut RestartStorageProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let automatic = scenario == AUTOMATIC_SCENARIO;
    let mut mismatches = Vec::new();
    for step in case.trace.as_deref().unwrap_or(&[]) {
        if let Err(error) = check_trace_fields(&step.expected, READ_FIELDS) {
            mismatches.push(format!("{} step {}: {error}", case.id, step.sequence));
        }
    }
    let mut harness = TestHarness::from_source(SOURCE)
        .map_err(|error| format!("{} failed to compile: {error}", case.id))?;
    harness.set_input("retained_value", Value::DInt(42));
    harness.set_input("transient_value", Value::DInt(70));
    harness.set_input("ordinary_value", Value::DInt(80));
    let before = observed_state(
        &harness.get_output("retained_value"),
        &harness.get_output("transient_value"),
        &harness.get_output("ordinary_value"),
        Vec::new(),
    );

    let (retained, transient, ordinary, state_before, state, last_error) = match scenario {
        "COLD_RESTART_STORAGE" => {
            let state_before = projected_resource_state(harness.runtime());
            harness
                .restart(RestartMode::Cold)
                .map_err(|error| format!("cold restart failed: {error}"))?;
            (
                harness.get_output("retained_value"),
                harness.get_output("transient_value"),
                harness.get_output("ordinary_value"),
                state_before,
                projected_resource_state(harness.runtime()),
                "none".to_string(),
            )
        }
        "WARM_RESTART_STORAGE" => {
            let state_before = projected_resource_state(harness.runtime());
            harness
                .restart(RestartMode::Warm)
                .map_err(|error| format!("warm restart failed: {error}"))?;
            (
                harness.get_output("retained_value"),
                harness.get_output("transient_value"),
                harness.get_output("ordinary_value"),
                state_before,
                projected_resource_state(harness.runtime()),
                "none".to_string(),
            )
        }
        "AUTOMATIC_FAULT_RESTART_STORAGE" => {
            let mut runner =
                ResourceRunner::new(harness.into_runtime(), TraceClock, Duration::from_millis(1));
            let mut limiter = AutomaticRestartLimiter::default();
            let resource_state = Arc::new(Mutex::new(ResourceState::Running));
            let resource_error = Arc::new(Mutex::new(None));
            let supervision = supervisor::ScanSupervision::new(
                Arc::clone(&resource_state),
                Arc::clone(&resource_error),
            );
            let state_before = resource_state_name(&resource_state);
            if !try_automatic_restart(
                &mut runner,
                &mut limiter,
                &RuntimeError::DivisionByZero,
                &resource_state,
                &resource_error,
                &supervision,
            ) {
                return Err("automatic fault restart was not accepted".to_string());
            }
            let state = resource_state_name(&resource_state);
            let last_error = if recover_mutex_lock(resource_error.lock()).is_none() {
                "none".to_string()
            } else {
                "present".to_string()
            };
            (
                runner
                    .runtime()
                    .storage()
                    .get_global("retained_value")
                    .cloned(),
                runner
                    .runtime()
                    .storage()
                    .get_global("transient_value")
                    .cloned(),
                runner
                    .runtime()
                    .storage()
                    .get_global("ordinary_value")
                    .cloned(),
                state_before,
                state,
                last_error,
            )
        }
        other => return Err(format!("unreviewed restart scenario {other}")),
    };

    let expected_retain = if scenario == "COLD_RESTART_STORAGE" {
        1
    } else {
        42
    };
    let values_match = retained == Some(Value::DInt(expected_retain))
        && transient == Some(Value::DInt(2))
        && ordinary == Some(Value::DInt(3))
        && last_error == "none"
        && (!automatic || state == "running");
    if !values_match {
        mismatches.push(format!(
            "{scenario} observed retain={retained:?}, non_retain={transient:?}, ordinary={ordinary:?}, state={state}, last_error={last_error}"
        ));
    }
    probe.target = Some(serde_json::json!({
        "last_error": last_error,
        "non_retain": format!("{transient:?}"),
        "ordinary": format!("{ordinary:?}"),
        "resource_state": state,
        "retain": format!("{retained:?}"),
    }));
    let after = observed_state(
        &retained,
        &transient,
        &ordinary,
        if last_error == "none" {
            Vec::new()
        } else {
            vec![last_error.clone()]
        },
    );
    // Each scenario has one step, and that step is the restart. Its declared
    // resource_state is "unchanged" (the state after equals the state before) or an
    // exact state name.
    let [step] = case.trace.as_deref().unwrap_or(&[]) else {
        return Err(format!("{} must contain exactly one trace step", case.id));
    };
    let declared_state = step
        .expected
        .get("resource_state")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            format!(
                "{} step {} must declare resource_state",
                case.id, step.sequence
            )
        })?;
    let state_holds = match declared_state {
        "unchanged" => state_before == state,
        exact => state == exact,
    };
    if !state_holds {
        mismatches.push(format!(
            "{scenario} resource_state expected {declared_state}, observed {state_before} before and {state} after"
        ));
    }
    let observed = ObservedOperation {
        outcome: "accept_value",
        fault_surface: None,
        status_before: Some(state_before.as_str()),
        status_after: Some(state.as_str()),
        before: &before,
        after: &after,
    };
    match check_case_expectation(case, &observed) {
        Ok(declared) => mismatches.extend(declared),
        Err(error) => mismatches.push(error),
    }
    let passed = mismatches.is_empty();
    Ok(CaseExecution {
        result: if passed {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!passed).then(|| mismatches.join("; ")),
        observed_status: Some(
            if passed {
                "restart_storage_match"
            } else {
                "restart_storage_mismatch"
            }
            .to_string(),
        ),
    })
}

/// The resource state of a runtime that no scheduler drives. The runner loop reports a
/// latched fault as `Faulted` and a resource that runs as `Running`, so a harness runtime
/// is observed through its fault latch with the same names.
fn projected_resource_state(runtime: &crate::Runtime) -> String {
    if runtime.faulted() {
        "faulted".to_string()
    } else {
        "running".to_string()
    }
}

/// The scheduler state of a resource, named the way the trace names it.
fn resource_state_name(state: &Mutex<ResourceState>) -> String {
    format!("{:?}", *recover_mutex_lock(state.lock())).to_lowercase()
}

/// Snapshot of the retained, non-retained and ordinary globals that this runner reads.
fn observed_state(
    retained: &Option<Value>,
    transient: &Option<Value>,
    ordinary: &Option<Value>,
    diagnostics: Vec<String>,
) -> StateSnapshot {
    let retained = format!("{retained:?}");
    StateSnapshot {
        process_image_hash: None,
        retain_hash: Some(retained.clone()),
        target: Some(serde_json::json!(retained)),
        siblings: BTreeMap::from([
            (
                "non_retain".to_string(),
                serde_json::json!(format!("{transient:?}")),
            ),
            (
                "ordinary".to_string(),
                serde_json::json!(format!("{ordinary:?}")),
            ),
        ]),
        diagnostics,
    }
}

#[derive(Default)]
struct RestartStorageProbe {
    target: Option<serde_json::Value>,
    next_snapshot_is_after: bool,
}

impl StateProbe for RestartStorageProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_after {
            self.target = None;
        }
        self.next_snapshot_is_after = !self.next_snapshot_is_after;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.target.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
