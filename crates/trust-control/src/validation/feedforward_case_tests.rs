//! Tests of the measured-disturbance case of a feedforward (specification 35, C8). The judgement is checked on traces whose samples are
//! written out; the case itself is checked on the heater of the feedforward tests, whose feedforward cancels its disturbance in closed
//! form (`K_ff = −K_d/K_u = 0.5 %/kW` with `T_lead = 8 s` and `T_lag = 3 s`), so its measurement stays at the operating value.

#![allow(clippy::float_cmp)] // The judged outcome's fields are copied exactly from the trace, which the tests compare with `==`.

use super::*;
use crate::tuning::structures::fixtures::{heater_model, requirements};

fn requirement(maximum_error: f64, settling_time_seconds: f64) -> TuningRequirements {
    requirements(1.0, maximum_error, 0.1, settling_time_seconds, 80.0)
}

#[test]
fn a_trace_within_the_allowance_from_the_settling_time_passes_and_its_early_samples_are_not_judged()
{
    // Samples every 10 s; the settling time is 20 s, so the first two samples (5.0 and 2.0) are not judged, and the judged ones are 0.1 or less.
    let trace = [5.0, 2.0, 0.1, 0.05, -0.02];
    let outcome = judge(&trace, 10.0, &requirement(0.3, 20.0)).expect("three samples are judged");
    assert!(outcome.passed, "{outcome:?}");
    assert_eq!(outcome.largest_after_settling, 0.1);
    assert_eq!(outcome.allowance, 0.3);
}

#[test]
fn a_sample_outside_the_allowance_after_the_settling_time_fails_the_case() {
    let trace = [0.0, 0.0, 0.5, 0.1, 0.0];
    let outcome = judge(&trace, 10.0, &requirement(0.3, 20.0)).expect("three samples are judged");
    assert!(!outcome.passed, "{outcome:?}");
    assert_eq!(outcome.largest_after_settling, 0.5);
}

#[test]
fn a_sample_at_the_settling_time_is_judged_and_its_negative_deviation_counts_by_magnitude() {
    // The sample at 20 s is judged: −0.4 is 0.4 in magnitude, which is above the 0.3 allowance.
    let trace = [0.0, 0.0, -0.4];
    let outcome = judge(&trace, 10.0, &requirement(0.3, 20.0)).expect("one sample is judged");
    assert!(!outcome.passed, "{outcome:?}");
    assert_eq!(outcome.largest_after_settling, 0.4);
}

#[test]
fn a_test_that_ends_before_its_settling_time_judges_nothing_and_is_refused() {
    let refusal =
        judge(&[0.0, 0.0], 10.0, &requirement(0.3, 20.0)).expect_err("no sample is judged");
    assert_eq!(refusal.0, NO_JUDGED_SAMPLE);
}

#[test]
fn the_heater_with_its_closed_form_feedforward_stays_within_the_allowance_after_the_settling_time()
{
    // The feedforward cancels the disturbance on the model, so after the 40 s settling time the water stays at its operating value.
    let channels = FeedforwardChannels {
        controller_input: 0,
        disturbance_input: 1,
        measurement: 0,
    };
    let gains = ParallelGains {
        proportional_gain: 2.5,
        integral_gain: 0.5,
        derivative_gain: 0.0,
    };
    let terms = FeedforwardTerms {
        gain: 0.5,
        lead_seconds: Some(8.0),
        lag_seconds: Some(3.0),
    };
    let outcome = measured_disturbance_case(
        &heater_model(),
        &channels,
        gains,
        terms,
        10.0,
        0.1,
        &requirement(0.3, 40.0),
    )
    .expect("the heater's case runs");
    assert!(outcome.passed, "{outcome:?}");
    assert!(
        outcome.largest_after_settling < 0.3,
        "{}",
        outcome.largest_after_settling
    );
}

#[test]
fn without_a_feedforward_the_disturbance_leaves_a_tight_allowance_after_the_settling_time() {
    // With no feedforward the disturbance moves the water after its settling time, so no allowance of 10⁻⁹ holds.
    let channels = FeedforwardChannels {
        controller_input: 0,
        disturbance_input: 1,
        measurement: 0,
    };
    let gains = ParallelGains {
        proportional_gain: 2.5,
        integral_gain: 0.5,
        derivative_gain: 0.0,
    };
    let terms = FeedforwardTerms {
        gain: 0.0,
        lead_seconds: None,
        lag_seconds: None,
    };
    let outcome = measured_disturbance_case(
        &heater_model(),
        &channels,
        gains,
        terms,
        10.0,
        0.1,
        &requirement(1e-9, 40.0),
    )
    .expect("the heater's case runs");
    assert!(!outcome.passed, "{outcome:?}");
}
