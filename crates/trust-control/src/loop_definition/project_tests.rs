//! A program's own law in the loop contract (specification 35, C2): the typed block, the shape the engine serves, the shapes it parks
//! by name, and the rules that refuse a block by the field it breaks.

use serde_json::{json, Value};

use super::tests::{complete_loop, decode, with};
use super::*;
use crate::tuning::structure::Structure;

/// The parked feature of a `parallel-pid` law that declares tuning axes, components or model parameters (C5).
const TUNING_SEARCH: &str =
    "the program's own law with tuning axes, components or model parameters";

/// The loop with the program's own law `project` in place of its controller law, designed by the project (C2).
fn project_loop(project: Value) -> Value {
    let mut value = complete_loop();
    let controller = value["controller"].as_object_mut().expect("the controller");
    controller.remove("law");
    controller.insert("design".to_owned(), json!("project"));
    controller.insert("project".to_owned(), project);
    value
}

/// The program's own law of a served shape: a `parallel-pid` seed of `family` that names the loop's `Main.Parameters` gains.
fn served_project(family: &str, integral: bool, derivative: bool) -> Value {
    let mut seed = json!({
        "kind": "parallel-pid",
        "family": family,
        "proportionalSymbol": "Main.Parameters.Kp",
    });
    if integral {
        seed["integralSymbol"] = json!("Main.Parameters.Ki");
    }
    if derivative {
        seed["derivativeSymbol"] = json!("Main.Parameters.Kd");
    }
    json!({
        "family": family,
        "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": seed,
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000,
    })
}

/// The served loop with one field of its program's own law replaced.
fn broken(path: &[&str], replacement: Value) -> Value {
    with(
        project_loop(served_project("PI", true, false)),
        path,
        replacement,
    )
}

/// The loop with two dimensionless parameters, the reference weights that a component may name (C5).
fn with_dimensionless_parameters(mut value: Value) -> Value {
    let parameters = value["parameterBinding"]["parameters"]
        .as_array_mut()
        .expect("the parameters");
    for symbol in ["Main.Parameters.Wp", "Main.Parameters.Wd"] {
        parameters.push(json!({
            "symbol": symbol,
            "iec_type": "LREAL",
            "unit": "1",
            "convention": "reference weight",
            "minimum": 0,
            "maximum": 1,
        }));
    }
    value
}

/// The refusal of a loop that does not decode, by its text.
fn malformed(value: &Value) -> String {
    match decode(value) {
        Err(LoopRejection::Malformed(message)) => message,
        other => panic!("expected a malformed loop, got {other:?}"),
    }
}

/// The program's own law of a decoded loop.
fn decoded_project(value: &Value) -> ControllerProject {
    decode(value)
        .expect("the loop decodes")
        .controller
        .project
        .expect("a project block")
}

#[test]
fn a_parallel_pid_law_of_the_served_shape_decodes_runs_and_is_never_an_observation_loop() {
    let value = project_loop(served_project("PI", true, false));
    let loop_definition = decode(&value).expect("the served shape decodes");
    loop_definition
        .rust_engine_support()
        .expect("the engine serves the program's own PI law (C2)");
    assert!(
        !loop_definition.is_observation_or_open_loop(),
        "a project law is never a C12 loop"
    );
    loop_definition
        .require_controller_law()
        .expect("the law stages have a law to act on");
    let project = decoded_project(&value);
    assert_eq!(project.parked_feature(), None);
    assert_eq!(project.family, "PI");
    assert_eq!(project.maximum_candidates, 4);
    assert_eq!(project.maximum_cycles, 50_000);
    assert_eq!(project.wall_budget_ms, 60_000);
}

#[test]
fn the_families_offered_are_p_always_and_the_terms_the_seed_names() {
    let cases = [
        ("P", false, false, vec![Structure::P]),
        ("PD", false, true, vec![Structure::P, Structure::PD]),
        (
            "PID",
            true,
            true,
            vec![Structure::P, Structure::PI, Structure::PD, Structure::PID],
        ),
    ];
    for (family, integral, derivative, offered) in cases {
        let project = decoded_project(&project_loop(served_project(family, integral, derivative)));
        assert_eq!(project.parked_feature(), None, "{family} is served");
        assert_eq!(project.offered_families(), offered, "{family}");
    }
}

#[test]
fn every_other_shape_of_a_program_law_is_parked_by_its_name_and_the_raw_file_names_it_too() {
    let mut cases: Vec<(Value, String)> = Vec::new();
    let mut without_seed = served_project("PI", true, false);
    without_seed
        .as_object_mut()
        .expect("the block")
        .remove("seed");
    cases.push((
        project_loop(without_seed),
        "the program's own law without a seed".to_owned(),
    ));
    let mut with_axes = served_project("PI", true, false);
    with_axes["axes"] = json!([{ "symbol": "Main.Parameters.Kp", "values": [1.0, 2.0] }]);
    cases.push((project_loop(with_axes), TUNING_SEARCH.to_owned()));
    let mut with_components = served_project("PI", true, false);
    with_components["components"] = json!([{
        "kind": "reference-weighting",
        "proportionalSymbol": "Main.Parameters.Wp",
        "derivativeSymbol": "Main.Parameters.Wd",
        "proportionalWeights": [0.5, 1.0],
        "derivativeWeights": [0.0],
    }]);
    cases.push((
        with_dimensionless_parameters(project_loop(with_components)),
        TUNING_SEARCH.to_owned(),
    ));
    let mut with_model_parameters = served_project("P", false, false);
    with_model_parameters["modelParameters"] =
        json!({ "Main.Parameters.Kp": { "kind": "plant-gain" } });
    cases.push((
        project_loop(with_model_parameters),
        TUNING_SEARCH.to_owned(),
    ));
    let mut other_family = served_project("PI", true, false);
    other_family["family"] = json!("cascade");
    cases.push((
        project_loop(other_family),
        "the program's own law of family cascade".to_owned(),
    ));
    for (value, feature) in cases {
        let loop_definition =
            decode(&value).expect("a parked shape is read in full where its fields are whole");
        let failure = loop_definition
            .rust_engine_support()
            .expect_err("a parked shape is refused");
        assert_eq!(failure.cause, Cause::ControllerCapability, "{feature}");
        assert!(failure.message.contains(&feature), "{}", failure.message);
        assert!(!loop_definition.is_observation_or_open_loop(), "{feature}");
        assert_eq!(
            parked_feature_of(&value),
            Some(feature),
            "the raw file names the same feature"
        );
    }
}

/// The first half of the broken program laws of the contract, each with the field its refusal names.
fn broken_laws_first() -> Vec<(Value, &'static str)> {
    let nine_axes = Value::Array(
        (0..9)
            .map(|_| json!({ "symbol": "Main.Parameters.Kp", "values": [1.0] }))
            .collect(),
    );
    vec![
        (
            broken(&["controller", "project", "maximumCandidates"], json!(0)),
            "maximumCandidates",
        ),
        (
            broken(&["controller", "project", "maximumCandidates"], json!(65)),
            "maximumCandidates",
        ),
        (
            broken(&["controller", "project", "maximumCycles"], json!(0)),
            "maximumCycles",
        ),
        (
            broken(
                &["controller", "project", "maximumCycles"],
                json!(10_000_001),
            ),
            "maximumCycles",
        ),
        (
            broken(&["controller", "project", "wallBudgetMs"], json!(3_600_001)),
            "wallBudgetMs",
        ),
        (
            broken(
                &["controller", "project", "objective", "direction"],
                json!("up"),
            ),
            "direction",
        ),
        (
            broken(
                &["controller", "project", "objective", "requirementId"],
                json!(""),
            ),
            "requirementId",
        ),
        (
            broken(&["controller", "project", "convention"], json!("other")),
            "convention",
        ),
        (
            broken(&["controller", "project", "family"], json!("bogus")),
            "family",
        ),
        (
            broken(
                &["controller", "project", "initialization", "kind"],
                json!("control-pid-v1"),
            ),
            "initialization",
        ),
        (
            broken(&["controller", "project", "axes"], nine_axes),
            "axes",
        ),
        (
            broken(
                &["controller", "project", "axes"],
                json!([{ "symbol": "Main.Parameters.Kp", "values": [] }]),
            ),
            "values",
        ),
        (
            broken(
                &["controller", "project", "axes"],
                json!([{ "symbol": "Main.Missing", "values": [1.0] }]),
            ),
            "Main.Missing",
        ),
    ]
}

/// The second half of the broken program laws of the contract, each with the field its refusal names.
fn broken_laws_second() -> Vec<(Value, &'static str)> {
    vec![
        (
            broken(
                &["controller", "project", "axes"],
                json!([{ "symbol": "Main.Parameters.Kp", "values": [101.0] }]),
            ),
            "outside its bounds",
        ),
        (
            broken(
                &["controller", "project", "axes"],
                json!([
                    { "symbol": "Main.Parameters.Kp", "values": [1.0] },
                    { "symbol": "Main.Parameters.Kp", "values": [2.0] },
                ]),
            ),
            "another axis",
        ),
        (
            broken(
                &["controller", "project", "seed"],
                json!({ "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "Main.Parameters.Kp" }),
            ),
            "integral",
        ),
        (
            broken(
                &["controller", "project", "seed", "proportionalSymbol"],
                json!("Main.Missing"),
            ),
            "Main.Missing",
        ),
        (
            broken(
                &["controller", "project", "seed"],
                json!({ "kind": "pid-x" }),
            ),
            "seed.kind",
        ),
        (
            broken(
                &["controller", "project", "seed", "integralSymbol"],
                json!("Main.Parameters.Kp"),
            ),
            "one variable",
        ),
        (
            broken(
                &["controller", "project", "modelParameters"],
                json!({ "Main.Missing": { "kind": "plant-gain" } }),
            ),
            "Main.Missing",
        ),
        (
            broken(
                &["controller", "project", "modelParameters"],
                json!({ "Main.Parameters.Kp": { "kind": "warp" } }),
            ),
            "modelParameters",
        ),
        (
            broken(
                &["controller", "project", "requiredScenarioIds"],
                json!([""]),
            ),
            "requiredScenarioIds",
        ),
        (
            broken(
                &["controller", "project", "additionalModes"],
                json!([{ "symbol": "main.auto", "manualValue": true }]),
            ),
            "repeats",
        ),
        (
            broken(
                &["controller", "project", "additionalModes"],
                json!([{ "symbol": "Main.Hold", "manualValue": true, "extra": 1 }]),
            ),
            "manualValue",
        ),
        (
            broken(
                &["controller", "project", "components"],
                json!([{ "kind": "low-pass", "timeConstantSymbol": "Main.Parameters.Kp", "timeConstantsSeconds": [0.1], "order": 1 }]),
            ),
            "components",
        ),
    ]
}

/// Each broken program law of the contract with the field its refusal names (`decodeProjectController`).
fn broken_laws() -> Vec<(Value, &'static str)> {
    let mut cases = broken_laws_first();
    cases.extend(broken_laws_second());
    cases
}

#[test]
fn a_program_law_that_breaks_a_rule_of_the_contract_is_refused_by_the_field_it_breaks() {
    for (value, field) in broken_laws() {
        let message = malformed(&value);
        assert!(message.contains(field), "{field}: {message}");
    }
}

#[test]
fn a_real_axis_value_must_be_exact_in_single_precision() {
    let mut value = project_loop(served_project("PI", true, false));
    value["parameterBinding"]["parameters"][0]["iec_type"] = json!("REAL");
    value["controller"]["project"]["axes"] =
        json!([{ "symbol": "Main.Parameters.Kp", "values": [0.5] }]);
    assert!(decode(&value).is_ok(), "0.5 is exact in single precision");
    value["controller"]["project"]["axes"] =
        json!([{ "symbol": "Main.Parameters.Kp", "values": [0.1] }]);
    assert!(malformed(&value).contains("exact REAL"));
}

#[test]
fn a_component_and_an_axis_may_not_name_the_same_variable() {
    let mut value = with_dimensionless_parameters(project_loop(served_project("PI", true, false)));
    value["controller"]["project"]["components"] = json!([{
        "kind": "reference-weighting",
        "proportionalSymbol": "Main.Parameters.Wp",
        "derivativeSymbol": "Main.Parameters.Wd",
        "proportionalWeights": [0.5],
        "derivativeWeights": [0.5],
    }]);
    value["controller"]["project"]["axes"] =
        json!([{ "symbol": "Main.Parameters.Wp", "values": [0.5] }]);
    assert!(malformed(&value).contains("the same variable"));
}

#[test]
fn a_served_shape_that_breaks_a_rule_is_malformed_and_is_not_parked() {
    let value = broken(&["controller", "project", "maximumCandidates"], json!(0));
    assert_eq!(parked_feature_of(&value), None);
    assert!(malformed(&value).contains("maximumCandidates"));
}

#[test]
fn a_program_law_needs_the_parameter_binding_to_check_its_variables() {
    let mut value = project_loop(served_project("PI", true, false));
    value
        .as_object_mut()
        .expect("the loop")
        .remove("parameterBinding");
    assert!(malformed(&value).contains("parameter binding"));
}

#[test]
fn a_loop_that_declares_a_controller_law_and_a_program_law_is_refused_by_the_engine() {
    let mut value = project_loop(served_project("PI", true, false));
    value["controller"]["law"] = complete_loop()["controller"]["law"].clone();
    let loop_definition = decode(&value).expect("the file decodes");
    let failure = loop_definition
        .rust_engine_support()
        .expect_err("two laws are refused");
    assert_eq!(failure.cause, Cause::Implementation);
    assert!(failure.message.contains("both"), "{}", failure.message);
}

#[test]
fn a_served_program_law_keeps_the_cascade_and_the_declared_analysis_parked() {
    let mut cascade = project_loop(served_project("PI", true, false));
    cascade["cascade"] = json!({ "kind": "outer" });
    let failure = decode(&cascade)
        .expect("decoded")
        .rust_engine_support()
        .expect_err("a cascade is parked");
    assert_eq!(failure.cause, Cause::ControllerCapability);
    assert!(failure.message.contains("a cascade"), "{}", failure.message);
    // A declared analysis the page accepts, in the loop's units (%): a malformed one is refused at open (`analysis_rules.rs`), so
    // the parking is tested with a well-formed analysis.
    let mut analysis = project_loop(served_project("PI", true, false));
    analysis["analysis"] = json!({
        "plant": {
            "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 1.0], "delaySeconds": 0.0,
            "inputUnit": "%", "outputUnit": "%", "revision": "sha256:plant", "source": "provided", "validation": "unvalidated"
        },
        "controller": {
            "numerator": [{ "parameter": "Kp" }], "denominator": [1.0],
            "parameters": { "Kp": { "value": 1.0, "minimum": 0.0, "maximum": 100.0, "unit": "%/%" } }
        },
        "rootLocus": { "parameter": "Kp", "values": [1.0, 2.0], "selectedValue": 1.0 }
    });
    let failure = decode(&analysis)
        .expect("decoded")
        .rust_engine_support()
        .expect_err("a declared analysis is parked");
    assert!(
        failure.message.contains("a declared analysis"),
        "{}",
        failure.message
    );
}

#[test]
fn a_program_law_is_written_back_as_the_block_it_reads() {
    let value = project_loop(served_project("PID", true, true));
    let loop_definition = decode(&value).expect("decoded");
    let written = serde_json::to_string(&loop_definition).expect("the loop serializes");
    assert_eq!(
        LoopDefinition::decode(&written).expect("the written loop decodes"),
        loop_definition
    );
    let written: Value = serde_json::from_str(&written).expect("json");
    assert_eq!(
        written["controller"]["project"],
        value["controller"]["project"]
    );
}
