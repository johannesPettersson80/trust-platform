//! The connected run of a project step test whose loop declares a hold parameter (specification 35, C12 Hold parameter), on the fake runtime
//! of `identify::test_support`. The run reads the hold that the runtime applies before it records anything, refuses it as the test's
//! review does, and plans the recording of that hold as its duration: the baseline, every scan of the hold and one restoration sample.
//! The connected runtime reads the hold from the runtime's parameter record.

#![allow(clippy::float_cmp)] // The plan's duration is a whole number of milliseconds, compared exactly on purpose.

use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::json;
use trust_control::experiment::{ControlExperimentPlan, RecordingRecoveryOwner};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_run::StartRead;
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;

use super::run::RunPorts;
use super::test_support::{
    empty_journal, folder, live_sample, passes, tank_loop, FakeClock, FakeExperiment,
    FakeRecording, FakeReport, FakeRuntime, FakeState, Sampler, StartBehaviour, Study, Verdict,
};
use super::{ConnectedRuntime, IdentifyOutcome, IdentifyRequest, IdentifyRuntime, RunPurpose};
use crate::runtime_client::fake_runtime::{answer, FakeRuntime as ControlEndpoint};
use crate::runtime_client::runtime_recording::{
    PendingReleases, RecordingOwnerPersistence, RecordingStartError, RecordingStartPlan,
};

const NOT_REPORTED: &str =
    "The runtime did not report P1.HoldSeconds; inspect and apply the experiment settings first.";
const OUTSIDE_BOUNDS: &str =
    "Apply a P1.HoldSeconds value within 20–40 s before reviewing the project test.";

/// The tank as a project step test (C12): the project holds its step for the applied `P1.HoldSeconds`, 20 to 40 s, after a 5 s baseline, in
/// a 100 ms loop whose declared maximum duration is `maximum_seconds`.
fn hold_tank(maximum_seconds: f64) -> LoopDefinition {
    let mut value = serde_json::to_value(tank_loop("rev-1")).expect("the tank encodes");
    value["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0,
        "durationSeconds": maximum_seconds, "seed": 1, "baselineSeconds": 5,
        "holdParameter": "P1.HoldSeconds",
        "activation": { "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" }
    });
    value["parameterPurpose"] = json!("experiment");
    value["parameterBinding"] = json!({
        "loop_id": "tank", "task_name": "MainTask",
        "parameters": [{
            "symbol": "P1.HoldSeconds", "iec_type": "REAL", "unit": "s",
            "convention": "held step duration", "minimum": 20, "maximum": 40
        }],
        "mode": { "symbol": "P1.Run", "manual_value": false }
    });
    serde_json::from_value(value).expect("the project step test decodes")
}

/// The fake runtime of the tank, which also reports the hold its parameter record holds (E7.2) and counts how often it is asked.
struct HoldRuntime {
    tank: FakeRuntime,
    hold: Result<Option<f64>, Failure>,
    reads: AtomicUsize,
}

impl HoldRuntime {
    /// The runtime of `state` that reports `hold`.
    fn new(state: &Arc<FakeState>, hold: Result<Option<f64>, Failure>) -> Self {
        Self {
            tank: FakeRuntime::new(state),
            hold,
            reads: AtomicUsize::new(0),
        }
    }
}

impl IdentifyRuntime for HoldRuntime {
    type Recording = FakeRecording;
    type Experiment = FakeExperiment;

    fn start_recording(
        &self,
        loop_definition: &LoopDefinition,
        plan: &RecordingStartPlan,
        persistence: &dyn RecordingOwnerPersistence,
    ) -> Result<FakeRecording, RecordingStartError<FakeRecording>> {
        self.tank
            .start_recording(loop_definition, plan, persistence)
    }

    fn recover_recording(
        &self,
        loop_definition: &LoopDefinition,
        owner: &RecordingRecoveryOwner,
    ) -> Result<FakeRecording, Failure> {
        self.tank.recover_recording(loop_definition, owner)
    }

    fn experiment(&self) -> FakeExperiment {
        self.tank.experiment()
    }

    fn start_experiment(&self, plan: &ControlExperimentPlan) -> StartRead {
        self.tank.start_experiment(plan)
    }

    fn experiment_status(&self, experiment_id: &str) -> StatusRead {
        self.tank.experiment_status(experiment_id)
    }

    fn applied_hold_seconds(
        &self,
        _loop_definition: &LoopDefinition,
    ) -> Result<Option<f64>, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.hold.clone()
    }
}

/// Runs the project step test of `loop_definition` on `runtime`, with the tank's live sample and a journal of its own in the folder `name`,
/// which is removed once the run has ended.
fn run_hold(
    runtime: &HoldRuntime,
    state: &Arc<FakeState>,
    loop_definition: &LoopDefinition,
    name: &str,
) -> Result<IdentifyOutcome<Verdict>, Failure> {
    let folder = folder(name);
    let journal = empty_journal(&folder);
    let mut study = Study::new(state);
    let request = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let clock = FakeClock::new(state);
    let report = FakeReport::new(state, false);
    let mut sampler = Sampler(Some(live_sample()));
    let outcome = super::run::run(
        RunPorts {
            runtime,
            sampler: &mut sampler,
            retention: &mut study,
            clock: &clock,
            report: &report,
            journal: &journal,
        },
        &request,
        RunPurpose::ProjectTest,
    );
    let _ = fs::remove_dir_all(&folder);
    outcome
}

/// The run of the tank, declared to record at most `maximum_seconds`, that `hold` leaves refused: its failure, and the runtime it ran on.
fn refused(
    hold: Result<Option<f64>, Failure>,
    maximum_seconds: f64,
    name: &str,
) -> (Failure, Arc<FakeState>) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let runtime = HoldRuntime::new(&state, hold);
    let failure = run_hold(&runtime, &state, &hold_tank(maximum_seconds), name)
        .expect_err("the hold is refused");
    (failure, state)
}

#[test]
fn the_run_plans_the_recording_of_the_applied_hold() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let runtime = HoldRuntime::new(&state, Ok(Some(30.0)));
    // How the whole campaign ends is not this test's: it reads the plan that the runtime was asked to start.
    let _ = run_hold(&runtime, &state, &hold_tank(600.0), "hold-plan");
    let plans = state.started_plans();
    assert_eq!(
        plans.len(),
        1,
        "the run starts its experiment once: {:?}",
        state.calls()
    );
    assert_eq!(
        plans[0].duration_ms, 35_100.0,
        "5 s + 30 s + 0.1 s, not the 600 s the loop may record"
    );
    assert_eq!(
        plans[0].stop_conditions[1], "35.1 s timeout",
        "the plan names the timeout the review names"
    );
    assert_eq!(
        runtime.reads.load(Ordering::SeqCst),
        1,
        "the hold is read once, before anything is recorded"
    );
}

#[test]
fn a_hold_that_cannot_be_recorded_is_refused_before_anything_is_recorded() {
    let unread = Failure::new(NOT_REPORTED, Cause::Runtime, NextAction::None);
    let (failure, state) = refused(Err(unread.clone()), 600.0, "hold-unread");
    assert_eq!(failure, unread, "the runtime's own reason stands");

    // A runtime that reports no hold at all, and one that reports a hold above its parameter's maximum.
    for (hold, name) in [(None, "hold-none"), (Some(41.0), "hold-above")] {
        let (failure, state) = refused(Ok(hold), 600.0, name);
        assert_eq!(failure.message, OUTSIDE_BOUNDS, "{hold:?}");
        assert_eq!(
            (failure.cause, failure.next_action),
            (Cause::Settings, NextAction::None),
            "{hold:?}"
        );
        assert_eq!(state.count("recording.start"), 0, "{hold:?}");
        assert!(state.started_plans().is_empty(), "{hold:?}");
    }

    // A 30 s hold needs 35.1 s, which a loop declared to record 30 s cannot.
    let (failure, longer) = refused(Ok(Some(30.0)), 30.0, "hold-exceeds");
    assert_eq!(
        failure.message,
        "A 30 s hold needs 35.1 s, more than the declared 30 s maximum."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    assert_eq!(longer.count("recording.start"), 0);
    assert_eq!(longer.started_plans().len(), 0, "no plan was started");

    assert_eq!(state.count("recording.start"), 0, "nothing was recorded");
    assert!(state.started_plans().is_empty(), "nothing was started");
}

#[test]
fn the_connected_runtime_reads_the_applied_hold_from_the_runtimes_parameter_record() {
    let endpoint = ControlEndpoint::start(|request| {
        Some(answer(
            request,
            json!({ "values": { "P1.HoldSeconds": 30.0, "P1.SelectedStepPoints": 10.0 } }),
        ))
    });
    let connection = endpoint.connection(None);
    let pending = Arc::new(PendingReleases::default());
    let connected = ConnectedRuntime::new(&connection, &pending);
    assert_eq!(
        connected
            .applied_hold_seconds(&hold_tank(600.0))
            .expect("the hold is read"),
        Some(30.0)
    );
    assert_eq!(endpoint.requests_of("control.parameters.status").len(), 1);
    // A loop that declares no hold parameter reads nothing.
    assert_eq!(
        connected
            .applied_hold_seconds(&tank_loop("rev-1"))
            .expect("no hold parameter, no read"),
        None
    );
    assert_eq!(
        endpoint.requests_of("control.parameters.status").len(),
        1,
        "nothing more was read"
    );
}
