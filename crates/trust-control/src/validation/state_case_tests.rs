// The perturbation is compared as the case writes it, exactly.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use super::*;
use crate::commissioning::fixture_loops::parallel_pid_loop;
use crate::loop_definition::LoopDefinition;
use crate::model::general::decode_general_model;
use crate::model::GeneralModel;
use crate::tuning::state_controller::fixtures::transfer_function_model;

/// Lesson 68's control loop and its supplied model: the state-feedback law on the two-tank cascade (C4).
const LESSON_68_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/control-loop.json"
);
const LESSON_68_MODEL: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/provided-model.json"
);

fn lesson_68() -> LoopDefinition {
    LoopDefinition::decode(LESSON_68_LOOP).expect("lesson 68's loop file decodes")
}

fn lesson_68_model() -> GeneralModel {
    let value: serde_json::Value =
        serde_json::from_str(LESSON_68_MODEL).expect("lesson 68's model file is JSON");
    decode_general_model(&value).expect("lesson 68's model file decodes")
}

/// A plan of lesson 68's shape: one second per scan and the two minutes of its validation.
fn plan() -> VerificationPlan {
    VerificationPlan {
        step_ms: 1000,
        duration_ms: 120_000,
        max_total_cycles: 1000,
        record_every: 1,
        initial_values: BTreeMap::new(),
        observed_symbols: Vec::new(),
        requirements: Vec::new(),
    }
}

#[test]
fn lesson_68_moves_each_level_by_one_per_cent_of_its_range_from_its_configured_start() {
    // Each level's range is -0.2 to 0.2 m, so one per cent is 0.004 m; both start at zero, the configured initial state.
    let perturbed = perturbed_state(&lesson_68(), &lesson_68_model())
        .expect("the perturbation builds")
        .expect("a state law has one");
    assert_eq!(perturbed.len(), 2);
    assert!((perturbed[0] - 0.004).abs() < 1e-12, "{}", perturbed[0]);
    assert!((perturbed[1] - 0.004).abs() < 1e-12, "{}", perturbed[1]);
}

#[test]
fn a_state_at_the_top_of_its_range_is_moved_down_instead() {
    let mut loop_definition = lesson_68();
    loop_definition
        .channels
        .as_mut()
        .expect("lesson 68 has channels")
        .initial_state = Some(vec![0.2, 0.0]);
    let perturbed = perturbed_state(&loop_definition, &lesson_68_model())
        .expect("the perturbation builds")
        .expect("a state law has one");
    assert!((perturbed[0] - 0.196).abs() < 1e-12, "{}", perturbed[0]);
    assert!((perturbed[1] - 0.004).abs() < 1e-12, "{}", perturbed[1]);
}

#[test]
fn a_loop_without_a_state_law_and_a_transfer_function_model_have_no_recovery() {
    assert!(perturbed_state(&parallel_pid_loop(), &lesson_68_model())
        .expect("no law")
        .is_none());
    assert!(perturbed_state(&lesson_68(), &transfer_function_model())
        .expect("no state to perturb")
        .is_none());
}

#[test]
fn the_recovery_case_is_the_nominal_plan_under_its_own_identity() {
    let nominal = plan();
    let case = recovery_case(&lesson_68(), &lesson_68_model(), &nominal)
        .expect("the case builds")
        .expect("a state law has one");
    assert_eq!(case.id, STATE_RECOVERY_ID);
    assert_eq!(case.plan, nominal);
    assert_eq!(
        case.events,
        [] as [crate::validation::scenario_contract::ScenarioEvent; 0]
    );
    assert_eq!(
        case.effects,
        [] as [crate::validation::engineer_scenario_effects::EngineerScenarioEffect; 0]
    );
    assert!(case.initial_overrides.is_empty());
}

#[test]
fn the_recovery_plant_starts_the_selected_model_from_the_perturbed_state() {
    let plant = recovery_plant(&lesson_68(), &lesson_68_model(), &BTreeMap::new())
        .expect("the route builds")
        .expect("a state law has a recovery plant");
    assert_eq!(plant.model, lesson_68_model());
}
