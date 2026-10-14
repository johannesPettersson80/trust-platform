//! Focused tests of the connected run and of the Cancel, against a fake runtime whose answers carry exactly the fields annex
//! E names and the identifiers the code under test sent.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::experiment::{
    ControlEvidenceSource, ControlExperimentPlan, ControlExperimentRecovery, ControllerMode,
    ExperimentSample, PlanSignal, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::baseline::BaselineSummary;
use trust_control::identification::campaign::journal::{
    CampaignPhase, IdentificationCampaignJournal,
};
use trust_control::identification::campaign::{FollowSample, CANCELLED_TEXT};
use trust_control::identification::connected_record::{BoundsCheck, ValidationRun};
use trust_control::identification::connected_run::{StartRead, PHYSICAL_TARGET_TEXT};
use trust_control::identification::independent_validation::{RequirementStatus, RunEvaluation};
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::evidence::{recording_evidence, RecordingEvidence};
use trust_control::recording::{
    Acquisition, Completeness, EvidenceSource, RecordedRun, StopReason,
};
use trust_control::signals::{Quality, SignalBinding};

use super::cancel::cancel_journaled;
use super::run::{run, RunPorts};
use super::{
    IdentifyOutcome, IdentifyRecording, IdentifyRequest, IdentifyRuntime, LoopSampler,
    RecordingRetention, RunPurpose,
};
use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::recording_coordinator::RecordingSession;
use crate::runtime_client::runtime_recording::{
    RecordingOwnerPersistence, RecordingStartError, RecordingStartPlan,
};
use crate::workflow::identify_campaign::{
    CampaignClock, CampaignReport, ExperimentClient, RecordingFollower,
};
use crate::workflow::identify_recovery::cancel_operation_on;

/// What the fake runtime does with an experiment's start.
#[derive(Debug, Clone, Copy)]
enum Start {
    /// The runtime refuses the start before any experiment exists (E0.4).
    Refused,
    /// The runtime starts the experiment, and the acknowledgement is lost (E11.4).
    Lost,
    /// The start never reaches the runtime, and no acknowledgement comes.
    Unheard,
}

/// The fake runtime: its process clock, its one experiment record (E5.6) and the requests it received, in order.
struct State {
    start: Start,
    status_readable: bool,
    now_ms: AtomicU64,
    experiment: Mutex<Option<Value>>,
    calls: Mutex<Vec<String>>,
}

impl State {
    fn new(start: Start, status_readable: bool) -> Arc<Self> {
        Arc::new(Self {
            start,
            status_readable,
            now_ms: AtomicU64::new(0),
            experiment: Mutex::new(None),
            calls: Mutex::new(Vec::new()),
        })
    }

    fn call(&self, request: String) {
        self.calls.lock().push(request);
    }

    #[allow(clippy::cast_precision_loss)] // A test clock of minutes.
    fn now(&self) -> f64 {
        self.now_ms.load(Ordering::SeqCst) as f64
    }

    /// The answer to `control.experiment.status`: the record of the experiment the runtime holds, or the refusal of an
    /// identifier it does not hold (E5.6, E0.4).
    fn status(&self, experiment_id: &str) -> StatusRead {
        self.call(format!("experiment.status {experiment_id}"));
        if !self.status_readable {
            return StatusRead::Unreadable("transport lost".to_owned());
        }
        match self.experiment.lock().as_ref() {
            Some(held) if held["experiment_id"] == experiment_id => {
                StatusRead::Answer(held.clone())
            }
            _ => StatusRead::Refused {
                code: Some("experiment_not_found".to_owned()),
                text: "No experiment holds this identifier.".to_owned(),
            },
        }
    }
}

/// The status record of an experiment of the tank loop's setpoint (E5.6), with every key the annex names.
fn record(
    experiment_id: &str,
    status: &str,
    restoration: &str,
    stop_reason: Option<&str>,
) -> Value {
    json!({
        "experiment_id": experiment_id, "source": "simulation", "kind": "setpoint_step", "task": "$background",
        "symbol": "Main.TIC101_Setpoint", "status": status, "stop_reason": stop_reason, "restoration": restoration,
        "restoration_reason": null, "elapsed_ms": 0, "duration_ms": 570_000, "reviewed_scan": 7, "started_scan": 8,
        "last_applied": null, "relay_feedback": null, "abort_binding": null
    })
}

struct FakeRuntime {
    state: Arc<State>,
    loop_definition: LoopDefinition,
}

impl FakeRuntime {
    fn new(state: &Arc<State>) -> Self {
        Self {
            state: Arc::clone(state),
            loop_definition: tank_loop(),
        }
    }

    fn recording(&self, recording_owner: RecordingRecoveryOwner) -> FakeRecording {
        FakeRecording {
            state: Arc::clone(&self.state),
            loop_definition: self.loop_definition.clone(),
            owner: recording_owner,
            next_second: 0.0,
        }
    }
}

impl IdentifyRuntime for FakeRuntime {
    type Recording = FakeRecording;
    type Experiment = FakeExperiment;

    fn start_recording(
        &self,
        _loop_definition: &LoopDefinition,
        plan: &RecordingStartPlan,
        persistence: &dyn RecordingOwnerPersistence,
    ) -> Result<FakeRecording, RecordingStartError<FakeRecording>> {
        let recording_owner = owner(&plan.recording_id);
        persistence
            .persist(&recording_owner)
            .map_err(RecordingStartError::Failed)?;
        self.state
            .call(format!("recording.start {}", plan.recording_id));
        Ok(self.recording(recording_owner))
    }

    fn recover_recording(
        &self,
        _loop_definition: &LoopDefinition,
        recording_owner: &RecordingRecoveryOwner,
    ) -> Result<FakeRecording, Failure> {
        Ok(self.recording(recording_owner.clone()))
    }

    fn experiment(&self) -> FakeExperiment {
        FakeExperiment {
            state: Arc::clone(&self.state),
        }
    }

    fn start_experiment(&self, plan: &ControlExperimentPlan) -> StartRead {
        self.state.call(format!("experiment.start {}", plan.id));
        match self.state.start {
            Start::Refused => StartRead::Refused {
                code: Some("outside_limits".to_owned()),
                text: "The value 47 lies outside the limits of Main.TIC101_Setpoint.".to_owned(),
            },
            Start::Lost => {
                *self.state.experiment.lock() = Some(record(&plan.id, "running", "pending", None));
                StartRead::Unreadable("the start acknowledgement was lost".to_owned())
            }
            Start::Unheard => {
                StartRead::Unreadable("the start acknowledgement was lost".to_owned())
            }
        }
    }

    fn experiment_status(&self, experiment_id: &str) -> StatusRead {
        self.state.status(experiment_id)
    }
}

/// The recording of the tank: one sample per second of process time, steady at 45 degC with a small periodic noise.
struct FakeRecording {
    state: Arc<State>,
    loop_definition: LoopDefinition,
    owner: RecordingRecoveryOwner,
    next_second: f64,
}

impl RecordingFollower for FakeRecording {
    fn follow(&mut self) -> Result<Vec<FollowSample>, Failure> {
        let limit = self.state.now() / 1_000.0;
        let mut page = Vec::new();
        while self.next_second <= limit + 1e-9 {
            let seconds = self.next_second;
            page.push(FollowSample {
                elapsed_seconds: seconds,
                good: true,
                output: Some(45.0 + 0.025 * ((1.7 * seconds).sin() + (2.3 * seconds).cos())),
                input: None,
            });
            self.next_second += 1.0;
        }
        Ok(page)
    }
}

impl RecordingSession for FakeRecording {
    type Recording = RecordedRun;

    fn loop_definition(&self) -> &LoopDefinition {
        &self.loop_definition
    }

    fn stop(&mut self) -> Result<RecordedRun, Failure> {
        self.state
            .call(format!("recording.stop {}", self.owner.recording_id));
        Ok(stopped(&self.owner))
    }

    fn release(&mut self) -> Result<(), Failure> {
        self.state
            .call(format!("recording.release {}", self.owner.recording_id));
        Ok(())
    }

    fn recovery_owner(&self) -> Option<RecordingRecoveryOwner> {
        Some(self.owner.clone())
    }
}

impl IdentifyRecording for FakeRecording {
    fn owner(&self) -> RecordingRecoveryOwner {
        self.owner.clone()
    }

    fn evidence(&self, run: &RecordedRun) -> RecordingEvidence {
        recording_evidence(run, None)
    }
}

struct FakeExperiment {
    state: Arc<State>,
}

impl FakeExperiment {
    /// Ends the held experiment as `operation` does, with its restoration confirmed (E5.5).
    fn end(
        &self,
        experiment_id: &str,
        operation: &str,
        status: &str,
        stop_reason: &str,
    ) -> Result<(), Failure> {
        self.state
            .call(format!("experiment.{operation} {experiment_id}"));
        let mut held = self.state.experiment.lock();
        match held.as_mut() {
            Some(current) if current["experiment_id"] == experiment_id => {
                *current = record(experiment_id, status, "restored", Some(stop_reason));
                Ok(())
            }
            _ => Err(Failure::from_runtime(
                "experiment_not_found",
                "No experiment holds this identifier.",
            )),
        }
    }
}

impl ExperimentClient for FakeExperiment {
    fn status(&mut self, experiment_id: &str) -> Result<Value, Failure> {
        match self.state.status(experiment_id) {
            StatusRead::Answer(answer) => Ok(answer),
            StatusRead::Refused { code, text } => Err(Failure::from_runtime(
                code.as_deref().unwrap_or_default(),
                &text,
            )),
            StatusRead::Unreadable(text) => Err(Failure::unreachable(&text)),
        }
    }

    fn finish(&mut self, experiment_id: &str) -> Result<(), Failure> {
        self.end(experiment_id, "finish", "completed", "finished_early")
    }

    fn abort(&mut self, experiment_id: &str) -> Result<(), Failure> {
        self.end(experiment_id, "abort", "aborted", "user_abort")
    }
}

/// A clock that moves only when the run waits on it; process time follows it.
struct FakeClock {
    state: Arc<State>,
}

impl CampaignClock for FakeClock {
    fn now_ms(&self) -> f64 {
        self.state.now()
    }

    fn wait(&self, milliseconds: u64) {
        self.state.now_ms.fetch_add(milliseconds, Ordering::SeqCst);
    }
}

struct Sampler(Option<ExperimentSample>);

impl LoopSampler for Sampler {
    fn sample(&mut self) -> Result<Option<ExperimentSample>, Failure> {
        Ok(self.0.clone())
    }
}

/// The study: the identifiers of the recordings kept, in order.
#[derive(Default)]
struct Kept(Vec<String>);

impl RecordingRetention for Kept {
    fn retain(&mut self, run: &RecordedRun, _evidence: &RecordingEvidence) -> Result<(), Failure> {
        self.0.push(run.recording_id.clone());
        Ok(())
    }
}

#[derive(Default)]
struct Report {
    texts: RefCell<Vec<String>>,
}

impl CampaignReport for Report {
    fn is_cancelled(&self) -> bool {
        false
    }

    fn progress(&self, text: &str) {
        self.texts.borrow_mut().push(text.to_owned());
    }
}

/// The evaluation of the record, which these tests never reach.
#[derive(Debug, Clone, PartialEq)]
struct Verdict;

impl RunEvaluation for Verdict {
    fn status(&self) -> RequirementStatus {
        RequirementStatus::Pass
    }
}

#[allow(clippy::unnecessary_wraps)] // It stands in for the evaluator callback, which returns a Result.
fn passes(_run: &ValidationRun, _checks: &[BoundsCheck]) -> Result<Verdict, Failure> {
    Ok(Verdict)
}

/// The tank of the `TypeScript` campaign tests: 0 to 100 degC, a 100 ms task, a 600 s maximum, Automatic.
fn tank_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": "rev-1",
        "projectId": "project", "runtimeId": "runtime",
        "controller": { "instance": "Main.TIC101", "type": "Control.PID", "revision": "sha256:test",
            "action": "direct", "taskPeriodMs": 100 },
        "signals": {
            "setpoint": { "symbol": "Main.TIC101_Setpoint", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "Main.TT101.Value", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "Main.TIC101_Output", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "experiment": { "execution": "control-studio", "kind": "observation", "amplitude": 0,
            "durationSeconds": 600, "seed": 1 }
    }))
    .expect("the tank loop")
}

/// The live sample of the tank in Auto: the setpoint and the measurement at 45 degC, the output at 30 %.
fn live_sample(source: EvidenceSource) -> ExperimentSample {
    let signal = |symbol: &str, value: f64, unit: &str| PlanSignal {
        symbol: symbol.to_owned(),
        address: None,
        value,
        unit: unit.to_owned(),
        quality: Quality::Good,
    };
    ExperimentSample {
        scan: 7,
        source,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("Main.TIC101_Setpoint", 45.0, "degC")),
        process_variable: signal("Main.TT101.Value", 45.0, "degC"),
        manipulated_variable: signal("Main.TIC101_Output", 30.0, "%"),
        additional: Vec::new(),
    }
}

/// The owner of a recording of the tank's measurement.
fn owner(recording_id: &str) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: recording_id.to_owned(),
        project_id: "project".to_owned(),
        runtime_id: "runtime".to_owned(),
        loop_id: "tank".to_owned(),
        loop_revision: "rev-1".to_owned(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some("sha256:program".to_owned()),
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".to_owned(),
            address: None,
            symbol: Some("Main.TT101.Value".to_owned()),
            iec_type: "LREAL".to_owned(),
        }],
    }
}

/// The run `control.recording.stop` returns for a recording of `recording_owner` that holds no sample (E4.7).
fn stopped(recording_owner: &RecordingRecoveryOwner) -> RecordedRun {
    RecordedRun {
        recording_id: recording_owner.recording_id.clone(),
        loop_revision: recording_owner.loop_revision.clone(),
        program_revision: recording_owner.program_revision.clone(),
        bindings: vec![SignalBinding {
            id: "processVariable".to_owned(),
            address: None,
            symbol: Some("Main.TT101.Value".to_owned()),
            iec_type: "LREAL".to_owned(),
            unit: None,
        }],
        stop_reason: StopReason::Stopped,
        samples: Vec::new(),
        acquisition: Acquisition {
            method: "runtime_scan_buffer".to_owned(),
            completeness: Completeness::Incomplete,
            observed_sample_count: 0,
            scan_range: None,
            missing_scan_ranges: Vec::new(),
            dropped_observed_samples: 0,
            actual_cycle_timing_available: false,
            freshness_available: false,
            reason: "The recording holds no samples.".to_owned(),
        },
    }
}

/// A journaled setpoint step of the tank.
fn plan() -> ControlExperimentPlan {
    serde_json::from_value(json!({
        "id": "experiment-1", "source": "simulation", "kind": "setpoint_step",
        "affectedSignal": "Main.TIC101_Setpoint", "valueType": "LREAL", "from": 45, "to": 47, "unit": "degC",
        "minimum": 0, "maximum": 100, "durationMs": 570_000, "expectedScan": 7, "task": "$background",
        "stopConditions": ["Abort"], "restorationValue": 45, "requiresPhysicalApproval": false,
        "trajectory": [{ "atMs": 0, "value": 47 }]
    }))
    .expect("the plan")
}

fn baseline() -> BaselineSummary {
    BaselineSummary {
        mean: 45.0,
        noise: 0.03,
        half_range: 0.05,
        spacing_seconds: 1.0,
        window_seconds: 30.0,
        input_mean: None,
        input_noise: None,
    }
}

/// A journal folder of its own for one test.
fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("trust-identify-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    folder
}

/// The tank's journal as a reopened panel reads it.
fn reopened(folder: &Path) -> Vec<JournalEntry> {
    Journal::open(folder, "project", "tank")
        .expect("the journal reopens")
        .entries()
        .to_vec()
}

/// The operations the fake runtime received, in order, without their identifiers.
fn operations(state: &State) -> Vec<String> {
    state
        .calls
        .lock()
        .iter()
        .map(|call| call.split(' ').next().unwrap_or_default().to_owned())
        .collect()
}

/// Runs an identification of the tank on `state`, with the live sample from `source`, journaled in `folder`.
fn identify(
    state: &Arc<State>,
    source: EvidenceSource,
    folder: &Path,
    kept: &mut Kept,
) -> Result<IdentifyOutcome<Verdict>, Failure> {
    let loop_definition = tank_loop();
    let runtime = FakeRuntime::new(state);
    let clock = FakeClock {
        state: Arc::clone(state),
    };
    let mut sampler = Sampler(Some(live_sample(source)));
    let report = Report::default();
    let journal = Mutex::new(Journal::open(folder, "project", "tank").expect("an empty journal"));
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    run(
        RunPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention: kept,
            clock: &clock,
            report: &report,
            journal: &journal,
        },
        &request,
        RunPurpose::Identify,
    )
}

/// The tank's journal holding a step whose experiment was dispatched, as a run left it.
fn journaled_step(folder: &Path) -> Mutex<Journal> {
    let mut journal = Journal::open(folder, "project", "tank").expect("an empty journal");
    journal
        .persist(JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::step_phase(
                "rev-1",
                owner("recording-1"),
                plan(),
                baseline(),
                30.0,
                0,
            )),
        })
        .expect("the step is journaled");
    journal
        .persist(JournalEntry::Experiment {
            recovery: Box::new(ControlExperimentRecovery {
                schema_version: 2,
                experiment_id: "experiment-1".to_owned(),
                project_id: "project".to_owned(),
                loop_id: "tank".to_owned(),
                loop_revision: "rev-1".to_owned(),
                plan: Some(plan()),
                recording_owner: Some(owner("recording-1")),
            }),
        })
        .expect("the experiment is journaled");
    Mutex::new(journal)
}

#[test]
fn a_refused_start_ends_the_run_without_reconciliation_and_keeps_the_recording() {
    let state = State::new(Start::Refused, true);
    let folder = folder("refused-start");
    let mut kept = Kept::default();
    let failure = identify(&state, EvidenceSource::SimulatedPlant, &folder, &mut kept)
        .expect_err("the runtime refused the start");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::OpenLimits),
        "the refusal keeps the action its code gives"
    );
    assert_eq!(
        operations(&state),
        [
            "recording.start",
            "experiment.start",
            "recording.stop",
            "recording.release"
        ],
        "the start is sent once and never reconciled; the recording is stopped, kept, then released"
    );
    assert_eq!(kept.0.len(), 1, "the baseline recording is kept once");
    assert_eq!(
        reopened(&folder),
        Vec::<JournalEntry>::new(),
        "nothing is left owned"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn an_unconfirmed_start_whose_status_cannot_be_read_is_left_to_the_runtime_with_its_journal() {
    let state = State::new(Start::Lost, false);
    let folder = folder("unresolved-start");
    let mut kept = Kept::default();
    let failure = identify(&state, EvidenceSource::SimulatedPlant, &folder, &mut kept)
        .expect_err("the start stays unresolved");
    assert_eq!(
        failure.message,
        "The experiment start was not confirmed and its status cannot be read."
    );
    assert_eq!(
        operations(&state),
        ["recording.start", "experiment.start", "experiment.status"],
        "the start is never sent again, the experiment never aborted and the recording never stopped"
    );
    assert_eq!(kept.0, Vec::<String>::new());
    let entries = reopened(&folder);
    assert_eq!(entries.len(), 2, "{entries:?}");
    let JournalEntry::IdentificationCampaign { campaign } = &entries[0] else {
        panic!("the campaign keeps its recording's place: {entries:?}");
    };
    assert_eq!(campaign.phase, CampaignPhase::Step);
    assert_eq!(
        entries[0].operation_id(),
        campaign.recording.recording_id,
        "the campaign is journaled under its recording"
    );
    let journaled_plan = campaign.plan.as_ref().expect("the step's plan");
    assert_eq!(
        entries[1].operation_id(),
        journaled_plan.id,
        "the experiment is journaled before its start"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_start_the_runtime_never_received_ends_the_run_and_clears_the_journal() {
    let state = State::new(Start::Unheard, true);
    let folder = folder("absent-start");
    let mut kept = Kept::default();
    let failure = identify(&state, EvidenceSource::SimulatedPlant, &folder, &mut kept)
        .expect_err("no experiment ran");
    assert_eq!(
        failure.message,
        "The experiment start was not confirmed, and the runtime holds no experiment with its identity, so none ran."
    );
    assert_eq!(
        operations(&state),
        [
            "recording.start",
            "experiment.start",
            "experiment.status",
            "recording.stop",
            "recording.release"
        ]
    );
    assert_eq!(kept.0.len(), 1, "the baseline recording is kept");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_physical_target_is_refused_before_anything_is_recorded_or_written() {
    let state = State::new(Start::Refused, true);
    let folder = folder("physical-target");
    let mut kept = Kept::default();
    let failure = identify(&state, EvidenceSource::PhysicalPlant, &folder, &mut kept)
        .expect_err("a physical target needs approval");
    assert_eq!(failure.message, PHYSICAL_TARGET_TEXT);
    assert_eq!(operations(&state), Vec::<String>::new());
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

#[test]
fn cancel_aborts_the_journaled_running_experiment_confirms_its_restoration_and_keeps_the_recording()
{
    let state = State::new(Start::Lost, true);
    *state.experiment.lock() = Some(record("experiment-1", "running", "pending", None));
    let folder = folder("cancel-running");
    let journal = journaled_step(&folder);
    let mut kept = Kept::default();
    let clock = FakeClock {
        state: Arc::clone(&state),
    };
    cancel_journaled(
        &FakeRuntime::new(&state),
        &clock,
        &journal,
        &tank_loop(),
        &mut kept,
    )
    .expect("cancelled");
    assert_eq!(
        operations(&state),
        [
            "experiment.status",
            "experiment.abort",
            "experiment.status",
            "recording.stop",
            "recording.release"
        ]
    );
    assert_eq!(kept.0, ["recording-1"]);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn cancel_never_aborts_an_experiment_the_runtime_does_not_hold() {
    let state = State::new(Start::Unheard, true);
    let folder = folder("cancel-absent");
    let journal = journaled_step(&folder);
    let mut kept = Kept::default();
    let clock = FakeClock {
        state: Arc::clone(&state),
    };
    cancel_journaled(
        &FakeRuntime::new(&state),
        &clock,
        &journal,
        &tank_loop(),
        &mut kept,
    )
    .expect("cancelled");
    assert_eq!(
        operations(&state),
        ["experiment.status", "recording.stop", "recording.release"],
        "nothing ran, so nothing is aborted"
    );
    assert_eq!(kept.0, ["recording-1"]);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

/// A journaled Test experiment of the tank, with its reviewed plan and the recording it runs with (E11.2), and no campaign.
fn journaled_test_experiment(folder: &Path) -> Mutex<Journal> {
    let mut journal = Journal::open(folder, "project", "tank").expect("an empty journal");
    journal
        .persist(JournalEntry::Experiment {
            recovery: Box::new(ControlExperimentRecovery {
                schema_version: 2,
                experiment_id: "experiment-1".to_owned(),
                project_id: "project".to_owned(),
                loop_id: "tank".to_owned(),
                loop_revision: "rev-1".to_owned(),
                plan: Some(plan()),
                recording_owner: Some(owner("recording-1")),
            }),
        })
        .expect("the experiment is journaled");
    Mutex::new(journal)
}

#[test]
fn cancel_of_a_journaled_test_experiment_aborts_it_keeps_its_recording_and_clears_its_owner() {
    // §0.1b, E5.5, E11.6: a Test's experiment is cancelled as a campaign's experiment is. It is reattached by its identifier,
    // aborted through the restoration path while it runs, and its restoration confirmed. Its recording is kept, and its
    // journal entry, which holds the owner, is cleared.
    let state = State::new(Start::Lost, true);
    *state.experiment.lock() = Some(record("experiment-1", "running", "pending", None));
    let folder = folder("cancel-test-experiment");
    let journal = journaled_test_experiment(&folder);
    let mut kept = Kept::default();
    let clock = FakeClock {
        state: Arc::clone(&state),
    };
    cancel_operation_on(
        &FakeRuntime::new(&state),
        &clock,
        &journal,
        &tank_loop(),
        "experiment-1",
        &mut kept,
    )
    .expect("cancelled");
    assert_eq!(
        operations(&state),
        [
            "experiment.status",
            "experiment.abort",
            "experiment.status",
            "recording.stop",
            "recording.release"
        ]
    );
    assert_eq!(kept.0, ["recording-1"]);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn cancel_of_a_journaled_test_experiment_the_runtime_does_not_hold_aborts_nothing() {
    // E11.4, E11.5: an experiment the runtime does not hold never ran, so nothing is aborted. Its recording is still kept and
    // its owner cleared.
    let state = State::new(Start::Unheard, true);
    let folder = folder("cancel-test-absent");
    let journal = journaled_test_experiment(&folder);
    let mut kept = Kept::default();
    let clock = FakeClock {
        state: Arc::clone(&state),
    };
    cancel_operation_on(
        &FakeRuntime::new(&state),
        &clock,
        &journal,
        &tank_loop(),
        "experiment-1",
        &mut kept,
    )
    .expect("cancelled");
    assert_eq!(
        operations(&state),
        ["experiment.status", "recording.stop", "recording.release"],
        "nothing ran, so nothing is aborted"
    );
    assert_eq!(kept.0, ["recording-1"]);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

/// E11.6 and §0.1b: naming the experiment of the journaled step cancels its campaign, as the campaign's Cancel does. The running
/// experiment is aborted through its restoration path, the recording is kept, and both journal entries are cleared.
#[test]
fn cancel_of_the_steps_experiment_is_the_campaigns_cancel() {
    let state = State::new(Start::Lost, true);
    *state.experiment.lock() = Some(record("experiment-1", "running", "pending", None));
    let folder = folder("cancel-step-experiment");
    let journal = journaled_step(&folder);
    let mut kept = Kept::default();
    let clock = FakeClock {
        state: Arc::clone(&state),
    };
    cancel_operation_on(
        &FakeRuntime::new(&state),
        &clock,
        &journal,
        &tank_loop(),
        "experiment-1",
        &mut kept,
    )
    .expect("cancelled");
    assert_eq!(
        operations(&state),
        [
            "experiment.status",
            "experiment.abort",
            "experiment.status",
            "recording.stop",
            "recording.release"
        ]
    );
    assert_eq!(kept.0, ["recording-1"]);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

/// A Cancel that is already requested when the run starts stops it before its work: nothing is recorded, started or kept, nothing
/// is journaled, and the run answers with the cancelled sentence. This is the rule of the worker tests' cancelled calculation,
/// which publishes no result and keeps nothing (`control-studio-identification-worker.test.ts`); the operation's own cancel is
/// `operation.rs`'s test.
#[test]
fn a_cancelled_run_records_starts_and_keeps_nothing_and_says_it_was_cancelled() {
    struct Cancelled;

    impl CampaignReport for Cancelled {
        fn is_cancelled(&self) -> bool {
            true
        }

        fn progress(&self, _text: &str) {}
    }

    let state = State::new(Start::Lost, true);
    let folder = folder("run-cancelled-before-work");
    let loop_definition = tank_loop();
    let runtime = FakeRuntime::new(&state);
    let clock = FakeClock {
        state: Arc::clone(&state),
    };
    let mut sampler = Sampler(Some(live_sample(EvidenceSource::SimulatedPlant)));
    let journal = Mutex::new(Journal::open(&folder, "project", "tank").expect("an empty journal"));
    let mut kept = Kept::default();
    let report = Cancelled;
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run(
        RunPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention: &mut kept,
            clock: &clock,
            report: &report,
            journal: &journal,
        },
        &request,
        RunPurpose::Identify,
    );
    let Err(failure) = outcome else {
        panic!("a run cancelled before its work gives no result");
    };
    assert_eq!(failure.message, CANCELLED_TEXT);
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Unclassified, NextAction::None)
    );
    assert_eq!(
        operations(&state),
        Vec::<String>::new(),
        "the runtime receives no request"
    );
    assert_eq!(kept.0, Vec::<String>::new(), "nothing is kept");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}
