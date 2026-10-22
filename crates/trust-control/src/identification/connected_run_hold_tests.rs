//! The hold of a project step test in the connected run's rules (specification 35, C12 Hold parameter): the recording of the hold the
//! runtime applies is the baseline, every scan of the hold and one restoration sample, for the review and for the step's plan, and each
//! way the hold cannot be recorded is refused with its sentence and its next action. The loop of Lesson 21 has a 5 s baseline, a 100 ms
//! period, a hold of 20 to 40 s and a declared maximum of 45.1 s.

#![allow(clippy::float_cmp)] // Whole milliseconds and reviewed plan values are compared exactly on purpose.

use super::*;
use crate::experiment::PlanSignal;
use crate::recording::fixtures::{hold_loop, loop_definition};
use crate::signals::Quality;

/// One signal of a live sample of the fixture loop, good and in percent.
fn signal(symbol: &str, address: &str, value: f64) -> PlanSignal {
    PlanSignal {
        symbol: symbol.to_owned(),
        address: Some(address.to_owned()),
        value,
        unit: "%".to_owned(),
        quality: Quality::Good,
    }
}

/// A live sample of the fixture loop on a simulated target: the setpoint and the measurement at 45 %, the output at 30 %.
fn live() -> ExperimentSample {
    ExperimentSample {
        scan: 7,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("LevelSP", "%ML0", 45.0)),
        process_variable: signal("LevelPV", "%IL0", 45.0),
        manipulated_variable: signal("ValveMV", "%QL0", 30.0),
        additional: Vec::new(),
    }
}

/// The steady baseline of the campaign tests: 45 %, a 30 s window at one sample per second.
fn steady() -> BaselineSummary {
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

#[test]
fn the_applied_hold_is_recorded_with_the_baseline_and_one_restoration_sample() {
    let loop_def = hold_loop();
    for (hold, recording_ms) in [
        (30.0, 35_100.0),
        (20.0, 25_100.0),
        (40.0, 45_100.0),
        (25.05, 30_200.0),
    ] {
        assert_eq!(
            campaign_hold_ms(&loop_def, Some(hold)).expect("a hold the declared maximum records"),
            Some(recording_ms),
            "a {hold} s hold records 5 s + the hold + 0.1 s"
        );
    }
    // A loop that declares no hold parameter records its declared duration whatever value is given.
    let mut undeclared = hold_loop();
    undeclared
        .experiment
        .as_mut()
        .expect("the experiment is declared")
        .hold_parameter = None;
    for loop_without_hold in [undeclared, loop_definition()] {
        for applied in [None, Some(30.0)] {
            assert_eq!(
                campaign_hold_ms(&loop_without_hold, applied)
                    .expect("no hold parameter, no refusal"),
                None
            );
        }
    }
}

#[test]
fn a_hold_that_is_missing_or_outside_its_bounds_is_refused_and_one_above_the_maximum_opens_limits()
{
    let sentence = "Apply a P1.HoldSeconds value within 20–40 s before reviewing the project test.";
    for applied in [None, Some(f64::NAN), Some(19.9), Some(40.1)] {
        let refusal = campaign_hold_ms(&hold_loop(), applied)
            .expect_err("no finite hold within its declared bounds");
        assert_eq!(refusal.message, sentence, "{applied:?}");
        assert_eq!(
            (refusal.cause, refusal.next_action),
            (Cause::Settings, NextAction::None),
            "the engineer applies another value with the experiment settings: {applied:?}"
        );
    }
    // A 30 s hold needs 35.1 s, which a declared maximum of 30 s cannot record.
    let mut shorter = hold_loop();
    shorter
        .experiment
        .as_mut()
        .expect("the experiment is declared")
        .duration_seconds = Some(30.0);
    let refusal = campaign_hold_ms(&shorter, Some(30.0)).expect_err("35.1 s exceeds 30 s");
    assert_eq!(
        refusal.message,
        "A 30 s hold needs 35.1 s, more than the declared 30 s maximum."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLimits),
        "a longer maximum is one fix (B3.9)"
    );
}

#[test]
fn the_step_plan_of_a_project_step_test_records_its_applied_hold() {
    let loop_def = hold_loop();
    let project_sequence = IdentificationMethod {
        kind: ControlExperimentKind::ProjectSequence,
        manual: false,
        automatic: true,
    };
    // The step has 45 s left after the baseline; the plan still records the applied hold, not that time.
    let planned = step_plan(
        &loop_def,
        Some(&live()),
        project_sequence,
        &steady(),
        45_000.0,
        false,
        Some(30.0),
    )
    .expect("the project's step test is planned from its applied hold");
    assert_eq!(planned.kind, ControlExperimentKind::ProjectSequence);
    assert_eq!(
        planned.duration_ms, 35_100.0,
        "5 s + 30 s + 0.1 s, not the 45 s the step could be held"
    );
    assert_eq!(
        planned.stop_conditions[1], "35.1 s timeout",
        "the plan names the same timeout as the review"
    );
    let refusal = step_plan(
        &loop_def,
        Some(&live()),
        project_sequence,
        &steady(),
        45_000.0,
        false,
        None,
    )
    .expect_err("a hold parameter without an applied value is not planned");
    assert_eq!(
        refusal.message,
        "Apply a P1.HoldSeconds value within 20–40 s before reviewing the project test."
    );
}
