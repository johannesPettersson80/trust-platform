#![allow(clippy::float_cmp)] // Limits, zero increments and manual outputs are exact by construction, so they are compared exactly.

use super::*;
use crate::tuning::structure::is_exactly_zero;
use crate::tuning::texts;

/// The record of `identify-tune`'s published-vector test (its limits and weights), with the positional gains.
fn parameters() -> ControlPidParameters {
    ControlPidParameters {
        proportional_gain: 2.0,
        integral_gain: 0.0,
        derivative_gain: 0.5,
        proportional_setpoint_weight: 0.0,
        derivative_setpoint_weight: 0.25,
        bias: 1.0,
        output_minimum: -100.0,
        output_maximum: 100.0,
        rate_minimum: -1000.0,
        rate_maximum: 1000.0,
        tracking_time: 1.0,
        anti_windup: AntiWindup::BackCalculation,
    }
}

fn auto(setpoint: f64, process_value: f64, feed_forward: f64, cycle_time: f64) -> ControlPidInput {
    ControlPidInput {
        setpoint,
        process_value,
        feed_forward,
        manual_output: 0.0,
        tracking_output: 0.0,
        mode: ControlPidMode::Auto,
        cycle_time_seconds: cycle_time,
    }
}

fn at_zero() -> ControlPidState {
    ControlPidState::at_operating_point(0.0, 0.0)
}

#[test]
fn positional_vectors_match_the_published_values() {
    // First update: 1 + 2·(1 − 0.2) + 0.5·(0.25·10 − 2) + 0.3 = 3.15. Second: 1 + 2·0.6 + 0.5·(0 − 1) + 0.3 = 2.
    let first = update_control_pid(&parameters(), &at_zero(), &auto(1.0, 0.2, 0.3, 0.1))
        .expect("the record and the cycle time are valid");
    assert!((first.output - 3.15).abs() <= 1e-12, "{}", first.output);
    let second = update_control_pid(&parameters(), &first.state, &auto(1.0, 0.4, 0.3, 0.2))
        .expect("the record and the cycle time are valid");
    assert!((second.output - 2.0).abs() <= 1e-12, "{}", second.output);
}

#[test]
fn incremental_vectors_match_the_published_values() {
    let incremental = ControlPidParameters {
        integral_gain: 0.5,
        derivative_gain: 0.1,
        proportional_setpoint_weight: 0.5,
        derivative_setpoint_weight: 0.0,
        ..parameters()
    };
    // First update: 2·0.5 + 0.5·0.1·1 + 0.2 = 1.25. Second: 1.25 − 0.4 + 0.04 − 0.2 + 0.1 = 0.79.
    let first = update_control_pid(&incremental, &at_zero(), &auto(1.0, 0.0, 0.2, 0.1))
        .expect("the record and the cycle time are valid");
    assert!((first.output - 1.25).abs() <= 1e-12, "{}", first.output);
    let second = update_control_pid(&incremental, &first.state, &auto(1.0, 0.2, 0.3, 0.1))
        .expect("the record and the cycle time are valid");
    assert!((second.output - 0.79).abs() <= 1e-12, "{}", second.output);
}

#[test]
fn back_calculation_with_a_tracking_time_of_one_cycle_clamps_to_the_limit() {
    // Kp = Ki = 1, b = 1, Δt = Tt = 0.1 and a maximum of 1: the nominal output 1.1 is pulled to the
    // saturated candidate 1 (spec 32 §3.13, control-signal clamping). The combined method gives the same.
    let clamped = ControlPidParameters {
        proportional_gain: 1.0,
        integral_gain: 1.0,
        derivative_gain: 0.0,
        proportional_setpoint_weight: 1.0,
        output_maximum: 1.0,
        bias: 0.0,
        tracking_time: 0.1,
        ..parameters()
    };
    let step = update_control_pid(&clamped, &at_zero(), &auto(1.0, 0.0, 0.0, 0.1))
        .expect("the record and the cycle time are valid");
    assert!((step.output - 1.0).abs() <= 1e-12, "{}", step.output);
    assert!(
        (step.state.nominal_output - 1.0).abs() <= 1e-12,
        "{}",
        step.state.nominal_output
    );
    let combined = ControlPidParameters {
        anti_windup: AntiWindup::Combined,
        ..clamped
    };
    let combined_step = update_control_pid(&combined, &at_zero(), &auto(1.0, 0.0, 0.0, 0.1))
        .expect("the record and the cycle time are valid");
    assert!((combined_step.state.nominal_output - 1.0).abs() <= 1e-12);
}

#[test]
fn manual_mode_takes_the_manual_output_within_its_limits() {
    let manual = ControlPidInput {
        setpoint: 1.0,
        process_value: 0.0,
        feed_forward: 0.0,
        manual_output: 0.7,
        tracking_output: 0.0,
        mode: ControlPidMode::Manual,
        cycle_time_seconds: 0.1,
    };
    let step = update_control_pid(&parameters(), &at_zero(), &manual)
        .expect("the record and the cycle time are valid");
    assert_eq!(step.output, 0.7);
    assert_eq!(step.state.nominal_output, 0.7);
}

#[test]
fn track_mode_preloads_the_tracking_output_into_the_incremental_form() {
    // With no proportional or derivative gain and no change of setpoint or process value, the nominal
    // output stays at the tracking output.
    let incremental = ControlPidParameters {
        integral_gain: 1.0,
        proportional_gain: 0.0,
        derivative_gain: 0.0,
        ..parameters()
    };
    let track = ControlPidInput {
        setpoint: 0.0,
        process_value: 0.0,
        feed_forward: 0.0,
        manual_output: 0.0,
        tracking_output: 0.5,
        mode: ControlPidMode::Track,
        cycle_time_seconds: 0.1,
    };
    let step = update_control_pid(&incremental, &at_zero(), &track)
        .expect("the record and the cycle time are valid");
    assert!((step.output - 0.5).abs() <= 1e-12, "{}", step.output);
}

#[test]
fn settings_and_cycle_times_that_cannot_be_used_are_refused() {
    let reversed_rates = ControlPidParameters {
        rate_minimum: 5.0,
        rate_maximum: -5.0,
        ..parameters()
    };
    assert_eq!(
        update_control_pid(&reversed_rates, &at_zero(), &auto(1.0, 0.0, 0.0, 0.1)),
        Err(Refusal::new(texts::CANDIDATE_RATE_ORDER))
    );
    let reversed_outputs = ControlPidParameters {
        output_minimum: 1.0,
        output_maximum: 1.0,
        ..parameters()
    };
    assert_eq!(
        validate_parameters(&reversed_outputs),
        Err(Refusal::new(texts::CANDIDATE_OUTPUT_ORDER))
    );
    let no_tracking = ControlPidParameters {
        tracking_time: 0.0,
        ..parameters()
    };
    assert_eq!(
        validate_parameters(&no_tracking),
        Err(Refusal::new(texts::SETTINGS_TRACKING_TIME))
    );
    let not_a_number = ControlPidParameters {
        bias: f64::NAN,
        ..parameters()
    };
    assert_eq!(
        validate_parameters(&not_a_number),
        Err(Refusal::new(texts::CANDIDATE_NOT_FINITE))
    );
    assert_eq!(
        update_control_pid(&parameters(), &at_zero(), &auto(1.0, 0.0, 0.0, 0.0)),
        Err(Refusal::new(CYCLE_TIME_NOT_POSITIVE))
    );
}

#[test]
fn only_an_exact_zero_selects_the_positional_form() {
    assert!(is_exactly_zero(0.0));
    assert!(is_exactly_zero(-0.0));
    assert!(!is_exactly_zero(1e-300));
}

#[test]
fn the_state_starts_at_the_operating_point() {
    let state = ControlPidState::at_operating_point(2.0, 3.0);
    assert_eq!(state.setpoint, 3.0);
    assert_eq!(state.process_value, 3.0);
    assert_eq!(state.nominal_output, 2.0);
    assert_eq!(state.saturated_output, 2.0);
    assert_eq!(state.setpoint_rate, 0.0);
    assert_eq!(state.process_value_rate, 0.0);
    assert_eq!(state.feed_forward, 0.0);
}

#[test]
fn the_law_keeps_its_state_between_periods() {
    // Positional with Kp = 2 and no derivative: the command is 2·(setpoint − measurement).
    let proportional = ControlPidParameters {
        derivative_gain: 0.0,
        bias: 0.0,
        ..parameters()
    };
    let mut law = ControlPidLaw::new(proportional, at_zero()).expect("the record is valid");
    let first = law.command(1.0, 0.0, 0.1).expect("the cycle time is valid");
    assert!((first - 2.0).abs() <= 1e-12, "{first}");
    let second = law.command(1.0, 0.5, 0.1).expect("the cycle time is valid");
    assert!((second - 1.0).abs() <= 1e-12, "{second}");
    assert_eq!(law.state().process_value, 0.5);
    assert_eq!(law.state().setpoint, 1.0);
}
