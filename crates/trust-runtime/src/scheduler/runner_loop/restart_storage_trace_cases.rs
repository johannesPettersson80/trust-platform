use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::harness::TestHarness;
use crate::value::Value;
use crate::RestartMode;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

use super::*;

const TEST_ID: &str = "TEST_RUNTIME_RESTART_STORAGE_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_RESTART_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:1696fc81dafdb5ba446beaf83545a1f9521511ebd645ca3753208957aefa9a34";

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
    assert!(failed.is_empty(), "restart storage failures: {}", failed.join("; "));
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
    let mut harness = TestHarness::from_source(SOURCE)
        .map_err(|error| format!("{} failed to compile: {error}", case.id))?;
    harness.set_input("retained_value", Value::DInt(42));
    harness.set_input("transient_value", Value::DInt(70));
    harness.set_input("ordinary_value", Value::DInt(80));

    let (retained, transient, ordinary, state, last_error) = match scenario {
        "COLD_RESTART_STORAGE" => {
            harness
                .restart(RestartMode::Cold)
                .map_err(|error| format!("cold restart failed: {error}"))?;
            (
                harness.get_output("retained_value"),
                harness.get_output("transient_value"),
                harness.get_output("ordinary_value"),
                "unchanged".to_string(),
                "none".to_string(),
            )
        }
        "WARM_RESTART_STORAGE" => {
            harness
                .restart(RestartMode::Warm)
                .map_err(|error| format!("warm restart failed: {error}"))?;
            (
                harness.get_output("retained_value"),
                harness.get_output("transient_value"),
                harness.get_output("ordinary_value"),
                "unchanged".to_string(),
                "none".to_string(),
            )
        }
        "AUTOMATIC_FAULT_RESTART_STORAGE" => {
            let mut runner = ResourceRunner::new(
                harness.into_runtime(),
                TraceClock,
                Duration::from_millis(1),
            );
            let mut limiter = AutomaticRestartLimiter::default();
            let resource_state = Arc::new(Mutex::new(ResourceState::Running));
            let resource_error = Arc::new(Mutex::new(None));
            if !try_automatic_restart(
                &mut runner,
                &mut limiter,
                &RuntimeError::DivisionByZero,
                &resource_state,
                &resource_error,
            ) {
                return Err("automatic fault restart was not accepted".to_string());
            }
            let state = format!("{:?}", *recover_mutex_lock(resource_state.lock()));
            let last_error = if recover_mutex_lock(resource_error.lock()).is_none() {
                "none".to_string()
            } else {
                "present".to_string()
            };
            (
                runner.runtime().storage().get_global("retained_value").cloned(),
                runner.runtime().storage().get_global("transient_value").cloned(),
                runner.runtime().storage().get_global("ordinary_value").cloned(),
                state.to_lowercase(),
                last_error,
            )
        }
        other => return Err(format!("unreviewed restart scenario {other}")),
    };

    let expected_retain = if scenario == "COLD_RESTART_STORAGE" { 1 } else { 42 };
    let passed = retained == Some(Value::DInt(expected_retain))
        && transient == Some(Value::DInt(2))
        && ordinary == Some(Value::DInt(3))
        && last_error == "none"
        && (scenario != "AUTOMATIC_FAULT_RESTART_STORAGE" || state == "running");
    probe.target = Some(serde_json::json!({
        "last_error": last_error,
        "non_retain": format!("{transient:?}"),
        "ordinary": format!("{ordinary:?}"),
        "resource_state": state,
        "retain": format!("{retained:?}"),
    }));
    Ok(CaseExecution {
        result: if passed {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!passed).then(|| {
            format!(
                "{scenario} observed retain={retained:?}, non_retain={transient:?}, ordinary={ordinary:?}, state={state}, last_error={last_error}"
            )
        }),
        observed_status: Some(if passed {
            "restart_storage_match"
        } else {
            "restart_storage_mismatch"
        }
        .to_string()),
    })
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
