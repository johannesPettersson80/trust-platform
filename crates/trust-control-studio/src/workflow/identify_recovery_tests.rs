//! Tests of the reconciliation of an unknown identification start and of the recovery of an interrupted identification
//! (`control-studio-identification-recovery.test.ts`, and what `control-studio-lifecycle.test.ts` tests of a reopened
//! campaign), against the fake runtime of `identify::test_support`, whose answers carry exactly the fields annex E names and
//! the identifiers the code under test started or journaled.
//!
//! The `TypeScript` tests stubbed the fit; here the run and the resume fit with the fit of the product. The `TypeScript` tests'
//! recording holds the plant input at 30 % while the setpoint steps, so the fit of the one kept record finds no model; the
//! tests check that the fit ran on that record after it was kept and released, and give its answer. The mode transfer moves
//! with V4, so the `TypeScript` tests' mode calls are not checked.

use std::fs;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::json;
use trust_control::experiment::{
    ControlExperimentKind, ControlExperimentPlan, ControllerMode, ExperimentSample,
    PlanAuxiliarySignal, PlanSignal,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::journal::NO_RUNNING_EXPERIMENT_TEXT;
use trust_control::identification::connected_run::{identification_cancelled, FITTING_TEXT};
use trust_control::identification::texts::MISSING_NONZERO_STEP;
use trust_control::loop_definition::{ExperimentKind, LoopDefinition};
use trust_control::signals::Quality;

use super::{cancel_recovery, resume, RecoveryPorts, ResumedIdentification, RUNNING_TEXT};
use crate::journal::{Journal, JournalEntry};
use crate::workflow::identify::run::{run, RunPorts};
use crate::workflow::identify::test_support::{
    cancel_journaled, empty_journal, folder, journaled_baseline, journaled_step, live_sample,
    passes, plan, reopened, tank_loop, whole_text, FakeClock, FakeReport, FakeRuntime, FakeState,
    Sampler, StartBehaviour, Study, Verdict,
};
use crate::workflow::identify::{IdentifyOutcome, IdentifyRequest, RunPurpose};

/// Runs an identification of the tank on `state`, journaled in `journal` (`ConnectedIdentification.run`).
fn identify(
    state: &Arc<FakeState>,
    journal: &Mutex<Journal>,
) -> Result<IdentifyOutcome<Verdict>, Failure> {
    let loop_definition = tank_loop("rev-1");
    let runtime = FakeRuntime::new(state);
    let clock = FakeClock::new(state);
    let report = FakeReport::new(state, false);
    let mut sampler = Sampler(Some(live_sample()));
    let mut study = Study::new(state);
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
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
            journal,
        },
        &request,
        RunPurpose::Identify,
    )
}

/// Resumes the campaign journaled in `journal` for the tank at `revision` on `state` (`IdentificationRecovery.resume`). The
/// operation is cancelled from the start when `cancelled` says so.
fn resume_tank(
    state: &Arc<FakeState>,
    journal: &Mutex<Journal>,
    revision: &str,
    cancelled: bool,
) -> Result<ResumedIdentification<Verdict>, Failure> {
    let loop_definition = tank_loop(revision);
    let runtime = FakeRuntime::new(state);
    let clock = FakeClock::new(state);
    let report = FakeReport::new(state, cancelled);
    let mut sampler = Sampler(Some(live_sample()));
    let mut study = Study::new(state);
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    resume(
        RecoveryPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention: &mut study,
            clock: &clock,
            report: &report,
            journal,
        },
        &request,
    )
}

/// The tank with a second plant input (C7, C8): a measured load (`P1.LoadKw`, 0 to 50 kW) that is stepped through its own Manual command
/// with an identification step of 10, beside the tank's valve (`Main.TIC101_Output`). The loop declares an explicit output step.
fn two_input_tank(revision: &str) -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": revision,
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

/// Resumes the campaign journaled in `journal` for the two-input tank at `revision` on `state`, with the live sample `sample`.
fn resume_two_inputs(
    state: &Arc<FakeState>,
    journal: &Mutex<Journal>,
    sample: ExperimentSample,
) -> Result<ResumedIdentification<Verdict>, Failure> {
    resume_loop(state, journal, &two_input_tank("rev-1"), sample)
}

/// Resumes the campaign journaled in `journal` for the two-input `loop_definition` on `state`, with the live sample `sample`.
fn resume_loop(
    state: &Arc<FakeState>,
    journal: &Mutex<Journal>,
    loop_definition: &LoopDefinition,
    sample: ExperimentSample,
) -> Result<ResumedIdentification<Verdict>, Failure> {
    let runtime = FakeRuntime::new(state);
    let clock = FakeClock::new(state);
    let report = FakeReport::new(state, false);
    let mut sampler = Sampler(Some(sample));
    let mut study = Study::new(state);
    let request = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    resume(
        RecoveryPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention: &mut study,
            clock: &clock,
            report: &report,
            journal,
        },
        &request,
    )
}

/// Cancels the recovery of the campaign journaled in `journal` for the tank at `revision` on `state`, finished by the
/// identification's Cancel (`IdentificationRecovery.cancel`).
fn cancel_tank(
    state: &Arc<FakeState>,
    journal: &Mutex<Journal>,
    revision: &str,
) -> Result<(), Failure> {
    let loop_definition = tank_loop(revision);
    let runtime = FakeRuntime::new(state);
    let clock = FakeClock::new(state);
    let report = FakeReport::new(state, false);
    let mut study = Study::new(state);
    cancel_recovery(&runtime, &clock, &report, journal, || {
        cancel_journaled(&runtime, &clock, journal, &loop_definition, &mut study)
    })
}

/// Whether the fit was reported after the recording was released, so it fitted the record of the kept recording (`fit`
/// after `completeRecording`).
fn fitted_after_release(state: &FakeState) -> bool {
    let calls = state.calls();
    let fitting = format!("progress {FITTING_TEXT}");
    let released = calls
        .iter()
        .position(|call| call.starts_with("recording.release"));
    let fitted = calls.iter().position(|call| *call == fitting);
    matches!((released, fitted), (Some(released), Some(fitted)) if released < fitted)
}

/// How many fits the operation reported. `fit_records` reports its progress once per call, so a run that fits its one kept
/// record reports it once.
fn fits_reported(state: &FakeState) -> usize {
    let fitting = format!("progress {FITTING_TEXT}");
    state
        .calls()
        .iter()
        .filter(|call| **call == fitting)
        .count()
}

/// The answer of the fit to the `TypeScript` tests' recording, whose plant input stays at 30 %: the excitation fit of the one
/// kept record refuses it as the missing nonzero step (N2.2, N2.6), so another method is the fix (B3.9).
fn assert_no_model_of_a_constant_input(failure: &Failure) {
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Unclassified, NextAction::ChooseMethod),
        "{failure:?}"
    );
    assert_eq!(
        failure.message,
        "The recording is missing a nonzero step: the plant input never changes, so there is no input change to fit.",
        "{failure:?}"
    );
    assert_eq!(failure.detail.as_deref(), Some(MISSING_NONZERO_STEP));
}

#[test]
fn a_start_the_runtime_holds_as_running_is_followed_to_its_end_without_a_second_start() {
    let state = FakeState::new(StartBehaviour::Running, 0);
    let folder = folder("start-running");
    let Err(failure) = identify(&state, &empty_journal(&folder)) else {
        panic!("the fit of a constant plant input finds no model");
    };
    assert_eq!(state.count("experiment.start"), 1, "the start is sent once");
    assert_eq!(state.count("experiment.abort"), 0, "nothing is aborted");
    assert!(
        fitted_after_release(&state),
        "the followed experiment is fitted: {:?}",
        state.calls()
    );
    assert_no_model_of_a_constant_input(&failure);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_start_the_runtime_holds_as_already_ended_is_concluded_and_its_journal_cleared_with_no_second_start(
) {
    let state = FakeState::new(StartBehaviour::Terminal, 0);
    let folder = folder("start-terminal");
    let Err(failure) = identify(&state, &empty_journal(&folder)) else {
        panic!("the fit of a constant plant input finds no model");
    };
    assert_eq!(state.count("experiment.start"), 1, "the start is sent once");
    assert_eq!(state.count("experiment.abort"), 0, "nothing is aborted");
    assert!(
        fitted_after_release(&state),
        "the ended experiment is concluded and its record fitted: {:?}",
        state.calls()
    );
    assert_no_model_of_a_constant_input(&failure);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_completed_identification_keeps_its_recording_then_releases_the_runtime_buffer_once() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let folder = folder("keep-before-release");
    let Err(failure) = identify(&state, &empty_journal(&folder)) else {
        panic!("the fit of a constant plant input finds no model");
    };
    let kept: Vec<String> = state
        .operations()
        .into_iter()
        .filter(|operation| {
            ["recording.stop", "retain", "recording.release"].contains(&operation.as_str())
        })
        .collect();
    assert_eq!(
        kept,
        ["recording.stop", "retain", "recording.release"],
        "the samples are kept before the buffer is released, exactly once"
    );
    let fitting = format!("progress {FITTING_TEXT}");
    assert_eq!(
        state
            .calls()
            .iter()
            .filter(|call| **call == fitting)
            .count(),
        1,
        "the kept record is fitted once"
    );
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    assert_no_model_of_a_constant_input(&failure);
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn reopening_reattaches_a_running_experiment_finishes_it_once_settled_then_fits_and_never_aborts_it(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-running");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let Err(failure) = resume_tank(&state, &journal, "rev-1", false) else {
        panic!("the fit of a constant plant input finds no model");
    };
    assert_eq!(
        state.count("experiment.finish"),
        1,
        "the settled step is finished once"
    );
    assert_eq!(state.count("experiment.abort"), 0, "nothing is aborted");
    assert_eq!(
        state.count("experiment.start"),
        0,
        "nothing is started on reopening"
    );
    assert_eq!(state.count("retain"), 1, "the recording is kept once");
    assert!(
        fitted_after_release(&state),
        "the reattached campaign is fitted: {:?}",
        state.calls()
    );
    assert_no_model_of_a_constant_input(&failure);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

/// The `TypeScript` tests closed the panel while the campaign was reattached, so its follower refused the next page. Closing
/// has no counterpart in the engine; a follow that fails is the same event for the campaign (A8.6, A10.4).
#[test]
fn a_reattached_campaign_that_stops_following_leaves_its_experiment_recording_and_journal_to_the_runtime(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    state.refuse_follows();
    let folder = folder("resume-released");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let Err(failure) = resume_tank(&state, &journal, "rev-1", false) else {
        panic!("the follow was refused");
    };
    assert_eq!(failure.message, "Control Studio cannot reach the runtime.");
    assert_eq!(
        state.operations(),
        ["experiment.status", "experiment.status"],
        "only status reads: nothing is aborted, finished, stopped or kept"
    );
    assert_eq!(
        state.experiment(),
        Some(("running", None)),
        "the runtime keeps the experiment"
    );
    assert_eq!(
        reopened(&folder).len(),
        2,
        "the journal stays for reopening"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn reopening_an_identification_whose_experiment_never_started_keeps_its_recording_and_clears_its_journal(
) {
    let state = FakeState::new(StartBehaviour::Absent, 30_000);
    let folder = folder("resume-absent");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let Err(failure) = resume_tank(&state, &journal, "rev-1", false) else {
        panic!("the experiment never ran");
    };
    assert_eq!(
        whole_text(&failure),
        "The interrupted experiment never started, so nothing was applied and the original command is restored. No experiment holds this identifier."
    );
    assert_eq!(
        state.operations(),
        [
            "experiment.status",
            "recording.stop",
            "retain",
            "recording.release"
        ],
        "no start is sent on reopening and nothing is aborted; the recording is kept"
    );
    assert_eq!(
        reopened(&folder),
        Vec::<JournalEntry>::new(),
        "nothing is left owned, so the recovery is complete"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn reopening_a_campaign_whose_experiment_has_ended_reads_it_by_its_identity_and_starts_or_aborts_nothing(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 600_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-ended");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let Err(failure) = resume_tank(&state, &journal, "rev-1", false) else {
        panic!("the fit of a constant plant input finds no model");
    };
    let writes: Vec<String> = state
        .operations()
        .into_iter()
        .filter(|operation| {
            operation.starts_with("experiment.") && operation != "experiment.status"
        })
        .collect();
    assert_eq!(
        writes,
        Vec::<String>::new(),
        "only status reads reach the experiment on reopening"
    );
    assert_eq!(
        state.experiment(),
        Some(("completed", Some("duration_complete")))
    );
    assert_eq!(
        state.count("retain"),
        1,
        "the journaled operation completes: its recording is kept"
    );
    assert_no_model_of_a_constant_input(&failure);
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_reattached_experiment_whose_status_cannot_be_read_keeps_its_journal_and_sends_nothing_further()
{
    let state = FakeState::new(StartBehaviour::Unreadable, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-unreadable");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let Err(failure) = resume_tank(&state, &journal, "rev-1", false) else {
        panic!("the status cannot be read");
    };
    assert_eq!(
        whole_text(&failure),
        "The status of experiment experiment-1 could not be read: transport lost. The experiment is left to the runtime; reopening reconciles it."
    );
    assert_eq!(
        state.operations(),
        ["experiment.status"],
        "no second start, no abort and no stop"
    );
    assert_eq!(
        reopened(&folder).len(),
        2,
        "the journal is kept for the next reconciliation"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn the_engineers_cancel_of_a_resumed_campaign_aborts_its_experiment_keeps_the_recording_and_clears_the_journal(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-cancelled");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let Err(failure) = resume_tank(&state, &journal, "rev-1", true) else {
        panic!("the experiment was aborted");
    };
    assert_eq!(
        failure,
        identification_cancelled(),
        "the hold the Cancel aborted ends as the cancellation, so the verb ends as interrupted (section 0.1b)"
    );
    assert_eq!(
        state.count("experiment.abort"),
        1,
        "the engineer's Cancel aborts through the runtime's restoration path"
    );
    assert_eq!(state.experiment(), Some(("aborted", Some("user_abort"))));
    assert_eq!(state.count("retain"), 1, "the recording is kept");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_journal_without_a_dispatched_step_of_this_loop_revision_has_no_running_experiment_to_resume() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    let none = folder("resume-none");
    let baseline_only = folder("resume-baseline");
    let other_revision = folder("resume-other-revision");
    let cases = [
        (empty_journal(&none), "rev-1"),
        (journaled_baseline(&baseline_only), "rev-1"),
        (
            journaled_step(&other_revision, &plan("experiment-1", 540_000), 30.0, 0),
            "rev-2",
        ),
    ];
    for (journal, revision) in &cases {
        let Err(failure) = resume_tank(&state, journal, revision, false) else {
            panic!("no step of this revision was dispatched");
        };
        assert_eq!(whole_text(&failure), NO_RUNNING_EXPERIMENT_TEXT);
        assert_eq!(failure.next_action, NextAction::Identify);
    }
    assert_eq!(
        state.operations(),
        Vec::<String>::new(),
        "nothing is read, started or stopped"
    );
    assert_eq!(
        reopened(&baseline_only).len(),
        1,
        "a journal that cannot be resumed is left for its Cancel"
    );
    let _ = fs::remove_dir_all(&none);
    fs::remove_dir_all(&baseline_only).expect("clean up");
    fs::remove_dir_all(&other_revision).expect("clean up");
}

// C7, C8: a resumed campaign that has not stepped every plant input follows its journaled step to its end, then steps the rest in the
// same recording. The load is stepped through its own Manual command, and the recording and the journal are then kept and cleared.
#[test]
fn a_resumed_campaign_continues_with_the_second_plant_input_in_the_same_recording() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-second-input");
    let step: ControlExperimentPlan = serde_json::from_value(json!({
        "id": "experiment-1", "source": "simulation", "kind": "mv_step",
        "affectedSignal": "Main.TIC101_Output", "valueType": "LREAL", "from": 30, "to": 40, "unit": "%",
        "minimum": 0, "maximum": 100, "durationMs": 540_000, "expectedScan": 300, "task": "$background",
        "stopConditions": ["Abort"], "restorationValue": 30, "requiresPhysicalApproval": false,
        "trajectory": [{ "atMs": 0, "value": 40 }]
    }))
    .expect("the valve's step");
    let journal = journaled_step(&folder, &step, 30.0, 1);
    // The live sample is in Manual, where an output step runs, and it measures the load.
    let mut sample = live_sample();
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
    // The fake recording holds no load values, so the fit of the two inputs refuses it, and the refusal names the load.
    let Err(failure) = resume_two_inputs(&state, &journal, sample) else {
        panic!("the fake recording holds no load to fit");
    };
    let started = state.started_plans();
    assert_eq!(
        started.len(),
        1,
        "the second input is started once, after the journaled step"
    );
    assert_eq!(
        started[0].affected_signal, "P1.LoadKw",
        "through its own Manual command"
    );
    assert_eq!(
        state.count("retain"),
        1,
        "the recording of both inputs is kept once"
    );
    assert_eq!(
        reopened(&folder),
        Vec::<JournalEntry>::new(),
        "the journal is cleared"
    );
    assert!(
        fitted_after_release(&state),
        "the fit reads the kept recording"
    );
    assert!(
        whole_text(&failure).contains("P1.LoadKw"),
        "the fit names the load it could not read: {failure:?}"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

// C7, C8: a resumed campaign plans its remaining input by the method Identify chose for the loop, not by the kind of the journaled step. The
// valve's own step is journaled as a manual-output step; the load has no step, so it takes Identify's choice in Manual, a PRBS under this
// policy.
#[test]
fn a_resumed_campaign_plans_its_remaining_input_by_the_method_identify_chose() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-identify-choice");
    let step: ControlExperimentPlan = serde_json::from_value(json!({
        "id": "experiment-1", "source": "simulation", "kind": "mv_step",
        "affectedSignal": "Main.TIC101_Output", "valueType": "LREAL", "from": 30, "to": 34, "unit": "%",
        "minimum": 0, "maximum": 100, "durationMs": 540_000, "expectedScan": 300, "task": "$background",
        "stopConditions": ["Abort"], "restorationValue": 30, "requiresPhysicalApproval": false,
        "trajectory": [{ "atMs": 0, "value": 34 }]
    }))
    .expect("the valve's own step");
    let journal = journaled_step(&folder, &step, 30.0, 1);
    let mut loop_definition = two_input_tank("rev-1");
    loop_definition.identification_policy = Some(json!({
        "openLoopPermitted": true, "sustainedHoldPermitted": true, "information": "frequency"
    }));
    if let Some(experiment) = loop_definition.experiment.as_mut() {
        experiment.kind = ExperimentKind::Observation;
        experiment.amplitude = 0.0;
    }
    if let Some(channels) = loop_definition.channels.as_mut() {
        channels.inputs[0].experiment_step = Some(4.0);
        channels.inputs[1].experiment_step = None;
    }
    // The live sample is in Manual, where the policy's frequency content asks for a PRBS, and it measures the load.
    let mut sample = live_sample();
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
    // The fake recording holds no load values, so the fit refuses it after the load's step; only the started steps are read here.
    let _ = resume_loop(&state, &journal, &loop_definition, sample);
    let started = state.started_plans();
    assert_eq!(
        started.len(),
        1,
        "the load is started once, after the journaled step"
    );
    assert_eq!(
        started[0].affected_signal, "P1.LoadKw",
        "through its own Manual command"
    );
    assert_eq!(
        started[0].kind,
        ControlExperimentKind::Prbs,
        "Identify's choice in Manual for the load, not the journaled step's kind"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_revision_mismatch_with_a_running_experiment_is_followed_and_restored_only_once_it_has_ended() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 10_000);
    state.restart("experiment-1", 0, 60_000, 2.0);
    let folder = folder("cancel-running");
    let journal = journaled_step(&folder, &plan("experiment-1", 60_000), 0.0, 0);
    cancel_tank(&state, &journal, "rev-2").expect("restored once the experiment has ended");
    assert!(
        state.calls().contains(&format!("progress {RUNNING_TEXT}")),
        "the operation says why it waits"
    );
    assert_eq!(
        state.experiment(),
        Some(("completed", Some("duration_complete"))),
        "the experiment ran to its end: nothing aborted or finished it"
    );
    assert!(
        state.now_ms() >= 60_000,
        "nothing is restored underneath a running experiment"
    );
    let operations = state.operations();
    assert!(
        operations.iter().all(|operation| !matches!(
            operation.as_str(),
            "experiment.start" | "experiment.abort" | "experiment.finish"
        )),
        "only status reads reach the experiment: {operations:?}"
    );
    assert_eq!(
        operations[operations.len() - 3..],
        ["recording.stop", "retain", "recording.release"],
        "then the original recording is stopped, kept and released"
    );
    assert_eq!(
        reopened(&folder),
        Vec::<JournalEntry>::new(),
        "nothing is left owned"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_cancelled_recovery_whose_experiment_status_cannot_be_read_restores_nothing() {
    let state = FakeState::new(StartBehaviour::Unreadable, 10_000);
    state.restart("experiment-1", 0, 60_000, 2.0);
    let folder = folder("cancel-unreadable");
    let journal = journaled_step(&folder, &plan("experiment-1", 60_000), 0.0, 0);
    let failure = cancel_tank(&state, &journal, "rev-1").expect_err("nothing can be restored");
    assert_eq!(
        whole_text(&failure),
        "The status of experiment experiment-1 could not be read: transport lost. Nothing was restored: reconnect the original simulator and try again."
    );
    assert_eq!(state.operations(), ["experiment.status"]);
    assert_eq!(reopened(&folder).len(), 2, "the journal stays");
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_cancelled_recovery_without_a_dispatched_step_keeps_its_recording_and_clears_the_journal_at_once(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 10_000);
    let folder = folder("cancel-baseline");
    let journal = journaled_baseline(&folder);
    cancel_tank(&state, &journal, "rev-1").expect("restored");
    assert_eq!(
        state.operations(),
        ["recording.stop", "retain", "recording.release"],
        "no experiment is read, started or aborted"
    );
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&folder).expect("clean up");
}

/// The `TypeScript` test of a followed start counts the fits of its run: its one kept record is fitted once. The fit's answer
/// to the constant plant input is not the subject of this test, so it is not read here.
#[test]
fn a_followed_start_is_fitted_once_on_its_kept_recording() {
    let state = FakeState::new(StartBehaviour::Running, 0);
    let folder = folder("start-running-fitted-once");
    let _ = identify(&state, &empty_journal(&folder));
    assert_eq!(
        fits_reported(&state),
        1,
        "the followed experiment's record is fitted once: {:?}",
        state.calls()
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

/// The `TypeScript` test of a reattached experiment counts its fits: the one kept record is fitted once, after the hold.
#[test]
fn a_reattached_experiment_is_fitted_once_on_its_kept_recording() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-running-fitted-once");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let _ = resume_tank(&state, &journal, "rev-1", false);
    assert_eq!(
        fits_reported(&state),
        1,
        "the reattached campaign's record is fitted once: {:?}",
        state.calls()
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

/// Reopening reads the journaled experiment by its identity, and starts neither a recording nor an experiment. The writes of
/// the experiment are checked by the test above. The `TypeScript` test of a reopened campaign reads the experiment and sends
/// nothing else, so the read and the absence of a recording start are checked here.
#[test]
fn reopening_a_campaign_whose_experiment_has_ended_starts_no_recording() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 600_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let folder = folder("resume-ended-no-recording");
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    let _ = resume_tank(&state, &journal, "rev-1", false);
    assert!(
        state.count("experiment.status") > 0,
        "the journaled experiment is read by its identity: {:?}",
        state.operations()
    );
    assert_eq!(
        state.count("recording.start"),
        0,
        "no recording is started on reopening: {:?}",
        state.operations()
    );
    assert_eq!(
        state.count("experiment.start"),
        0,
        "no experiment is started on reopening"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}
