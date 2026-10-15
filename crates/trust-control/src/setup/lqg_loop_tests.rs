//! Tests of the LQG generator (`lqgLoopGenerator.ts`): the loop assertions of `control-studio-loop-setup-lqg.test.ts` on its two-tank
//! program and its estimator model, the refusals of the LQG choices (`loopSetupForm.ts` and the generator), and the decode of the
//! written loop by the loop contract and by the state controller's record. The estimator's gains are designed by Tune (C4).

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use crate::failure::Failure;
use crate::json::decode_value;
use crate::loop_definition::{LoopDefinition, LoopScope};
use crate::model::{decode_general_model, GeneralModel};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::state_feedback_loop::{state_feedback_loop_definition, StateFeedbackLoopDraft};
use crate::tuning::state_controller::{decode_state_controller_binding, StateControllerFamily};

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:setup".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The two tanks of `control-studio-loop-setup-lqg.test.ts`: the program measures tank 2 only, and estimates both levels.
fn tanks() -> CompiledCatalog {
    decode(json!({
        "symbols": [
            { "symbol": "P1.Tank2M", "type": "LREAL", "address": "%IL8" },
            { "symbol": "P1.PumpM3PerS", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.Tank2SetpointM", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ManualM3PerS", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.Enabled", "type": "BOOL" },
            { "symbol": "P1.K1", "type": "LREAL" },
            { "symbol": "P1.K2", "type": "LREAL" },
            { "symbol": "P1.N", "type": "LREAL" },
            { "symbol": "P1.L1", "type": "LREAL" },
            { "symbol": "P1.L2", "type": "LREAL" },
            { "symbol": "P1.X1Hat", "type": "LREAL" },
            { "symbol": "P1.X2Hat", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 1000 }] }
        ],
        "links": [],
        "writes": ["P1.PumpM3PerS", "P1.X1Hat", "P1.X2Hat"]
    }))
}

/// Lesson 67's one-second model of the cascade around 0.25 m, which the estimator implements (`estimatorModel` of the test).
fn estimator_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "discrete-state-space", "revision": "two-tanks", "source": "provided",
        "validation": "validated", "evidence": ["lesson"], "delaySeconds": 0, "sampleTimeSeconds": 1,
        "inputs": [{ "id": "pump-flow", "unit": "m3/s", "operatingRange": { "minimum": 0, "maximum": 0.02 } }],
        "outputs": [{ "id": "tank-2-level", "unit": "m", "operatingRange": { "minimum": 0.05, "maximum": 0.45 } }],
        "states": [
            { "id": "tank-1-deviation", "unit": "m", "operatingRange": { "minimum": -0.2, "maximum": 0.2 } },
            { "id": "tank-2-deviation", "unit": "m", "operatingRange": { "minimum": -0.2, "maximum": 0.2 } }
        ],
        "a": [[0.96, 0], [0.04, 0.96]], "b": [[2], [0]], "c": [[0, 1]], "d": [[0]],
        "operatingPoint": { "inputs": [0.01], "outputs": [0.25] }
    }))
    .expect("the estimator's model decodes")
}

/// The LQG draft of the two tanks, as the form sends it: one estimate row for each of the model's two states.
fn lqg_json() -> Value {
    json!({
        "name": "Tank 2 level",
        "action": "direct",
        "family": "LQG",
        "instance": "P1",
        "manipulatedVariable": { "symbol": "P1.PumpM3PerS", "unit": "m3/s", "minimum": 0, "maximum": 0.02 },
        "processVariable": { "symbol": "P1.Tank2M", "unit": "m", "minimum": 0, "maximum": 0.5 },
        "setpoint": { "symbol": "P1.Tank2SetpointM" },
        "referenceGain": "P1.N",
        "mode": { "symbol": "P1.Enabled", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualM3PerS",
        "estimator": {
            "modelFile": "lqg-model.json",
            "measurementNoise": 1e-6,
            "states": [
                { "estimate": "P1.X1Hat", "gain": "P1.K1", "observerGain": "P1.L1", "processNoise": 1e-6 },
                { "estimate": "P1.X2Hat", "gain": "P1.K2", "observerGain": "P1.L2", "processNoise": 1e-6 }
            ]
        }
    })
}

fn draft_of(value: Value) -> StateFeedbackLoopDraft {
    decode_value(value).expect("the draft decodes")
}

fn write(value: Value, model: Option<&GeneralModel>) -> LoopDefinition {
    state_feedback_loop_definition(&tanks(), &scope(), &draft_of(value), model)
        .expect("the draft is written")
}

/// The refusal a draft gets with the given model, as its whole sentence.
fn refusal(value: Value, model: Option<&GeneralModel>) -> String {
    let failure = state_feedback_loop_definition(&tanks(), &scope(), &draft_of(value), model)
        .expect_err("the draft is refused");
    text_of(&failure)
}

/// The LQG draft with one edit applied to it.
fn edited(edit: impl FnOnce(&mut Value)) -> Value {
    let mut value = lqg_json();
    edit(&mut value);
    value
}

#[test]
fn the_lqg_draft_writes_a_loop_whose_program_estimates_the_states_with_the_chosen_model() {
    // control-studio-loop-setup-lqg.test.ts: "writes an LQG loop whose program estimates the states with the chosen model".
    let model = estimator_model();
    let loop_definition = write(lqg_json(), Some(&model));
    loop_definition
        .validate(&scope())
        .expect("the loop passes the checks that need no runtime");
    let record = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .expect("a state law");
    assert_eq!(record["family"], "LQG");
    assert_eq!(record["gainSymbols"], json!([["P1.K1", "P1.K2"]]));
    assert_eq!(record["referenceGainSymbols"], json!([["P1.N"]]));
    assert_eq!(record["observerGainSymbols"], json!([["P1.L1"], ["P1.L2"]]));
    assert_eq!(
        record["estimatorStateSymbols"],
        json!(["P1.X1Hat", "P1.X2Hat"])
    );
    assert_eq!(record["estimatorModel"]["sampleTimeSeconds"], json!(1.0));
    let states = record["initialization"]["states"]
        .as_array()
        .expect("the estimates are declared states");
    let started: Vec<(&str, &str, Option<f64>)> = states
        .iter()
        .map(|state| {
            (
                state["symbol"].as_str().unwrap_or_default(),
                state["unit"].as_str().unwrap_or_default(),
                state["initial"].as_f64(),
            )
        })
        .collect();
    assert_eq!(
        started,
        [("P1.X1Hat", "m", Some(0.0)), ("P1.X2Hat", "m", Some(0.0))],
        "the estimates start at the operating point"
    );
    let channels = loop_definition.channels.as_ref().expect("the channels");
    assert_eq!(
        channels.outputs.len(),
        1,
        "only the controlled output is measured"
    );
    assert_eq!(channels.outputs[0].model_channel_id, "tank-2-level");
    assert_eq!(
        channels.inputs[0].model_channel_id, "pump-flow",
        "the supplied model maps by its own channels"
    );
    let mut symbols: Vec<&str> = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a parameter record")
        .parameters
        .iter()
        .map(|parameter| parameter.symbol.as_str())
        .collect();
    symbols.sort_unstable();
    assert_eq!(symbols, ["P1.K1", "P1.K2", "P1.L1", "P1.L2", "P1.N"]);
}

#[test]
fn the_written_lqg_record_holds_the_sampled_model_and_the_noise_the_state_controller_reads() {
    // The state controller's record (decodeStateControllerBinding): the estimator's matrices, the diagonal process noise, and the
    // measurement noise, as the model and the draft give them.
    let model = estimator_model();
    let loop_definition = write(lqg_json(), Some(&model));
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    let decoded = LoopDefinition::decode(&text).expect("the written loop decodes");
    decoded
        .rust_engine_support()
        .expect("the engine serves an LQG law with channels");
    let record = decoded
        .controller
        .state_feedback
        .as_ref()
        .expect("a state law");
    let binding = decode_state_controller_binding(record, decoded.parameter_binding.as_ref())
        .expect("the state controller reads the record");
    assert_eq!(binding.family, StateControllerFamily::Lqg);
    let estimator = binding.estimator_model.expect("the estimator's model");
    assert_eq!(estimator.a, vec![vec![0.96, 0.0], vec![0.04, 0.96]]);
    assert_eq!(estimator.b, vec![vec![2.0], vec![0.0]]);
    assert_eq!(estimator.c, vec![vec![0.0, 1.0]]);
    assert_eq!(estimator.sample_time_seconds, 1.0);
    assert_eq!(
        binding.process_noise,
        Some(vec![vec![1e-6, 0.0], vec![0.0, 1e-6]])
    );
    assert_eq!(binding.measurement_noise, Some(vec![vec![1e-6]]));
    assert_eq!(
        binding.observer_gain_symbols,
        Some(vec![vec!["P1.L1".to_string()], vec!["P1.L2".to_string()]])
    );
}

#[test]
fn a_draft_that_misses_an_lqg_choice_names_it_in_the_form_s_sentence() {
    // loopSetupForm.ts's LQG draft, in its order: the reference gain, the model file, each estimate row, the measurement noise, then the
    // mode and the manual output.
    let model = estimator_model();
    let refuse = |edit: fn(&mut Value)| refusal(edited(edit), Some(&model));
    assert_eq!(
        refuse(|value| value["referenceGain"] = json!("")),
        "Choose the variable holding the reference gain."
    );
    assert_eq!(
        refuse(|value| value["estimator"]["modelFile"] = json!("")),
        "Choose the model file the program's estimator implements."
    );
    assert_eq!(
        refuse(|value| value["estimator"]["states"][1]["estimate"] = json!("")),
        "Choose the variable holding the estimate of model state 2."
    );
    assert_eq!(
        refuse(|value| value["estimator"]["states"][0]["observerGain"] = json!("")),
        "Choose the variable holding the observer gain of model state 1."
    );
    assert_eq!(
        refuse(|value| value["estimator"]["states"][0]["processNoise"] = json!(0)),
        "Enter the positive process-noise variance of model state 1."
    );
    assert_eq!(
        refuse(|value| value["estimator"]["measurementNoise"] = json!(0)),
        "Enter the positive measurement-noise variance."
    );
    assert_eq!(
        refuse(|value| value["mode"]["symbol"] = json!("")),
        "Choose the Auto/Manual signal."
    );
    assert_eq!(
        refuse(|value| value["manualOutput"] = json!("")),
        "Choose the manual output."
    );
}

#[test]
fn the_estimator_rows_follow_the_model_and_its_shape() {
    // lqgLoopGenerator.ts: one estimate for each state of the model, and a state-space model with one input and one output.
    let model = estimator_model();
    let one_row = edited(|value| {
        let first = value["estimator"]["states"][0].clone();
        value["estimator"]["states"] = json!([first]);
    });
    assert_eq!(
        refusal(one_row, Some(&model)),
        "Give one estimate, gain and observer gain for each of the model's (at most three) states."
    );
    assert_eq!(
        refusal(lqg_json(), None),
        "The estimator's model must be a state-space model with one input and the controlled output as its one output."
    );
    let transfer = decode_general_model(&json!({
        "kind": "transfer-function", "numerator": [2], "denominator": [60, 1], "delaySeconds": 5,
        "revision": "oven", "source": "fitted", "validation": "validated", "evidence": ["recording"],
        "inputs": [{ "id": "u", "unit": "m3/s", "operatingRange": { "minimum": 0, "maximum": 0.02 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": 0, "maximum": 0.5 } }],
        "operatingPoint": { "inputs": [0.01], "outputs": [0.25] }
    }))
    .expect("the transfer function decodes");
    assert_eq!(
        refusal(lqg_json(), Some(&transfer)),
        "The estimator's model must be a state-space model with one input and the controlled output as its one output."
    );
}
