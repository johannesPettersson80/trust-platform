//! Tests of the campaign rules (`control-studio-long-process-campaign.test.ts`, the rule assertions: limits, refusals, texts,
//! the baseline, the hold, the early finish, the stop reasons, the restoration, the progress texts, the time scale, the
//! silence limit and the journal). The loop and the plan are the JSON the runtime and the study store carry.

#![allow(clippy::float_cmp)] // Several assertions compare the exact values a rule returns, such as 2.0 or 30.0.

use serde_json::{json, Value};

use super::baseline::{BaselineStep, BaselineSummary, BaselineTracker};
use super::hold::{hold_state, HoldState, HoldStep, HoldTracker};
use super::journal::{CampaignPhase, IdentificationCampaignJournal, NO_RUNNING_EXPERIMENT_TEXT};
use super::{
    baseline_budget_ms, campaign_maximum_ms, campaign_recording_ms, experiment_stop_failure,
    format_duration, format_rate, require_campaign_limits, restoration_confirmed, silence_limit_ms,
    FollowSample, Silence, Watch, SILENT_RUNTIME_TEXT,
};
use crate::experiment::{ControlExperimentPlan, RecordingRecoveryOwner, RestorationState};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;

/// A tank measured in 0 to 100 degC, with a setpoint of 45 degC and a 100 ms task (the `TypeScript` `tankLoop`).
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

/// A single setpoint step from 45 to 47 degC that holds for `duration_ms`, or a two-point plan when `points` is 2.
fn setpoint_plan(duration_ms: f64, points: usize) -> ControlExperimentPlan {
    let trajectory: Vec<Value> = if points == 1 {
        vec![json!({ "atMs": 0, "value": 47 })]
    } else {
        vec![
            json!({ "atMs": 0, "value": 47 }),
            json!({ "atMs": 1000, "value": 46 }),
        ]
    };
    serde_json::from_value(json!({
        "id": "experiment-1", "source": "simulation", "kind": "setpoint_step",
        "affectedSignal": "Main.TIC101_Setpoint", "valueType": "LREAL", "from": 45, "to": 47,
        "unit": "degC", "minimum": 0, "maximum": 100, "durationMs": duration_ms, "expectedScan": 300,
        "stopConditions": ["Abort"], "restorationValue": 45, "requiresPhysicalApproval": false,
        "trajectory": trajectory
    }))
    .expect("the setpoint plan")
}

fn recording_owner() -> RecordingRecoveryOwner {
    serde_json::from_value(json!({
        "schemaVersion": 1, "recordingId": "recording-1", "projectId": "project", "runtimeId": "runtime",
        "loopId": "tank", "loopRevision": "rev-1", "source": "Simulation",
        "bindings": [{ "id": "pv", "symbol": "Main.TT101.Value", "iec_type": "LREAL" }]
    }))
    .expect("the recording owner")
}

/// The text a refusal says in full: its detail when it says more than its first sentence.
fn whole_text(failure: &Failure) -> &str {
    failure.detail.as_deref().unwrap_or(&failure.message)
}

fn near(actual: f64, expected: f64, tolerance: f64, message: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{message}: {actual} is not {expected}"
    );
}

/// The `TypeScript` simulated first-order plant on process time: a step of `change` with a 50 s time constant from `start`,
/// a ramp, a drift, and a periodic disturbance of amplitude 0.025.
fn plant(base: f64, drift: f64, ramp: f64, change: f64, start: f64, seconds: f64) -> f64 {
    let active = seconds >= start;
    let step = if active {
        change * (1.0 - (-(seconds - start) / 50.0).exp())
    } else {
        0.0
    };
    let ramp_part = if active {
        ramp * (seconds - start)
    } else {
        0.0
    };
    base + drift * seconds
        + step
        + ramp_part
        + 0.025 * ((1.7 * seconds).sin() + (2.3 * seconds).cos())
}

fn sample(seconds: f64, value: f64) -> FollowSample {
    FollowSample {
        elapsed_seconds: seconds,
        good: true,
        output: Some(value),
    }
}

/// One sample per second from `from` through `to`, read as the runtime's pages arrive.
fn seconds_from(from: u32, to: u32, value: impl Fn(f64) -> f64) -> Vec<FollowSample> {
    (from..=to)
        .map(|tick| {
            let seconds = f64::from(tick);
            sample(seconds, value(seconds))
        })
        .collect()
}

fn baseline_summary() -> BaselineSummary {
    BaselineSummary {
        mean: 45.0,
        noise: 0.03,
        half_range: 0.05,
        spacing_seconds: 1.0,
        window_seconds: 30.0,
    }
}

#[test]
fn durations_and_rates_are_read_in_plain_words() {
    assert_eq!(format_duration(720.0), "12 min");
    assert_eq!(format_duration(13_800.0), "3 h 50 min");
    assert_eq!(format_duration(45.0), "45 s");
    assert_eq!(format_duration(7_200.0), "2 h");
    assert_eq!(format_duration(125.0), "2 min", "125 s rounds to 2 min");
    assert_eq!(format_rate(0.01, "degC"), "0.6 degC/min");
    assert_eq!(
        format_rate(-0.01, ""),
        "0.6/min",
        "a rate is shown without its sign"
    );
}

#[test]
fn the_maximum_duration_is_the_declared_one_within_the_limits() {
    assert_eq!(campaign_maximum_ms(&tank_loop()).expect("600 s"), 600_000);
    let mut missing = tank_loop();
    missing.experiment = None;
    let refusal = campaign_maximum_ms(&missing).expect_err("the limits are required");
    assert_eq!(
        whole_text(&refusal),
        "Set the maximum experiment duration in Limits before identification."
    );
    let mut short = tank_loop();
    if let Some(experiment) = short.experiment.as_mut() {
        experiment.duration_seconds = Some(0.5);
    }
    let refusal = campaign_maximum_ms(&short).expect_err("below one second");
    assert_eq!(
        whole_text(&refusal),
        "The maximum experiment duration must be between 1 s and 24 h."
    );
}

#[test]
fn the_baseline_budget_and_the_recording_follow_the_maximum_duration() {
    near(
        baseline_budget_ms(600_000),
        300_000.0,
        1e-9,
        "five minutes at least",
    );
    near(
        baseline_budget_ms(6_000_000),
        1_200_000.0,
        1e-9,
        "a fifth of 100 minutes",
    );
    // The TypeScript test: a 600 s maximum and a five-minute budget record 900 s (B3.4).
    assert_eq!(
        campaign_recording_ms(&tank_loop()).expect("recording"),
        900_000
    );
}

#[test]
fn a_missing_limit_names_the_field_that_is_missing() {
    assert!(require_campaign_limits(&tank_loop()).is_ok());
    let mut not_a_number = tank_loop();
    not_a_number.limits.output_maximum = Some(f64::NAN);
    let refusal =
        require_campaign_limits(&not_a_number).expect_err("the output maximum is not a number");
    assert_eq!(
        whole_text(&refusal),
        "The plant input limits are missing: limits.outputMinimum and limits.outputMaximum must be set, with the minimum below the maximum."
    );
    let mut no_maximum = tank_loop();
    no_maximum.limits.output_maximum = None;
    let refusal = require_campaign_limits(&no_maximum).expect_err("the output maximum is missing");
    assert_eq!(
        whole_text(&refusal),
        "The plant input limits are missing: limits.outputMaximum must be set, with the minimum below the maximum."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    let mut no_minimum = tank_loop();
    no_minimum.limits.output_minimum = None;
    let refusal = require_campaign_limits(&no_minimum).expect_err("the output minimum is missing");
    assert_eq!(
        whole_text(&refusal),
        "The plant input limits are missing: limits.outputMinimum must be set, with the minimum below the maximum."
    );
    let mut no_duration = tank_loop();
    if let Some(experiment) = no_duration.experiment.as_mut() {
        experiment.duration_seconds = None;
    }
    let refusal = require_campaign_limits(&no_duration).expect_err("the duration is missing");
    assert_eq!(
        whole_text(&refusal),
        "The maximum experiment duration is missing: experiment.durationSeconds must be set, between 1 s and 24 h."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    let mut no_experiment = tank_loop();
    no_experiment.experiment = None;
    let refusal = require_campaign_limits(&no_experiment).expect_err("no experiment is declared");
    assert_eq!(
        whole_text(&refusal),
        "The maximum experiment duration is missing: experiment.durationSeconds must be set, between 1 s and 24 h."
    );
    let mut no_range = tank_loop();
    no_range.signals.process_variable.range.minimum = f64::NAN;
    let refusal = require_campaign_limits(&no_range).expect_err("the permitted range is missing");
    assert_eq!(
        whole_text(&refusal),
        "The permitted range of Main.TT101.Value is missing: signals.processVariable.range must be set."
    );
}

#[test]
fn the_silence_limit_is_five_seconds_or_three_sample_intervals_when_longer() {
    assert_eq!(
        silence_limit_ms(&tank_loop(), None).expect("a 100 ms task"),
        5_000
    );
    let mut slow = tank_loop();
    slow.controller.task_period_ms = 2_000.0;
    // A 2 s task samples every execution: three intervals are 6 s, longer than 5 s.
    assert_eq!(silence_limit_ms(&slow, None).expect("a 2 s task"), 6_000);
}

#[test]
fn the_silence_limit_follows_the_record_period_of_the_campaigns_recording_with_its_settling_time() {
    // A7, A4.2: three intervals between the samples followed, when longer than 5 s. The tank records 900 s with a 0.1 s task.
    // Undeclared, its settling time is a tenth of its 600 s maximum, 60 s, which gives 3 executions, a 0.3 s record period, and
    // the 5 s floor. A 3000 s maximum is recorded for 3600 s (600 s of baseline and 3000 s of step). A declared settling time of
    // 2000 s gives about 200 samples over it: 100 executions, a 10 s record period, and a limit of 30 s. Undeclared, the same
    // maximum has a 300 s settling time, 15 executions and a 1.5 s record period, so its limit is still the 5 s floor.
    assert_eq!(
        silence_limit_ms(&tank_loop(), None).expect("a 0.1 s record period"),
        5_000
    );
    let mut long = tank_loop();
    if let Some(experiment) = long.experiment.as_mut() {
        experiment.duration_seconds = Some(3_000.0);
    }
    assert_eq!(
        silence_limit_ms(&long, Some(2_000.0)).expect("a 10 s record period"),
        30_000
    );
}

#[test]
fn a_silent_runtime_stops_the_campaign_after_its_limit_and_any_sample_resets_it() {
    let mut silence = Silence::starting_at(0.0, 5_000);
    assert!(
        silence.observe_page(0, 5_000.0).is_ok(),
        "exactly at the limit is still in time"
    );
    let refusal = silence
        .observe_page(0, 5_001.0)
        .expect_err("past the limit the runtime is silent");
    assert_eq!(whole_text(&refusal), SILENT_RUNTIME_TEXT);
    assert!(
        silence.observe_page(3, 6_000.0).is_ok(),
        "a sample resets the supervision"
    );
    assert!(silence.observe_page(0, 10_999.0).is_ok());
}

#[test]
fn an_experiment_that_ends_normally_has_no_stop_failure_and_any_other_end_names_its_reason() {
    assert_eq!(
        experiment_stop_failure(true, Some("duration_complete")),
        None
    );
    assert_eq!(experiment_stop_failure(true, Some("finished_early")), None);
    assert_eq!(
        experiment_stop_failure(true, Some("user_abort")).as_deref(),
        Some("The experiment stopped before its hold ended: it was aborted. The recording is kept and is not qualified for a fit.")
    );
    assert_eq!(
        experiment_stop_failure(true, Some("mode_changed")).as_deref(),
        Some("The experiment stopped before its hold ended: the loop left the mode the experiment requires. The recording is kept and is not qualified for a fit.")
    );
    assert_eq!(
        experiment_stop_failure(true, None).as_deref(),
        Some("The experiment stopped before its hold ended: stop reason unknown. The recording is kept and is not qualified for a fit.")
    );
    assert_eq!(
        experiment_stop_failure(false, Some("duration_complete")).as_deref(),
        Some("The experiment stopped before its hold ended: stop reason duration_complete. The recording is kept and is not qualified for a fit.")
    );
}

#[test]
fn a_restoration_is_confirmed_only_by_the_restored_or_not_required_state() {
    assert!(restoration_confirmed(RestorationState::Restored));
    assert!(restoration_confirmed(RestorationState::NotRequired));
    assert!(!restoration_confirmed(RestorationState::Pending));
    assert!(!restoration_confirmed(RestorationState::Failed));
}

#[test]
fn the_watch_keeps_good_finite_outputs_and_reports_a_time_scale_beyond_five_percent() {
    let mut watch = Watch::new();
    let observed = watch.observe(
        &[
            sample(0.0, 10.0),
            FollowSample {
                elapsed_seconds: 1.0,
                good: false,
                output: Some(11.0),
            },
            FollowSample {
                elapsed_seconds: 2.0,
                good: true,
                output: Some(f64::NAN),
            },
            FollowSample {
                elapsed_seconds: 3.0,
                good: true,
                output: None,
            },
            sample(20.0, 12.0),
        ],
        0.0,
    );
    assert_eq!(observed.len(), 2, "only the good finite outputs are kept");
    assert!((watch.newest_seconds() - 20.0).abs() < 1e-12);
    assert_eq!(
        watch.time_scale(10_000.0),
        Some(2.0),
        "20 s of process time over 10 s of wall time"
    );

    let mut steady = Watch::new();
    steady.observe(&[sample(0.0, 1.0), sample(10.4, 1.0)], 0.0);
    assert_eq!(
        steady.time_scale(10_000.0),
        None,
        "4 % is within the tolerance (A3.6)"
    );
}

#[test]
fn a_five_minute_baseline_completes_in_process_time_when_the_samples_arrive_at_once() {
    // The TypeScript test: 400 s of samples arrive at once; the window is 5 % of 100 minutes, five minutes.
    let plant_value = |seconds: f64| plant(45.0, 0.0, 0.0, 0.0, f64::INFINITY, seconds);
    let mut tracker = BaselineTracker::new(
        &tank_loop().signals.process_variable,
        100 * 60_000,
        100.0,
        None,
        true,
    );
    let mut watch = Watch::new();
    let step = tracker
        .feed(&mut watch, &seconds_from(0, 400, plant_value), 0.0)
        .expect("a steady output is not refused");
    let BaselineStep::Steady(summary) = &step else {
        panic!("the output is steady over the window: {step:?}");
    };
    near(
        summary.window_seconds,
        300.0,
        1e-9,
        "100 minutes at 5 % is a 5-minute window",
    );
    assert!(
        (summary.mean - 45.0).abs() < 0.1,
        "the mean is {}",
        summary.mean
    );
    assert!(summary.half_range > 0.0);
}

#[test]
fn a_drifting_output_is_refused_with_its_rate_and_the_setpoint_step_offer() {
    // The TypeScript test: 0.01 degC/s of drift, a 3600 s maximum, so the budget is 720 s of process time.
    let drifting = |seconds: f64| plant(45.0, 0.01, 0.0, 0.0, f64::INFINITY, seconds);
    let mut tracker = BaselineTracker::new(
        &tank_loop().signals.process_variable,
        3_600_000,
        100.0,
        None,
        true,
    );
    let mut watch = Watch::new();
    let mut refusal = None;
    for tick in 0..=720_u32 {
        let seconds = f64::from(tick);
        match tracker.feed(
            &mut watch,
            &[sample(seconds, drifting(seconds))],
            seconds * 1_000.0,
        ) {
            Ok(BaselineStep::Waiting { .. }) => {}
            Ok(BaselineStep::Steady(summary)) => {
                panic!("a drifting output is never steady: {summary:?}")
            }
            Err(failure) => {
                refusal = Some((tick, failure));
                break;
            }
        }
    }
    let (tick, failure) = refusal.expect("the budget is spent");
    assert_eq!(
        tick, 720,
        "the refusal comes when the budget of 720 s is spent"
    );
    assert_eq!(
        whole_text(&failure),
        "Main.TT101.Value is still changing by 0.6 degC/min. Wait for it to settle, or identify with a setpoint step."
    );
    assert_eq!(
        failure.message,
        "Main.TT101.Value is still changing by 0.6 degC/min."
    );
}

#[test]
fn the_baseline_progress_is_reported_when_due_and_names_the_rate() {
    let drifting = |seconds: f64| plant(45.0, 0.01, 0.0, 0.0, f64::INFINITY, seconds);
    let mut tracker = BaselineTracker::new(
        &tank_loop().signals.process_variable,
        6_000_000,
        100.0,
        None,
        false,
    );
    let mut watch = Watch::new();
    let first = tracker
        .feed(&mut watch, &seconds_from(0, 30, drifting), 30_000.0)
        .expect("the budget is not spent");
    assert_eq!(
        first,
        BaselineStep::Waiting {
            progress: Some(
                "Waiting for Main.TT101.Value to settle (changing 0.6 degC/min)…".to_string()
            )
        }
    );
    let quiet = tracker
        .feed(&mut watch, &seconds_from(31, 35, drifting), 35_000.0)
        .expect("still waiting");
    assert_eq!(
        quiet,
        BaselineStep::Waiting { progress: None },
        "five seconds is not ten"
    );
    let due = tracker
        .feed(&mut watch, &seconds_from(36, 40, drifting), 40_000.0)
        .expect("still waiting");
    assert!(
        matches!(due, BaselineStep::Waiting { progress: Some(_) }),
        "ten seconds since the last text"
    );
}

#[test]
fn a_settled_single_step_is_finished_early_once_and_a_ramp_that_never_settles_is_not() {
    let baseline = baseline_summary();
    let settles = |seconds: f64| plant(45.0, 0.0, 0.0, 2.0, 0.0, seconds);
    let mut hold = HoldTracker::new(&setpoint_plan(540_000.0, 1), &baseline, 0.0);
    let mut watch = Watch::new();
    let mut finishes = 0;
    for tick in 1..=600_u32 {
        let seconds = f64::from(tick);
        let step = hold.feed(
            &mut watch,
            HoldState::Holding,
            &[sample(seconds, settles(seconds))],
            seconds * 1_000.0,
        );
        if matches!(
            step,
            HoldStep::Continue {
                finish_early: true,
                ..
            }
        ) {
            finishes += 1;
        }
    }
    assert_eq!(
        finishes, 1,
        "the response settled before the hold ended, so the step is finished once"
    );

    let ramping = |seconds: f64| plant(45.0, 0.0, 0.01, 2.0, 0.0, seconds);
    let mut never = HoldTracker::new(&setpoint_plan(540_000.0, 1), &baseline, 0.0);
    let mut watch = Watch::new();
    let early = (1..=600_u32).any(|tick| {
        let seconds = f64::from(tick);
        matches!(
            never.feed(
                &mut watch,
                HoldState::Holding,
                &[sample(seconds, ramping(seconds))],
                seconds * 1_000.0
            ),
            HoldStep::Continue {
                finish_early: true,
                ..
            }
        )
    });
    assert!(
        !early,
        "a response that never settles runs to its maximum duration"
    );
}

#[test]
fn a_multi_point_plan_is_never_finished_early() {
    let baseline = baseline_summary();
    let settles = |seconds: f64| plant(45.0, 0.0, 0.0, 2.0, 0.0, seconds);
    let mut hold = HoldTracker::new(&setpoint_plan(540_000.0, 2), &baseline, 0.0);
    let mut watch = Watch::new();
    let early = (1..=600_u32).any(|tick| {
        let seconds = f64::from(tick);
        matches!(
            hold.feed(
                &mut watch,
                HoldState::Holding,
                &[sample(seconds, settles(seconds))],
                seconds * 1_000.0
            ),
            HoldStep::Continue {
                finish_early: true,
                ..
            }
        )
    });
    assert!(!early, "only a single-step experiment is finished early");
}

#[test]
fn the_hold_progress_names_the_elapsed_time_and_the_duration() {
    let mut hold = HoldTracker::new(&setpoint_plan(540_000.0, 1), &baseline_summary(), 0.0);
    let mut watch = Watch::new();
    let step = hold.feed(
        &mut watch,
        HoldState::Holding,
        &seconds_from(1, 60, |_| 45.0),
        60_000.0,
    );
    assert_eq!(
        step,
        HoldStep::Continue {
            finish_early: false,
            progress: Some(
                "Holding the step until the response settles: 1 min of at most 9 min.".to_string()
            ),
        }
    );
}

#[test]
fn the_hold_reads_the_experiment_state_and_refuses_a_status_outside_the_vocabulary() {
    assert_eq!(
        hold_state(&json!({ "status": "running", "restoration": "pending" })).expect("running"),
        HoldState::Holding
    );
    assert_eq!(
        hold_state(&json!({ "status": "completed", "restoration": "pending" }))
            .expect("awaiting restoration"),
        HoldState::Holding,
        "an ended experiment keeps being polled until its restoration is confirmed"
    );
    assert_eq!(
        hold_state(&json!({ "status": "completed", "restoration": "restored", "stop_reason": "duration_complete" })).expect("terminal"),
        HoldState::Ended
    );
    let refusal =
        hold_state(&json!({ "status": "paused", "restoration": "pending" })).expect_err("invalid");
    assert_eq!(
        whole_text(&refusal),
        "The runtime returned an invalid experiment status."
    );
}

#[test]
fn the_journal_keeps_the_campaign_in_camel_case_and_resumes_only_a_dispatched_step_of_its_revision()
{
    let baseline = baseline_summary();
    let step = IdentificationCampaignJournal::step_phase(
        "rev-1",
        recording_owner(),
        setpoint_plan(540_000.0, 1),
        baseline,
        30.0,
        2,
    );
    let stored = serde_json::to_value(&step).expect("json");
    assert_eq!(stored["schemaVersion"], 1);
    assert_eq!(stored["loopRevision"], "rev-1");
    assert_eq!(stored["phase"], "step");
    assert_eq!(stored["stepStartSeconds"], 30.0);
    assert_eq!(stored["channelsRemaining"], 2);
    assert_eq!(stored["baseline"]["halfRange"], 0.05);
    let decoded: IdentificationCampaignJournal =
        serde_json::from_value(stored).expect("the journal decodes");
    assert_eq!(decoded, step, "the record reads back as it was written");
    let (plan, resumed) = step
        .resumable_step("rev-1")
        .expect("a dispatched step resumes");
    assert_eq!(plan.id, "experiment-1");
    assert_eq!(*resumed, baseline);

    let refusal = step
        .resumable_step("rev-2")
        .expect_err("another loop revision never resumes");
    assert_eq!(whole_text(&refusal), NO_RUNNING_EXPERIMENT_TEXT);
    let waiting = IdentificationCampaignJournal::baseline_phase("rev-1", recording_owner(), 0);
    assert_eq!(waiting.phase, CampaignPhase::Baseline);
    assert!(
        waiting.resumable_step("rev-1").is_err(),
        "a baseline has no running experiment"
    );
}
