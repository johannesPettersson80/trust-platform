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
    ControlEvidenceSource, ControlExperimentKind, ControlExperimentPlan, ControlExperimentRecovery,
    ControllerMode, ExperimentSample, ExperimentValue, PlanAuxiliarySignal, PlanSignal,
    RecordingRecoveryBinding, RecordingRecoveryOwner,
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
use trust_control::loop_definition::{ExperimentKind, LoopDefinition};
use trust_control::messages::CampaignRestoration;
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

    #[allow(clippy::cast_precision_loss)] // A test clock of minutes.
    fn process_seconds(&self, process_time_ms: u64) -> Option<f64> {
        Some(process_time_ms as f64 / 1_000.0)
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

/// The tank with a second plant input (C7, C8): a measured load (`P1.LoadKw`, 0 to 50 kW) stepped through its own Manual command with an
/// identification step of 10, beside the valve (`Main.TIC101_Output`), which the loop's own command moves. The loop declares an output step.
fn two_input_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": "rev-1",
        "projectId": "project", "runtimeId": "runtime",
        "controller": { "instance": "Main.TIC101", "type": "Control.PID", "revision": "sha256:test",
            "action": "direct", "taskPeriodMs": 100, "manualOutputSignal": "Main.TIC101_Manual",
            "manualOutputType": "LREAL" },
        "signals": {
            "setpoint": { "symbol": "Main.TIC101_Setpoint", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "Main.TT101.Value", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "Main.TIC101_Output", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "additional": [{ "id": "load", "channel": "load", "role": "disturbance", "symbol": "P1.LoadKw",
                "iecType": "LREAL", "direction": "read", "unit": "kW", "range": { "minimum": 0, "maximum": 50 } }]
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "experiment": { "execution": "control-studio", "kind": "mv_step", "amplitude": 10,
            "durationSeconds": 600, "seed": 1 },
        "channels": {
            "inputs": [
                { "modelChannelId": "valve", "signalId": "manipulatedVariable",
                  "manualOutputSignal": "Main.TIC101_Manual", "manualOutputType": "LREAL",
                  "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 }, "experimentStep": 10 },
                { "modelChannelId": "load", "signalId": "load",
                  "manualOutputSignal": "P1.LoadKw", "manualOutputType": "LREAL",
                  "actuator": { "kind": "continuous", "minimum": 0, "maximum": 50 }, "experimentStep": 10 }
            ],
            "outputs": [{ "modelChannelId": "temperature", "signalId": "processVariable",
                "setpointSignalId": "setpoint" }]
        }
    }))
    .expect("the two-input tank decodes")
}

/// A live sample of the two-input tank in Manual, where an output step runs: the valve at 30 %, the tank at 45 degC, and the load measured
/// at 20 kW.
fn manual_sample_with_load() -> ExperimentSample {
    let mut sample = super::test_support::live_sample();
    sample.controller_mode = ControllerMode::Manual;
    sample.additional.push(PlanAuxiliarySignal {
        id: "load".to_owned(),
        signal: PlanSignal {
            symbol: "P1.LoadKw".to_owned(),
            address: None,
            value: 20.0,
            unit: "kW".to_owned(),
            quality: Quality::Good,
        },
    });
    sample
}

// C7, C8: one campaign steps each plant input in turn, each through its own experiment and Manual command, in one recording. The record
// keeps every plan in input order, and the journal is cleared once nothing of the run is left running.
#[test]
fn a_two_input_campaign_steps_each_input_in_turn_in_one_recording() {
    use super::test_support::{
        empty_journal, folder as scratch, passes, reopened, FakeClock, FakeReport, FakeRuntime,
        FakeState, Sampler, StartBehaviour, Study as FakeStudy,
    };
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let scratch_folder = scratch("two-input-campaign");
    let journal = empty_journal(&scratch_folder);
    let loop_definition = two_input_loop();
    let runtime = FakeRuntime::new(&state);
    let clock = FakeClock::new(&state);
    let report = FakeReport::new(&state, false);
    let mut sampler = Sampler(Some(manual_sample_with_load()));
    let mut study = FakeStudy::new(&state);
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run(
        RunPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention: &mut study,
            clock: &clock,
            report: &report,
            journal: &journal,
        },
        &request,
        RunPurpose::RecordValidation,
    )
    .expect("both plant inputs are stepped and recorded");
    let super::IdentifyResult::Recorded(record) = outcome.result else {
        panic!("the independent check records its run");
    };
    let started = state.started_plans();
    assert_eq!(started.len(), 2, "one experiment per plant input");
    assert_eq!(
        started[0].affected_signal, "Main.TIC101_Manual",
        "the valve steps through the loop's own Manual command, first"
    );
    assert_eq!(
        started[1].affected_signal, "P1.LoadKw",
        "the load steps through its own Manual command, second"
    );
    let experiments = record
        .request
        .experiments
        .clone()
        .expect("the record keeps the plan of every input");
    let kept: Vec<&str> = experiments.iter().map(|plan| plan.id.as_str()).collect();
    let sent: Vec<&str> = started.iter().map(|plan| plan.id.as_str()).collect();
    assert_eq!(kept, sent, "the record keeps the plans in input order");
    assert_eq!(
        state.count("recording.start"),
        1,
        "one recording holds both steps"
    );
    assert_eq!(state.count("retain"), 1, "the recording is kept once");
    assert_eq!(
        reopened(&scratch_folder),
        Vec::<JournalEntry>::new(),
        "the journal is cleared once nothing of the run is left running"
    );
    fs::remove_dir_all(&scratch_folder).expect("clean up");
}

/// The two-input tank of `two_input_loop` with no declared method, so that Identify chooses the method of each input without its own step
/// (B3.3), and with each input's own identification step as the arguments give it (C7, C8).
fn two_input_loop_with_steps(valve_step: Option<f64>, load_step: Option<f64>) -> LoopDefinition {
    let mut loop_definition = two_input_loop();
    if let Some(experiment) = loop_definition.experiment.as_mut() {
        experiment.kind = ExperimentKind::Observation;
        experiment.amplitude = 0.0;
    }
    if let Some(channels) = loop_definition.channels.as_mut() {
        channels.inputs[0].experiment_step = valve_step;
        channels.inputs[1].experiment_step = load_step;
    }
    loop_definition
}

/// Runs one identification of the two-input tank `loop_definition` on `state`, with `sample` as the loop's live sample, and says whether it
/// ended without a failure. The fake recording holds no load values, so a run that gets past its steps is refused by its fit.
fn run_two_inputs(
    state: &Arc<super::test_support::FakeState>,
    folder: &Path,
    loop_definition: &LoopDefinition,
    sample: ExperimentSample,
) -> Result<(), Failure> {
    use super::test_support::{
        empty_journal, passes, FakeClock, FakeReport, FakeRuntime, Sampler, Study as FakeStudy,
    };
    let journal = empty_journal(folder);
    let runtime = FakeRuntime::new(state);
    let clock = FakeClock::new(state);
    let report = FakeReport::new(state, false);
    let mut sampler = Sampler(Some(sample));
    let mut study = FakeStudy::new(state);
    let request = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    run(
        RunPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention: &mut study,
            clock: &clock,
            report: &report,
            journal: &journal,
        },
        &request,
        RunPurpose::Identify,
    )
    .map(|_| ())
}

/// Asserts that `plan` moves `signal` by a manual-output step from `from` to `to`, as an input with its own step is stepped (C7, C8).
#[allow(clippy::float_cmp)] // The planned values are the sums of whole numbers, which are exact.
fn assert_manual_output_step(plan: &ControlExperimentPlan, signal: &str, from: f64, to: f64) {
    assert_eq!(
        plan.kind,
        ControlExperimentKind::MvStep,
        "{signal} is stepped by a manual-output step"
    );
    assert_eq!(plan.affected_signal, signal);
    let ExperimentValue::Number(start) = plan.from else {
        panic!("{signal} starts from a number");
    };
    let ExperimentValue::Number(end) = plan.to else {
        panic!("{signal} moves to a number");
    };
    assert_eq!((start, end), (from, to), "{signal} moves by its own step");
}

// C7, C8: when every input declares its own step, one campaign steps each input at its own step, each by a manual-output step of its size.
#[test]
fn campaign_steps_both_inputs_at_their_own_manual_output_steps() {
    use super::test_support::{folder as scratch, FakeState, StartBehaviour};
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let folder = scratch("own-steps-both");
    let loop_definition = two_input_loop_with_steps(Some(10.0), Some(5.0));
    // The fit refuses the fake recording after both steps, which hold no load values, so only the steps that started are read here.
    let _ = run_two_inputs(&state, &folder, &loop_definition, manual_sample_with_load());
    let started = state.started_plans();
    assert_eq!(started.len(), 2, "one experiment per plant input");
    assert_manual_output_step(&started[0], "Main.TIC101_Manual", 30.0, 40.0);
    assert_manual_output_step(&started[1], "P1.LoadKw", 20.0, 25.0);
    assert_eq!(
        state.count("recording.start"),
        1,
        "one recording holds both steps"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

// C7, C8: an input without its own step takes the method Identify chose for the loop, beside the input stepped at its own step.
#[test]
fn campaign_steps_an_input_without_a_step_by_the_method_identify_chose() {
    use super::test_support::{folder as scratch, FakeState, StartBehaviour};
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let folder = scratch("own-step-first-only");
    let loop_definition = two_input_loop_with_steps(Some(4.0), None);
    let _ = run_two_inputs(&state, &folder, &loop_definition, manual_sample_with_load());
    let started = state.started_plans();
    assert_eq!(started.len(), 2, "one experiment per plant input");
    assert_manual_output_step(&started[0], "Main.TIC101_Manual", 30.0, 34.0);
    // The load has no step, so Identify chooses a plant-input step in Manual. Automatic sizes it at 10 % of the load's 50 kW range,
    // which is within the 30 kW of headroom above the 20 kW measured.
    assert_manual_output_step(&started[1], "P1.LoadKw", 20.0, 25.0);
    fs::remove_dir_all(&folder).expect("clean up");
}

// C7, C8: an input without its own step would take a setpoint step in Auto, which cannot share a recording with a manual-output step, so
// the campaign is refused by name before it records or starts anything.
#[test]
fn campaign_refuses_an_input_that_would_take_a_setpoint_step_before_it_records() {
    use super::test_support::{folder as scratch, FakeState, StartBehaviour};
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let folder = scratch("own-step-in-auto");
    let loop_definition = two_input_loop_with_steps(Some(4.0), None);
    let mut sample = manual_sample_with_load();
    sample.controller_mode = ControllerMode::Auto;
    let failure = run_two_inputs(&state, &folder, &loop_definition, sample)
        .expect_err("the inputs need both controller modes");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLoopSetup)
    );
    assert!(
        failure.message.contains("P1.LoadKw"),
        "the refusal names the load: {}",
        failure.message
    );
    assert!(state.started_plans().is_empty(), "nothing is started");
    assert_eq!(state.count("recording.start"), 0, "nothing is recorded");
    // The refusal comes before the campaign journals anything, so the folder may never have been created.
    if let Err(error) = fs::remove_dir_all(&folder) {
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::NotFound,
            "clean up: {error}"
        );
    }
}

#[test]
fn a_hold_confirms_the_campaigns_restoration_only_when_the_runtime_says_it_restored_or_needed_none()
{
    let status = |restoration: Value| json!({ "status": "completed", "restoration": restoration });
    assert_eq!(
        super::run::hold_restoration(&status(json!("restored"))),
        Some(CampaignRestoration::Restored),
        "a restoration the runtime confirmed is the campaign's restoration (C12)"
    );
    assert_eq!(
        super::run::hold_restoration(&status(json!("not_required"))),
        Some(CampaignRestoration::NotRequired),
        "an experiment that wrote no output needed no restoration"
    );
    assert_eq!(
        super::run::hold_restoration(&status(json!("pending"))),
        None,
        "a restoration not yet confirmed is no outcome to report"
    );
    assert_eq!(super::run::hold_restoration(&status(json!("failed"))), None);
    assert_eq!(
        super::run::hold_restoration(&json!({ "status": "completed" })),
        None,
        "a status without a restoration is no outcome to report"
    );
}
