//! Tests of the envelope cases (`engineerEnvelopeScenarios.ts`, SISO cases). The `TypeScript` suite asserts none of the cases'
//! contents, so the expectations are the rules of that function: the case order, the reviewed values, and the generator
//! and mode of each case. The loop is the engineer fixture of the suite (control-loop.json, as its `pidLoop` builds it).

#![allow(clippy::float_cmp)] // The noise bounds are the exact negation and value of the reviewed band, compared on purpose.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::failure::{Cause, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::{decode_general_model, GeneralModel};
use crate::validation::engineer_verification_plan::{engineer_requirements, EngineerRequirements};
use crate::validation::texts;

const FIXTURE: &str = include_str!(
    "../../../../editors/vscode/src/test/fixtures/control-studio-engineer/control-loop.json"
);

/// The engineer fixture as the suite's `pidLoop` builds it. Its task period is 20 ms.
fn fixture_loop() -> LoopDefinition {
    let mut value: Value = crate::json::parse_text(FIXTURE).expect("the fixture is JSON");
    // The Rust engine refuses a declared analysis (`rust_engine_support`, a parked feature): the suite's loop drops it.
    if let Some(fields) = value.as_object_mut() {
        fields.remove("analysis");
        fields.remove("analysisParameterSymbols");
    }
    value["controller"]["type"] = json!("Control.PID");
    value["controller"]["design"] = json!("pid");
    value["controller"]["law"] = json!({
        "kind": "pid",
        "parameterRecordSymbol": "Main.Parameters",
        "parameterRevisionSymbol": "Main.ParameterRevision",
        "proportionalGainSymbol": "Main.Kp",
        "integralGainSymbol": "Main.Ki",
        "derivativeGainSymbol": "Main.Kd"
    });
    value["parameterBinding"]["parameters"] = json!([
        { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "V/m", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "V/(m*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Kd", "iec_type": "LREAL", "unit": "V*s/m", "convention": "derivative gain", "minimum": 0, "maximum": 100 }
    ]);
    LoopDefinition::decode(&value.to_string()).expect("the fixture decodes")
}

/// The suite's campaign model: a first-order process with a 0.5 s delay, validated as `model-4`.
fn fixture_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4.0, 1.0],
        "delaySeconds": 0.5,
        "revision": "model-4",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "inputs": [{ "id": "u", "unit": "V", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }]
    }))
    .expect("the fixture model decodes")
}

/// The suite's campaign task, without any reviewed envelope bound.
fn fixture_task() -> EngineerRequirements {
    engineer_requirements(&json!({
        "referenceChange": 1.0,
        "maximumError": 0.2,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 15.0,
        "durationSeconds": 30.0
    }))
    .expect("the fixture requirements decode")
}

fn fixture_gains() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Kp".to_owned(), 1.0),
        ("Main.Ki".to_owned(), 0.2),
        ("Main.Kd".to_owned(), 0.0),
    ])
}

fn envelope(task: &EngineerRequirements) -> Vec<CampaignCase> {
    engineer_envelope_scenarios(
        &fixture_loop(),
        &fixture_model(),
        task,
        &fixture_gains(),
        &BTreeMap::new(),
    )
    .expect("the envelope is valid")
}

#[test]
fn without_a_reviewed_bound_there_is_no_envelope_case() {
    assert!(envelope(&fixture_task()).is_empty());
}

#[test]
fn a_reviewed_load_is_a_constant_input_effect_on_a_regulation_plan() {
    let mut task = fixture_task();
    task.constant_input_disturbance = Some(0.5);
    let cases = envelope(&task);
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].id, "load-rejection");
    assert_eq!(
        cases[0].effects,
        vec![EngineerScenarioEffect {
            id: "reviewed-load".to_owned(),
            target: EffectTarget::ProcessInput,
            channel: 0,
            mode: EffectMode::Add,
            start_seconds: 0.0,
            end_seconds: None,
            generator: EffectGenerator::Constant,
            value: Some(0.5),
            minimum: None,
            maximum: None,
            duration_seconds: None,
        }]
    );
}

#[test]
fn a_reviewed_noise_band_is_uniform_noise_between_its_bounds_on_the_measured_output() {
    let mut task = fixture_task();
    task.measurement_noise_band = Some(0.05);
    let cases = envelope(&task);
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].id, "measurement-noise");
    let effect = &cases[0].effects[0];
    assert_eq!(effect.id, "reviewed-noise");
    assert_eq!(effect.target, EffectTarget::Measurement);
    assert_eq!(effect.mode, EffectMode::Add);
    assert_eq!(effect.generator, EffectGenerator::UniformWhite);
    assert_eq!(effect.minimum, Some(-0.05));
    assert_eq!(effect.maximum, Some(0.05));
}

#[test]
fn a_gain_fraction_makes_a_lower_and_an_upper_gain_case_in_that_order() {
    let mut task = fixture_task();
    task.gain_uncertainty_fraction = Some(0.2);
    let cases = envelope(&task);
    let ids: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids, ["lower-gain", "upper-gain"]);
    let values: Vec<f64> = cases
        .iter()
        .map(|case| case.effects[0].value.expect("a gain case has a value"))
        .collect();
    // 1 + (-1) * 0.2 and 1 + 1 * 0.2: the multipliers of the lower and the upper gain.
    assert!((values[0] - 0.8).abs() <= 1e-12, "{}", values[0]);
    assert!((values[1] - 1.2).abs() <= 1e-12, "{}", values[1]);
    for case in &cases {
        let effect = &case.effects[0];
        assert_eq!(effect.id, "gain-0");
        assert_eq!(effect.target, EffectTarget::ProcessInput);
        assert_eq!(effect.channel, 0);
        assert_eq!(effect.mode, EffectMode::Multiply);
        assert_eq!(effect.generator, EffectGenerator::Constant);
    }
}

#[test]
fn all_reviewed_bounds_create_their_cases_in_the_typescript_order() {
    let mut task = fixture_task();
    task.constant_input_disturbance = Some(0.5);
    task.measurement_noise_band = Some(0.05);
    task.gain_uncertainty_fraction = Some(0.2);
    let cases = envelope(&task);
    let ids: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "load-rejection",
            "measurement-noise",
            "lower-gain",
            "upper-gain"
        ]
    );
}

#[test]
fn an_unordered_noise_band_is_refused_as_noise_bounds() {
    let mut task = fixture_task();
    task.measurement_noise_band = Some(-0.1);
    let failure = engineer_envelope_scenarios(
        &fixture_loop(),
        &fixture_model(),
        &task,
        &fixture_gains(),
        &BTreeMap::new(),
    )
    .expect_err("a negative band gives unordered noise bounds");
    assert_eq!(failure.message, texts::noise_bounds_invalid());
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
}
