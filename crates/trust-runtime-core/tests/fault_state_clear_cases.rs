use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::watchdog::{FaultPolicy, FaultSubsystem};
use verification_cases::{
    check_case_expectation, check_trace_fields, run_case_file, CaseExecution, CaseRecord,
    CaseResult, ObservedOperation, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_FAULT_STATE_CLEAR_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_FAULT_STATE_CLEAR_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:c460f6eebcb67abcbc6e6ac481c7d07c468339391b81d302f75087f5ccb57f8c";

#[test]
fn fault_state_clear_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime-core must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = FaultStateClearProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_fault_clear_case)
        .expect("fault-state clear artifact must be written");
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
        "fault-state clear failures: {}",
        failed.join("; ")
    );
}

fn run_fault_clear_case(
    case: &CaseRecord,
    probe: &mut FaultStateClearProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let (observed, expected, operations) = execute_scenario(scenario)?;
    probe.trace = observed.iter().map(Observation::render).collect();
    let mut failures = (observed != expected)
        .then(|| format!("expected {expected:?}, observed {observed:?}"))
        .into_iter()
        .collect::<Vec<_>>();
    // Each clear is one operation under test. A clear writes the fault record and reports no
    // error; the policy is not part of the clear.
    for (before, after) in operations {
        let before_state = fault_record_state(before);
        let after_state = fault_record_state(after);
        let operation = ObservedOperation {
            outcome: "accept_value",
            fault_surface: None,
            status_before: None,
            status_after: None,
            before: &before_state,
            after: &after_state,
        };
        match check_case_expectation(case, &operation) {
            Ok(mismatches) => failures.extend(mismatches),
            Err(error) => failures.push(error),
        }
    }
    for trace_step in case.trace.as_deref().unwrap_or(&[]) {
        if let Err(error) = check_trace_fields(&trace_step.expected, &[]) {
            failures.push(error);
        }
    }
    Ok(CaseExecution {
        result: if failures.is_empty() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!failures.is_empty()).then(|| failures.join("; ")),
        observed_status: Some(if failures.is_empty() {
            format!("{scenario}:contract_matched")
        } else {
            format!("{scenario}:contract_mismatch")
        }),
    })
}

type ClearOperation = (Observation, Observation);

/// Observations before and after each step, and the clear operations performed.
type ScenarioRun = (Vec<Observation>, Vec<Observation>, Vec<ClearOperation>);

fn execute_scenario(scenario: &str) -> Result<ScenarioRun, String> {
    let result = match scenario {
        "RECORDED_FAULT_CLEAR" => {
            let mut faults = FaultSubsystem::new();
            faults.set_policy(FaultPolicy::SafeHalt);
            faults.record(RuntimeError::WatchdogTimeout);
            let before = observe(&faults);
            faults.clear();
            let after = observe(&faults);
            (
                vec![before, after],
                vec![
                    Observation::new(true, Some("watchdog_timeout"), "safe_halt"),
                    Observation::new(false, None, "safe_halt"),
                ],
                vec![(before, after)],
            )
        }
        "HEALTHY_CLEAR_IDEMPOTENT" => {
            let mut faults = FaultSubsystem::new();
            faults.set_policy(FaultPolicy::Restart);
            let initial = observe(&faults);
            faults.clear();
            let first = observe(&faults);
            faults.clear();
            let second = observe(&faults);
            let expected = Observation::new(false, None, "restart");
            (
                vec![first, second],
                vec![expected, expected],
                vec![(initial, first), (first, second)],
            )
        }
        other => return Err(format!("unreviewed fault-state clear scenario {other}")),
    };
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Observation {
    faulted: bool,
    last_fault: Option<&'static str>,
    policy: &'static str,
}

impl Observation {
    const fn new(faulted: bool, last_fault: Option<&'static str>, policy: &'static str) -> Self {
        Self {
            faulted,
            last_fault,
            policy,
        }
    }

    fn render(&self) -> String {
        format!(
            "faulted={},last_fault={:?},policy={}",
            self.faulted, self.last_fault, self.policy
        )
    }
}

/// The fault record and the policy at one side of a clear. The record is the target, and a
/// clear must leave the policy (a sibling) unchanged.
fn fault_record_state(observation: Observation) -> StateSnapshot {
    StateSnapshot {
        process_image_hash: None,
        retain_hash: None,
        target: Some(
            format!(
                "faulted={},last_fault={:?}",
                observation.faulted, observation.last_fault
            )
            .into(),
        ),
        siblings: [("policy".to_string(), observation.policy.into())]
            .into_iter()
            .collect(),
        diagnostics: Vec::new(),
    }
}

fn observe(faults: &FaultSubsystem) -> Observation {
    let last_fault = faults.last_fault().map(|error| match error {
        RuntimeError::WatchdogTimeout => "watchdog_timeout",
        _ => "other",
    });
    let policy = match faults.policy() {
        FaultPolicy::Halt => "halt",
        FaultPolicy::SafeHalt => "safe_halt",
        FaultPolicy::Restart => "restart",
    };
    Observation::new(faults.is_faulted(), last_fault, policy)
}

#[derive(Default)]
struct FaultStateClearProbe {
    trace: Vec<String>,
}

impl StateProbe for FaultStateClearProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: Some(self.trace.join("|")),
            target: None,
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
