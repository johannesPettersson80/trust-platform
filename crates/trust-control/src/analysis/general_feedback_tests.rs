use super::*;
use crate::model::decode_general_model;
use serde_json::{json, Value};

use crate::analysis::declared::DeclaredControlAnalysis;
use crate::analysis::feedback_declared::declared_feedback_definition;
use crate::analysis::feedback_margins::report;
use trust_control_math::classical::{Sweep, Transfer};
use trust_control_math::frequency::logarithmic_grid;
use trust_control_math::nyquist::nyquist_stability;

fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -100.0, "maximum": 100.0 } })
}

fn model(value: &Value) -> GeneralModel {
    decode_general_model(value).expect("a valid model")
}

/// The custom shaft of control-studio-general-feedback.test.ts: a continuous state-space plant with
/// A = [[0, 1], [0, −8]], B = [0, 1]ᵀ and C = [1, 0], and a controller of gain 12.
fn shaft_plant() -> Value {
    json!({
        "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "continuous-state-space",
        "inputs": [channel("torque", "Nm")], "outputs": [channel("angle", "rad")],
        "states": [channel("angle", "rad"), channel("speed", "rad/s")],
        "a": [[0.0, 1.0], [0.0, -8.0]], "b": [[0.0], [1.0]], "c": [[1.0, 0.0]], "d": [[0.0]]
    })
}

fn gain_controller(numerator: f64) -> Value {
    json!({
        "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function",
        "inputs": [channel("error", "rad")], "outputs": [channel("command", "Nm")],
        "numerator": [numerator], "denominator": [1.0]
    })
}

fn definition(plant: &Value, controller: &Value) -> FeedbackDefinition {
    FeedbackDefinition {
        plant: model(plant),
        controller: model(controller),
        plant_channel: ChannelPair {
            input_id: "torque".to_string(),
            output_id: "angle".to_string(),
        },
        controller_channel: ChannelPair {
            input_id: "error".to_string(),
            output_id: "command".to_string(),
        },
        declared_controller: None,
    }
}

fn with_delay(mut value: Value, delay: f64) -> Value {
    value["delaySeconds"] = json!(delay);
    value
}

#[test]
fn a_custom_shaft_channel_closes_to_its_exact_fixed_dynamics_poles() {
    let result = analyze_general_feedback(
        &definition(&shaft_plant(), &gain_controller(12.0)),
        &[1.0, 2.0, 4.0],
    )
    .expect("the analysis");
    let ClosedLoop::Qualified { poles, stable, .. } = &result.closed_loop else {
        panic!("the closed loop qualifies");
    };
    assert!(stable);
    #[allow(clippy::cast_possible_truncation)] // The poles are small whole numbers by construction.
    let mut reals: Vec<i64> = poles.iter().map(|pole| pole.real.round() as i64).collect();
    reals.sort_unstable();
    assert_eq!(reals, vec![-6, -2]);
    // At ω = 1: L = 12/(s(s + 8)) = (−12 − 96j)/65.
    assert!((result.open_loop[0].real + 12.0 / 65.0).abs() < 1e-9);
    assert!((result.open_loop[0].imaginary + 96.0 / 65.0).abs() < 1e-9);
    assert!(result.nyquist_scope.contains("sampled"));
}

#[test]
fn a_pure_delay_keeps_the_poles_unavailable_without_inventing_them() {
    let result = analyze_general_feedback(
        &definition(&with_delay(shaft_plant(), 0.5), &gain_controller(12.0)),
        &[1.0, 2.0, 4.0],
    )
    .expect("the analysis");
    match result.closed_loop {
        ClosedLoop::Unavailable { reason } => assert!(reason.contains("delay")),
        ClosedLoop::Qualified { .. } => panic!("a delayed loop has no finite poles here"),
    }
}

#[test]
fn unit_and_domain_mismatches_and_a_singular_feedthrough_are_refused_before_closure() {
    let mut units = definition(&shaft_plant(), &gain_controller(12.0));
    units.controller = model(&json!({
        "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function", "inputs": [channel("error", "rad")], "outputs": [channel("command", "V")],
        "numerator": [12.0], "denominator": [1.0]
    }));
    assert_eq!(
        analyze_general_feedback(&units, &[1.0, 2.0]),
        Err(FeedbackRefusal::InvalidDefinition)
    );
    // The plant's feedthrough is 2 and the controller's is −0.5: 1 + 2·(−0.5) = 0.
    let algebraic = FeedbackDefinition {
        plant: model(&json!({
            "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
            "kind": "transfer-function", "inputs": [channel("torque", "Nm")], "outputs": [channel("angle", "rad")],
            "numerator": [2.0], "denominator": [1.0]
        })),
        controller: model(&gain_controller(-0.5)),
        plant_channel: ChannelPair {
            input_id: "torque".to_string(),
            output_id: "angle".to_string(),
        },
        controller_channel: ChannelPair {
            input_id: "error".to_string(),
            output_id: "command".to_string(),
        },
        declared_controller: None,
    };
    assert_eq!(
        analyze_general_feedback(&algebraic, &[1.0, 2.0]),
        Err(FeedbackRefusal::SingularAlgebraicLoop)
    );
}

#[test]
fn a_delayed_feedthrough_is_not_an_instantaneous_algebraic_loop() {
    let plant = json!({
        "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.5,
        "kind": "transfer-function", "inputs": [channel("torque", "Nm")], "outputs": [channel("angle", "rad")],
        "numerator": [2.0], "denominator": [1.0]
    });
    let input = FeedbackDefinition {
        plant: model(&plant),
        controller: model(&gain_controller(-0.5)),
        plant_channel: ChannelPair {
            input_id: "torque".to_string(),
            output_id: "angle".to_string(),
        },
        controller_channel: ChannelPair {
            input_id: "error".to_string(),
            output_id: "command".to_string(),
        },
        declared_controller: None,
    };
    let result = analyze_general_feedback(&input, &[1.0, 2.0]).expect("the analysis");
    assert!(matches!(result.closed_loop, ClosedLoop::Unavailable { .. }));
}

#[test]
fn a_finite_range_classical_margin_is_estimated_without_an_infinite_gain_margin() {
    // L = 2/(s + 1): the gain crossover is √3 and the phase margin is 120°; no phase crossover is in range.
    let input = FeedbackDefinition {
        plant: model(&json!({
            "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
            "kind": "transfer-function", "inputs": [channel("torque", "Nm")], "outputs": [channel("angle", "rad")],
            "numerator": [1.0], "denominator": [1.0, 1.0]
        })),
        controller: model(&gain_controller(2.0)),
        plant_channel: ChannelPair {
            input_id: "torque".to_string(),
            output_id: "angle".to_string(),
        },
        controller_channel: ChannelPair {
            input_id: "error".to_string(),
            output_id: "command".to_string(),
        },
        declared_controller: None,
    };
    let result = analyze_general_feedback(&input, &[0.1, 1.0, 2.0, 10.0]).expect("the analysis");
    match result.margins {
        Margins::Estimated {
            gain_crossover_rad_per_second,
            phase_margin_degrees,
            gain_margin_db,
            ..
        } => {
            assert!((gain_crossover_rad_per_second - 3f64.sqrt()).abs() < 0.01);
            assert!((phase_margin_degrees - 120.0).abs() < 0.1);
            assert_eq!(gain_margin_db, None);
        }
        Margins::Unavailable { reason } => panic!("the margins estimate: {reason}"),
    }
}

#[test]
fn a_delay_too_large_for_the_grid_leaves_the_margins_unavailable_and_keeps_the_nyquist_count() {
    let input = FeedbackDefinition {
        plant: model(&with_delay(
            json!({
                "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
                "kind": "transfer-function", "inputs": [channel("torque", "Nm")], "outputs": [channel("angle", "rad")],
                "numerator": [1.0], "denominator": [1.0, 1.0]
            }),
            100.0,
        )),
        controller: model(&gain_controller(2.0)),
        plant_channel: ChannelPair {
            input_id: "torque".to_string(),
            output_id: "angle".to_string(),
        },
        controller_channel: ChannelPair {
            input_id: "error".to_string(),
            output_id: "command".to_string(),
        },
        declared_controller: None,
    };
    let result = analyze_general_feedback(&input, &[0.1, 1.0, 10.0]).expect("the analysis");
    assert!(matches!(result.margins, Margins::Unavailable { .. }));
    assert!(
        result.nyquist.is_some(),
        "the Nyquist owner keeps the declared delay"
    );
}

#[test]
fn a_sampled_closed_loop_is_stable_only_inside_the_unit_circle() {
    // Ported: the plant x⁺ = 0.8x + 0.2u and the controller with D = 0.2 close to poles with real part 0.63.
    let sampled = |kind: &str,
                   time: f64,
                   a: f64,
                   b: f64,
                   c: f64,
                   d: f64,
                   input: &str,
                   output: &str,
                   unit_in: &str,
                   unit_out: &str| {
        json!({
            "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
            "kind": kind, "sampleTimeSeconds": time,
            "inputs": [channel(input, unit_in)], "outputs": [channel(output, unit_out)],
            "states": [channel("x", "rad")], "a": [[a]], "b": [[b]], "c": [[c]], "d": [[d]]
        })
    };
    let plant = sampled(
        "discrete-state-space",
        0.1,
        0.8,
        0.2,
        1.0,
        0.0,
        "torque",
        "angle",
        "Nm",
        "rad",
    );
    let controller = sampled(
        "discrete-state-space",
        0.1,
        0.5,
        0.1,
        1.0,
        0.2,
        "error",
        "command",
        "rad",
        "Nm",
    );
    let input = FeedbackDefinition {
        plant: model(&plant.clone()),
        controller: model(&controller),
        plant_channel: ChannelPair {
            input_id: "torque".to_string(),
            output_id: "angle".to_string(),
        },
        controller_channel: ChannelPair {
            input_id: "error".to_string(),
            output_id: "command".to_string(),
        },
        declared_controller: None,
    };
    let result = analyze_general_feedback(&input, &[1.0, 2.0]).expect("the analysis");
    let ClosedLoop::Qualified { poles, stable, .. } = result.closed_loop else {
        panic!("the sampled closed loop qualifies");
    };
    assert!(stable);
    assert!((poles[0].real - 0.63).abs() < 1e-9);
    let mismatched = sampled(
        "discrete-state-space",
        0.2,
        0.5,
        0.1,
        1.0,
        0.2,
        "error",
        "command",
        "rad",
        "Nm",
    );
    let refused = FeedbackDefinition {
        plant: model(&plant),
        controller: model(&mismatched),
        plant_channel: ChannelPair {
            input_id: "torque".to_string(),
            output_id: "angle".to_string(),
        },
        controller_channel: ChannelPair {
            input_id: "error".to_string(),
            output_id: "command".to_string(),
        },
        declared_controller: None,
    };
    assert_eq!(
        analyze_general_feedback(&refused, &[1.0, 2.0]),
        Err(FeedbackRefusal::InvalidDefinition)
    );
}

#[test]
fn a_saved_analysis_must_match_its_frequency_grid() {
    let input = definition(&shaft_plant(), &gain_controller(12.0));
    let frequencies = [1.0, 2.0, 4.0];
    let result = analyze_general_feedback(&input, &frequencies).expect("the analysis");
    let saved = SavedGeneralFeedback {
        definition: input,
        result,
    };
    assert!(check_saved_general_feedback(&saved, &frequencies).is_ok());
    assert!(check_saved_general_feedback(&saved, &[1.0, 2.0, 5.0]).is_err());
}

/// The sweep of the Nyquist counts of control-studio-numerics.test.ts: 10⁻⁴ to 10⁴ rad/s, 80 points per decade.
fn wide_sweep() -> Sweep {
    Sweep {
        minimum_rad_per_second: 1e-4,
        maximum_rad_per_second: 1e4,
        points_per_decade: 80,
    }
}

/// The sweep of the delayed-oven counts of control-studio-numerics.test.ts: from 10⁻⁴ rad/s to `maximum`, 500 points per decade.
fn oven_sweep(maximum: f64) -> Sweep {
    Sweep {
        minimum_rad_per_second: 1e-4,
        maximum_rad_per_second: maximum,
        points_per_decade: 500,
    }
}

/// The reason a Nyquist count of `transfer` on `sweep` is refused with, as the analysis reports it.
fn nyquist_refusal(transfer: &Transfer, sweep: Sweep) -> String {
    match report(nyquist_stability(transfer, sweep)) {
        NyquistReport::Rejected { reason } => reason,
        NyquistReport::Qualified { summary } => panic!("expected a refusal, got {summary:?}"),
    }
}

#[test]
fn a_pole_on_the_contour_refuses_the_nyquist_count() {
    // control-studio-numerics.test.ts ("qualifies classical stability analysis against polynomial and complex-number oracles"):
    // 2/(s² + s) has a pole at the origin, which lies on the contour.
    let transfer = Transfer {
        numerator: vec![2.0],
        denominator: vec![1.0, 1.0, 0.0],
        delay_seconds: 0.0,
    };
    assert_eq!(nyquist_refusal(&transfer, wide_sweep()), "pole-on-contour");
}

#[test]
fn a_delay_at_the_critical_point_and_a_sweep_that_does_not_close_refuse_the_nyquist_count() {
    // control-studio-numerics.test.ts ("counts delayed-oven Nyquist encirclements without losing phase rotations"): the contour
    // of 2/(20s + 1) with the delay at which it passes −1, and the same loop on a sweep that stops at 0.2 rad/s.
    let critical_delay = (std::f64::consts::PI - 3f64.sqrt().atan()) / (3f64.sqrt() / 20.0);
    let critical = Transfer {
        numerator: vec![2.0],
        denominator: vec![20.0, 1.0],
        delay_seconds: critical_delay,
    };
    assert_eq!(
        nyquist_refusal(&critical, oven_sweep(10.0)),
        "contour-near-critical-point"
    );
    let delayed = Transfer {
        numerator: vec![2.0],
        denominator: vec![20.0, 1.0],
        delay_seconds: 5.0,
    };
    assert_eq!(
        nyquist_refusal(&delayed, oven_sweep(0.2)),
        "insufficient-contour-closure"
    );
}

#[test]
fn the_nyquist_count_states_p_n_and_z_with_clockwise_encirclements_positive() {
    // control-studio-chart.test.ts ("the Nyquist count states P, N with clockwise encirclements positive, and Z = P + N"). The
    // sentence is built from the count's figures, which the test gives directly; `nyquist_count_text` is the analysis's own.
    let unstable = NyquistSummary {
        encirclements_counterclockwise: -2,
        open_loop_right_half_plane_poles: 0,
        closed_loop_right_half_plane_poles: 2,
        closed_loop_stable: false,
        minimum_distance_to_minus_one: 0.5,
        endpoint_magnitude: 0.0,
    };
    assert_eq!(
        nyquist_count_text(&unstable),
        "P = 0 open-loop right-half-plane poles. N = 2 clockwise encirclements of −1. Z = P + N = 2 closed-loop right-half-plane poles: the closed loop is unstable."
    );
    let stable = NyquistSummary {
        encirclements_counterclockwise: 0,
        open_loop_right_half_plane_poles: 0,
        closed_loop_right_half_plane_poles: 0,
        closed_loop_stable: true,
        minimum_distance_to_minus_one: 0.5,
        endpoint_magnitude: 0.0,
    };
    assert_eq!(
        nyquist_count_text(&stable),
        "P = 0 open-loop right-half-plane poles. N = 0 clockwise encirclements of −1. Z = P + N = 0 closed-loop right-half-plane poles: the closed loop is stable."
    );
}

/// The plant of control-studio-general-model-session.test.ts: 2/(s + 2) from `drive` in V to `speed` in rpm, with the revision `m`.
fn drive_plant(numerator: &[f64], denominator: &[f64]) -> GeneralModel {
    model(&json!({
        "revision": "m", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function",
        "inputs": [channel("drive", "V")], "outputs": [channel("speed", "rpm")],
        "numerator": numerator, "denominator": denominator
    }))
}

#[test]
fn a_declared_controller_law_closes_on_the_plant_and_keeps_the_law_beside_it() {
    // control-studio-general-model-session.test.ts ("routes a declared controller law through the bounded feedback worker"):
    // the law Kp with Kp = 2 on 2/(s + 2), analysed on the session's three-point grid from 1 to 10 rad/s.
    let analysis: DeclaredControlAnalysis = crate::json::decode_value(json!({
        "plant": { "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 2.0], "delaySeconds": 0.0,
            "inputUnit": "V", "outputUnit": "rpm", "revision": "declared-plant", "source": "provided", "validation": "validated" },
        "controller": { "numerator": [{ "parameter": "Kp" }], "denominator": [1.0],
            "parameters": { "Kp": { "value": 2.0, "minimum": 0.0, "maximum": 10.0, "unit": "V/rpm" } } },
        "rootLocus": { "parameter": "Kp", "values": [0.0, 2.0, 4.0], "selectedValue": 2.0 }
    }))
    .expect("the declared analysis");
    let definition = declared_feedback_definition(&analysis, &drive_plant(&[2.0], &[1.0, 2.0]))
        .expect("the selected plant and the declared law give a feedback definition");
    assert_eq!(
        definition
            .declared_controller
            .as_ref()
            .map(|law| law.numerator.clone()),
        Some(vec![2.0]),
        "the declared law is kept beside the plant"
    );
    let grid = [1.0, 10f64.sqrt(), 10.0];
    let result =
        analyze_general_feedback(&definition, &grid).expect("the declared loop is analysed");
    assert!(
        matches!(
            result.closed_loop,
            ClosedLoop::Qualified { stable: true, .. }
        ),
        "{:?}",
        result.closed_loop
    );
    assert!(result
        .open_loop
        .iter()
        .all(|point| [point.real, point.imaginary, point.magnitude]
            .iter()
            .all(|value| value.is_finite())));
    assert!(result
        .sensitivity
        .as_ref()
        .is_some_and(|points| points.iter().all(|point| [
            point.sensitivity_magnitude_db,
            point.complementary_sensitivity_magnitude_db,
            point.control_sensitivity_magnitude_db
        ]
        .iter()
        .all(|value| value.is_finite()))));
    assert!(
        result.nyquist.is_some(),
        "the delay-free declared loop goes through the shared Nyquist count"
    );
    let scope = result.nyquist_scope.to_lowercase();
    assert!(
        ["contour", "sampled", "curve"]
            .iter()
            .any(|word| scope.contains(*word)),
        "{scope}"
    );
}

#[test]
fn an_improper_declared_derivative_plus_proportional_law_keeps_its_law_and_its_control_sensitivity()
{
    // control-studio-general-model-session.test.ts ("retains and analyzes an improper declared derivative plus proportional law"):
    // the law 3s + 2 (Kd = 3, Kp = 2) on 2/(s + 2). Its control sensitivity at 1 rad/s is 20·log10(|2 + 3j| / |3.8 + 1.6j|) dB.
    let analysis: DeclaredControlAnalysis = crate::json::decode_value(json!({
        "plant": { "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 1.0, 0.0], "delaySeconds": 0.0,
            "inputUnit": "V", "outputUnit": "rpm", "revision": "declared-pd", "source": "provided", "validation": "validated" },
        "controller": { "numerator": [{ "parameter": "Kd" }, { "parameter": "Kp" }], "denominator": [1.0],
            "parameters": {
                "Kp": { "value": 2.0, "minimum": 0.0, "maximum": 10.0, "unit": "V/rpm" },
                "Kd": { "value": 3.0, "minimum": 3.0, "maximum": 3.0, "unit": "V·s/rpm" } } },
        "rootLocus": { "parameter": "Kp", "values": [0.0, 2.0, 4.0], "selectedValue": 2.0 }
    }))
    .expect("the declared analysis");
    let definition = declared_feedback_definition(&analysis, &drive_plant(&[2.0], &[1.0, 2.0]))
        .expect("the selected plant and the PD law give a feedback definition");
    assert_eq!(
        definition.plant.identity().revision,
        "m",
        "the selected plant is kept"
    );
    assert_eq!(
        definition
            .declared_controller
            .as_ref()
            .map(|law| law.numerator.clone()),
        Some(vec![3.0, 2.0])
    );
    let result =
        analyze_general_feedback(&definition, &[1.0, 10.0]).expect("the declared loop is analysed");
    let at_one = result
        .sensitivity
        .as_ref()
        .and_then(|points| {
            points
                .iter()
                .find(|point| (point.frequency_rad_per_second - 1.0).abs() < 1e-12)
        })
        .expect("the sensitivity at 1 rad/s");
    let expected = 20.0 * (2f64.hypot(3.0) / 3.8f64.hypot(1.6)).log10();
    assert!(
        (at_one.control_sensitivity_magnitude_db - expected).abs() < 1e-9,
        "{} against {expected}",
        at_one.control_sensitivity_magnitude_db
    );

    // The unstable case of the same test: the law −3 on 1/(s + 1) closes on the positive pole s = 2.
    let unstable: DeclaredControlAnalysis = crate::json::decode_value(json!({
        "plant": { "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 1.0], "delaySeconds": 0.0,
            "inputUnit": "V", "outputUnit": "rpm", "revision": "declared-unstable", "source": "provided", "validation": "validated" },
        "controller": { "numerator": [-3.0], "denominator": [1.0],
            "parameters": {
                "Kp": { "value": -3.0, "minimum": -3.0, "maximum": 0.0, "unit": "V/rpm" },
                "Kd": { "value": 0.0, "minimum": 0.0, "maximum": 0.0, "unit": "V·s/rpm" } } },
        "rootLocus": { "parameter": "Kp", "values": [-3.0], "selectedValue": -3.0 }
    }))
    .expect("the unstable declared analysis");
    let definition = declared_feedback_definition(&unstable, &drive_plant(&[1.0], &[1.0, 1.0]))
        .expect("the unstable law gives a feedback definition");
    let result =
        analyze_general_feedback(&definition, &[1.0, 10.0]).expect("the declared loop is analysed");
    match result.closed_loop {
        ClosedLoop::Qualified { poles, .. } => assert!(
            poles.iter().any(|pole| pole.real > 1.0),
            "declared closed-loop roots keep their positive pole: {poles:?}"
        ),
        ClosedLoop::Unavailable { reason } => {
            panic!("the closed loop is unavailable: {reason}")
        }
    }
}

#[test]
fn a_siso_continuous_state_space_plant_closes_with_the_declared_law_on_its_full_denominator() {
    // control-studio-general-model: "connects supported SISO continuous state-space models to declared analysis" (specification 35, B9.9:
    // the feedback tool closes the selected model with the loop's declared law; C0: the analysis uses the model as Identify accepted it
    // and never rescales it). The selected model x' = [[0, 1], [−2, −3]]x + [0, 1]ᵀu, y = [1, 0]x is 1/(s² + 3s + 2), whatever plant
    // the declared analysis names.
    let selected = model(&json!({
        "revision": "engineer-model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "continuous-state-space",
        "inputs": [channel("u", "V")], "outputs": [channel("y", "m")],
        "states": [channel("x1", "m"), channel("x2", "m/s")],
        "a": [[0.0, 1.0], [-2.0, -3.0]], "b": [[0.0], [1.0]], "c": [[1.0, 0.0]], "d": [[0.0]]
    }));
    let analysis: DeclaredControlAnalysis = crate::json::decode_value(json!({
        "plant": { "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 1.0], "delaySeconds": 0.0,
            "inputUnit": "V", "outputUnit": "m", "revision": "declared", "source": "provided", "validation": "validated" },
        "controller": { "numerator": [{ "parameter": "Kp" }], "denominator": [1.0],
            "parameters": { "Kp": { "value": 1.0, "minimum": 0.0, "maximum": 2.0, "unit": "V/m" } } },
        "rootLocus": { "parameter": "Kp", "values": [1.0], "selectedValue": 1.0 }
    }))
    .expect("the declared analysis");
    let definition = declared_feedback_definition(&analysis, &selected)
        .expect("a supported SISO state-space model connects to the declared law");
    assert_eq!(
        definition.plant.identity().revision,
        "engineer-model-1",
        "the analysis uses the selected model's identity"
    );
    let plant =
        linearised(&definition.plant).expect("the state-space plant has a transfer function");
    assert_eq!(
        plant.denominator,
        vec![1.0, 3.0, 2.0],
        "its full state denominator is kept, not the declared plant's"
    );
    assert_eq!(plant.numerator, vec![1.0]);
    let result =
        analyze_general_feedback(&definition, &[1.0, 10.0]).expect("the declared loop is analysed");
    match result.closed_loop {
        // The characteristic polynomial (s² + 3s + 2) + 1·Kp is s² + 3s + 3, whose roots −1.5 ± 0.87j lie in the left half-plane.
        ClosedLoop::Qualified { poles, stable, .. } => {
            assert_eq!(poles.len(), 2);
            assert!(stable, "{poles:?}");
        }
        ClosedLoop::Unavailable { reason } => {
            panic!("the closed loop is unavailable: {reason}")
        }
    }
}

/// The plant 1/(s + 1) from `torque` to `angle` with a delay of one second.
fn delayed_first_order_plant() -> Value {
    json!({
        "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 1.0,
        "kind": "transfer-function", "inputs": [channel("torque", "Nm")], "outputs": [channel("angle", "rad")],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    })
}

/// The PI controller 0.5·(s + 1)/s, that is Kp = 0.5 and Ti = 1 s, from `error` to `command`.
fn pi_controller() -> Value {
    json!({
        "revision": "model-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function", "inputs": [channel("error", "rad")], "outputs": [channel("command", "Nm")],
        "numerator": [0.5, 0.5], "denominator": [1.0, 0.0]
    })
}

#[test]
fn a_delayed_loop_plots_a_continuous_phase_and_keeps_its_margins() {
    // N4.3, N7.7 and N7.8: the plant e^(−s)/(s + 1) with the PI controller 0.5·(s + 1)/s, whose zero cancels the plant's pole:
    // L(s) = 0.5·e^(−s)/s, with magnitude 0.5/ω and phase −90° − ω rad. On 64 points from 0.01 to 10 rad/s:
    // - the plotted phase at 10 rad/s is −90° − 10 rad = −90° − 572.957795° = −662.957795°, and no step reaches 180° (the
    //   widest step, 1.04 rad/s, turns it by about 60°);
    // - the gain crossover is 0.5/ω = 1 at ω = 0.5 rad/s, where the phase is −90° − 0.5 rad = −118.647890°, so the phase
    //   margin is 61.352110°;
    // - the phase crossover is −90° − ω rad = −180° at ω = π/2 rad/s, where |L| = 1/π, so the gain margin is
    //   20·log10(π) = 9.942997 dB.
    // The margins are read on the loop's response, not on the plotted phase. The bounded estimate interpolates between its
    // samples, 100 to the decade, which keeps it within about 0.001° and 0.001 dB of these values. Catches a wrapped loop phase,
    // and a margin that moves with the plotted phase.
    let frequencies = logarithmic_grid(0.01, 10.0, 64).expect("the grid");
    let result = analyze_general_feedback(
        &definition(&delayed_first_order_plant(), &pi_controller()),
        &frequencies,
    )
    .expect("the analysis");
    let last = result.open_loop.last().expect("the loop has points");
    assert!(
        (last.phase_degrees + 662.957_795_130_823).abs() < 1e-9,
        "{}",
        last.phase_degrees
    );
    for pair in result.open_loop.windows(2) {
        let step = pair[1].phase_degrees - pair[0].phase_degrees;
        assert!(
            step.abs() < 180.0,
            "a step of {step}° at {} rad/s",
            pair[1].frequency_rad_per_second
        );
    }
    for point in &result.open_loop {
        let omega = point.frequency_rad_per_second;
        assert!(
            (point.phase_degrees - (-90.0 - omega.to_degrees())).abs() < 1e-9,
            "{omega}: {}",
            point.phase_degrees
        );
        assert!((point.phase_radians - point.phase_degrees.to_radians()).abs() < 1e-12);
    }
    match result.margins {
        Margins::Estimated {
            gain_crossover_rad_per_second,
            phase_margin_degrees,
            gain_margin_db,
            ..
        } => {
            assert!(
                (gain_crossover_rad_per_second - 0.5).abs() < 1e-6,
                "{gain_crossover_rad_per_second}"
            );
            assert!(
                (phase_margin_degrees - 61.352_110_243_459).abs() < 0.01,
                "{phase_margin_degrees}"
            );
            let gain_margin = gain_margin_db.expect("the phase crosses −180° in range");
            assert!(
                (gain_margin - 9.942_997_453_883).abs() < 0.01,
                "{gain_margin}"
            );
        }
        Margins::Unavailable { reason } => panic!("the margins estimate: {reason}"),
    }
}

#[test]
fn on_a_coarse_grid_the_loop_delay_phase_is_added_in_closed_form() {
    // N4.3: the same loop on 0.1, 1 and 10 rad/s has the plotted phase −90° − ω rad: −95.729578°, −147.295780° and
    // −662.957795°. Catches a plain unwrap of the loop's response: its principal phase at 10 rad/s is 57.04°, which that unwrap
    // reads as −302.96°, a turn above the plotted phase.
    let result = analyze_general_feedback(
        &definition(&delayed_first_order_plant(), &pi_controller()),
        &[0.1, 1.0, 10.0],
    )
    .expect("the analysis");
    let phases: Vec<f64> = result
        .open_loop
        .iter()
        .map(|point| point.phase_degrees)
        .collect();
    let expected = [
        -95.729_577_951_308,
        -147.295_779_513_082,
        -662.957_795_130_823,
    ];
    assert_eq!(phases.len(), expected.len());
    for (phase, expected) in phases.iter().zip(expected) {
        assert!((phase - expected).abs() < 1e-9, "{phases:?}");
    }
}

#[test]
fn a_declared_law_on_a_delayed_plant_plots_the_delay_in_closed_form() {
    // N4.3 and N7.6: the law Kp = 2 on the selected plant 2·e^(−s)/(s + 2) gives L(s) = 4·e^(−s)/(s + 2), with the plotted
    // phase −atan(ω/2) − ω rad. On 1, √10 and 10 rad/s: −26.565051° − 57.295780° = −83.860831°, −57.688467° − 181.185164° =
    // −238.873630°, and −78.690068° − 572.957795° = −651.647863°. Catches a declared loop whose delay is unwrapped with its
    // response: the principal phases −83.86°, 121.13° and 68.35° unwrap to −83.86°, −238.87° and −291.65°, a turn above the
    // plotted phase at 10 rad/s.
    let analysis: DeclaredControlAnalysis = crate::json::decode_value(json!({
        "plant": { "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 2.0], "delaySeconds": 0.0,
            "inputUnit": "V", "outputUnit": "rpm", "revision": "declared-plant", "source": "provided", "validation": "validated" },
        "controller": { "numerator": [{ "parameter": "Kp" }], "denominator": [1.0],
            "parameters": { "Kp": { "value": 2.0, "minimum": 0.0, "maximum": 10.0, "unit": "V/rpm" } } },
        "rootLocus": { "parameter": "Kp", "values": [0.0, 2.0, 4.0], "selectedValue": 2.0 }
    }))
    .expect("the declared analysis");
    let delayed_drive = model(&json!({
        "revision": "m", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 1.0,
        "kind": "transfer-function",
        "inputs": [channel("drive", "V")], "outputs": [channel("speed", "rpm")],
        "numerator": [2.0], "denominator": [1.0, 2.0]
    }));
    let definition = declared_feedback_definition(&analysis, &delayed_drive)
        .expect("the delayed plant and the declared law give a feedback definition");
    let result = analyze_general_feedback(&definition, &[1.0, 10f64.sqrt(), 10.0])
        .expect("the declared loop is analysed");
    assert_eq!(result.controller_source, ControllerSource::DeclaredLaw);
    let phases: Vec<f64> = result
        .open_loop
        .iter()
        .map(|point| point.phase_degrees)
        .collect();
    let expected = [
        -83.860_830_690_16,
        -238.873_630_338_73,
        -651.647_862_656_803,
    ];
    assert_eq!(phases.len(), expected.len());
    for (phase, expected) in phases.iter().zip(expected) {
        assert!((phase - expected).abs() < 1e-9, "{phases:?}");
    }
}
