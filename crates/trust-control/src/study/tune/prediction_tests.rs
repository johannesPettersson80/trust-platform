//! Tests of the predicted response of a Tune candidate (specification 35, B4.3, B4.5 and B9.6; numerical annex N8.3): the
//! settling time and the overshoot on hand-built responses, the traces of a SIMC PI on a first-order model with delay, the
//! two traces of the settings of the running controller (B4.5), and the reason of a candidate that has no prediction.

use trust_control_math::identify::{LowOrderModelKind, ParameterInterval};

use super::*;
use crate::identification::identify_result::tests::plant_loop;
use crate::loop_definition::ControllerLaw;
use crate::tuning::candidate::DEFAULT_RATE_LIMIT_PER_S;
use crate::tuning::prediction::RecordShaping;
use crate::tuning::settings_pins::mapped_parameters;
use crate::tuning::shortfall::OutputLimits;

/// The first-order model with delay of these tests: `1.5·exp(−2·s)/(4·s + 1)`.
fn first_order() -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::FirstOrderDeadTime,
        gain: 1.5,
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(2.0),
        normalized_error: Some(0.05),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

/// The predictor of `model` on a 100 ms task from the operating point zero, with outputs between −100 and 100.
fn predictor(model: &IdentifiedModel) -> SelectedModelPredictor<'_> {
    SelectedModelPredictor {
        model,
        task_period_ms: 100.0,
        operating_input: 0.0,
        operating_output: 0.0,
        output_limits: OutputLimits {
            minimum: -100.0,
            maximum: 100.0,
        },
        rate_limits: ParameterInterval {
            minimum: -DEFAULT_RATE_LIMIT_PER_S,
            maximum: DEFAULT_RATE_LIMIT_PER_S,
        },
        shaping: RecordShaping::default(),
    }
}

/// A unit setpoint change with 0.04 of allowed error and a band of 0.02, settled by 40 s of an 80 s test.
fn requirements() -> TuningRequirements {
    TuningRequirements {
        reference_change: 1.0,
        maximum_error: 0.04,
        settling_band: 0.02,
        settling_time_seconds: 40.0,
        duration_seconds: 80.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

/// The Balanced SIMC PI of the first-order model: `Kp = 4/9`, `Ti = 4 s`.
fn simc_pi() -> ParallelGains {
    ParallelGains {
        proportional_gain: 4.0 / 9.0,
        integral_gain: 1.0 / 9.0,
        derivative_gain: 0.0,
    }
}

/// A response with the given process values, one per second from time zero.
fn response(values: &[f64]) -> Vec<ResponsePoint> {
    values
        .iter()
        .zip(0_u32..)
        .map(|(&value, second)| ResponsePoint {
            time_seconds: f64::from(second),
            setpoint: 1.0,
            candidate_process_value: value,
            candidate_manipulated: 0.0,
            current_process_value: None,
            current_manipulated: None,
        })
        .collect()
}

#[test]
fn the_settling_time_is_the_arrival_in_the_band_for_good() {
    // Outside the ±0.1 band at 0 s and 1 s, inside from 2 s on: the response arrives at 2 s, also when that is after a
    // deadline (the outcome reports the miss; the time stays visible).
    let settled = response(&[0.0, 0.5, 0.95, 1.02, 0.99]);
    assert_eq!(settling_time_seconds(&settled, 1.0, 0.1), Some(2.0));
    let reenters = response(&[0.0, 0.95, 1.3, 1.05, 1.0]);
    assert_eq!(
        settling_time_seconds(&reenters, 1.0, 0.1),
        Some(3.0),
        "a sample inside the band before a later excursion is not the arrival"
    );
    let leaves = response(&[0.0, 0.95, 1.0, 1.2]);
    assert_eq!(
        settling_time_seconds(&leaves, 1.0, 0.1),
        None,
        "the last sample is outside the band"
    );
    let inside = response(&[1.0, 1.05]);
    assert_eq!(settling_time_seconds(&inside, 1.0, 0.1), Some(0.0));
}

#[test]
fn the_overshoot_is_the_excursion_beyond_the_target_in_the_direction_of_the_step() {
    let rising = response(&[0.0, 1.2, 1.0]);
    let percent = overshoot_percent(&rising, 1.0, 1.0).expect("a response");
    assert!((percent - 20.0).abs() < 1e-9, "{percent}");
    // A step down overshoots below its target: −1.1 against −1 is 10 % of the change.
    let falling = response(&[0.0, -1.1, -1.0]);
    let percent = overshoot_percent(&falling, -1.0, -1.0).expect("a response");
    assert!((percent - 10.0).abs() < 1e-9, "{percent}");
    let monotone = response(&[0.0, 0.5, 0.9]);
    assert_eq!(overshoot_percent(&monotone, 1.0, 1.0), Some(0.0));
    assert_eq!(overshoot_percent(&monotone, 1.0, 0.0), None);
}

#[test]
fn the_predicted_response_of_a_simc_pi_has_the_loop_traces_and_settles() {
    let model = first_order();
    let loop_definition = plant_loop();
    let predicted = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &requirements(),
        TraceNames::of(&loop_definition),
        None,
    );
    assert_eq!(predicted.unavailable, None);
    let response = predicted.response.expect("a predicted response");
    // The time zero sample, then one sample per 100 ms period over the 80 s test.
    assert_eq!(response.time_seconds.len(), 801);
    let names: Vec<(TuneRole, &str, &str)> = response
        .traces
        .iter()
        .map(|trace| (trace.role, trace.symbol.as_str(), trace.unit.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            (TuneRole::Setpoint, "Plant.Target", "m"),
            (TuneRole::Measurement, "Plant.Position", "m"),
            (TuneRole::Command, "Plant.Command", "V"),
        ]
    );
    assert!(response
        .traces
        .iter()
        .all(|trace| trace.values.len() == response.time_seconds.len()));
    // The loop is L(s) = exp(−2·s)/(6·s): it rises without overshoot and enters the ±0.02 band near 15.7 s.
    let settling = predicted
        .settling_time_seconds
        .expect("it settles by the 40 s deadline");
    assert!((14.5..=17.0).contains(&settling), "{settling}");
    let overshoot = predicted.overshoot_percent.expect("a predicted response");
    assert!((0.0..1.0).contains(&overshoot), "{overshoot}");
}

#[test]
fn a_candidate_without_a_prediction_says_why() {
    let model = IdentifiedModel {
        stable: false,
        ..first_order()
    };
    let loop_definition = plant_loop();
    let predicted = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &requirements(),
        TraceNames::of(&loop_definition),
        None,
    );
    assert_eq!(predicted.response, None);
    assert_eq!(
        predicted.unavailable.as_deref(),
        Some("Engineering plots require a valid stable plant model.")
    );
    assert_eq!(
        (predicted.settling_time_seconds, predicted.overshoot_percent),
        (None, None)
    );
    let outcome = candidate_outcome(
        Outcome::Unpredicted,
        &predictor(&model),
        simc_pi(),
        &requirements(),
    );
    assert_eq!(
        outcome,
        CandidateOutcome::Unpredicted {
            reason: "Engineering plots require a valid stable plant model.".to_string()
        }
    );
}

#[test]
fn a_long_prediction_keeps_its_extremes_for_the_plot_and_its_numbers_from_every_period() {
    let model = first_order();
    let loop_definition = plant_loop();
    let short = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &requirements(),
        TraceNames::of(&loop_definition),
        None,
    );
    // Two hours at the 100 ms period: 72,001 predicted points, more than the plot keeps (B4.5).
    let long_requirements = TuningRequirements {
        duration_seconds: 7_200.0,
        ..requirements()
    };
    let long = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &long_requirements,
        TraceNames::of(&loop_definition),
        None,
    );
    let response = long.response.expect("a predicted response");
    assert!(
        response.time_seconds.len() <= 2 * 3 * 1_000 + 2,
        "{} points",
        response.time_seconds.len()
    );
    assert!(response
        .traces
        .iter()
        .all(|trace| trace.values.len() == response.time_seconds.len()));
    assert_eq!(response.time_seconds.first(), Some(&0.0));
    assert!(response
        .time_seconds
        .last()
        .is_some_and(|last| (last - 7_200.0).abs() < 1e-6));
    // The numbers come from every period, so they match the short test's. The plot loses no peak: its highest measurement
    // is at least the highest of the first 80 s, which the short test holds in full (this response rises without overshoot,
    // so its highest value is its last).
    assert_eq!(long.settling_time_seconds, short.settling_time_seconds);
    assert_eq!(long.overshoot_percent, short.overshoot_percent);
    let peak = |response: &TuneResponse| {
        response.traces[1]
            .values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let short_response = short.response.expect("a predicted response");
    assert!(peak(&response) >= peak(&short_response) - 1e-12);
}

#[test]
fn the_current_settings_add_two_traces_on_the_candidates_time_base_and_leave_the_candidate_alone() {
    let model = first_order();
    let loop_definition = plant_loop();
    let running = PredictedGains::parallel(0.5, 0.05, 0.0);
    let compared = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &requirements(),
        TraceNames::of(&loop_definition),
        Some(running),
    );
    let alone = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &requirements(),
        TraceNames::of(&loop_definition),
        None,
    );
    let response = compared.response.expect("a predicted response");
    let roles: Vec<TuneRole> = response.traces.iter().map(|trace| trace.role).collect();
    assert_eq!(
        roles,
        [
            TuneRole::Setpoint,
            TuneRole::Measurement,
            TuneRole::Command,
            TuneRole::CurrentMeasurement,
            TuneRole::CurrentCommand,
        ]
    );
    // The current traces carry the loop's measurement and command, with their units, on the candidate's times.
    let symbols: Vec<(&str, &str)> = response.traces[3..]
        .iter()
        .map(|trace| (trace.symbol.as_str(), trace.unit.as_str()))
        .collect();
    assert_eq!(symbols, [("Plant.Position", "m"), ("Plant.Command", "V")]);
    assert!(response
        .traces
        .iter()
        .all(|trace| trace.values.len() == response.time_seconds.len()));
    // The candidate keeps its traces, numbers and time base, as it has them without the current settings.
    let alone_response = alone.response.expect("a predicted response");
    assert_eq!(response.time_seconds, alone_response.time_seconds);
    assert_eq!(response.traces[..3], alone_response.traces[..]);
    assert_eq!(compared.settling_time_seconds, alone.settling_time_seconds);
    assert_eq!(compared.overshoot_percent, alone.overshoot_percent);
    // Weaker gains are another run, so the current measurement is not the candidate's.
    assert_ne!(response.traces[3].values, response.traces[1].values);
}

#[test]
fn a_running_controller_that_cannot_be_simulated_leaves_its_traces_out() {
    let model = first_order();
    let loop_definition = plant_loop();
    // A negative measurement filter time is refused by the simulation, so the running settings have no run to plot.
    let refused = PredictedGains {
        measurement_filter_seconds: -1.0,
        ..PredictedGains::parallel(0.5, 0.05, 0.0)
    };
    let compared = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &requirements(),
        TraceNames::of(&loop_definition),
        Some(refused),
    );
    let alone = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &requirements(),
        TraceNames::of(&loop_definition),
        None,
    );
    let response = compared.response.as_ref().expect("a predicted response");
    assert_eq!(
        response.traces.len(),
        3,
        "the candidate's three traces stand"
    );
    assert_eq!(compared.response, alone.response);
    assert_eq!(compared.settling_time_seconds, alone.settling_time_seconds);
}

#[test]
fn the_plot_keeps_the_extremes_of_the_current_traces_as_well() {
    let model = first_order();
    let loop_definition = plant_loop();
    // A lightly damped running controller: its measurement overshoots, so its extremes lie inside the test.
    let running = PredictedGains::parallel(1.0, 0.2, 0.0);
    // Two hours at the 100 ms period, as the long-prediction test runs them: more points than the plot keeps (B4.5).
    let long_requirements = TuningRequirements {
        duration_seconds: 7_200.0,
        ..requirements()
    };
    let predicted = candidate_prediction(
        &predictor(&model),
        simc_pi(),
        &long_requirements,
        TraceNames::of(&loop_definition),
        Some(running),
    );
    let response = predicted.response.expect("a predicted response");
    assert!(
        response.time_seconds.len() <= 2 * 5 * PLOT_GROUPS + 2,
        "{} points",
        response.time_seconds.len()
    );
    assert!(response
        .traces
        .iter()
        .all(|trace| trace.values.len() == response.time_seconds.len()));
    // The whole running run, as the prediction holds it: the reduced traces keep its highest measurement and its lowest command.
    let conditions = trial_conditions(
        &predictor(&model),
        &long_requirements,
        long_requirements.reference_change,
        0.0,
    );
    let whole = predict_response(
        &model,
        &predictor(&model).predicted_gains(simc_pi()),
        Some(&running),
        &conditions,
    )
    .expect("both runs simulate");
    let highest = whole
        .points
        .iter()
        .filter_map(|point| point.current_process_value)
        .fold(f64::NEG_INFINITY, f64::max);
    let lowest = whole
        .points
        .iter()
        .filter_map(|point| point.current_manipulated)
        .fold(f64::INFINITY, f64::min);
    let kept_highest = response.traces[3]
        .values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let kept_lowest = response.traces[4]
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    assert_eq!((kept_highest, kept_lowest), (highest, lowest));
}

#[test]
fn the_running_settings_are_the_gains_their_bound_parameters_hold_with_the_weights_they_hold() {
    let model = first_order();
    let mut loop_definition = LoopDefinition::decode(include_str!(
        "../../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");
    // The example's law names no setpoint weight, so this test names the bound one, whose running value then counts.
    if let Some(ControllerLaw::Pid(pid)) = &mut loop_definition.controller.law {
        pid.proportional_setpoint_weight_symbol =
            Some("P1.Parameters.ProportionalSetpointWeight".to_owned());
    }
    let mapped = mapped_parameters(&loop_definition);
    let on_model = predictor(&model);
    let running = BTreeMap::from([
        ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
        ("P1.Parameters.IntegralGain".to_owned(), 0.25),
        ("P1.Parameters.ProportionalSetpointWeight".to_owned(), 0.5),
    ]);
    assert_eq!(
        current_prediction(&on_model, &loop_definition, &mapped, &running),
        Some(PredictedGains {
            proportional_gain: 2.0,
            integral_gain: 0.25,
            derivative_gain: 0.0,
            proportional_setpoint_weight: 0.5,
            derivative_setpoint_weight: 0.0,
            measurement_filter_seconds: 0.0,
        })
    );
    // Without a proportional gain there is no comparison to predict.
    let integral_only = BTreeMap::from([("P1.Parameters.IntegralGain".to_owned(), 0.25)]);
    assert_eq!(
        current_prediction(&on_model, &loop_definition, &mapped, &integral_only),
        None
    );
}
