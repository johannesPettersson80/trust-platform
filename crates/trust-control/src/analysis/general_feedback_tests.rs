use super::*;
use crate::model::decode_general_model;
use serde_json::{json, Value};

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
