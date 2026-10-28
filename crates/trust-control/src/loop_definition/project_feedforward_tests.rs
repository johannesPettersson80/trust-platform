//! The measured-disturbance feedforward's seed in the loop contract (specification 35, C8): the typed `feedforward-pid` seed the engine
//! serves, its rules (family PI, the lead and the lag together, distinct bound variables), the family and the tuning axes that park it,
//! and its round trip through the loop file.

use serde_json::{json, Value};

use super::tests::{complete_loop, decode};
use super::*;
use crate::tuning::structure::Structure;

/// The feedforward gain, and the lead and lag times, as the loop's parameter binding names them (C8).
const GAIN: &str = "Main.Parameters.FeedforwardGain";
const LEAD: &str = "Main.Parameters.LeadS";
const LAG: &str = "Main.Parameters.LagS";

/// The program's feedforward block with `seed` and no tuning axis (C8).
fn feedforward_project(family: &str, seed: &Value) -> Value {
    json!({
        "family": family,
        "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": seed,
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 1,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000,
    })
}

/// A complete feedforward seed: the PI's three variables, the gain, and the lead and lag when `lead_lag` is set (C8).
fn feedforward_seed(lead_lag: bool) -> Value {
    let mut seed = json!({
        "kind": "feedforward-pid",
        "family": "PI",
        "proportionalSymbol": "Main.Parameters.Kp",
        "integralSymbol": "Main.Parameters.Ki",
        "derivativeSymbol": "Main.Parameters.Kd",
        "gainSymbol": GAIN,
    });
    if lead_lag {
        seed["leadSymbol"] = json!(LEAD);
        seed["lagSymbol"] = json!(LAG);
    }
    seed
}

/// The loop with the feedforward program law `project` in place of its controller law, and the feedforward's variables bound (C8).
fn feedforward_loop(project: Value) -> Value {
    let mut value = complete_loop();
    let controller = value["controller"].as_object_mut().expect("the controller");
    controller.remove("law");
    controller.insert("design".to_owned(), json!("project"));
    controller.insert("project".to_owned(), project);
    let parameters = value["parameterBinding"]["parameters"]
        .as_array_mut()
        .expect("the parameters");
    for (symbol, unit, convention, minimum, maximum) in [
        (GAIN, "%/kW", "Feedforward gain", -1e6, 1e6),
        (LEAD, "s", "Feedforward lead time", 0.0, 1e4),
        (LAG, "s", "Feedforward lag time", 0.001, 1e4),
    ] {
        parameters.push(json!({
            "symbol": symbol,
            "iec_type": "LREAL",
            "unit": unit,
            "convention": convention,
            "minimum": minimum,
            "maximum": maximum,
        }));
    }
    value
}

/// The feedforward block of a loop that is decoded whole.
fn block_of(value: &Value) -> ControllerProject {
    decode(value)
        .expect("the loop decodes")
        .controller
        .project
        .expect("the loop has its program's own law")
}

#[test]
fn a_complete_feedforward_seed_is_served_and_offers_only_the_pi_family() {
    let value = feedforward_loop(feedforward_project("feedforward", &feedforward_seed(true)));
    let project = block_of(&value);
    assert_eq!(project.parked_feature(), None);
    let seed = project.feedforward().expect("a feedforward seed");
    assert_eq!(seed.gain_symbol, GAIN);
    assert_eq!(
        seed.lead_lag
            .as_ref()
            .map(|names| names.lead_symbol.as_str()),
        Some(LEAD)
    );
    assert_eq!(project.offered_families(), vec![Structure::PI]);
    let loop_definition = decode(&value).expect("the loop decodes");
    loop_definition
        .rust_engine_support()
        .expect("the engine serves a feedforward seed");
}

#[test]
fn a_feedforward_without_its_lead_and_lag_is_served_with_the_gain_alone() {
    let value = feedforward_loop(feedforward_project("feedforward", &feedforward_seed(false)));
    let project = block_of(&value);
    assert!(project
        .feedforward()
        .is_some_and(|seed| seed.lead_lag.is_none()));
    assert_eq!(project.parked_feature(), None);
}

#[test]
fn a_feedforward_with_a_lead_but_no_lag_is_malformed() {
    let mut seed = feedforward_seed(false);
    seed["leadSymbol"] = json!(LEAD);
    let value = feedforward_loop(feedforward_project("feedforward", &seed));
    let rejection = decode(&value).expect_err("the lead and the lag come together");
    assert!(
        format!("{rejection:?}").contains("without the other"),
        "{rejection:?}"
    );
}

#[test]
fn a_feedforward_seed_that_is_not_of_family_pi_or_names_one_variable_twice_is_malformed() {
    let mut family_pid = feedforward_seed(true);
    family_pid["family"] = json!("PID");
    let value = feedforward_loop(feedforward_project("feedforward", &family_pid));
    assert!(decode(&value).is_err(), "a feedforward is PI");

    let mut repeated = feedforward_seed(true);
    repeated["gainSymbol"] = json!("Main.Parameters.Kp");
    let value = feedforward_loop(feedforward_project("feedforward", &repeated));
    assert!(decode(&value).is_err(), "one variable is two gains");
}

#[test]
fn a_feedforward_seed_in_another_project_family_is_parked_by_its_family() {
    let value = feedforward_loop(feedforward_project("PI", &feedforward_seed(true)));
    let loop_definition = decode(&value).expect("the loop decodes");
    let failure = loop_definition
        .rust_engine_support()
        .expect_err("only the feedforward family takes this seed");
    assert_eq!(failure.cause, Cause::ControllerCapability);
    assert!(
        failure
            .message
            .contains("the program's own law of family PI"),
        "{}",
        failure.message
    );
    assert_eq!(
        parked_feature_of(&value),
        Some("the program's own law of family PI".to_owned())
    );
}

#[test]
fn a_feedforward_with_tuning_axes_is_parked_by_its_axes() {
    let mut project = feedforward_project("feedforward", &feedforward_seed(true));
    project["axes"] = json!([{ "symbol": "Main.Parameters.Kp", "values": [1.0] }]);
    let value = feedforward_loop(project);
    let loop_definition = decode(&value).expect("the loop decodes");
    let failure = loop_definition
        .rust_engine_support()
        .expect_err("a search is parked");
    assert!(
        failure
            .message
            .contains("tuning axes, components or model parameters"),
        "{}",
        failure.message
    );
}

#[test]
fn the_feedforward_seed_writes_back_as_the_loop_file_holds_it() {
    let seed = feedforward_seed(true);
    let project = feedforward_project("feedforward", &seed);
    let block = ControllerProject::from_value(&project).expect("the block decodes");
    let written = block.to_value();
    assert_eq!(written["seed"], seed);
    assert_eq!(written["family"], json!("feedforward"));
}
