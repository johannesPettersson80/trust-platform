use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::watchdog::{FaultAction, FaultPolicy, FaultSubsystem};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_FAULT_POLICY_DECISION_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_FAULT_POLICY_DECISION_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:def9623c89e584a0f72286e070d1ca7590e7cf60815bacf12cf9ee70bef684f0";

#[test]
fn fault_policy_decision_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime-core must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = FaultPolicyDecisionProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_fault_policy_case)
        .expect("fault-policy decision artifact must be written");
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
        "fault-policy decision failures: {}",
        failed.join("; ")
    );
}

fn run_fault_policy_case(
    case: &CaseRecord,
    probe: &mut FaultPolicyDecisionProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let (observed, expected) = execute_scenario(scenario)?;
    probe.trace = vec![observed.render()];
    let failures = (observed != expected)
        .then(|| format!("expected {expected:?}, observed {observed:?}"))
        .into_iter()
        .collect::<Vec<_>>();
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

fn execute_scenario(scenario: &str) -> Result<(Observation, Observation), String> {
    let (policy, expected_action, expected_safe_state) = match scenario {
        "HALT_POLICY" => (FaultPolicy::Halt, "halt", false),
        "SAFE_HALT_POLICY" => (FaultPolicy::SafeHalt, "safe_halt", true),
        "RESTART_POLICY" => (FaultPolicy::Restart, "restart", false),
        other => return Err(format!("unreviewed fault-policy scenario {other}")),
    };
    let mut faults = FaultSubsystem::new();
    faults.set_policy(policy);
    let healthy_before = HolderObservation::capture(&faults);
    let healthy_decision = faults.decision();
    let healthy_after = HolderObservation::capture(&faults);
    faults.record(RuntimeError::WatchdogTimeout);
    let faulted_before = HolderObservation::capture(&faults);
    let faulted_decision = faults.decision();
    let faulted_after = HolderObservation::capture(&faults);
    let observed = Observation {
        healthy_action: action_name(healthy_decision.action),
        healthy_apply_safe_state: healthy_decision.apply_safe_state,
        healthy_before,
        healthy_after,
        faulted_action: action_name(faulted_decision.action),
        faulted_apply_safe_state: faulted_decision.apply_safe_state,
        faulted_before,
        faulted_after,
    };
    let expected_healthy = HolderObservation {
        policy: policy_name(policy),
        faulted: false,
        last_fault: None,
    };
    let expected_faulted = HolderObservation {
        policy: policy_name(policy),
        faulted: true,
        last_fault: Some("watchdog_timeout"),
    };
    let expected = Observation {
        healthy_action: expected_action,
        healthy_apply_safe_state: expected_safe_state,
        healthy_before: expected_healthy,
        healthy_after: expected_healthy,
        faulted_action: expected_action,
        faulted_apply_safe_state: expected_safe_state,
        faulted_before: expected_faulted,
        faulted_after: expected_faulted,
    };
    Ok((observed, expected))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Observation {
    healthy_action: &'static str,
    healthy_apply_safe_state: bool,
    healthy_before: HolderObservation,
    healthy_after: HolderObservation,
    faulted_action: &'static str,
    faulted_apply_safe_state: bool,
    faulted_before: HolderObservation,
    faulted_after: HolderObservation,
}

impl Observation {
    fn render(&self) -> String {
        format!(
            "healthy_action={},healthy_apply_safe_state={},healthy_before={:?},healthy_after={:?},faulted_action={},faulted_apply_safe_state={},faulted_before={:?},faulted_after={:?}",
            self.healthy_action,
            self.healthy_apply_safe_state,
            self.healthy_before,
            self.healthy_after,
            self.faulted_action,
            self.faulted_apply_safe_state,
            self.faulted_before,
            self.faulted_after
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HolderObservation {
    policy: &'static str,
    faulted: bool,
    last_fault: Option<&'static str>,
}

impl HolderObservation {
    fn capture(faults: &FaultSubsystem) -> Self {
        Self {
            policy: policy_name(faults.policy()),
            faulted: faults.is_faulted(),
            last_fault: faults.last_fault().map(|error| match error {
                RuntimeError::WatchdogTimeout => "watchdog_timeout",
                _ => "other",
            }),
        }
    }
}

const fn policy_name(policy: FaultPolicy) -> &'static str {
    match policy {
        FaultPolicy::Halt => "halt",
        FaultPolicy::SafeHalt => "safe_halt",
        FaultPolicy::Restart => "restart",
    }
}

const fn action_name(action: FaultAction) -> &'static str {
    match action {
        FaultAction::Halt => "halt",
        FaultAction::SafeHalt => "safe_halt",
        FaultAction::Restart => "restart",
    }
}

#[derive(Default)]
struct FaultPolicyDecisionProbe {
    trace: Vec<String>,
}

impl StateProbe for FaultPolicyDecisionProbe {
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
