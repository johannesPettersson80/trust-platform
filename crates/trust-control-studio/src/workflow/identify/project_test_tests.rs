//! The end of a project test whose fit its recording cannot support (C12, B3.5), on the fake runtime of `identify::test_support`: the
//! test finishes with its recording, its samples and its restoration, and the fit's whole refusal as its note, and it keeps no model.
//! The same campaign run as an Identify is still refused by its fit. A project test also records every execution of its hold, where a
//! campaign that steps the plant follows the process (A4.2), and it waits for no steady baseline: its recording starts with its plan,
//! which holds the activation off for the baseline the project declares, where a campaign waits for its steady baseline first (C12,
//! B3.4 step 2).

#![allow(clippy::float_cmp)] // The plans' times are whole milliseconds and the clock's whole seconds, compared exactly on purpose.

use std::fs;
use std::sync::Arc;

use serde_json::json;
use trust_control::failure::Failure;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::CampaignRestoration;

use super::test_support::{
    assert_no_model_of_a_held_input, empty_journal, folder, passes, run_tank, tank_loop,
    whole_text, FakeState, StartBehaviour, Study, Verdict,
};
use super::{IdentifyOutcome, IdentifyRequest, IdentifyResult, RunPurpose};

/// Runs the tank's campaign as `purpose` on a fake runtime whose measurement ramps while its step is held, so the fit of the held
/// record refuses (`long_campaign_tests`, test 14). The journal is the folder `name`, which is removed once the campaign has ended.
fn held_campaign(name: &str, purpose: RunPurpose) -> Result<IdentifyOutcome<Verdict>, Failure> {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.ramp_while_stepped(0.01);
    let folder = folder(name);
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank(&state, &journal, &mut study, &request, purpose);
    let _ = fs::remove_dir_all(&folder);
    outcome
}

/// A project test whose fit refuses finishes: its recording is kept, its samples and its restoration are answered, and the refusal
/// is its note, whole. No model is fitted, so the outcome holds no Identify result (C12, B3.5).
#[test]
fn a_project_test_whose_fit_refuses_finishes_with_its_note_and_keeps_no_model() {
    let Err(refused) = held_campaign("project-test-refused-identify", RunPurpose::Identify) else {
        panic!("the fit of the held record refuses");
    };
    assert_no_model_of_a_held_input(&refused);
    let Ok(outcome) = held_campaign("project-test-finished", RunPurpose::ProjectTest) else {
        panic!("a project test whose fit refuses still finishes");
    };
    assert!(
        matches!(outcome.result, IdentifyResult::Recorded(_)),
        "the record is kept without a fit"
    );
    assert_eq!(
        outcome.fit_refusal.as_deref(),
        Some(whole_text(&refused)),
        "the note is the fit's whole refusal"
    );
    assert_eq!(
        outcome.restoration,
        Some(CampaignRestoration::Restored),
        "the restoration the experiment confirmed"
    );
    assert!(
        !outcome.recorded_samples.is_empty(),
        "the samples of the kept recording"
    );
    assert!(!outcome.recording_id.is_empty(), "the kept recording");
}

/// The tank as a project test that declares no hold parameter (C12): its project owns the scenario, which the campaign only observes
/// for the declared `duration_seconds`, after the `baseline_seconds` the project declares when it declares one.
fn scripted(duration_seconds: f64, baseline_seconds: Option<f64>) -> LoopDefinition {
    let mut value = serde_json::to_value(tank_loop("rev-1")).expect("the tank encodes");
    value["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0,
        "durationSeconds": duration_seconds, "seed": 1,
        "activation": { "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" }
    });
    if let Some(baseline) = baseline_seconds {
        value["experiment"]["baselineSeconds"] = json!(baseline);
    }
    serde_json::from_value(value).expect("the project test decodes")
}

/// Lesson 13's shape: a project test that declares no baseline and the declared 30.1 s, the 30 s hold and one more sample.
fn scripted_tank() -> LoopDefinition {
    scripted(30.1, None)
}

/// How many recordings the run of `loop_definition` as `purpose` starts, and the expected settling time of the first one, which its
/// record period follows (A4.2). How the whole campaign ends is not this test's: it reads the recording the runtime was asked to
/// start. The journal is the folder `name`, which is removed afterwards.
fn recording_started(
    loop_definition: &LoopDefinition,
    purpose: RunPurpose,
    name: &str,
) -> (usize, Option<u64>) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let folder = folder(name);
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let request = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let _ = run_tank(&state, &journal, &mut study, &request, purpose);
    let _ = fs::remove_dir_all(&folder);
    let starts = state.recording_starts();
    (
        starts.len(),
        starts.first().and_then(|plan| plan.expected_settling_ms),
    )
}

/// A project test records every execution of its hold (C12): its recording starts with no expected settling time, so that its record
/// period is one execution, whatever the process is. A campaign that steps the plant starts its recording with a tenth of the maximum
/// duration (A4.2), which here is 60 s of the tank's 600 s.
#[test]
fn a_project_test_records_every_execution_and_a_campaign_follows_the_process() {
    assert_eq!(
        recording_started(
            &scripted_tank(),
            RunPurpose::ProjectTest,
            "project-record-period"
        ),
        (1, None),
        "the project test's recording carries no settling time, so every execution is recorded"
    );
    assert_eq!(
        recording_started(
            &tank_loop("rev-1"),
            RunPurpose::Identify,
            "campaign-record-period"
        ),
        (1, Some(60_000)),
        "the campaign's recording follows a tenth of the maximum duration"
    );
}

/// Runs the project test of `loop_definition` as `purpose` on a fake runtime that starts at process time zero: the runtime, and what the
/// run gave. The journal is the folder `name`, which is removed afterwards.
fn run_scripted(
    loop_definition: &LoopDefinition,
    purpose: RunPurpose,
    name: &str,
) -> (Arc<FakeState>, Result<IdentifyOutcome<Verdict>, Failure>) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let folder = folder(name);
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let request = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank(&state, &journal, &mut study, &request, purpose);
    let _ = fs::remove_dir_all(&folder);
    (state, outcome)
}

/// The executions of the loop's task in which a plan of `duration_ms` keeps its activation in force (specification 35, A3.5; annex
/// E5.4): the runtime counts a duration to the execution at which the restoration takes effect, so it queues the restoring write at the
/// last execution before the duration has run, and a write queued at an execution takes effect at the next. The activation, written at
/// the first execution, is therefore in force from the second to the last, and the execution after it is the restoration sample.
/// `trust-runtime` pins the count (`control_studio_experiment_timeout_restores_without_an_editor_request`).
fn activation_executions(duration_ms: f64, task_period_ms: f64) -> f64 {
    (duration_ms / task_period_ms).ceil() - 1.0
}

/// A project test records from the start of its plan (C12, B3.4): Lesson 61 declares 2 s of baseline in 92.1 s, and its recording
/// covers them whole, so that the first active sample lies 2 s into the recording, within one sample. The run waits for no steady
/// baseline, its recording plans no wait, and the record's baseline window is the declared baseline.
#[test]
fn a_project_test_records_from_its_plan_and_waits_for_no_steady_baseline() {
    let (state, outcome) = run_scripted(
        &scripted(92.1, Some(2.0)),
        RunPurpose::ProjectTest,
        "project-no-wait",
    );
    let plans = state.started_plans();
    assert_eq!(
        plans.len(),
        1,
        "the run starts its experiment once: {:?}",
        state.calls()
    );
    assert_eq!(
        state.step_start_seconds(),
        0.0,
        "the experiment starts with the recording: the process clock has not moved"
    );
    assert_eq!(
        plans[0].trajectory.last().map(|point| point.at_ms),
        Some(2_000.0),
        "the plan enables the project 2 s after it starts, so the first active sample lies 2 s into the recording"
    );
    assert_eq!(
        plans[0].duration_ms, 92_100.0,
        "the plan lasts the declared duration, whose last instant is the restoration sample"
    );
    let starts = state.recording_starts();
    assert_eq!(starts.len(), 1, "one recording");
    assert_eq!(
        starts[0].planned_duration_ms, 102_100,
        "the declared 92.1 s and the 10 s restoration margin, and not the 392.1 s of a campaign's wait for its steady baseline"
    );
    assert_eq!(
        starts[0].expected_settling_ms, None,
        "every execution is recorded"
    );

    let outcome = outcome.expect("the project test completes");
    let IdentifyResult::Recorded(record) = outcome.result else {
        panic!("the fit of the tank's held input refuses, so the test keeps its recording");
    };
    assert_eq!(
        record.request.baseline_start_seconds, None,
        "the fit starts with the recording"
    );
    let end = record
        .request
        .baseline_end_seconds
        .expect("the record names the declared baseline");
    assert!(
        (end - 1.9).abs() < 1e-9,
        "the record's baseline is the declared 2 s, up to its last sample before the project is enabled, where the fit takes its \
         operating point from (N1.3): {end}"
    );
}

/// A project test that declares no baseline waits for the steady baseline a campaign waits for (B3.4 step 2), so that its fit has the
/// samples before the first input change that it takes its operating point from. Its plan then enables the activation at its first
/// execution (C12), and all 300 scans of Lesson 13's 30 s hold are recorded: its 30.1 s plan keeps the activation in force for 300
/// executions of the 100 ms task, and the execution after them is the restoration sample. A plan of 30.0 s, the declared duration less
/// one sample, would keep it for 299.
#[test]
fn a_project_test_without_a_baseline_waits_for_a_steady_one_and_records_every_scan_of_its_hold() {
    let (state, outcome) = run_scripted(
        &scripted_tank(),
        RunPurpose::ProjectTest,
        "project-no-baseline",
    );
    let plans = state.started_plans();
    assert_eq!(plans.len(), 1, "{:?}", state.calls());
    assert!(
        plans[0].trajectory.is_empty(),
        "no baseline: the activation is written at the first execution of the plan"
    );
    assert!(
        state.step_start_seconds() >= 1.9,
        "the plan starts after a steady baseline of at least 20 samples of the 100 ms task, not with the recording: {} s",
        state.step_start_seconds()
    );
    assert_eq!(plans[0].duration_ms, 30_100.0);
    assert_eq!(
        activation_executions(plans[0].duration_ms, 100.0),
        300.0,
        "the 30 s hold of the lesson's program takes 300 scans, all in force"
    );
    assert_eq!(
        activation_executions(30_000.0, 100.0),
        299.0,
        "the declared duration less one sample would end the hold one scan early"
    );
    assert_eq!(
        state.recording_starts()[0].planned_duration_ms,
        330_100,
        "the recording plans the five-minute wait for a steady baseline and then the declared 30.1 s"
    );

    let outcome = outcome.expect("the project test completes");
    let IdentifyResult::Recorded(record) = outcome.result else {
        panic!("the fit of the tank's held input refuses, so the test keeps its recording");
    };
    let end = record
        .request
        .baseline_end_seconds
        .expect("the record names the end of the steady baseline the fit takes its operating point from (B3.4)");
    let start = record.request.baseline_start_seconds.unwrap_or(0.0);
    assert!(
        start < end && end <= state.step_start_seconds() + 1e-9,
        "the steady baseline lies before the plan starts: {start} s to {end} s, plan at {} s",
        state.step_start_seconds()
    );
}

/// A campaign still waits for its steady baseline before its step, and its recording plans that wait (B3.4 step 2). The tank's 600 s
/// maximum has a baseline window of 30 s, 5 % of it, and a wait of up to five minutes.
#[test]
fn a_campaign_still_waits_for_its_steady_baseline_before_its_step() {
    let (state, _outcome) =
        run_scripted(&tank_loop("rev-1"), RunPurpose::Identify, "campaign-wait");
    assert!(
        state.step_start_seconds() >= 30.0,
        "the step starts after a steady baseline window of 30 s, at {} s",
        state.step_start_seconds()
    );
    assert_eq!(
        state.recording_starts()[0].planned_duration_ms,
        900_000,
        "the recording plans the five-minute wait and then the 600 s maximum"
    );
}
