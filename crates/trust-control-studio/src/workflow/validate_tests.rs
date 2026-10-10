//! Tests of the isolated test runs of Validate (`workflow::validate`): a fake runtime answers each request with the fields the
//! protocol annex names (E8.4), so the run's journal, its polling, its deadline, its cancellation and the order of a scenario's
//! two runs are read as the engine makes them. The timings are passed in, so no test waits for the engine's production margins.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use trust_control::validation::engineer_verification_plan::VerificationPlan;
use trust_control::validation::isolated_run_protocol::{
    IsolatedExecution, IsolatedJobState, IsolatedRunObservation, IsolatedRunRequest,
};

use super::*;
use crate::operation::cancelled;
use crate::runtime_client::fake_runtime::{answer, FakeRuntime};

const PROJECT: &str = "project:fixture";
const LOOP: &str = "loop:tank";
const RUNTIME: &str = "runtime:local";
const REVISION: &str = "sha256:project-1";
const JOB: &str = "isolated-test-job-1";

/// Short waits, so that a test of a deadline or an unconfirmed cancel ends in milliseconds.
const QUICK: JobTiming = JobTiming {
    poll: Duration::from_millis(1),
    margin: Duration::ZERO,
    confirmation: Duration::from_millis(200),
};

/// A fresh folder for the journals, named for the test.
fn storage(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "trust-control-validate-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).expect("a storage folder");
    folder
}

fn open_journal(folder: &Path) -> Journal {
    Journal::open(folder, PROJECT, LOOP).expect("the journal")
}

fn identity() -> RunIdentity {
    RunIdentity {
        project_id: PROJECT.to_string(),
        runtime_id: RUNTIME.to_string(),
        loop_id: LOOP.to_string(),
        project_revision: REVISION.to_string(),
    }
}

/// A valid test run of one channel on a ten-scan grid, with the given identifier and wall budget.
fn request_with(job: &str, wall_budget_ms: u64) -> IsolatedRunRequest {
    IsolatedRunRequest {
        request_id: job.to_string(),
        expected_project_revision: REVISION.to_string(),
        execution: IsolatedExecution::IsolatedSimulation,
        step_ms: 10,
        duration_ms: 100,
        seed: 1,
        max_total_cycles: 100,
        wall_budget_ms,
        record_every: 1,
        initial_values: BTreeMap::new(),
        events: Vec::new(),
        observations: vec![IsolatedRunObservation {
            symbol: "LevelPV".to_string(),
            unit: "%".to_string(),
        }],
        parameter_overrides: Vec::new(),
        simulation_override: None,
        feedback_relay: None,
    }
}

fn request(wall_budget_ms: u64) -> IsolatedRunRequest {
    request_with(JOB, wall_budget_ms)
}

/// Runs one job of the test's identity with the given cancellation, wall budget and timing.
fn run(
    client: &mut RuntimeClient,
    journal: &mut Journal,
    is_cancelled: &dyn Fn() -> bool,
    wall_budget_ms: u64,
    timing: JobTiming,
) -> Result<IsolatedRunStatus, Failure> {
    let progress = |_: &str| {};
    let mut session = RunSession {
        client,
        journal,
        identity: identity(),
        is_cancelled,
        progress: &progress,
        timing,
    };
    run_job(&mut session, &request(wall_budget_ms))
}

/// The job a request names: the start names it as its `request_id`, every other request as its `job_id` (E8.4).
fn job_of(request: &Value) -> String {
    let params = &request["params"];
    params["request_id"]
        .as_str()
        .or_else(|| params["job_id"].as_str())
        .unwrap_or(JOB)
        .to_string()
}

/// The answer to a start of the job: queued on the revision it was reviewed on (E8.4).
fn started(request: &Value) -> Value {
    answer(
        request,
        json!({ "job_id": job_of(request), "status": "queued", "project_revision": REVISION }),
    )
}

/// The status answer of the job in `state` (E8.4): no run, and the provenance of an isolated simulation.
fn status(request: &Value, state: &str, cancel_requested: bool) -> Value {
    answer(
        request,
        json!({
            "job_id": job_of(request),
            "project_revision": REVISION,
            "status": state,
            "cancel_requested": cancel_requested,
            "source_basis": "loaded-runtime-sources",
            "io_provenance": "isolated-simulation-no-physical-drivers",
            "captured_sample_count": 0,
        }),
    )
}

/// Whether a request asks for the job's results, as the engine asks once the job has ended.
fn asks_for_results(request: &Value) -> bool {
    request["params"]["include_results"] == json!(true)
}

/// A campaign case with no requirements: the identity of a scenario is all these tests read.
fn case(id: &str) -> CampaignCase {
    CampaignCase {
        id: id.to_string(),
        plan: VerificationPlan {
            step_ms: 10,
            duration_ms: 100,
            max_total_cycles: 100,
            record_every: 1,
            initial_values: BTreeMap::new(),
            observed_symbols: vec!["LevelPV".to_string()],
            requirements: Vec::new(),
        },
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
        requirements: Vec::new(),
    }
}

#[test]
fn a_run_the_runtime_ends_is_read_with_its_results_and_leaves_the_journal() {
    let folder = storage("ends");
    let reads = Arc::new(AtomicUsize::new(0));
    let fake = FakeRuntime::start({
        let reads = Arc::clone(&reads);
        move |request| match request["type"].as_str()? {
            "control.simulation.run.start" => Some(started(request)),
            "control.simulation.run.status" => {
                let read = reads.fetch_add(1, Ordering::SeqCst);
                let state = if read < 2 && !asks_for_results(request) {
                    "running"
                } else {
                    "failed"
                };
                Some(status(request, state, false))
            }
            _ => None,
        }
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    let ended = run(&mut client, &mut journal, &|| false, 10_000, QUICK).expect("the run ends");
    assert_eq!(ended.status, IsolatedJobState::Failed);
    assert!(
        open_journal(&folder).entries().is_empty(),
        "the ended run leaves the journal once its status is read"
    );
    assert_eq!(fake.requests_of("control.simulation.run.start").len(), 1);
    assert!(
        fake.requests_of("control.simulation.run.status")
            .iter()
            .any(asks_for_results),
        "the ended run is read with its results (E8.4)"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn the_identity_is_journaled_before_the_start_is_dispatched() {
    let folder = storage("journaled-first");
    let journaled_at_start = Arc::new(AtomicBool::new(false));
    let fake = FakeRuntime::start({
        let folder = folder.clone();
        let journaled_at_start = Arc::clone(&journaled_at_start);
        move |request| match request["type"].as_str()? {
            "control.simulation.run.start" => {
                // The runtime reads the journal as the start arrives: the identity must already be there (E11.2).
                let held = open_journal(&folder)
                    .entries()
                    .iter()
                    .any(|entry| entry.operation_id() == JOB);
                journaled_at_start.store(held, Ordering::SeqCst);
                Some(started(request))
            }
            "control.simulation.run.status" => Some(status(request, "failed", false)),
            _ => None,
        }
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    run(&mut client, &mut journal, &|| false, 10_000, QUICK).expect("the run ends");
    assert!(
        journaled_at_start.load(Ordering::SeqCst),
        "the identity is journaled before the start is sent (E11.2, A8.5)"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_start_without_an_answer_is_read_by_its_identity_and_never_started_again() {
    let folder = storage("lost-start");
    let fake = FakeRuntime::start(|request| match request["type"].as_str()? {
        // An answer that is not a control response: the start's outcome is unknown (E11.4).
        "control.simulation.run.start" => Some(json!(["no", "answer"])),
        "control.simulation.run.status" => Some(status(request, "failed", false)),
        _ => None,
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    let ended = run(&mut client, &mut journal, &|| false, 10_000, QUICK)
        .expect("the job the runtime holds is read by its identity");
    assert_eq!(ended.status, IsolatedJobState::Failed);
    assert_eq!(
        fake.requests_of("control.simulation.run.start").len(),
        1,
        "a lost acknowledgement is never answered with a second start (E11.4)"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_run_the_runtime_stops_answering_for_is_cancelled_and_its_end_confirmed() {
    let folder = storage("timed-out");
    let fake = FakeRuntime::start(|request| match request["type"].as_str()? {
        "control.simulation.run.start" => Some(started(request)),
        "control.simulation.run.status" => Some(status(request, "running", false)),
        "control.simulation.run.cancel" => Some(status(request, "cancelled", true)),
        _ => None,
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    // The wall budget of one millisecond passes at once, and the runtime never reports the run's end.
    let failure =
        run(&mut client, &mut journal, &|| false, 1, QUICK).expect_err("the run is stopped");
    assert_eq!(
        failure.message,
        "Verification monitoring timed out; cancellation requested."
    );
    assert_eq!(fake.requests_of("control.simulation.run.cancel").len(), 1);
    assert!(
        open_journal(&folder).entries().is_empty(),
        "a confirmed end leaves the journal"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_cancelled_operation_cancels_the_run_and_ends_with_the_cancellation() {
    let folder = storage("cancelled");
    let cancel = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let fake = FakeRuntime::start({
        let cancel = Arc::clone(&cancel);
        let reads = Arc::clone(&reads);
        move |request| match request["type"].as_str()? {
            "control.simulation.run.start" => Some(started(request)),
            "control.simulation.run.status" => {
                // The engineer cancels while the run runs: the second read is the moment.
                if reads.fetch_add(1, Ordering::SeqCst) == 1 {
                    cancel.store(true, Ordering::SeqCst);
                }
                Some(status(request, "running", false))
            }
            "control.simulation.run.cancel" => Some(status(request, "cancelled", true)),
            _ => None,
        }
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    let is_cancelled = || cancel.load(Ordering::SeqCst);
    let failure = run(&mut client, &mut journal, &is_cancelled, 10_000, QUICK)
        .expect_err("the run is cancelled");
    assert_eq!(
        failure,
        cancelled(),
        "the operation reports its cancellation"
    );
    assert_eq!(fake.requests_of("control.simulation.run.cancel").len(), 1);
    assert_eq!(
        open_journal(&folder).entries(),
        [] as [crate::journal::JournalEntry; 0]
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_cancellation_the_runtime_does_not_confirm_keeps_the_identity_journaled() {
    let folder = storage("unconfirmed");
    let cancel = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let fake = FakeRuntime::start({
        let cancel = Arc::clone(&cancel);
        let reads = Arc::clone(&reads);
        move |request| match request["type"].as_str()? {
            "control.simulation.run.start" => Some(started(request)),
            "control.simulation.run.status" => {
                if reads.fetch_add(1, Ordering::SeqCst) == 1 {
                    cancel.store(true, Ordering::SeqCst);
                }
                Some(status(request, "running", false))
            }
            // The runtime accepts the cancel and never reports the end.
            "control.simulation.run.cancel" => Some(status(request, "running", true)),
            _ => None,
        }
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    let is_cancelled = || cancel.load(Ordering::SeqCst);
    let failure = run(&mut client, &mut journal, &is_cancelled, 10_000, QUICK)
        .expect_err("the end is not confirmed");
    assert!(
        failure.message.starts_with("The runtime did not confirm"),
        "{failure:?}"
    );
    let held = open_journal(&folder);
    assert_eq!(
        held.entries()
            .iter()
            .map(JournalEntry::operation_id)
            .collect::<Vec<_>>(),
        vec![JOB],
        "the identity stays journaled until the runtime reports the end (E11.4, E11.5)"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_run_cancelled_before_its_start_is_never_dispatched_and_nothing_is_journaled() {
    let folder = storage("cancelled-first");
    let fake = FakeRuntime::start(|request| Some(started(request)));
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    let failure = run(&mut client, &mut journal, &|| true, 10_000, QUICK)
        .expect_err("the run is cancelled before it starts");
    assert_eq!(failure, cancelled());
    assert!(fake.requests().is_empty(), "nothing is sent to the runtime");
    assert_eq!(
        open_journal(&folder).entries(),
        [] as [crate::journal::JournalEntry; 0]
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_scenario_runs_its_current_settings_then_its_candidate_for_each_case_in_campaign_order() {
    let folder = storage("order");
    let fake = FakeRuntime::start(|request| match request["type"].as_str()? {
        "control.simulation.run.start" => Some(started(request)),
        "control.simulation.run.status" => Some(status(request, "failed", false)),
        _ => None,
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    let progress = |_: &str| {};
    let never = || false;
    let mut session = RunSession {
        client: &mut client,
        journal: &mut journal,
        identity: identity(),
        is_cancelled: &never,
        progress: &progress,
        timing: QUICK,
    };
    let order = Mutex::new(Vec::new());
    let build = |case: &CampaignCase, settings: Settings, job: &str| {
        order
            .lock()
            .expect("the order")
            .push((case.id.clone(), settings));
        Ok::<_, Failure>(request_with(job, 10_000))
    };
    let current = vec![case("target-response"), case("load-rejection")];
    let candidate = vec![case("target-response"), case("load-rejection")];
    let runs = run_scenarios(&mut session, &current, &candidate, &build)
        .expect("every scenario runs to its end");
    assert_eq!(
        runs.iter().map(|run| run.id.as_str()).collect::<Vec<_>>(),
        vec!["target-response", "load-rejection"]
    );
    assert_eq!(
        *order.lock().expect("the order"),
        vec![
            ("target-response".to_string(), Settings::Current),
            ("target-response".to_string(), Settings::Candidate),
            ("load-rejection".to_string(), Settings::Current),
            ("load-rejection".to_string(), Settings::Candidate),
        ],
        "each scenario's current run comes before its candidate run"
    );
    assert_eq!(fake.requests_of("control.simulation.run.start").len(), 4);
    assert_eq!(
        open_journal(&folder).entries(),
        [] as [crate::journal::JournalEntry; 0]
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn campaigns_that_name_other_scenarios_run_nothing() {
    let folder = storage("differ");
    let fake = FakeRuntime::start(|request| Some(started(request)));
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("the fake runtime");
    let mut journal = open_journal(&folder);
    let progress = |_: &str| {};
    let never = || false;
    let mut session = RunSession {
        client: &mut client,
        journal: &mut journal,
        identity: identity(),
        is_cancelled: &never,
        progress: &progress,
        timing: QUICK,
    };
    let build =
        |_: &CampaignCase, _: Settings, job: &str| Ok::<_, Failure>(request_with(job, 10_000));
    let failure = run_scenarios(
        &mut session,
        &[case("target-response")],
        &[case("load-rejection")],
        &build,
    )
    .expect_err("the campaigns differ");
    assert_eq!(failure.cause, Cause::Implementation);
    assert!(fake.requests().is_empty(), "no run is started");
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn an_isolated_job_identifier_is_fresh_and_a_runtime_identifier() {
    let first = new_job_id();
    let second = new_job_id();
    assert_ne!(first, second, "each job has its own identifier");
    assert!(
        first.len() <= 128
            && first
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
        "the runtime accepts {first} as a request identifier (E8.4)"
    );
}
