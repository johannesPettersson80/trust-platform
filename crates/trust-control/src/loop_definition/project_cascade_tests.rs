//! The cascade's seed in the loop contract (specification 35, C6): the typed `cascade-pid` seed the engine serves, its rules (family PI, two
//! different instances, six distinct variables), the family that parks it, and its round trip through the loop file.

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::tests::{complete_loop, decode};
use crate::tuning::structure::Structure;

/// The six gain variables of the vessel's two PIs, as the loop's parameter binding names them (C6).
const INNER_P: &str = "Main.InnerParameters.ProportionalGain";
const INNER_I: &str = "Main.InnerParameters.IntegralGain";
const INNER_D: &str = "Main.InnerParameters.DerivativeGain";
const OUTER_P: &str = "Main.OuterParameters.ProportionalGain";
const OUTER_I: &str = "Main.OuterParameters.IntegralGain";
const OUTER_D: &str = "Main.OuterParameters.DerivativeGain";

/// A complete cascade seed: both controllers' instances and their three variables each (C6).
fn cascade_seed() -> Value {
    json!({
        "kind": "cascade-pid",
        "family": "PI",
        "inner": {
            "instance": "Main.Inner",
            "proportionalSymbol": INNER_P,
            "integralSymbol": INNER_I,
            "derivativeSymbol": INNER_D,
        },
        "outer": {
            "instance": "Main.Outer",
            "proportionalSymbol": OUTER_P,
            "integralSymbol": OUTER_I,
            "derivativeSymbol": OUTER_D,
        },
    })
}

/// The program's cascade block with `seed` and no tuning axis (C6).
fn cascade_project(family: &str, seed: &Value) -> Value {
    json!({
        "family": family,
        "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": seed,
        "axes": [],
        "objective": { "requirementId": "Main.VesselC:target-error", "direction": "minimize" },
        "maximumCandidates": 1,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000,
    })
}

/// A loop whose program law is the cascade block, with the six gains bound as parameters (C6).
fn cascade_loop(project: Value) -> Value {
    let mut value = complete_loop();
    let controller = value["controller"].as_object_mut().expect("the controller");
    controller.remove("law");
    controller.insert("design".to_owned(), json!("project"));
    controller.insert("project".to_owned(), project);
    let parameters = value["parameterBinding"]["parameters"]
        .as_array_mut()
        .expect("the parameters");
    for symbol in [INNER_P, INNER_I, INNER_D, OUTER_P, OUTER_I, OUTER_D] {
        parameters.push(json!({
            "symbol": symbol,
            "iec_type": "LREAL",
            "unit": "1",
            "convention": "Control.PID gain",
            "minimum": -1e6,
            "maximum": 1e6,
        }));
    }
    value
}

#[test]
fn a_complete_cascade_seed_is_served_and_offers_only_pi() {
    let value = cascade_loop(cascade_project("cascade", &cascade_seed()));
    let loop_definition = decode(&value).expect("the loop decodes");
    assert!(
        loop_definition.rust_engine_support().is_ok(),
        "the Rust engine serves a cascade seed"
    );
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the program's own law");
    assert_eq!(project.offered_families(), vec![Structure::PI]);
    assert_eq!(
        project.cascade().map(|seed| seed.inner.instance.as_str()),
        Some("Main.Inner")
    );
}

#[test]
fn a_cascade_seed_in_another_family_is_parked_by_its_family() {
    let value = cascade_loop(cascade_project("PI", &cascade_seed()));
    let loop_definition = decode(&value).expect("the loop decodes");
    let refusal = loop_definition
        .rust_engine_support()
        .expect_err("a cascade seed belongs to the cascade family");
    assert!(refusal.message.contains("family PI"), "{}", refusal.message);
}

#[test]
fn a_cascade_seed_must_be_pi_name_two_controllers_and_hold_six_distinct_variables() {
    let mut not_pi = cascade_seed();
    not_pi["family"] = json!("P");
    assert!(ControllerProject::from_value(&cascade_project("cascade", &not_pi)).is_err());

    let mut same_instance = cascade_seed();
    same_instance["outer"]["instance"] = json!("Main.Inner");
    assert!(ControllerProject::from_value(&cascade_project("cascade", &same_instance)).is_err());

    let mut repeated = cascade_seed();
    repeated["outer"]["integralSymbol"] = json!(INNER_I);
    assert!(ControllerProject::from_value(&cascade_project("cascade", &repeated)).is_err());

    let mut unknown = cascade_seed();
    unknown["inner"]["gainSymbol"] = json!("Main.Gain");
    assert!(ControllerProject::from_value(&cascade_project("cascade", &unknown)).is_err());
}

#[test]
fn the_outer_controllers_mode_is_the_transfer_additional_mode() {
    let mut project = cascade_project("cascade", &cascade_seed());
    project["additionalModes"] = json!([{ "symbol": "Main.OuterAuto", "manualValue": false }]);
    let loop_definition = decode(&cascade_loop(project)).expect("the loop decodes");
    let members = crate::commissioning::project_law::additional_mode_members(&loop_definition);
    assert_eq!(
        members
            .iter()
            .map(|mode| (mode.symbol.as_str(), mode.manual_value))
            .collect::<Vec<_>>(),
        vec![("Main.OuterAuto", false)],
        "the outer controller's mode switches at the same boundary as the inner one's"
    );
}

#[test]
fn the_cascade_seed_writes_back_as_the_loop_file_holds_it() {
    let project = ControllerProject::from_value(&cascade_project("cascade", &cascade_seed()))
        .expect("the block decodes");
    assert_eq!(project.to_value()["seed"], cascade_seed());
}
