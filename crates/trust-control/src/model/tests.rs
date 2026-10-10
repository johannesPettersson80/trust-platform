#![allow(clippy::float_cmp)] // Exact coefficient lists are compared on purpose.

use serde_json::{json, Value};
use trust_control_math::linalg::DenseMatrix;
use trust_control_math::model::TransferFunction;

use super::*;

fn channel(id: &str, unit: &str) -> Value {
    json!({"id": id, "unit": unit, "operatingRange": {"minimum": -10.0, "maximum": 10.0}})
}

/// The `TypeScript` `base()` model: a continuous state-space model with two inputs and one state.
fn base() -> Value {
    json!({
        "revision": "engineer-model-1", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("heater", "W"), channel("fan", "V")],
        "outputs": [channel("temperature", "degC")],
        "states": [channel("thermal", "degC")],
        "kind": "continuous-state-space", "a": [[-2.0]], "b": [[1.0, 3.0]],
        "c": [[2.0]], "d": [[0.0, 1.0]]
    })
}

/// A single-input, single-output, two-state model with the given state matrices.
fn siso_two_state(a: Value, b: Value, c: Value) -> Value {
    let mut model = base();
    model["inputs"] = json!([channel("u", "V")]);
    model["outputs"] = json!([channel("y", "m")]);
    model["states"] = json!([channel("x1", "m"), channel("x2", "m/s")]);
    model["a"] = a;
    model["b"] = b;
    model["c"] = c;
    model["d"] = json!([[0.0]]);
    model
}

fn transfer_of(model: &GeneralModel) -> &TransferFunction {
    match model {
        GeneralModel::TransferFunction { transfer, .. } => transfer,
        other => panic!("a transfer function was expected, got {other:?}"),
    }
}

fn assert_matrix_close(actual: &DenseMatrix, expected: &[Vec<f64>], tolerance: f64) {
    assert_eq!(actual.len(), expected.len(), "row count");
    for (row, (actual_row, expected_row)) in actual.iter().zip(expected).enumerate() {
        for (column, (value, reference)) in actual_row.iter().zip(expected_row).enumerate() {
            assert!(
                (value - reference).abs() <= tolerance,
                "entry ({row}, {column}): {value} differs from {reference}"
            );
        }
    }
}

#[test]
fn a_state_space_model_converts_to_its_transfer_polynomials_without_cancelling_hidden_modes() {
    // general-model.test.ts: converts SISO continuous state-space without cancelling hidden modes.
    let model = siso_two_state(
        json!([[0.0, 1.0], [-2.0, -3.0]]),
        json!([[0.0], [1.0]]),
        json!([[1.0, 0.0]]),
    );
    let converted = linear_transfer_model(&model).expect("a SISO model converts");
    let transfer = transfer_of(&converted);
    assert_eq!(transfer.denominator, vec![1.0, 3.0, 2.0]);
    assert_eq!(transfer.numerator, vec![1.0]);
    assert_eq!(converted.identity().revision, "engineer-model-1");

    let hidden = siso_two_state(
        json!([[-1.0, 0.0], [0.0, 1.0]]),
        json!([[1.0], [0.0]]),
        json!([[1.0, 0.0]]),
    );
    let converted = linear_transfer_model(&hidden).expect("the hidden model converts");
    assert_eq!(transfer_of(&converted).denominator, vec![1.0, 0.0, -1.0]);
    assert_eq!(transfer_of(&converted).numerator, vec![1.0, -1.0]);

    assert_eq!(
        linear_transfer_model(&base()),
        Err(ModelRefusal::TransferAnalysisRequiresSiso),
        "the base model has two inputs: SISO is required"
    );
    let mut sampled = siso_two_state(
        json!([[0.0, 1.0], [-2.0, -3.0]]),
        json!([[0.0], [1.0]]),
        json!([[1.0, 0.0]]),
    );
    sampled["kind"] = json!("discrete-state-space");
    sampled["sampleTimeSeconds"] = json!(0.1);
    let refused = linear_transfer_model(&sampled).expect_err("two states are not one state");
    assert!(refused.to_string().contains("sampled") && refused.to_string().contains("controller"));
}

#[test]
fn a_one_state_sampled_model_becomes_its_held_input_transfer_function() {
    // controller-mappings.test.ts: tunes a positive one-state sampled model through its equivalent.
    let step = 0.02_f64;
    let pole = (-step / 2.0).exp();
    let sampled = json!({
        "kind": "discrete-state-space", "revision": "sampled-fit", "source": "fitted",
        "validation": "validated", "evidence": ["estimation", "independent"], "delaySeconds": 0.0,
        "sampleTimeSeconds": step, "a": [[pole]], "b": [[3.0 * (1.0 - pole)]], "c": [[1.0]], "d": [[0.0]],
        "inputs": [channel("u", "V")], "outputs": [channel("y", "m")], "states": [channel("x", "m")]
    });
    let before = sampled.clone();
    let transfer = linear_transfer_model(&sampled).expect("a positive one-state model");
    assert!((transfer_of(&transfer).denominator[1] - 0.5).abs() < 1e-12);
    assert!((transfer_of(&transfer).numerator[0] - 1.5).abs() < 1e-12);
    assert_eq!(transfer.identity().revision, "sampled-fit");
    assert_eq!(sampled, before, "the selected sampled model is unchanged");

    let mut integrator = sampled.clone();
    integrator["a"] = json!([[1.0]]);
    integrator["b"] = json!([[2.0 * step]]);
    let transfer = linear_transfer_model(&integrator).expect("an integrator");
    assert_eq!(transfer_of(&transfer).denominator, vec![1.0, 0.0]);
    assert!((transfer_of(&transfer).numerator[0] - 2.0).abs() < 1e-12);

    let mut negative = sampled.clone();
    negative["a"] = json!([[-0.5]]);
    let refused = linear_transfer_model(&negative).expect_err("a negative transition");
    assert!(refused.to_string().contains("sampled"));
    let mut feedthrough = sampled;
    feedthrough["d"] = json!([[1.0]]);
    let refused = linear_transfer_model(&feedthrough).expect_err("direct feedthrough");
    assert!(refused.to_string().contains("sampled"));
}

#[test]
fn decoding_accepts_a_rectangular_model_and_keeps_an_owned_snapshot() {
    // general-model.test.ts: the decode of a supplied rectangular model, and the owned snapshot.
    assert!(
        decode_general_model(&base()).is_some(),
        "a rectangular model is accepted"
    );

    let mut source = base();
    source["validation"] = json!("validated");
    source["evidence"] = json!(["run-29-validation"]);
    let decoded = decode_general_model(&source).expect("a validated model with evidence");
    source["inputs"][0]["unit"] = json!("changed");
    source["evidence"][0] = json!("changed");
    assert_eq!(decoded.identity().inputs[0].unit, "W");
    assert_eq!(
        decoded.identity().evidence,
        vec!["run-29-validation".to_owned()]
    );
}

#[test]
fn decoding_refuses_malformed_unsupported_and_falsely_validated_models() {
    // general-model.test.ts: rejects malformed, unsupported and falsely validated model contracts.
    let mut shape = base();
    shape["b"] = json!([[1.0]]);
    let mut nonfinite = base();
    nonfinite["a"] = json!([[Value::Null]]);
    let mut unvalidated_evidence = base();
    unvalidated_evidence["validation"] = json!("validated");
    let mut discrete_zero = base();
    discrete_zero["kind"] = json!("discrete-state-space");
    discrete_zero["sampleTimeSeconds"] = json!(0.0);
    let mut duplicated = base();
    duplicated["inputs"] = json!([channel("heater", "W"), channel("heater", "W")]);
    // A single-input, single-output function whose leading denominator coefficient is zero.
    let zero_denominator = json!({
        "kind": "transfer-function", "numerator": [1.0], "denominator": [0.0, 1.0],
        "revision": "zero-denominator", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [{"id": "u", "unit": "V", "operatingRange": {"minimum": -1.0, "maximum": 1.0}}],
        "outputs": [{"id": "y", "unit": "V", "operatingRange": {"minimum": -1.0, "maximum": 1.0}}]
    });
    for model in [
        shape,
        nonfinite,
        unvalidated_evidence,
        discrete_zero,
        duplicated,
        zero_denominator,
    ] {
        assert!(decode_general_model(&model).is_none(), "{model}");
    }
}

#[test]
fn the_two_tank_structure_has_its_ranks_and_matrices_and_hidden_observability() {
    // general-model.test.ts (lines 69-81) and three-page-analysis.test.ts (lines 42-47): the same
    // structure numbers, through calculate_general_state_structure.
    let two_tank = json!({
        "revision": "cascade", "source": "provided", "validation": "validated", "evidence": ["e"],
        "delaySeconds": 0.0,
        "inputs": [{"id": "pump-flow-deviation", "unit": "m3/s", "operatingRange": {"minimum": -0.05, "maximum": 0.05}}],
        "outputs": [{"id": "downstream-level-deviation", "unit": "m", "operatingRange": {"minimum": -0.05, "maximum": 0.05}}],
        "states": [{"id": "upstream-level-deviation", "unit": "m", "operatingRange": {"minimum": -0.05, "maximum": 0.05}},
                   {"id": "downstream-level-deviation", "unit": "m", "operatingRange": {"minimum": -0.05, "maximum": 0.05}}],
        "kind": "continuous-state-space",
        "a": [[-0.04, 0.0], [0.04, -0.04]], "b": [[2.0], [0.0]], "c": [[0.0, 1.0]], "d": [[0.0]]
    });
    let model = decode_general_model(&two_tank).expect("the two-tank model decodes");
    let GeneralModel::ContinuousStateSpace { system, .. } = &model else {
        panic!("a continuous state-space model was expected");
    };
    let GeneralStateStructure::Qualified {
        controllability,
        observability,
        scope,
    } = calculate_general_state_structure(system)
    else {
        panic!("the two-tank structure qualifies");
    };
    assert_eq!(controllability.rank, 2);
    assert_matrix_close(
        &controllability.matrix,
        &[vec![2.0, -0.08], vec![0.0, 0.08]],
        1e-12,
    );
    assert_eq!(observability.rank, 2);
    assert_matrix_close(
        &observability.matrix,
        &[vec![0.0, 1.0], vec![0.04, -0.04]],
        1e-12,
    );
    assert!(scope.contains("local unconstrained model"));
    assert!(is_general_state_structure(
        &serde_json::to_value(calculate_general_state_structure(system)).expect("serializable"),
        &model
    ));

    let mut hidden = two_tank;
    hidden["c"] = json!([[1.0, 0.0]]);
    let hidden_model = decode_general_model(&hidden).expect("the hidden model decodes");
    let GeneralModel::ContinuousStateSpace { system, .. } = &hidden_model else {
        panic!("a continuous state-space model was expected");
    };
    let GeneralStateStructure::Qualified { observability, .. } =
        calculate_general_state_structure(system)
    else {
        panic!("the hidden structure qualifies");
    };
    assert_eq!(
        observability.rank, 1,
        "C = [1, 0] hides the downstream state"
    );
}

#[test]
fn an_open_loop_response_is_checked_against_the_model_and_its_settings() {
    // A first-order plant dx/dt = −x + u with delay 0.2 s: the response at t = kT is the delayed step.
    let plant = json!({
        "revision": "first-order", "source": "provided", "validation": "unvalidated", "evidence": [],
        "delaySeconds": 0.2, "inputs": [channel("u", "1")], "outputs": [channel("y", "1")],
        "states": [channel("x", "1")], "kind": "continuous-state-space",
        "a": [[-1.0]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]]
    });
    let settings = json!({"connection": "open-loop", "stepSeconds": 0.1, "durationSeconds": 0.5,
                          "initialState": [0.0], "input": [1.0]});
    let response = analyze_general_model_time(&plant, &settings).expect("the response is computed");
    let outputs: Vec<f64> = response
        .points
        .iter()
        .map(|point| point.zero_state[0])
        .collect();
    let expected = [
        0.0,
        0.0,
        0.0,
        1.0 - (-0.1_f64).exp(),
        1.0 - (-0.2_f64).exp(),
        1.0 - (-0.3_f64).exp(),
    ];
    assert_eq!(outputs.len(), expected.len());
    for (value, reference) in outputs.iter().zip(expected) {
        assert!((value - reference).abs() <= 1e-9);
    }
    assert_eq!(response.state_coordinates, vec!["x (1)".to_owned()]);
    assert!(response.transform.starts_with("Y(s) ="));
    let stored = serde_json::to_value(&response).expect("the response is serializable");
    assert!(is_model_time_response(&stored, &response.model));

    let mut feedback = settings.clone();
    feedback["connection"] = json!("feedback");
    assert_eq!(
        analyze_general_model_time(&plant, &feedback),
        Err(ModelRefusal::FeedbackControllerMissing)
    );
    let mut unknown = settings;
    unknown["connection"] = json!("open_loop");
    assert_eq!(
        analyze_general_model_time(&plant, &unknown),
        Err(ModelRefusal::ChooseResponseSettings),
        "the connection must be exactly one of the two words"
    );
}

#[test]
fn a_transfer_function_with_a_zero_denominator_is_refused() {
    // general-model.test.ts: rejects malformed, unsupported and falsely validated model contracts. The TypeScript model
    // has two inputs, which the transfer-function rule refuses (one input and one output); a single-input model with the
    // same zero denominator is refused by the coefficient rule itself.
    let mut two_inputs = base();
    two_inputs["kind"] = json!("transfer-function");
    two_inputs["numerator"] = json!([1.0]);
    two_inputs["denominator"] = json!([0.0]);
    assert!(decode_general_model(&two_inputs).is_none());

    let single_input = json!({
        "kind": "transfer-function", "numerator": [1.0], "denominator": [0.0],
        "revision": "zero-polynomial", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u", "V")], "outputs": [channel("y", "V")]
    });
    assert!(
        decode_general_model(&single_input).is_none(),
        "a zero leading denominator coefficient is refused"
    );
}
