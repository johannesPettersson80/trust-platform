use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::task::{evaluate_task_readiness, TaskReadiness, TaskState};
use trust_runtime_core::value::Duration;
use verification_cases::{
    check_case_expectation, check_trace_fields, run_case_file, CaseExecution, CaseRecord,
    CaseResult, ObservedOperation, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_TASK_READINESS_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_TASK_READINESS_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:d1332083c4028808c0b3a798a46db0d0f6f9e08f632ad166326189de19b86fac";

#[test]
fn task_readiness_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime-core must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = TaskReadinessProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_task_case)
        .expect("task readiness artifact must be written");
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
        "task readiness failures: {}",
        failed.join("; ")
    );
}

fn run_task_case(
    case: &CaseRecord,
    probe: &mut TaskReadinessProbe,
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
    // Each evaluation is one operation under test. It reports a readiness value and never
    // rejects, so its outcome is accepted. It has no sibling tasks and writes no retained or
    // process-image state.
    for (before, after) in operations {
        let operation = ObservedOperation {
            outcome: "accept_value",
            fault_surface: None,
            status_before: None,
            status_after: None,
            before: &before,
            after: &after,
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

type StepOperation = (StateSnapshot, StateSnapshot);

/// Observations before and after each step, and the step operations performed.
type ScenarioRun = (Vec<Observation>, Vec<Observation>, Vec<StepOperation>);

fn execute_scenario(scenario: &str) -> Result<ScenarioRun, String> {
    let mut operations = Vec::new();
    let (observed, expected) = match scenario {
        "NON_POSITIVE_INTERVAL_SINGLE_LOW" => {
            let mut state = TaskState {
                last_single: false,
                last_run: Duration::from_millis(5),
                overrun_count: 7,
            };
            let zero = step(
                &mut state,
                Duration::ZERO,
                false,
                Duration::from_millis(100),
                &mut operations,
            );
            let negative = step(
                &mut state,
                Duration::from_nanos(-1),
                false,
                Duration::from_millis(100),
                &mut operations,
            );
            let expected = Observation::new(None, 0, false, 5, 7);
            (vec![zero, negative], vec![expected, expected])
        }
        "SINGLE_RISING_EDGE" => {
            let mut state = TaskState {
                last_single: false,
                last_run: Duration::from_millis(5),
                overrun_count: 3,
            };
            let observed = step(
                &mut state,
                Duration::from_millis(10),
                true,
                Duration::from_millis(20),
                &mut operations,
            );
            (
                vec![observed],
                vec![Observation::new(Some(20), 0, true, 5, 3)],
            )
        }
        "SINGLE_HELD_HIGH" => {
            let mut state = TaskState {
                last_single: true,
                last_run: Duration::ZERO,
                overrun_count: 2,
            };
            let observed = step(
                &mut state,
                Duration::from_millis(10),
                true,
                Duration::from_millis(30),
                &mut operations,
            );
            (vec![observed], vec![Observation::new(None, 0, true, 0, 2)])
        }
        "SINGLE_FALLING_REARM" => {
            let mut state = TaskState {
                last_single: true,
                last_run: Duration::from_millis(3),
                overrun_count: 1,
            };
            let falling = step(
                &mut state,
                Duration::ZERO,
                false,
                Duration::from_millis(7),
                &mut operations,
            );
            let rising = step(
                &mut state,
                Duration::ZERO,
                true,
                Duration::from_millis(8),
                &mut operations,
            );
            (
                vec![falling, rising],
                vec![
                    Observation::new(None, 0, false, 3, 1),
                    Observation::new(Some(8), 0, true, 3, 1),
                ],
            )
        }
        "BACKWARD_OR_EARLY_PERIODIC_SAMPLE" => {
            let mut state = TaskState {
                last_single: false,
                last_run: Duration::from_millis(100),
                overrun_count: 4,
            };
            let backward = step(
                &mut state,
                Duration::from_millis(10),
                false,
                Duration::from_millis(20),
                &mut operations,
            );
            let early = step(
                &mut state,
                Duration::from_millis(10),
                false,
                Duration::from_millis(109),
                &mut operations,
            );
            let expected = Observation::new(None, 0, false, 100, 4);
            (vec![backward, early], vec![expected, expected])
        }
        "ONE_PERIOD_DUE" => {
            let mut state = TaskState {
                last_single: false,
                last_run: Duration::from_millis(100),
                overrun_count: 4,
            };
            let observed = step(
                &mut state,
                Duration::from_millis(10),
                false,
                Duration::from_millis(110),
                &mut operations,
            );
            (
                vec![observed],
                vec![Observation::new(Some(110), 0, false, 110, 4)],
            )
        }
        "MULTIPLE_PERIODS_DUE" => {
            let mut state = TaskState {
                last_single: false,
                last_run: Duration::ZERO,
                overrun_count: u64::MAX - 1,
            };
            let observed = step(
                &mut state,
                Duration::from_millis(10),
                false,
                Duration::from_millis(35),
                &mut operations,
            );
            (
                vec![observed],
                vec![Observation::new(Some(10), 2, false, 30, u64::MAX)],
            )
        }
        other => return Err(format!("unreviewed task readiness scenario {other}")),
    };
    Ok((observed, expected, operations))
}

/// Evaluates one readiness step and records the task state on both sides of it.
fn step(
    state: &mut TaskState,
    interval: Duration,
    single_now: bool,
    now: Duration,
    operations: &mut Vec<StepOperation>,
) -> Observation {
    let before = task_state(state);
    let readiness = evaluate_task_readiness(state, interval, single_now, now);
    let after = task_state(state);
    operations.push((before, after));
    observe(readiness, state)
}

/// The task state on one side of a step. The state is the target: a step that is not due
/// leaves it unchanged, and a due step may change it.
fn task_state(state: &TaskState) -> StateSnapshot {
    StateSnapshot {
        process_image_hash: None,
        retain_hash: None,
        target: Some(
            format!(
                "single={},last_run_ms={},overrun={}",
                state.last_single,
                state.last_run.as_millis(),
                state.overrun_count
            )
            .into(),
        ),
        siblings: BTreeMap::new(),
        diagnostics: Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Observation {
    due_at_millis: Option<i64>,
    missed_intervals: u64,
    last_single: bool,
    last_run_millis: i64,
    overrun_count: u64,
}

impl Observation {
    const fn new(
        due_at_millis: Option<i64>,
        missed_intervals: u64,
        last_single: bool,
        last_run_millis: i64,
        overrun_count: u64,
    ) -> Self {
        Self {
            due_at_millis,
            missed_intervals,
            last_single,
            last_run_millis,
            overrun_count,
        }
    }

    fn render(&self) -> String {
        format!(
            "due={:?},missed={},single={},last_run={},overrun={}",
            self.due_at_millis,
            self.missed_intervals,
            self.last_single,
            self.last_run_millis,
            self.overrun_count
        )
    }
}

fn observe(readiness: TaskReadiness, state: &TaskState) -> Observation {
    Observation::new(
        readiness.due_at.map(Duration::as_millis),
        readiness.missed_intervals,
        state.last_single,
        state.last_run.as_millis(),
        state.overrun_count,
    )
}

#[derive(Default)]
struct TaskReadinessProbe {
    trace: Vec<String>,
}

impl StateProbe for TaskReadinessProbe {
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
