#![allow(clippy::float_cmp)] // The first samples are exact by construction (the operating point and zero delay), compared exactly on purpose.

use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::identify::ParameterInterval;

use super::*;
use crate::identification::plant::IdentifiedModel;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::OutputLimits;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::typed_pid::TypedDesign;

fn model(
    kind: LowOrderModelKind,
    gain: f64,
    time: Option<f64>,
    dead_time: Option<f64>,
) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds: time,
        second_time_constant_seconds: None,
        dead_time_seconds: dead_time,
        normalized_error: 0.0,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

fn conditions(
    period_seconds: f64,
    duration_seconds: f64,
    reference_change: f64,
) -> PredictionConditions {
    PredictionConditions {
        period_seconds,
        duration_seconds,
        reference_change,
        operating_input: 0.0,
        operating_output: 0.0,
        input_disturbance: 0.0,
        output_minimum: -1.0e9,
        output_maximum: 1.0e9,
        rate_minimum: -1.0e9,
        rate_maximum: 1.0e9,
    }
}

fn requirements(
    reference_change: f64,
    duration_seconds: f64,
    load: Option<f64>,
) -> TuningRequirements {
    TuningRequirements {
        reference_change,
        maximum_error: 0.2,
        settling_band: 0.1,
        settling_time_seconds: 4.0,
        duration_seconds,
        constant_input_disturbance: load,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

fn design(structure: Structure, proportional: f64, integral: f64) -> TypedDesign {
    TypedDesign {
        structure,
        gains: ParallelGains {
            proportional_gain: proportional,
            integral_gain: integral,
            derivative_gain: 0.0,
        },
    }
}

#[test]
fn the_prediction_starts_at_the_operating_point_and_steps_to_the_target() {
    // prediction-coordinates: K = 1, τ = 1, no delay, P = 2 with a bias of zero, operating point (2, 2), a step of 2
    // with a 0.02 s period over 4 s. The output settles at 8/3, as the positional law gives.
    let first_order = model(
        LowOrderModelKind::FirstOrderDeadTime,
        1.0,
        Some(1.0),
        Some(0.0),
    );
    let settings = PredictionConditions {
        period_seconds: 0.02,
        duration_seconds: 4.0,
        reference_change: 2.0,
        operating_input: 2.0,
        operating_output: 2.0,
        input_disturbance: 0.0,
        output_minimum: -20.0,
        output_maximum: 20.0,
        rate_minimum: -100.0,
        rate_maximum: 100.0,
    };
    let response = predict_response(
        &first_order,
        &PredictedGains::parallel(2.0, 0.0, 0.0),
        None,
        &settings,
    )
    .expect("the model and the conditions are valid");
    assert_eq!(response.points.len(), 201);
    assert_eq!(response.points[0].candidate_process_value, 2.0);
    assert_eq!(response.points[0].candidate_manipulated, 2.0);
    assert!((response.points[1].time_seconds - 0.02).abs() <= 1e-12);
    let last = response.points.last().expect("the response has points");
    assert!(
        (last.time_seconds - 4.0).abs() <= 1e-9,
        "{}",
        last.time_seconds
    );
    assert!(
        (last.candidate_process_value - 8.0 / 3.0).abs() <= 1e-4,
        "{}",
        last.candidate_process_value
    );
}

#[test]
fn the_integrating_shaft_peaks_at_the_published_values() {
    // integrating-prediction (examples/control/.../32-proportional-position-stiffness): J·θ'' + B·θ' = u with J = B = 1
    // is an integrator with a 1 s lag. Proportional control with 20 ms periods and a unit step over 8 s peaks at
    // 1.46 rad for Kp = 4 and 1.72 rad for Kp = 16 (the continuous responses are 1.44 and 1.67).
    let shaft = model(LowOrderModelKind::Integrating, 1.0, Some(1.0), None);
    let settings = PredictionConditions {
        period_seconds: 0.02,
        duration_seconds: 8.0,
        reference_change: 1.0,
        operating_input: 0.0,
        operating_output: 0.0,
        input_disturbance: 0.0,
        output_minimum: -20.0,
        output_maximum: 20.0,
        rate_minimum: -1.0e6,
        rate_maximum: 1.0e6,
    };
    let peak_of = |proportional: f64| {
        let response = predict_response(
            &shaft,
            &PredictedGains::parallel(proportional, 0.0, 0.0),
            None,
            &settings,
        )
        .expect("the shaft model and the conditions are valid");
        let peak = response
            .points
            .iter()
            .map(|point| point.candidate_process_value)
            .fold(f64::NEG_INFINITY, f64::max);
        (peak, response)
    };
    let (low_peak, low) = peak_of(4.0);
    let (high_peak, _) = peak_of(16.0);
    assert!((low_peak - 1.46).abs() < 0.01, "Kp 4 peak {low_peak}");
    assert!((high_peak - 1.72).abs() < 0.01, "Kp 16 peak {high_peak}");
    assert!(
        (low.points[1].candidate_manipulated - 4.0).abs() <= 1e-12,
        "the first command is Kp · 1 rad"
    );
    let last_time = low
        .points
        .last()
        .expect("the response has points")
        .time_seconds;
    assert!((last_time - 8.0).abs() <= 1e-9, "{last_time}");
}

#[test]
fn a_zero_delay_model_gains_no_period_of_delay() {
    // K/(s + 1) with P = 1, a unit step and a 0.1 s period: the command of the first period reaches the output in
    // that period, so the output after it is 1 − e^(−0.1), the first-order update from the command at once.
    let first_order = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let response = predict_response(
        &first_order,
        &PredictedGains::parallel(1.0, 0.0, 0.0),
        None,
        &conditions(0.1, 1.0, 1.0),
    )
    .expect("the model and the conditions are valid");
    let expected = 1.0 - (-0.1_f64).exp();
    assert!(
        (response.points[1].candidate_process_value - expected).abs() <= 1e-12,
        "{}",
        response.points[1].candidate_process_value
    );
}

#[test]
fn a_load_trial_settles_at_the_plant_gain_times_the_load() {
    // With a proportional gain of zero the controller adds nothing, so the load alone holds the output at K·load.
    let first_order = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let mut trial = conditions(0.01, 20.0, 0.0);
    trial.input_disturbance = 0.5;
    let response = predict_response(
        &first_order,
        &PredictedGains::parallel(0.0, 0.0, 0.0),
        None,
        &trial,
    )
    .expect("the model and the conditions are valid");
    let last = response
        .points
        .last()
        .expect("the response has points")
        .candidate_process_value;
    assert!((last - 0.5).abs() <= 1e-6, "{last}");
}

#[test]
fn the_current_settings_keep_their_own_series_on_the_same_conditions() {
    // K = 1, τ = 1: P = 1 settles at half of the step and P = 2 at two thirds, so the two series end apart.
    let first_order = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let response = predict_response(
        &first_order,
        &PredictedGains::parallel(2.0, 0.0, 0.0),
        Some(&PredictedGains::parallel(1.0, 0.0, 0.0)),
        &conditions(0.01, 30.0, 1.0),
    )
    .expect("the model and the conditions are valid");
    let last = response.points.last().expect("the response has points");
    assert!(
        (last.candidate_process_value - 2.0 / 3.0).abs() <= 1e-6,
        "{}",
        last.candidate_process_value
    );
    let current = last
        .current_process_value
        .expect("the current series is predicted");
    assert!((current - 0.5).abs() <= 1e-6, "{current}");
    assert!(last.current_manipulated.is_some());
}

#[test]
fn invalid_models_and_conditions_are_refused_with_their_sentences() {
    let proportional = PredictedGains::parallel(1.0, 0.0, 0.0);
    let valid = conditions(0.1, 1.0, 1.0);
    let gainless = model(LowOrderModelKind::FirstOrder, 0.0, Some(1.0), None);
    let refusal =
        predict_response(&gainless, &proportional, None, &valid).expect_err("zero gain is refused");
    assert_eq!(refusal.0, INVALID_MODEL);
    let no_lag = model(LowOrderModelKind::FirstOrder, 1.0, Some(0.0), None);
    let refusal =
        predict_response(&no_lag, &proportional, None, &valid).expect_err("zero lag is refused");
    assert_eq!(refusal.0, TIME_CONSTANT);
    let first_order = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let refusal = predict_response(
        &first_order,
        &proportional,
        None,
        &conditions(0.0, 1.0, 1.0),
    )
    .expect_err("a zero period is refused");
    assert_eq!(refusal.0, CONDITIONS);
    let refusal = predict_response(
        &first_order,
        &proportional,
        None,
        &conditions(0.01, 1.0e12, 1.0),
    )
    .expect_err("a horizon above the scan limit is refused");
    assert_eq!(refusal.0, CONDITIONS);
    let filtered = PredictedGains {
        measurement_filter_seconds: -1.0,
        ..proportional
    };
    let refusal = predict_response(&first_order, &filtered, None, &valid)
        .expect_err("a negative filter is refused");
    assert_eq!(refusal.0, FILTER);
    let not_finite = PredictedGains::parallel(f64::NAN, 0.0, 0.0);
    let refusal = predict_response(&first_order, &not_finite, None, &valid)
        .expect_err("a gain that is not finite is refused");
    assert_eq!(refusal.0, crate::tuning::texts::CANDIDATE_NOT_FINITE);
}

#[test]
fn the_selected_model_predictor_answers_the_setpoint_and_the_load_trials() {
    // N5.12: the setpoint trial steps the setpoint by the requirement's change from the operating output; the load trial
    // keeps the setpoint and applies the declared disturbance, and it is answered only when a load is declared.
    let first_order = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let predictor = SelectedModelPredictor {
        model: &first_order,
        task_period_ms: 10.0,
        operating_input: 0.0,
        operating_output: 0.0,
        output_limits: OutputLimits {
            minimum: -10.0,
            maximum: 10.0,
        },
        rate_limits: ParameterInterval {
            minimum: -1.0e6,
            maximum: 1.0e6,
        },
        shaping: RecordShaping::default(),
    };
    let step_requirements = requirements(1.0, 20.0, Some(0.5));
    let step = predictor
        .predict(
            &design(Structure::PI, 1.0, 1.0),
            Trial::SetpointChange,
            &step_requirements,
        )
        .expect("the setpoint trial is predicted");
    assert_eq!(step[0].time_seconds, 0.0);
    assert_eq!(step[0].process_value, 0.0);
    assert_eq!(step.len(), 2001);
    let final_value = step.last().expect("the trial has points").process_value;
    assert!((final_value - 1.0).abs() <= 1e-6, "{final_value}");
    // The proportional design's load trial settles at K·load, with the setpoint unchanged.
    let load = predictor
        .predict(
            &design(Structure::P, 0.0, 0.0),
            Trial::InputDisturbance,
            &step_requirements,
        )
        .expect("the load trial is predicted when a load is declared");
    let load_final = load.last().expect("the trial has points").process_value;
    assert!((load_final - 0.5).abs() <= 1e-6, "{load_final}");
    let no_load = requirements(1.0, 20.0, None);
    assert!(predictor
        .predict(
            &design(Structure::P, 0.0, 0.0),
            Trial::InputDisturbance,
            &no_load
        )
        .is_none());
}
