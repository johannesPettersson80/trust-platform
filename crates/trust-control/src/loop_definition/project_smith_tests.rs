//! The Smith predictor's seed in the loop contract (specification 35, C9): the typed `smith-pid` seed the engine serves, its rules (family PI,
//! the library block's four variables together, distinct variables), the family and the tuning axes that park it, and its round trip through
//! the loop file. The keys are those `validSmithSeed` names, with the library block's four variables optional together.

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::tests::{complete_loop, decode};
use crate::tuning::structure::Structure;

/// The model's gain, time constant and delay, and the library block's variables, as the loop's parameter binding names them (C9).
const GAIN: &str = "Main.Parameters.ModelGain";
const TIME_CONSTANT: &str = "Main.Parameters.ModelTau";
const DELAY: &str = "Main.Parameters.ModelDelay";
const OUTPUT: &str = "Main.Predictor.Out";
const MAXIMUM: &str = "Main.Parameters.MaximumCorrection";
const OPERATING_INPUT: &str = "Main.Parameters.OperatingInput";
const OPERATING_OUTPUT: &str = "Main.Parameters.OperatingOutput";

/// The program's predictor block with `seed` and no tuning axis (C9).
fn smith_project(family: &str, seed: &Value) -> Value {
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

/// A complete Smith seed: the PI's three variables and the model's three, and the library block's four when `library` is set (C9).
fn smith_seed(library: bool) -> Value {
    let mut seed = json!({
        "kind": "smith-pid",
        "family": "PI",
        "proportionalSymbol": "Main.Parameters.Kp",
        "integralSymbol": "Main.Parameters.Ki",
        "derivativeSymbol": "Main.Parameters.Kd",
        "gainSymbol": GAIN,
        "timeConstantSymbol": TIME_CONSTANT,
        "delaySymbol": DELAY,
    });
    if library {
        seed["predictorOutputSymbol"] = json!(OUTPUT);
        seed["maximumCorrectionSymbol"] = json!(MAXIMUM);
        seed["operatingInputSymbol"] = json!(OPERATING_INPUT);
        seed["operatingOutputSymbol"] = json!(OPERATING_OUTPUT);
    }
    seed
}

/// The loop with the Smith program law `project` in place of its controller law, and the model's variables bound (C9).
fn smith_loop(project: Value) -> Value {
    let mut value = complete_loop();
    let controller = value["controller"].as_object_mut().expect("the controller");
    controller.remove("law");
    controller.insert("design".to_owned(), json!("project"));
    controller.insert("project".to_owned(), project);
    let parameters = value["parameterBinding"]["parameters"]
        .as_array_mut()
        .expect("the parameters");
    for (symbol, unit, convention, minimum, maximum) in [
        (GAIN, "degC/%", "Smith predictor model gain", -1e6, 1e6),
        (
            TIME_CONSTANT,
            "s",
            "Smith predictor model time constant",
            0.001,
            1e5,
        ),
        (DELAY, "s", "Smith predictor model delay", 0.0, 1e4),
        (
            OPERATING_INPUT,
            "%",
            "Smith predictor operating input",
            0.0,
            100.0,
        ),
        (
            OPERATING_OUTPUT,
            "degC",
            "Smith predictor operating output",
            0.0,
            80.0,
        ),
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

/// The predictor block of a loop that is decoded whole.
fn block_of(value: &Value) -> ControllerProject {
    decode(value)
        .expect("the loop decodes")
        .controller
        .project
        .expect("the loop has its program's own law")
}

#[test]
fn a_complete_smith_seed_is_served_and_offers_only_the_pi_family() {
    let value = smith_loop(smith_project("Smith", &smith_seed(true)));
    let project = block_of(&value);
    assert_eq!(project.parked_feature(), None);
    let seed = project.smith().expect("a Smith predictor seed");
    assert_eq!(seed.gain_symbol, GAIN);
    assert_eq!(seed.time_constant_symbol, TIME_CONSTANT);
    assert_eq!(seed.delay_symbol, DELAY);
    let block = seed.library_block.as_ref().expect("the library block");
    assert_eq!(block.output_symbol, OUTPUT);
    assert_eq!(block.maximum_correction_symbol, MAXIMUM);
    assert_eq!(project.offered_families(), vec![Structure::PI]);
    decode(&value)
        .expect("the loop decodes")
        .rust_engine_support()
        .expect("the engine serves a Smith predictor");
}

#[test]
fn the_seed_without_the_library_block_is_served_with_the_model_values_alone() {
    let value = smith_loop(smith_project("Smith", &smith_seed(false)));
    let project = block_of(&value);
    assert_eq!(project.parked_feature(), None);
    let seed = project.smith().expect("a Smith predictor seed");
    assert!(seed.library_block.is_none());
    let block_seed = project.seed.as_ref().expect("the seed");
    assert_eq!(
        block_seed.variables(),
        vec![
            "Main.Parameters.Kp",
            "Main.Parameters.Ki",
            "Main.Parameters.Kd",
            GAIN,
            TIME_CONSTANT,
            DELAY,
        ]
    );
}

#[test]
fn the_library_block_names_all_four_variables_or_none() {
    for missing in [
        "predictorOutputSymbol",
        "maximumCorrectionSymbol",
        "operatingInputSymbol",
        "operatingOutputSymbol",
    ] {
        let mut seed = smith_seed(true);
        seed.as_object_mut().expect("the seed").remove(missing);
        let refusal = decode(&smith_loop(smith_project("Smith", &seed)))
            .expect_err("a partly named library block is malformed");
        assert!(
            refusal
                .to_string()
                .contains("names some of the library block's variables without the others"),
            "{missing}: {refusal}"
        );
    }
}

#[test]
fn a_smith_seed_of_another_family_or_with_a_repeated_variable_is_malformed() {
    let mut family = smith_seed(false);
    family["family"] = json!("PID");
    let refusal = decode(&smith_loop(smith_project("Smith", &family)))
        .expect_err("a Smith predictor is a PI");
    assert!(
        refusal.to_string().contains("must be of family PI"),
        "{refusal}"
    );

    let mut repeated = smith_seed(false);
    repeated["delaySymbol"] = json!(GAIN);
    let refusal = decode(&smith_loop(smith_project("Smith", &repeated)))
        .expect_err("a variable names one term only");
    assert!(
        refusal
            .to_string()
            .contains("names one variable for two of its terms"),
        "{refusal}"
    );

    let mut unknown = smith_seed(false);
    unknown["gain"] = json!(1.0);
    assert!(decode(&smith_loop(smith_project("Smith", &unknown))).is_err());
}

#[test]
fn a_smith_seed_with_tuning_axes_is_parked_by_its_axes() {
    let mut project = smith_project("Smith", &smith_seed(false));
    project["axes"] = json!([{ "symbol": "Main.Parameters.Kp", "values": [1.0, 2.0] }]);
    let value = smith_loop(project);
    let parked = ControllerProject::from_value(&value["controller"]["project"])
        .expect("the block decodes")
        .parked_feature();
    assert_eq!(
        parked.as_deref(),
        Some("the program's own law with tuning axes, components or model parameters")
    );
}

#[test]
fn a_smith_seed_in_another_project_family_is_parked_by_its_family() {
    let value = smith_loop(smith_project("PI", &smith_seed(false)));
    let parked = ControllerProject::from_value(&value["controller"]["project"])
        .expect("the block decodes")
        .parked_feature();
    assert_eq!(
        parked.as_deref(),
        Some("the program's own law of family PI")
    );
}

#[test]
fn the_raw_file_names_the_same_gate_as_the_typed_block() {
    let value = smith_loop(smith_project("Smith", &smith_seed(true)));
    assert_eq!(raw_parked_feature(&value["controller"]["project"]), None);
}

#[test]
fn the_smith_seed_writes_back_as_the_loop_file_holds_it() {
    let value = smith_loop(smith_project("Smith", &smith_seed(true)));
    let block = block_of(&value);
    let written = block.to_value();
    assert_eq!(written["seed"]["kind"], "smith-pid");
    assert_eq!(written["seed"]["predictorOutputSymbol"], OUTPUT);
    let read_again = ControllerProject::from_value(&written).expect("the written block decodes");
    assert_eq!(read_again, block);
}
