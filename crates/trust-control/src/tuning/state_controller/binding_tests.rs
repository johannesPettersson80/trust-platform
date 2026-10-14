//! The rules of `decodeStateControllerBinding`, asserted on records that a loop file carries. The fixtures are the
//! records that the TypeScript loop setup writes (`stateFeedbackLoopGenerator.ts`, `lqgLoopGenerator.ts`).

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::ParameterBinding;

/// The parameter record of a loop with the given symbols, each a `LREAL` from −100 to 100.
fn parameter_binding(symbols: &[&str]) -> ParameterBinding {
    let parameters: Vec<Value> = symbols
        .iter()
        .map(|symbol| {
            json!({ "symbol": symbol, "iec_type": "LREAL", "unit": "m3/s/m",
                "convention": "state feedback gain", "minimum": -100, "maximum": 100 })
        })
        .collect();
    crate::json::decode_value(json!({
        "loop_id": "loop:state", "task_name": "MainTask", "parameters": parameters,
        "mode": { "symbol": "Main.Auto", "manual_value": false }
    }))
    .expect("the parameter record decodes")
}

fn all_symbols() -> ParameterBinding {
    parameter_binding(&["Main.K1", "Main.K2", "Main.N", "Main.L1", "Main.L2"])
}

fn lqr_law() -> Value {
    json!({
        "family": "LQR", "convention": "sampled-deviation-negative-feedback",
        "gainSymbols": [["Main.K1", "Main.K2"]], "referenceGainSymbols": [["Main.N"]],
        "stateWeights": [[1, 0], [0, 1]], "inputWeights": [[1]]
    })
}

fn lqg_law() -> Value {
    json!({
        "family": "LQG", "convention": "sampled-deviation-negative-feedback",
        "gainSymbols": [["Main.K1", "Main.K2"]], "referenceGainSymbols": [["Main.N"]],
        "observerGainSymbols": [["Main.L1"], ["Main.L2"]],
        "estimatorModel": { "a": [[0.96, 0.0], [0.04, 0.96]], "b": [[2.0], [0.0]],
            "c": [[0.0, 1.0]], "sampleTimeSeconds": 1.0 },
        "processNoise": [[1e-6, 0.0], [0.0, 1e-6]], "measurementNoise": [[1e-6]]
    })
}

/// A law with one field replaced, or a field added when the key is new.
fn with(mut law: Value, key: &str, value: Value) -> Value {
    law[key] = value;
    law
}

#[test]
fn an_lqr_law_decodes_with_its_symbols_and_weights() {
    let binding = parameter_binding(&["Main.K1", "Main.K2", "Main.N"]);
    let decoded = decode_state_controller_binding(&lqr_law(), Some(&binding))
        .expect("the law is a usable LQR declaration");
    assert_eq!(decoded.family, StateControllerFamily::Lqr);
    assert_eq!(
        decoded.gain_symbols,
        vec![vec!["Main.K1".to_string(), "Main.K2".to_string()]]
    );
    assert_eq!(
        decoded.reference_gain_symbols,
        vec![vec!["Main.N".to_string()]]
    );
    assert_eq!(
        decoded.state_weights,
        Some(vec![vec![1.0, 0.0], vec![0.0, 1.0]])
    );
    assert!(decoded.observer_gain_symbols.is_none());
    assert!(decoded.estimator_model.is_none());
}

#[test]
fn a_law_without_a_parameter_record_is_not_decoded() {
    assert!(decode_state_controller_binding(&lqr_law(), None).is_none());
}

#[test]
fn a_symbol_the_record_does_not_declare_is_refused() {
    let binding = parameter_binding(&["Main.K1", "Main.N"]);
    assert!(decode_state_controller_binding(&lqr_law(), Some(&binding)).is_none());
}

#[test]
fn a_symbol_named_twice_is_refused() {
    let binding = parameter_binding(&["Main.K1", "Main.K2", "Main.N"]);
    let repeated = with(lqr_law(), "referenceGainSymbols", json!([["Main.K1"]]));
    assert!(decode_state_controller_binding(&repeated, Some(&binding)).is_none());
}

#[test]
fn an_unknown_family_convention_or_field_is_refused() {
    let binding = all_symbols();
    let pid_family = with(lqr_law(), "family", json!("PID"));
    assert!(decode_state_controller_binding(&pid_family, Some(&binding)).is_none());
    let convention = with(lqr_law(), "convention", json!("positional"));
    assert!(decode_state_controller_binding(&convention, Some(&binding)).is_none());
    let extra = with(lqr_law(), "runtimeWrite", json!(true));
    assert!(decode_state_controller_binding(&extra, Some(&binding)).is_none());
}

#[test]
fn a_weight_must_be_square_and_within_the_state_limit() {
    let binding = all_symbols();
    let rectangular = with(lqr_law(), "stateWeights", json!([[1.0, 0.0]]));
    assert!(decode_state_controller_binding(&rectangular, Some(&binding)).is_none());
    let too_many = vec![vec![0.0; 33]; 33];
    let oversized = with(lqr_law(), "inputWeights", json!(too_many));
    assert!(decode_state_controller_binding(&oversized, Some(&binding)).is_none());
}

#[test]
fn an_observer_gain_belongs_to_lqg_only_and_lqg_needs_one() {
    let binding = all_symbols();
    let observer = with(lqr_law(), "observerGainSymbols", json!([["Main.L1"]]));
    assert!(decode_state_controller_binding(&observer, Some(&binding)).is_none());
    let without_observer = with(lqg_law(), "observerGainSymbols", Value::Null);
    assert!(decode_state_controller_binding(&without_observer, Some(&binding)).is_none());
}

#[test]
fn an_lqg_law_needs_an_estimator_model_with_a_positive_sample_period() {
    let binding = all_symbols();
    let decoded = decode_state_controller_binding(&lqg_law(), Some(&binding))
        .expect("the LQG declaration is usable");
    let estimator = decoded
        .estimator_model
        .expect("LQG reads its estimator model");
    assert_eq!(estimator.sample_time_seconds, 1.0);
    assert_eq!(estimator.b, vec![vec![2.0], vec![0.0]]);
    let period = with(
        lqg_law(),
        "estimatorModel",
        json!({ "a": [[1.0]], "b": [[1.0]], "c": [[1.0]], "sampleTimeSeconds": 0.0 }),
    );
    assert!(decode_state_controller_binding(&period, Some(&binding)).is_none());
    let missing = with(lqg_law(), "estimatorModel", Value::Null);
    assert!(decode_state_controller_binding(&missing, Some(&binding)).is_none());
}

#[test]
fn a_declared_initialization_must_name_the_estimate_states_that_are_not_booleans() {
    let binding = all_symbols();
    let state = |symbol: &str, iec_type: &str| {
        json!({ "symbol": symbol, "iec_type": iec_type, "unit": "m", "minimum": -1.0,
            "maximum": 1.0, "initial": 0.0, "postScan": "unchanged" })
    };
    let initialization =
        |states: Vec<Value>| json!({ "kind": "declared-state-v1", "states": states });
    let law = |states: Vec<Value>, names: Value| {
        let with_init = with(lqg_law(), "initialization", initialization(states));
        with(with_init, "estimatorStateSymbols", names)
    };
    let usable = law(
        vec![state("Main.X1Hat", "LREAL"), state("Main.X2Hat", "LREAL")],
        json!(["Main.X1Hat", "Main.X2Hat"]),
    );
    let decoded = decode_state_controller_binding(&usable, Some(&binding))
        .expect("the estimate states are declared");
    assert_eq!(
        decoded.initialization.map(|init| init.states().len()),
        Some(2)
    );
    let one_name = law(
        vec![state("Main.X1Hat", "LREAL"), state("Main.X2Hat", "LREAL")],
        json!(["Main.X1Hat"]),
    );
    assert!(decode_state_controller_binding(&one_name, Some(&binding)).is_none());
    // A BOOL state is dimensionless in 0..=1 (C0), so it is declared that way and still refused as an estimate.
    let boolean_state = json!({ "symbol": "Main.X1Hat", "iec_type": "BOOL", "unit": "1",
        "minimum": 0.0, "maximum": 1.0, "initial": 0.0, "postScan": "unchanged" });
    let boolean = law(
        vec![boolean_state, state("Main.X2Hat", "LREAL")],
        json!(["Main.X1Hat", "Main.X2Hat"]),
    );
    assert!(decode_state_controller_binding(&boolean, Some(&binding)).is_none());
}

#[test]
fn desired_poles_must_not_be_empty() {
    let binding = all_symbols();
    let empty = with(lqr_law(), "desiredPoles", json!([]));
    assert!(decode_state_controller_binding(&empty, Some(&binding)).is_none());
    let placed = with(lqr_law(), "desiredPoles", json!([0.5, 0.6]));
    let decoded =
        decode_state_controller_binding(&placed, Some(&binding)).expect("two poles are declared");
    assert_eq!(decoded.desired_poles, Some(vec![0.5, 0.6]));
}
