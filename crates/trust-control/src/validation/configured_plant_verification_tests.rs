use super::*;
use serde_json::json;

/// The loop of the `TypeScript` tests: the process variable is read at `%IL0`, the manipulated variable is written at
/// `%QL0`, and the loop names the plant `plant-1`.
fn plant_loop() -> LoopDefinition {
    let mut loop_definition = crate::recording::fixtures::loop_definition();
    loop_definition.plant_id = Some("plant-1".to_owned());
    loop_definition
}

/// The plant of the tests: it takes the loop's output at `%QL0` and feeds back the measurement at `%IL0`.
fn drives() -> ConfiguredPlantBinding {
    ConfiguredPlantBinding {
        id: "plant-1".to_owned(),
        command_sources: vec!["%QL0".to_owned()],
        feedback_targets: vec!["%IL0".to_owned()],
    }
}

fn configured() -> VerificationPlant {
    VerificationPlant::Configured {
        plant_id: "plant-1".to_owned(),
    }
}

/// A declared scenario with the given model and operating outputs and no effects.
fn declared(
    id: &str,
    model: Option<Value>,
    operating_outputs: Option<Vec<f64>>,
) -> EngineerScenario {
    EngineerScenario {
        id: id.to_owned(),
        model,
        operating_outputs,
        effects: None,
        initial_values: std::collections::BTreeMap::new(),
        events: Vec::new(),
        requirements: Vec::new(),
    }
}

#[test]
fn the_configured_plant_drives_the_loop_only_when_it_names_the_loop_and_its_addresses_match() {
    // control-studio-apply-process-time.test.ts: the plant feeds back %IL0 and takes the output at %QL0.
    let loop_definition = plant_loop();
    assert!(configured_plant_drives_loop(
        &loop_definition,
        Some(&drives())
    ));
    let mut spaced = drives();
    spaced.feedback_targets = vec![" %il0 ".to_owned()];
    assert!(
        configured_plant_drives_loop(&loop_definition, Some(&spaced)),
        "addresses compare trimmed and in upper case"
    );
    let mut other_feedback = drives();
    other_feedback.feedback_targets = vec!["%IL9".to_owned()];
    assert!(
        !configured_plant_drives_loop(&loop_definition, Some(&other_feedback)),
        "a plant whose feedback does not reach the measured output is not this loop's plant"
    );
    let mut other_command = drives();
    other_command.command_sources = vec!["%QL9".to_owned()];
    assert!(
        !configured_plant_drives_loop(&loop_definition, Some(&other_command)),
        "a plant that does not take the loop's plant input is not this loop's plant"
    );
    let mut other_plant = drives();
    other_plant.id = "other".to_owned();
    assert!(!configured_plant_drives_loop(
        &loop_definition,
        Some(&other_plant)
    ));
    assert!(!configured_plant_drives_loop(&loop_definition, None));
    let unnamed = crate::recording::fixtures::loop_definition();
    assert!(
        !configured_plant_drives_loop(&unnamed, Some(&drives())),
        "a loop that names no plant is not driven by one"
    );
}

#[test]
fn the_configured_plant_runs_only_for_a_simulated_target_that_it_drives() {
    // control-studio-apply-process-time.test.ts: the plant choice of isolated checks (B5.3).
    let loop_definition = plant_loop();
    assert_eq!(
        verification_plant(&loop_definition, true, Some(&drives())),
        configured()
    );
    assert_eq!(
        verification_plant(&loop_definition, false, Some(&drives())),
        VerificationPlant::Model,
        "a physical target never uses the configured plant"
    );
    let mut other_feedback = drives();
    other_feedback.feedback_targets = vec!["%IL9".to_owned()];
    assert_eq!(
        verification_plant(&loop_definition, true, Some(&other_feedback)),
        VerificationPlant::Model
    );
    let mut other_command = drives();
    other_command.command_sources = vec!["%QL9".to_owned()];
    assert_eq!(
        verification_plant(&loop_definition, true, Some(&other_command)),
        VerificationPlant::Model
    );
    let mut other_plant = drives();
    other_plant.id = "other".to_owned();
    assert_eq!(
        verification_plant(&loop_definition, true, Some(&other_plant)),
        VerificationPlant::Model
    );
    assert_eq!(
        verification_plant(&loop_definition, true, None),
        VerificationPlant::Model,
        "without the saved bindings the model runs"
    );
}

#[test]
fn the_plant_binding_is_decoded_from_the_saved_configuration_of_the_named_plant() {
    // control-studio-apply-process-time.test.ts: readConfiguredPlantBinding, minus the request.
    let answer = json!({"plants": [{"id": "plant-1", "command_sources": ["%QL0"], "feedback_targets": ["%IL0"]}]});
    assert_eq!(
        decode_configured_plant_binding(&answer, "plant-1"),
        Some(drives())
    );
    let other = json!({"plants": [{"id": "other", "command_sources": [], "feedback_targets": []}]});
    assert_eq!(
        decode_configured_plant_binding(&other, "plant-1"),
        None,
        "a saved configuration without the plant gives no binding"
    );
    assert_eq!(
        decode_configured_plant_binding(&Value::Null, "plant-1"),
        None,
        "a runtime that gives no answer leaves the model in use"
    );
    let not_text = json!({"plants": [{"id": "plant-1", "command_sources": ["%QL0"], "feedback_targets": [5]}]});
    assert_eq!(
        decode_configured_plant_binding(&not_text, "plant-1"),
        None,
        "feedback targets must be text"
    );
    assert_eq!(
        decode_configured_plant_binding(&answer, ""),
        None,
        "a loop without a plant names none, and nothing is asked for it"
    );
}

#[test]
fn a_declared_scenario_named_like_a_generated_case_still_runs_on_the_project_plant() {
    // control-studio-configured-plant.test.ts: the declared drum-load trial needs the plant's load input, which the
    // supplied model does not have.
    let loop_definition = plant_loop();
    let trial = declared("load-rejection", None, None);
    assert!(uses_configured_plant(
        &loop_definition,
        "load-rejection",
        Some(&trial),
        &configured()
    ));
}

#[test]
fn generated_model_variation_cases_and_declared_model_cases_keep_their_model() {
    // control-studio-configured-plant.test.ts.
    let loop_definition = plant_loop();
    assert!(
        !uses_configured_plant(&loop_definition, "load-rejection", None, &configured()),
        "the generated load case disturbs the model"
    );
    let heavy = declared("heavy-payload", Some(json!({})), None);
    assert!(
        !uses_configured_plant(
            &loop_definition,
            "heavy-payload",
            Some(&heavy),
            &configured()
        ),
        "a declared model case uses its own model"
    );
    assert!(
        uses_configured_plant(&loop_definition, "target-response", None, &configured()),
        "the nominal case runs on the plant"
    );
}

#[test]
fn a_declared_scenario_with_operating_outputs_keeps_its_model_and_empty_effects_do_not_count() {
    let loop_definition = plant_loop();
    let operating = declared("operating-range", None, Some(vec![1.0]));
    assert!(!uses_configured_plant(
        &loop_definition,
        "operating-range",
        Some(&operating),
        &configured()
    ));
    let mut no_effects = declared("empty-effects", None, None);
    no_effects.effects = Some(Vec::new());
    assert!(uses_configured_plant(
        &loop_definition,
        "empty-effects",
        Some(&no_effects),
        &configured()
    ));
}

#[test]
fn a_model_choice_runs_every_scenario_on_the_model() {
    // control-studio-configured-plant.test.ts: the nominal case keeps the qualified model (B5.3).
    let loop_definition = plant_loop();
    assert!(!uses_configured_plant(
        &loop_definition,
        "target-response",
        None,
        &VerificationPlant::Model
    ));
}

#[test]
fn every_isolated_run_names_the_plant_it_ran_against() {
    // control-studio-apply-process-time.test.ts (A9).
    assert_eq!(
        isolated_plant_name(&configured(), "m1"),
        texts::configured_plant_name("plant-1")
    );
    assert_eq!(
        isolated_plant_name(&VerificationPlant::Model, "m1"),
        texts::model_name("m1")
    );
}
