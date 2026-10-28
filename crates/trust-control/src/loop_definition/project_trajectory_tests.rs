//! A trajectory-following law in the loop contract (specification 35, C11): the `trajectory-feedforward-pid` seed of five distinct bound
//! variables, served as a PID law, the refusals of its shape, and lesson 90 as the shipped file declares it.

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::tests::{complete_loop, decode};
use crate::loop_definition::LoopRejection;
use crate::tuning::structure::Structure;

/// The loop with the program's own law `project` in place of its controller law, designed by the project (C2).
fn project_loop(project: Value) -> Value {
    let mut value = complete_loop();
    let controller = value["controller"].as_object_mut().expect("the controller");
    controller.remove("law");
    controller.insert("design".to_owned(), json!("project"));
    controller.insert("project".to_owned(), project);
    value
}

/// The loop with the rate and acceleration feedforward gains bound beside the PID gains of `complete_loop` (C11).
fn with_feedforward_parameters(mut value: Value) -> Value {
    let parameters = value["parameterBinding"]["parameters"]
        .as_array_mut()
        .expect("the parameters");
    for (symbol, unit, convention) in [
        ("Main.Parameters.Kv", "N·m·s/rad", "Rate feedforward gain"),
        (
            "Main.Parameters.Ka",
            "N·m·s²/rad",
            "Acceleration feedforward gain",
        ),
    ] {
        parameters.push(json!({
            "symbol": symbol,
            "iec_type": "LREAL",
            "unit": unit,
            "convention": convention,
            "minimum": 0,
            "maximum": 10,
        }));
    }
    value
}

/// The program's own PID law of a trajectory seed of `family`, whose variables are the loop's `Main.Parameters` gains.
fn trajectory_project(family: &str) -> Value {
    json!({
        "family": "PID",
        "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": {
            "kind": "trajectory-feedforward-pid",
            "family": family,
            "proportionalSymbol": "Main.Parameters.Kp",
            "integralSymbol": "Main.Parameters.Ki",
            "derivativeSymbol": "Main.Parameters.Kd",
            "rateSymbol": "Main.Parameters.Kv",
            "accelerationSymbol": "Main.Parameters.Ka",
        },
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000,
    })
}

/// The served trajectory loop with one field of its seed replaced.
fn with_seed_field(name: &str, replacement: Value) -> Value {
    let mut project = trajectory_project("PID");
    project["seed"][name] = replacement;
    with_feedforward_parameters(project_loop(project))
}

/// The refusal of a loop that does not decode, by its text.
fn refusal(value: &Value) -> String {
    match decode(value) {
        Err(LoopRejection::Malformed(message)) => message,
        _ => panic!("the loop is malformed"),
    }
}

/// The controller block of a loop that decodes.
fn project_of(value: &Value) -> ControllerProject {
    decode(value)
        .expect("the loop decodes")
        .controller
        .project
        .expect("the project block")
}

#[test]
fn a_trajectory_seed_of_five_bound_variables_decodes_and_is_served_as_a_pid_law() {
    let value = with_feedforward_parameters(project_loop(trajectory_project("PID")));
    let loop_definition = decode(&value).expect("the trajectory seed decodes");
    loop_definition
        .rust_engine_support()
        .expect("the engine serves a trajectory-feedforward PID (C11)");
    let project = project_of(&value);
    assert_eq!(project.parked_feature(), None);
    assert_eq!(
        project.trajectory().map(|seed| seed.rate_symbol.as_str()),
        Some("Main.Parameters.Kv")
    );
    assert_eq!(
        project
            .trajectory()
            .map(|seed| seed.acceleration_symbol.as_str()),
        Some("Main.Parameters.Ka")
    );
    assert_eq!(
        project.offered_families(),
        vec![Structure::P, Structure::PI, Structure::PD, Structure::PID],
        "the seed names all three PID variables, so every family is offered"
    );
}

#[test]
fn the_trajectory_seed_names_exactly_its_seven_keys() {
    let mut extra = trajectory_project("PID");
    extra["seed"]["setpointSymbol"] = json!("Main.SetpointRad");
    assert!(refusal(&with_feedforward_parameters(project_loop(extra))).contains("setpointSymbol"));

    let mut missing = trajectory_project("PID");
    missing["seed"]
        .as_object_mut()
        .expect("the seed")
        .remove("accelerationSymbol");
    assert!(
        refusal(&with_feedforward_parameters(project_loop(missing))).contains("accelerationSymbol")
    );
}

#[test]
fn the_trajectory_seed_names_five_distinct_variables_that_are_bound_parameters() {
    let repeated = with_seed_field("rateSymbol", json!("Main.Parameters.Kp"));
    assert!(refusal(&repeated).contains("names one variable for two gains"));

    let unbound = with_seed_field("rateSymbol", json!("Main.Parameters.Kz"));
    assert!(refusal(&unbound).contains("Main.Parameters.Kz"));
}

#[test]
fn a_trajectory_law_with_a_tuning_axis_stays_parked_by_the_gate() {
    let mut project = trajectory_project("PID");
    project["axes"] = json!([{ "symbol": "Main.Parameters.Kp", "values": [1.0, 2.0] }]);
    let value = with_feedforward_parameters(project_loop(project));
    assert_eq!(
        project_of(&value).parked_feature().as_deref(),
        Some("the program's own law with tuning axes, components or model parameters")
    );
}

#[test]
fn a_trajectory_law_of_a_family_outside_p_to_pid_stays_parked_by_the_gate() {
    let mut project = trajectory_project("PID");
    project["family"] = json!("MPC");
    let value = with_feedforward_parameters(project_loop(project));
    assert_eq!(
        project_of(&value).parked_feature().as_deref(),
        Some("the program's own law of family MPC")
    );
}

#[test]
fn a_trajectory_seed_is_written_back_with_its_kind_and_its_feedforward_variables() {
    let value = with_feedforward_parameters(project_loop(trajectory_project("PI")));
    let written = project_of(&value).to_value();
    assert_eq!(written["seed"]["kind"], json!("trajectory-feedforward-pid"));
    assert_eq!(written["seed"]["family"], json!("PI"));
    assert_eq!(written["seed"]["rateSymbol"], json!("Main.Parameters.Kv"));
    assert_eq!(
        written["seed"]["accelerationSymbol"],
        json!("Main.Parameters.Ka")
    );
    assert_eq!(
        written["seed"].as_object().map(serde_json::Map::len),
        Some(7)
    );
}

#[test]
fn the_shipped_lesson_90_file_opens_on_the_engine_as_a_trajectory_law() {
    let text = include_str!(
        "../../../../editors/vscode/media/examples/control/09-industrial-control-structures/90-trajectory-feedforward/control-loop.json"
    );
    let loop_definition = LoopDefinition::decode(text).expect("lesson 90 decodes");
    loop_definition
        .rust_engine_support()
        .expect("the engine serves lesson 90 (C11)");
    assert!(
        loop_definition.controller.law.is_none(),
        "the program owns the law"
    );
    assert!(!loop_definition.is_observation_or_open_loop());
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("lesson 90's block");
    assert_eq!(project.parked_feature(), None);
    assert!(
        project.trajectory().is_some(),
        "its seed is the trajectory seed"
    );
    assert_eq!(
        project.offered_families(),
        vec![Structure::P, Structure::PI, Structure::PD, Structure::PID]
    );
}
