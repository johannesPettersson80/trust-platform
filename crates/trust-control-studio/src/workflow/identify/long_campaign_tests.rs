//! Tests of the connected run on a long process (`control-studio-long-process-campaign.test.ts`, tests 13 to 18 and 20), against
//! the fake runtime of `identify::test_support`, whose answers carry exactly the fields annex E names and the identifiers the
//! code under test started.
//!
//! The `TypeScript` tests stubbed the fit. Here the run fits with the fit of the product. Their recording holds the plant input
//! at 30 % while the setpoint steps, and a closed-loop record is fitted from the recorded plant input (B3.5), so the fit of the
//! one kept record finds no model; the tests check that answer where the `TypeScript` tests checked a stubbed model. The record's
//! evaluation (stage V3) is the passing evaluator of the test support. Test 15, whose method needs the other mode, runs on the
//! transfer fake of `identify_mode::test_support` as well, which receives the mode transfers. Test 19, the reattachment after a
//! restart, is the recovery's (`identify_recovery_tests.rs`).

use std::cell::RefCell;
use std::fs;
use std::sync::Arc;

use trust_control::experiment::{ControlExperimentKind, ControllerMode, ExperimentValue};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::{require_campaign_limits, SILENT_RUNTIME_TEXT};
use trust_control::identification::connected_record::{
    BoundsCheck, ValidationRun, ValidationSample,
};
use trust_control::identification::connected_run::{CHECKING_TEXT, FITTING_TEXT};
use trust_control::identification::recorded_limits::evaluate_recorded_limits;
use trust_control::loop_definition::{ExperimentKind, LoopDefinition};

use super::test_support::{
    assert_no_model_of_a_held_input, empty_journal, folder, live_sample, passes, reopened,
    run_tank, run_tank_with_sample, tank_loop, whole_text, FakeState, StartBehaviour, Study,
    Verdict,
};
use super::{IdentifyOutcome, IdentifyRequest, IdentifyResult, RunEvaluator, RunPurpose};
use crate::journal::JournalEntry;
use crate::workflow::identify_mode::test_support::{
    pid_tank_loop, run_with_transfers, TransferFake,
};

/// The tank's measured output, by symbol.
const MEASURED: &str = "Main.TT101.Value";

/// Runs a campaign of `loop_definition` on `state` for `purpose`, journaled in a folder of its own named `name`, with its
/// record judged by `evaluate`, and returns what it gave. Every campaign of these tests settles: once it has ended, nothing is
/// left journaled (A8.5, E11.6).
fn settled_campaign(
    state: &Arc<FakeState>,
    name: &str,
    loop_definition: &LoopDefinition,
    purpose: RunPurpose,
    evaluate: RunEvaluator<'_, Verdict>,
) -> Result<IdentifyOutcome<Verdict>, Failure> {
    let folder = folder(name);
    let journal = empty_journal(&folder);
    let mut study = Study::new(state);
    let request = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: None,
        evaluate,
    };
    let outcome = run_tank(state, &journal, &mut study, &request, purpose);
    assert_eq!(
        reopened(&folder),
        Vec::<JournalEntry>::new(),
        "nothing is left owned once the campaign has ended"
    );
    let _ = fs::remove_dir_all(&folder);
    outcome
}

/// Whether the fit was reported exactly once, after the recording was released: the run fitted the record of its kept
/// recording, once (`fit` after `completeRecording`).
fn fitted_once_after_release(state: &FakeState) -> bool {
    let calls = state.calls();
    let fitting = format!("progress {FITTING_TEXT}");
    let released = calls
        .iter()
        .position(|call| call.starts_with("recording.release"));
    let fits: Vec<usize> = calls
        .iter()
        .enumerate()
        .filter_map(|(index, call)| (*call == fitting).then_some(index))
        .collect();
    matches!((released, fits.as_slice()), (Some(released), [fitted]) if released < *fitted)
}

/// Test 13: a loop in Auto with a setpoint is identified by a setpoint step in Auto, so its mode is not changed, and the step
/// is finished once the response has settled before the hold ended (B3.3, B3.4 step 4).
#[test]
fn a_loop_in_auto_with_a_setpoint_steps_its_setpoint_without_a_mode_change_and_finishes_the_step_once_settled(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let Err(failure) = settled_campaign(
        &state,
        "long-auto-setpoint",
        &tank_loop("rev-1"),
        RunPurpose::Identify,
        &passes,
    ) else {
        panic!("the fit of the held plant input finds no model");
    };
    let started = state.started_plans();
    assert_eq!(started.len(), 1, "the step is sent once");
    assert_eq!(
        started[0].kind,
        ControlExperimentKind::SetpointStep,
        "a setpoint step in Auto keeps the loop in Auto"
    );
    assert_eq!(
        state.count("experiment.finish"),
        1,
        "the response settled before the hold ended, so the step is finished once"
    );
    assert_eq!(
        state.experiment(),
        Some(("completed", Some("finished_early")))
    );
    assert!(
        fitted_once_after_release(&state),
        "the kept record is fitted once"
    );
    assert_no_model_of_a_held_input(&failure);
}

/// Test 14: a response that never settles is held to the end of its hold, which the runtime ends by its duration; nothing
/// finishes it early (B3.4 step 4, A10.2).
#[test]
fn a_response_that_never_settles_runs_to_its_maximum_duration_without_an_early_finish() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.ramp_while_stepped(0.01);
    let Err(failure) = settled_campaign(
        &state,
        "long-never-settles",
        &tank_loop("rev-1"),
        RunPurpose::Identify,
        &passes,
    ) else {
        panic!("the fit of the held plant input finds no model");
    };
    assert_eq!(
        state.count("experiment.finish"),
        0,
        "nothing finishes a response that never settles"
    );
    assert_eq!(
        state.experiment(),
        Some(("completed", Some("duration_complete"))),
        "the runtime ends the hold by its duration"
    );
    assert!(
        fitted_once_after_release(&state),
        "the kept record is fitted once"
    );
    assert_no_model_of_a_held_input(&failure);
}

/// Test 15: an explicit plant-input method on a loop in Auto switches to Manual and returns to Auto (B3.4 step 1, A6.2). The
/// `TypeScript` test recorded the calls `enter-manual` and `restore` of a mode double; here the transfer fake receives the
/// transfers: the loop enters Manual before the recording starts, the step runs in Manual, and once the recording is kept the
/// loop returns to Auto, its commands written back in Manual first. Nothing is left owned once the campaign has ended.
#[test]
fn an_explicit_plant_input_method_on_a_loop_in_auto_switches_to_manual_and_returns_to_auto() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let transfers = TransferFake::start(true);
    let folder = folder("long-explicit-plant-input");
    let journal = empty_journal(&folder);
    let explicit = pid_tank_loop("rev-1", false);
    // The fit of the campaign's record is the other tests'; the mode returns whatever the campaign gave.
    let _ = run_with_transfers(&state, &transfers, &journal, &explicit, false);
    assert_eq!(
        transfers.events(),
        [
            "transfer.apply tank:identification-mode manual",
            "recording.start",
            "retain",
            "transfer.apply tank:identification-mode:commands manual",
            "transfer.apply tank:identification-mode auto"
        ],
        "the loop is in Manual for the step and back in Auto afterwards"
    );
    assert_eq!(
        state.started_plans().first().map(|plan| plan.kind),
        Some(ControlExperimentKind::MvStep)
    );
    assert!(transfers.auto(), "the loop is back in Auto");
    assert_eq!(
        reopened(&folder),
        Vec::<JournalEntry>::new(),
        "nothing is left owned once the campaign has ended"
    );
    let _ = fs::remove_dir_all(&folder);
}

/// Test 16: the independent model check of a loop in Auto records a setpoint step of the opposite sign, returns its record and
/// does not fit it (B5.2).
#[test]
fn the_independent_model_check_records_the_opposite_excitation_and_does_not_fit() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let outcome = settled_campaign(
        &state,
        "long-model-check",
        &tank_loop("rev-1"),
        RunPurpose::RecordValidation,
        &passes,
    )
    .expect("the check returns its recording");
    let IdentifyResult::Recorded(record) = &outcome.result else {
        panic!("the check is recorded, not fitted: {:?}", outcome.result);
    };
    let plan = &record.request.experiment;
    assert_eq!(
        state.started_plans(),
        std::slice::from_ref(plan),
        "the record is of the one step the runtime started"
    );
    assert_eq!(plan.kind, ControlExperimentKind::SetpointStep);
    let (ExperimentValue::Number(to), ExperimentValue::Number(restored)) =
        (plan.to, plan.restoration_value)
    else {
        panic!("a setpoint step moves a number: {plan:?}");
    };
    assert!(
        to < restored,
        "a setpoint step of the opposite sign to the identification step: from {restored} to {to}"
    );
    let recordings = state.recording_starts();
    assert_eq!(recordings.len(), 1, "one recording is started");
    assert_eq!(
        record.run.id, recordings[0].recording_id,
        "the check returns its recording"
    );
    assert_eq!(outcome.recording_id, recordings[0].recording_id);
    assert_ne!(
        record.run.samples.len(),
        0,
        "the recording holds the response"
    );
    let fitting = format!("progress {FITTING_TEXT}");
    assert_eq!(
        state
            .calls()
            .iter()
            .filter(|call| **call == fitting)
            .count(),
        0,
        "the check is not fitted"
    );
    assert!(
        state.calls().contains(&format!("progress {CHECKING_TEXT}")),
        "the operation says that it checks the model"
    );
}

/// Test 17: a campaign whose runtime sends no sample is stopped by the connection-loss timeout of A7 before any experiment
/// starts. Its recording is stopped and kept, then released, and its journal is cleared, since nothing of it is left running
/// (A3.4, A6.2, E11.6). The `TypeScript` test also read the attempt's status from the panel's snapshot, which the language
/// server does not keep: the run's failure is that status.
#[test]
fn the_five_second_supervision_rule_stops_a_campaign_that_receives_no_samples_and_keeps_its_recording(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.silence_follows();
    let Err(failure) = settled_campaign(
        &state,
        "long-silent",
        &tank_loop("rev-1"),
        RunPurpose::Identify,
        &passes,
    ) else {
        panic!("the runtime sends no samples");
    };
    assert_eq!(whole_text(&failure), SILENT_RUNTIME_TEXT);
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::ConnectRuntime)
    );
    assert_eq!(
        state.count("experiment.start"),
        0,
        "no experiment was started"
    );
    assert_eq!(
        state.operations(),
        [
            "recording.start",
            "recording.stop",
            "retain",
            "recording.release"
        ],
        "the recording is stopped and kept once, then released"
    );
    assert!(state.now_ms() > 5_000, "the five-second limit had passed");
}

/// Test 18, the run: the plant input limits and the maximum duration must be set before a campaign records anything. Each
/// refusal names what is missing and opens Limits (B3.4 step 1, B3.9).
#[test]
fn a_plant_input_limit_or_a_maximum_duration_that_is_not_set_is_refused_before_anything_is_recorded(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let mut unset_limit = tank_loop("rev-1");
    unset_limit.limits.output_maximum = Some(f64::NAN);
    let Err(failure) = settled_campaign(
        &state,
        "long-unset-limit",
        &unset_limit,
        RunPurpose::Identify,
        &passes,
    ) else {
        panic!("the plant input limits are required");
    };
    assert_eq!(
        whole_text(&failure),
        "The plant input limits are missing: limits.outputMinimum and limits.outputMaximum must be set, with the minimum below the maximum."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    let mut no_duration = tank_loop("rev-1");
    no_duration.experiment = None;
    let Err(failure) = settled_campaign(
        &state,
        "long-no-duration",
        &no_duration,
        RunPurpose::Identify,
        &passes,
    ) else {
        panic!("the maximum experiment duration is required");
    };
    assert_eq!(
        whole_text(&failure),
        "The maximum experiment duration is missing: experiment.durationSeconds must be set, between 1 s and 24 h."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    assert_eq!(
        state.operations(),
        Vec::<String>::new(),
        "nothing is recorded or written"
    );
}

/// Test 18, the loop file (B3.4 step 1, B3.9): the `TypeScript` test handed the run a loop without its output maximum or without
/// its experiment duration. Both are optional in the file, so such a loop loads; Identify then refuses it at the missing field,
/// with Limits as the next action.
#[test]
fn a_loop_file_without_its_output_maximum_or_its_duration_loads_and_names_the_missing_field() {
    let tank = serde_json::to_value(tank_loop("rev-1")).expect("the tank encodes");
    for (part, field, sentence) in [
        (
            "limits",
            "outputMaximum",
            "The plant input limits are missing: limits.outputMaximum must be set, with the minimum below the maximum.",
        ),
        (
            "experiment",
            "durationSeconds",
            "The maximum experiment duration is missing: experiment.durationSeconds must be set, between 1 s and 24 h.",
        ),
    ] {
        let mut missing = tank.clone();
        // The fixture's revision is a label; a loop file carries a `sha256:` revision, which decoding requires.
        missing["revision"] = serde_json::Value::from("sha256:tank");
        let removed = missing[part]
            .as_object_mut()
            .expect("the tank's part is an object")
            .remove(field);
        assert!(removed.is_some(), "the tank declares {part}.{field}");
        let Ok(loaded) = LoopDefinition::decode(&missing.to_string()) else {
            panic!("a loop without {part}.{field} loads (B3.4 step 1)");
        };
        let Err(refusal) = require_campaign_limits(&loaded) else {
            panic!("a loop without {part}.{field} is refused before a campaign");
        };
        assert_eq!(whole_text(&refusal), sentence);
        assert_eq!(
            (refusal.cause, refusal.next_action),
            (Cause::Settings, NextAction::OpenLimits)
        );
    }
}

/// Test 20, the campaign: an output still dying away from an earlier test delays the step until it is steady, and the wait
/// never shortens the step: a 600 s maximum and a 30 s window (5 % of it) hold the step 570 s. The recording covers the
/// baseline budget (the longer of 5 minutes and 20 % of the maximum) and then the maximum, with the record period of a tenth
/// of the maximum, as the settling time is not known yet; the record handed to the fit keeps the whole recording, which still
/// begins in the tail (B3.4, A4.2).
#[test]
fn a_long_wait_for_a_steady_start_never_shortens_the_step_and_the_recording_covers_the_wait_and_the_maximum(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.decaying_offset(4.0, 40.0);
    let evaluated = RefCell::new(None);
    let capture = |run: &ValidationRun, _checks: &[BoundsCheck]| -> Result<Verdict, Failure> {
        *evaluated.borrow_mut() = Some(run.clone());
        Ok(Verdict)
    };
    let Err(failure) = settled_campaign(
        &state,
        "long-wait",
        &tank_loop("rev-1"),
        RunPurpose::Identify,
        &capture,
    ) else {
        panic!("the fit of the held plant input finds no model");
    };
    assert_no_model_of_a_held_input(&failure);
    let started = state.started_plans();
    assert_eq!(
        started.len(),
        1,
        "the step is sent once the output is steady"
    );
    let step_seconds = state.step_start_seconds();
    assert!(
        step_seconds > 60.0,
        "the step waited for the decaying output to settle: it started at {step_seconds} s"
    );
    assert!(
        (started[0].duration_ms - 570_000.0).abs() <= 1e-9,
        "the step holds 570 s however long the wait took: {} ms",
        started[0].duration_ms
    );
    let recordings = state.recording_starts();
    assert_eq!(recordings.len(), 1, "one recording is started");
    assert_eq!(
        recordings[0].planned_duration_ms,
        300_000 + 600_000,
        "the baseline budget, then the maximum duration"
    );
    assert_eq!(
        recordings[0].expected_settling_ms,
        Some(60_000),
        "the record period follows a tenth of the maximum duration"
    );
    let run = evaluated
        .take()
        .expect("the record of the kept recording was evaluated");
    let first = run.samples.first().expect("the record holds samples");
    let measured = first.values[MEASURED]
        .value
        .expect("the first sample measures the output");
    assert!(
        measured > 48.0,
        "the record keeps the whole recording, which still begins in the tail: {measured}"
    );
}

/// Test 20, the record: the record names where the steady window starts, one 30 s window before the step, so the fit starts
/// there and the tail, 49 degrees at the start, never sets the operating point, while the record keeps the whole recording
/// (B3.4, N1.3). The `TypeScript` test read the record from its stubbed fit; the fit of an identification consumes its record,
/// so the window is read from the independent model check of the same campaign, whose record is built the same way and
/// returned.
#[test]
fn the_record_of_a_campaign_that_waited_starts_its_fit_one_window_before_the_step() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.decaying_offset(4.0, 40.0);
    let outcome = settled_campaign(
        &state,
        "long-wait-record",
        &tank_loop("rev-1"),
        RunPurpose::RecordValidation,
        &passes,
    )
    .expect("the check returns its record");
    let IdentifyResult::Recorded(record) = &outcome.result else {
        panic!("the check is recorded, not fitted: {:?}", outcome.result);
    };
    let step_seconds = state.step_start_seconds();
    let window_start = record
        .request
        .baseline_start_seconds
        .expect("the steady window starts after the recording");
    assert!(
        (window_start - (step_seconds - 30.0)).abs() <= 1.5,
        "the steady window starts one window before the step: {window_start} s, step at {step_seconds} s"
    );
    let measured = |sample: &ValidationSample| {
        sample.values[MEASURED]
            .value
            .expect("the sample measures the output")
    };
    let first = record
        .run
        .samples
        .first()
        .expect("the record holds samples");
    assert!(
        measured(first) > 48.0,
        "the record keeps the whole recording, which still begins in the tail"
    );
    let fitted = record
        .run
        .samples
        .iter()
        .find(|sample| sample.time_seconds >= window_start)
        .expect("a sample lies in the steady window");
    assert!(
        measured(fitted) < 45.6,
        "the fit starts in the steady window, not in the tail: {}",
        measured(fitted)
    );
}

/// Identify cannot tell whether a step is safe without the loop's mode: an available sample whose mode cannot be read refuses the
/// run before anything is recorded or written, and says to wait (B3.4 step 1, B3.9).
#[test]
fn an_unreadable_controller_mode_is_refused_before_anything_is_recorded_or_written() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let mut sample = live_sample();
    sample.controller_mode = ControllerMode::Unavailable;
    let loop_definition = tank_loop("rev-1");
    let folder = folder("long-unreadable-mode");
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank_with_sample(
        &state,
        &journal,
        &mut study,
        &request,
        RunPurpose::Identify,
        Some(sample),
    );
    let _ = fs::remove_dir_all(&folder);
    let Err(failure) = outcome else {
        panic!("an unreadable mode refuses the run");
    };
    assert_eq!(
        failure.message,
        "Identify cannot read the loop's mode: its mode signal is stale or bad, so it cannot tell whether the step is safe."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::Wait)
    );
    assert_eq!(
        state.calls(),
        Vec::<String>::new(),
        "nothing is recorded, started, kept or reported"
    );
}

/// The limits check of the connected record (B3.4 step 6, B3.9): a measured output that leaves its permitted range stops the
/// record. The recording is kept and is not qualified for a fit, nothing is fitted, and the refusal names the signal, the time, the
/// value and the bound.
#[test]
fn a_measured_output_that_leaves_its_range_keeps_the_recording_marks_it_not_qualified_and_fits_nothing(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    // The fake moves the measurement only through its plant model: a ramp of 5 degC/s while the step is held takes it from
    // 45 degC past its 100 degC maximum within the hold.
    state.ramp_while_stepped(5.0);
    let mut explicit = tank_loop("rev-1");
    let experiment = explicit
        .experiment
        .as_mut()
        .expect("the tank declares its experiment");
    experiment.kind = ExperimentKind::MvStep;
    experiment.amplitude = 65.0;
    let mut sample = live_sample();
    sample.controller_mode = ControllerMode::Manual;
    let folder = folder("long-measured-output-limit");
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let request = IdentifyRequest {
        loop_definition: &explicit,
        expected_settling_seconds: None,
        evaluate: &evaluate_recorded_limits,
    };
    let outcome = run_tank_with_sample(
        &state,
        &journal,
        &mut study,
        &request,
        RunPurpose::Identify,
        Some(sample),
    );
    let _ = fs::remove_dir_all(&folder);
    let Err(failure) = outcome else {
        panic!("a measured output above its range identifies nothing");
    };
    let text = failure
        .detail
        .as_deref()
        .unwrap_or(failure.message.as_str());
    assert!(
        text.starts_with(
            "The experiment stopped at a limit or with missing samples. Main.TT101.Value is "
        ),
        "{text}"
    );
    assert!(
        text.contains(" degC at ") && text.contains("above its maximum of 100 degC."),
        "{text}"
    );
    assert!(
        text.ends_with("The recording is kept and is not qualified for a fit."),
        "{text}"
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Unclassified, NextAction::None)
    );
    let calls = state.calls();
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.starts_with("retain"))
            .count(),
        1,
        "the recording is kept once"
    );
    assert!(
        !calls.iter().any(|call| call.contains(FITTING_TEXT)),
        "nothing is fitted from the recording"
    );
}
