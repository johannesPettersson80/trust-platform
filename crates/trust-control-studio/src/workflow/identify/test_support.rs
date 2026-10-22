//! Test support of the connected run and of its recovery (`control-studio-identification-recovery.test.ts`): a fake runtime
//! whose experiment starts, ends and answers on a process clock that the code's waits move, the simulated tank it steps, and the
//! ports a run or a recovery reads and writes. The tank loop and its journals are in `test_fixtures`, which this module
//! re-exports. Every answer carries exactly the fields annex E names (the start answer of E5.3, the status record of E5.6, the
//! stopped recording of E4.7) and the identifier the code under test started or journaled. Every call reaches one ordered log,
//! with the recordings the study keeps and the progress the operation reports, so a test reads what happened in its order.
//!
//! The long-process campaign tests (`control-studio-long-process-campaign.test.ts`) also give the tank a ramp while it is
//! stepped, an offset that decays, or a silent recording, and read the plans of the starts the runtime received.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::experiment::{
    ControlExperimentPlan, ExperimentSample, ExperimentValue, RecordingRecoveryOwner,
};
use trust_control::failure::Failure;
use trust_control::identification::campaign::FollowSample;
use trust_control::identification::connected_record::{BoundsCheck, ValidationRun};
use trust_control::identification::connected_run::StartRead;
use trust_control::identification::independent_validation::{RequirementStatus, RunEvaluation};
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::context::PlantContext;
use trust_control::recording::evidence::{recording_evidence, RecordingEvidence};
use trust_control::recording::{
    Acquisition, Completeness, RecordedRun, RecordedSample, ScanRange, StopReason,
};
use trust_control::signals::{Quality, SignalBinding};

use super::run::RunPorts;
use super::test_fixtures::owner;
use super::{
    IdentifyOutcome, IdentifyRecording, IdentifyRequest, IdentifyRuntime, LoopSampler,
    RecordingRetention, RunPurpose,
};
use crate::journal::Journal;
use crate::runtime_client::recording_coordinator::RecordingSession;
use crate::runtime_client::runtime_recording::{
    RecordingOwnerPersistence, RecordingStartError, RecordingStartPlan,
};
use crate::workflow::identify_campaign::{
    CampaignClock, CampaignReport, ExperimentClient, RecordingFollower,
};

// The identification's Cancel of a journaled campaign, which a cancelled recovery finishes with.
pub(crate) use super::cancel::cancel_journaled;
// The fixtures of the tank, which the tests of the run and of its recovery take from here.
pub(crate) use super::test_fixtures::{
    assert_no_model_of_a_held_input, empty_journal, folder, journaled_baseline, journaled_step,
    live_sample, plan, reopened, tank_loop, whole_text,
};

/// The runtime's text for an experiment identifier it does not hold (E5.5, E5.6).
const NOT_FOUND_TEXT: &str = "No experiment holds this identifier.";

/// What the fake runtime does with the start of an experiment (`StartOutcome` of the `TypeScript` test).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartBehaviour {
    /// It starts the experiment and acknowledges the start (E5.3).
    Acknowledged,
    /// It starts the experiment, and the acknowledgement is lost (E11.4).
    Running,
    /// It starts the experiment, which has ended before it is read, and the acknowledgement is lost.
    Terminal,
    /// The start never reaches it, so it holds no experiment of that identity.
    Absent,
    /// It starts the experiment, the acknowledgement is lost, and no status can be read.
    Unreadable,
}

/// The experiment record the fake runtime holds (E5.6), on its process clock in ms.
struct Held {
    id: String,
    reviewed_scan: u64,
    start_ms: u64,
    duration_ms: u64,
    ended_ms: Option<u64>,
    status: &'static str,
    restoration: &'static str,
    stop_reason: Option<&'static str>,
}

/// The simulated tank (`Plant` of the `TypeScript` test): a first-order response of `change` with a 50 s time constant from
/// `start_s` until `stop_s` of process time, around `base`, with a small periodic disturbance. A test may add a ramp of
/// `ramp` per second while the tank is stepped, and an offset of `offset` at process time zero that decays with the time
/// constant `offset_tau_s` (an earlier test still dying away); both are zero otherwise, which leaves the output unchanged.
#[derive(Clone, Copy)]
struct Plant {
    base: f64,
    start_s: f64,
    stop_s: f64,
    change: f64,
    ramp: f64,
    offset: f64,
    offset_tau_s: f64,
}

impl Plant {
    /// The measured output at `seconds` of process time (`measured`).
    fn measured(self, seconds: f64) -> f64 {
        let (step, ramp) = if seconds >= self.start_s && seconds < self.stop_s {
            let elapsed = seconds - self.start_s;
            (
                self.change * (1.0 - (-elapsed / 50.0).exp()),
                self.ramp * elapsed,
            )
        } else {
            (0.0, 0.0)
        };
        let decaying = if self.offset_tau_s > 0.0 {
            self.offset * (-seconds / self.offset_tau_s).exp()
        } else {
            0.0
        };
        let disturbance = 0.025 * ((1.7 * seconds).sin() + (2.3 * seconds).cos());
        self.base + decaying + step + ramp + disturbance
    }
}

/// The fake runtime's state, which its experiment, its recordings, the clock, the operation and the study share: the process
/// clock, the experiment it holds, the tank, the ordered log, and the plans of the starts it received.
pub(crate) struct FakeState {
    behaviour: StartBehaviour,
    now_ms: AtomicU64,
    held: Mutex<Option<Held>>,
    plant: Mutex<Plant>,
    follows_refused: AtomicBool,
    follows_silent: AtomicBool,
    calls: Mutex<Vec<String>>,
    experiment_plans: Mutex<Vec<ControlExperimentPlan>>,
    recording_plans: Mutex<Vec<RecordingStartPlan>>,
    /// How every experiment this runtime starts ends at once, when a test says so: its status and its stop reason (E5.7).
    ending: Mutex<Option<(&'static str, &'static str)>>,
    /// Whether the runtime refuses every page of a recording from the acknowledged start of an experiment on, as when the connection
    /// is lost during the hold.
    refuse_after_start: AtomicBool,
    /// The plant this runtime proves it runs, when a test says so (specification 34, §20.4).
    proven_plant: Mutex<Option<PlantContext>>,
}

impl FakeState {
    /// A runtime that treats a start as `behaviour` says, with its clock at `now_ms` of process time, no experiment, and the tank
    /// steady at 45 degC.
    pub(crate) fn new(behaviour: StartBehaviour, now_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            behaviour,
            now_ms: AtomicU64::new(now_ms),
            held: Mutex::new(None),
            plant: Mutex::new(Plant {
                base: 45.0,
                start_s: f64::INFINITY,
                stop_s: f64::INFINITY,
                change: 0.0,
                ramp: 0.0,
                offset: 0.0,
                offset_tau_s: 0.0,
            }),
            follows_refused: AtomicBool::new(false),
            follows_silent: AtomicBool::new(false),
            calls: Mutex::new(Vec::new()),
            experiment_plans: Mutex::new(Vec::new()),
            recording_plans: Mutex::new(Vec::new()),
            ending: Mutex::new(None),
            refuse_after_start: AtomicBool::new(false),
            proven_plant: Mutex::new(None),
        })
    }

    /// From now on the runtime refuses every page of a recording once it has acknowledged the start of an experiment.
    pub(crate) fn refuse_follows_once_started(&self) {
        self.refuse_after_start.store(true, Ordering::SeqCst);
    }

    /// From now on every experiment this runtime starts ends at once with `status` and `stop_reason`, its restoration confirmed, as
    /// when a safety condition stops it (E5.4, E5.7).
    pub(crate) fn end_experiments_at_start(&self, status: &'static str, stop_reason: &'static str) {
        *self.ending.lock() = Some((status, stop_reason));
    }

    /// From now on this runtime proves that it runs `plant` (specification 34, §20.4).
    pub(crate) fn prove_plant(&self, plant: PlantContext) {
        *self.proven_plant.lock() = Some(plant);
    }

    /// An experiment that was already running when the host reopened (`restart`, A8.5): it started at `start_ms` of process
    /// time, runs for `duration_ms`, and steps the tank by `change`.
    pub(crate) fn restart(
        &self,
        experiment_id: &str,
        start_ms: u64,
        duration_ms: u64,
        change: f64,
    ) {
        *self.held.lock() = Some(Held {
            id: experiment_id.to_owned(),
            reviewed_scan: 300,
            start_ms,
            duration_ms,
            ended_ms: None,
            status: "running",
            restoration: "pending",
            stop_reason: None,
        });
        let mut plant = self.plant.lock();
        plant.start_s = seconds(start_ms);
        plant.stop_s = f64::INFINITY;
        plant.change = change;
    }

    /// From now on every page of a recording is refused, as when the connection to the runtime is lost.
    pub(crate) fn refuse_follows(&self) {
        self.follows_refused.store(true, Ordering::SeqCst);
    }

    /// From now on every page of a recording is empty, as when the runtime stops sending samples (`silent`).
    pub(crate) fn silence_follows(&self) {
        self.follows_silent.store(true, Ordering::SeqCst);
    }

    /// From now on the tank also ramps by `per_second` while it is stepped, so its response never settles (`ramp`).
    pub(crate) fn ramp_while_stepped(&self, per_second: f64) {
        self.plant.lock().ramp = per_second;
    }

    /// From now on the tank's output lies `offset` above its rest at process time zero and decays with the time constant
    /// `time_constant_seconds`, as when an earlier test is still dying away (`offset` and `offsetTauS`).
    pub(crate) fn decaying_offset(&self, offset: f64, time_constant_seconds: f64) {
        let mut plant = self.plant.lock();
        plant.offset = offset;
        plant.offset_tau_s = time_constant_seconds;
    }

    /// The process time, in ms.
    pub(crate) fn now_ms(&self) -> u64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    /// The log in order: each request with the identifier it named, each recording the study kept (`retain`) and each
    /// progress text (`progress`).
    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().clone()
    }

    /// The requests the runtime received and the recordings the study kept, in order, without their identifiers and without
    /// the progress.
    pub(crate) fn operations(&self) -> Vec<String> {
        self.calls()
            .iter()
            .filter_map(|call| {
                let operation = call.split(' ').next().unwrap_or_default();
                (operation != "progress").then(|| operation.to_owned())
            })
            .collect()
    }

    /// How often `operation` was received.
    pub(crate) fn count(&self, operation: &str) -> usize {
        self.operations()
            .iter()
            .filter(|received| received.as_str() == operation)
            .count()
    }

    /// The status and the stop reason of the experiment the runtime holds, as it last answered them.
    pub(crate) fn experiment(&self) -> Option<(&'static str, Option<&'static str>)> {
        self.held
            .lock()
            .as_ref()
            .map(|held| (held.status, held.stop_reason))
    }

    /// The plans of the experiment starts the runtime received, in order (E5.3).
    pub(crate) fn started_plans(&self) -> Vec<ControlExperimentPlan> {
        self.experiment_plans.lock().clone()
    }

    /// The recording starts the runtime received, in order: each recording's identifier, its planned duration and the
    /// expected settling time its record period follows (E4.2, A4.2).
    pub(crate) fn recording_starts(&self) -> Vec<RecordingStartPlan> {
        self.recording_plans.lock().clone()
    }

    /// The process time at which the tank's last step started, in s; infinite before any step.
    pub(crate) fn step_start_seconds(&self) -> f64 {
        self.plant.lock().start_s
    }

    fn call(&self, entry: String) {
        self.calls.lock().push(entry);
    }

    /// The start of `plan` (E5.3), as this runtime treats it. A runtime that starts the experiment steps the tank from the
    /// plan's restoration value to its value.
    fn start(&self, plan: &ControlExperimentPlan) -> StartRead {
        self.call(format!("experiment.start {}", plan.id));
        self.experiment_plans.lock().push(plan.clone());
        match self.behaviour {
            StartBehaviour::Acknowledged => {
                self.accept(plan, false);
                let ending = *self.ending.lock();
                if let Some((status, stop_reason)) = ending {
                    let now_ms = self.now_ms();
                    if let Some(record) = self.held.lock().as_mut() {
                        self.end(record, status, stop_reason, now_ms);
                    }
                }
                if self.refuse_after_start.load(Ordering::SeqCst) {
                    self.follows_refused.store(true, Ordering::SeqCst);
                }
                return StartRead::Answer(json!({
                    "experiment_id": plan.id, "status": "running", "restoration": "pending",
                    "symbol": plan.affected_signal
                }));
            }
            StartBehaviour::Running | StartBehaviour::Unreadable => self.accept(plan, false),
            StartBehaviour::Terminal => self.accept(plan, true),
            StartBehaviour::Absent => {}
        }
        StartRead::Unreadable("the start acknowledgement was lost".to_owned())
    }

    /// Accepts the start of `plan` (`accept`): the experiment runs from now, or has already completed when `ended`, and a
    /// running one steps the tank.
    fn accept(&self, plan: &ControlExperimentPlan, ended: bool) {
        let now_ms = self.now_ms();
        let mut record = Held {
            id: plan.id.clone(),
            reviewed_scan: plan.expected_scan,
            start_ms: now_ms,
            duration_ms: whole_ms(plan.duration_ms),
            ended_ms: None,
            status: "running",
            restoration: "pending",
            stop_reason: None,
        };
        if ended {
            record.ended_ms = Some(now_ms);
            record.status = "completed";
            record.restoration = "restored";
            record.stop_reason = Some("duration_complete");
        } else {
            let from = number(plan.restoration_value);
            let mut plant = self.plant.lock();
            plant.base = from;
            plant.start_s = seconds(now_ms);
            plant.stop_s = f64::INFINITY;
            plant.change = number(plan.to) - from;
        }
        *self.held.lock() = Some(record);
    }

    /// The answer to `control.experiment.status` (E5.6): the record of the experiment the runtime holds under that identifier,
    /// or the refusal of an identifier it does not hold (E0.4).
    fn status(&self, experiment_id: &str) -> StatusRead {
        self.call(format!("experiment.status {experiment_id}"));
        if self.behaviour == StartBehaviour::Unreadable {
            return StatusRead::Unreadable("transport lost".to_owned());
        }
        let now_ms = self.now_ms();
        let mut held = self.held.lock();
        match held.as_mut() {
            Some(record) if record.id == experiment_id => {
                self.refresh(record, now_ms);
                StatusRead::Answer(status_record(record, now_ms))
            }
            _ => StatusRead::Refused {
                code: Some("experiment_not_found".to_owned()),
                text: NOT_FOUND_TEXT.to_owned(),
            },
        }
    }

    /// Finishes or aborts the experiment of that identifier (`operation`, E5.5): a running one ends at once with `status` and
    /// `stop_reason`; any other record is left as it is. An identifier the runtime does not hold is refused.
    fn operate(
        &self,
        operation: &str,
        experiment_id: &str,
        status: &'static str,
        stop_reason: &'static str,
    ) -> Result<(), Failure> {
        self.call(format!("experiment.{operation} {experiment_id}"));
        let now_ms = self.now_ms();
        let mut held = self.held.lock();
        let Some(record) = held.as_mut().filter(|record| record.id == experiment_id) else {
            return Err(Failure::from_runtime(
                "experiment_not_found",
                NOT_FOUND_TEXT,
            ));
        };
        self.refresh(record, now_ms);
        if record.status == "running" {
            self.end(record, status, stop_reason, now_ms);
        }
        Ok(())
    }

    /// Ends a running experiment whose duration has passed, at the end of its duration (E5.4).
    fn refresh(&self, record: &mut Held, now_ms: u64) {
        let due_ms = record.start_ms + record.duration_ms;
        if record.status == "running" && now_ms >= due_ms {
            self.end(record, "completed", "duration_complete", due_ms);
        }
    }

    /// Ends the experiment at `at_ms` with its restoration confirmed (E5.7); the tank returns to its base.
    fn end(&self, record: &mut Held, status: &'static str, stop_reason: &'static str, at_ms: u64) {
        record.status = status;
        record.restoration = "restored";
        record.stop_reason = Some(stop_reason);
        record.ended_ms = Some(at_ms);
        self.plant.lock().stop_s = seconds(at_ms);
    }
}

/// The status record of an experiment (E5.6), with every key the annex names: a setpoint step of the tank on the background
/// task, written by symbol, with no trajectory point reported.
fn status_record(record: &Held, now_ms: u64) -> Value {
    json!({
        "experiment_id": record.id, "source": "simulation", "kind": "setpoint_step", "task": "$background",
        "symbol": "Main.TIC101_Setpoint", "status": record.status, "stop_reason": record.stop_reason,
        "restoration": record.restoration, "restoration_reason": null,
        "elapsed_ms": record.ended_ms.unwrap_or(now_ms).saturating_sub(record.start_ms),
        "duration_ms": record.duration_ms, "reviewed_scan": record.reviewed_scan, "started_scan": record.start_ms / 100,
        "last_applied": null, "relay_feedback": null, "abort_binding": null
    })
}

/// The fake runtime a run or a recovery works on.
pub(crate) struct FakeRuntime {
    state: Arc<FakeState>,
}

impl FakeRuntime {
    /// The runtime of `state`.
    pub(crate) fn new(state: &Arc<FakeState>) -> Self {
        Self {
            state: Arc::clone(state),
        }
    }
}

impl IdentifyRuntime for FakeRuntime {
    type Recording = FakeRecording;
    type Experiment = FakeExperiment;

    fn start_recording(
        &self,
        loop_definition: &LoopDefinition,
        plan: &RecordingStartPlan,
        persistence: &dyn RecordingOwnerPersistence,
    ) -> Result<FakeRecording, RecordingStartError<FakeRecording>> {
        let recording_owner = owner(&plan.recording_id);
        persistence
            .persist(&recording_owner)
            .map_err(RecordingStartError::Failed)?;
        self.state
            .call(format!("recording.start {}", plan.recording_id));
        self.state.recording_plans.lock().push(plan.clone());
        Ok(FakeRecording::new(
            &self.state,
            loop_definition,
            recording_owner,
        ))
    }

    fn recover_recording(
        &self,
        loop_definition: &LoopDefinition,
        recording_owner: &RecordingRecoveryOwner,
    ) -> Result<FakeRecording, Failure> {
        Ok(FakeRecording::new(
            &self.state,
            loop_definition,
            recording_owner.clone(),
        ))
    }

    fn experiment(&self) -> FakeExperiment {
        FakeExperiment {
            state: Arc::clone(&self.state),
        }
    }

    fn start_experiment(&self, plan: &ControlExperimentPlan) -> StartRead {
        self.state.start(plan)
    }

    fn experiment_status(&self, experiment_id: &str) -> StatusRead {
        self.state.status(experiment_id)
    }

    fn plant_context(&self, _loop_definition: &LoopDefinition) -> Option<PlantContext> {
        self.state.proven_plant.lock().clone()
    }
}

/// A recording of the tank (`recorderFor`): one sample per second of process time up to the clock, each standing for ten
/// executions of the 100 ms task, so its sequence steps by ten (A4.2, E4.4).
pub(crate) struct FakeRecording {
    state: Arc<FakeState>,
    loop_definition: LoopDefinition,
    owner: RecordingRecoveryOwner,
    next_second: u64,
}

impl FakeRecording {
    fn new(
        state: &Arc<FakeState>,
        loop_definition: &LoopDefinition,
        recording_owner: RecordingRecoveryOwner,
    ) -> Self {
        Self {
            state: Arc::clone(state),
            loop_definition: loop_definition.clone(),
            owner: recording_owner,
            next_second: 0,
        }
    }

    /// The run `control.recording.stop` returns (E4.7): every sample followed so far, with the tank's setpoint, measurement
    /// and manipulated variable as the tank stands now, and a complete acquisition.
    fn stopped_run(&self) -> RecordedRun {
        let plant = *self.state.plant.lock();
        let samples: Vec<RecordedSample> = (0..self.next_second)
            .map(|second| {
                let at_ms = second * 1_000;
                RecordedSample {
                    scan: second * 10,
                    runtime_time_ns: Some(i64::try_from(at_ms * 1_000_000).expect("a test time")),
                    captured_at_ms: at_ms,
                    values: vec![Some(45.0), Some(plant.measured(seconds(at_ms))), Some(30.0)],
                    forced: vec![false; 3],
                    quality: vec![Quality::Good; 3],
                    gap: false,
                }
            })
            .collect();
        RecordedRun {
            recording_id: self.owner.recording_id.clone(),
            loop_revision: self.owner.loop_revision.clone(),
            program_revision: self.owner.program_revision.clone(),
            bindings: self
                .owner
                .bindings
                .iter()
                .map(|binding| SignalBinding {
                    id: binding.id.clone(),
                    address: binding.address.clone(),
                    symbol: binding.symbol.clone(),
                    iec_type: binding.iec_type.clone(),
                    unit: None,
                })
                .collect(),
            stop_reason: StopReason::Stopped,
            acquisition: Acquisition {
                method: "runtime_scan_buffer".to_owned(),
                completeness: Completeness::Complete,
                observed_sample_count: u64::try_from(samples.len()).expect("a test count"),
                scan_range: self.next_second.checked_sub(1).map(|last| ScanRange {
                    first: 0,
                    last: last * 10,
                }),
                missing_scan_ranges: Vec::new(),
                dropped_observed_samples: 0,
                actual_cycle_timing_available: true,
                freshness_available: false,
                reason: String::new(),
            },
            samples,
        }
    }
}

impl RecordingFollower for FakeRecording {
    fn follow(&mut self) -> Result<Vec<FollowSample>, Failure> {
        if self.state.follows_refused.load(Ordering::SeqCst) {
            return Err(Failure::unreachable("the runtime closed the connection"));
        }
        if self.state.follows_silent.load(Ordering::SeqCst) {
            return Ok(Vec::new());
        }
        let now_ms = self.state.now_ms();
        let plant = *self.state.plant.lock();
        let mut page = Vec::new();
        while self.next_second * 1_000 <= now_ms {
            let at = seconds(self.next_second * 1_000);
            page.push(FollowSample {
                elapsed_seconds: at,
                good: true,
                output: Some(plant.measured(at)),
                input: None,
            });
            self.next_second += 1;
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
        Ok(self.stopped_run())
    }

    fn release(&mut self) -> Result<(), Failure> {
        self.state
            .call(format!("recording.release {}", self.owner.recording_id));
        Ok(())
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

/// The experiment client of the fake runtime (E5.5, E5.6).
pub(crate) struct FakeExperiment {
    state: Arc<FakeState>,
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
        self.state
            .operate("finish", experiment_id, "completed", "finished_early")
    }

    fn abort(&mut self, experiment_id: &str) -> Result<(), Failure> {
        self.state
            .operate("abort", experiment_id, "aborted", "user_abort")
    }
}

/// A clock that moves only when the code waits on it; the runtime's process time follows it (`fakeClock`).
pub(crate) struct FakeClock {
    state: Arc<FakeState>,
}

impl FakeClock {
    /// The clock of `state`.
    pub(crate) fn new(state: &Arc<FakeState>) -> Self {
        Self {
            state: Arc::clone(state),
        }
    }
}

impl CampaignClock for FakeClock {
    fn now_ms(&self) -> f64 {
        as_f64(self.state.now_ms())
    }

    fn wait(&self, milliseconds: u64) {
        self.state.now_ms.fetch_add(milliseconds, Ordering::SeqCst);
    }
}

/// The operation a run or a recovery reports to: its progress reaches the log, and it is cancelled from the start when the
/// test says so (the engineer's Cancel).
pub(crate) struct FakeReport {
    state: Arc<FakeState>,
    cancelled: bool,
}

impl FakeReport {
    /// The operation of `state`, cancelled when `cancelled` says so.
    pub(crate) fn new(state: &Arc<FakeState>, cancelled: bool) -> Self {
        Self {
            state: Arc::clone(state),
            cancelled,
        }
    }
}

impl CampaignReport for FakeReport {
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn progress(&self, text: &str) {
        self.state.call(format!("progress {text}"));
    }
}

/// The study (`ports.retain`): it keeps each stopped recording, which the log records.
pub(crate) struct Study {
    state: Arc<FakeState>,
}

impl Study {
    /// The study of `state`.
    pub(crate) fn new(state: &Arc<FakeState>) -> Self {
        Self {
            state: Arc::clone(state),
        }
    }
}

impl RecordingRetention for Study {
    fn retain(&mut self, run: &RecordedRun, _evidence: &RecordingEvidence) -> Result<(), Failure> {
        self.state.call(format!("retain {}", run.recording_id));
        Ok(())
    }
}

/// The loop's live sample (`ports.sample()`): always the one given.
pub(crate) struct Sampler(pub(crate) Option<ExperimentSample>);

impl LoopSampler for Sampler {
    fn sample(&mut self) -> Result<Option<ExperimentSample>, Failure> {
        Ok(self.0.clone())
    }
}

/// The evaluation of a connected run (stage V3): it passes, as the `TypeScript` tests' evaluation of a complete recording does.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Verdict;

impl RunEvaluation for Verdict {
    fn status(&self) -> RequirementStatus {
        RequirementStatus::Pass
    }
}

/// The evaluator of the tests: every run passes.
#[allow(clippy::unnecessary_wraps)] // It stands in for the evaluator callback, which returns a Result.
pub(crate) fn passes(_run: &ValidationRun, _checks: &[BoundsCheck]) -> Result<Verdict, Failure> {
    Ok(Verdict)
}

/// Runs one connected run on the fake runtime of `state` (`ConnectedIdentification.run`, or `.recordValidation` for the
/// independent model check): the tank's live sample in Auto, the loop and the evaluation of `request`, the journal `journal`
/// and the study `retention`. The operation is not cancelled.
pub(crate) fn run_tank<K, E>(
    state: &Arc<FakeState>,
    journal: &Mutex<Journal>,
    retention: &mut K,
    request: &IdentifyRequest<'_, E>,
    purpose: RunPurpose,
) -> Result<IdentifyOutcome<E>, Failure>
where
    K: RecordingRetention,
    E: RunEvaluation + Clone,
{
    run_tank_with_sample(
        state,
        journal,
        retention,
        request,
        purpose,
        Some(live_sample()),
    )
}

/// Runs the tank as [`run_tank`] does, with the loop's live sample given: `None` when no fresh synchronized sample is available,
/// or a sample of another mode, for a test of what a run refuses.
pub(crate) fn run_tank_with_sample<K, E>(
    state: &Arc<FakeState>,
    journal: &Mutex<Journal>,
    retention: &mut K,
    request: &IdentifyRequest<'_, E>,
    purpose: RunPurpose,
    sample: Option<ExperimentSample>,
) -> Result<IdentifyOutcome<E>, Failure>
where
    K: RecordingRetention,
    E: RunEvaluation + Clone,
{
    let runtime = FakeRuntime::new(state);
    let clock = FakeClock::new(state);
    let report = FakeReport::new(state, false);
    let mut sampler = Sampler(sample);
    super::run::run(
        RunPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention,
            clock: &clock,
            report: &report,
            journal,
        },
        request,
        purpose,
    )
}

/// The number a plan's value holds; a Boolean is 0 or 1.
fn number(value: ExperimentValue) -> f64 {
    match value {
        ExperimentValue::Number(number) => number,
        ExperimentValue::Boolean(boolean) => f64::from(u8::from(boolean)),
    }
}

/// A plan's whole number of ms.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A test plan's duration is a whole, positive number of ms.
fn whole_ms(milliseconds: f64) -> u64 {
    milliseconds as u64
}

/// A time of the test clock in ms, as a float.
#[allow(clippy::cast_precision_loss)] // The test clock stays far below 2⁵³ ms.
fn as_f64(milliseconds: u64) -> f64 {
    milliseconds as f64
}

/// A time of the test clock in ms, in seconds.
fn seconds(milliseconds: u64) -> f64 {
    as_f64(milliseconds) / 1_000.0
}
