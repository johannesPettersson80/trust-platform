//! The declared analysis of a loop and its parameter map, checked when the loop opens (`analysis_rules.rs`, specification 35, B6.1
//! and C12): the shipped root-locus lesson opens, and each rule the page applies refuses its loop.

use serde_json::{json, Value};

use super::*;

/// Lesson 45 (root locus): a project-scripted observation loop with a declared analysis and a parameter map (B6).
const ROOT_LOCUS: &str = include_str!(
    "../../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"
);

fn lesson() -> Value {
    serde_json::from_str(ROOT_LOCUS).expect("the shipped lesson is JSON")
}

/// The refusal of a loop that the engine opens with a failure, as its rejection reads.
fn refused(value: &Value) -> String {
    match LoopDefinition::decode(&value.to_string()) {
        Ok(_) => panic!("the loop must be refused at open"),
        Err(rejection) => rejection.to_string(),
    }
}

#[test]
fn the_shipped_root_locus_lesson_opens_with_its_analysis_and_parameter_map() {
    assert!(LoopDefinition::decode(ROOT_LOCUS).is_ok());
}

#[test]
fn a_loop_without_an_analysis_or_a_parameter_map_opens() {
    let mut value = lesson();
    let fields = value.as_object_mut().expect("the lesson is an object");
    fields.remove("analysis");
    fields.remove("analysisParameterSymbols");
    assert!(LoopDefinition::decode(&value.to_string()).is_ok());
}

#[test]
fn an_analysis_whose_sweep_does_not_hold_its_selected_value_is_refused_at_open() {
    let mut value = lesson();
    value["analysis"]["rootLocus"]["selectedValue"] = json!(99.0);
    assert!(refused(&value).contains("The declared model or parameter sweep is invalid."));
}

#[test]
fn an_analysis_that_does_not_decode_is_refused_at_open() {
    let mut value = lesson();
    value["analysis"]["plant"] = json!(3);
    assert!(refused(&value).contains("the declared analysis is malformed"));
}

#[test]
fn a_parameter_map_without_a_declared_analysis_is_refused_at_open() {
    let mut value = lesson();
    value
        .as_object_mut()
        .expect("the lesson is an object")
        .remove("analysis");
    assert!(refused(&value).contains("need a declared analysis and a parameter binding"));
}

#[test]
fn a_parameter_map_that_names_an_unbound_symbol_is_refused_at_open() {
    let mut value = lesson();
    value["analysisParameterSymbols"]["Kp"] = json!("P1.Missing");
    assert!(refused(&value).contains("P1.Missing is not a bound parameter"));
}

#[test]
fn a_parameter_map_that_binds_two_analysis_parameters_to_one_symbol_is_refused_at_open() {
    let mut value = lesson();
    // Make Kd's unit and range fit P1.Kp, so that only the second use of P1.Kp is refused.
    value["analysis"]["controller"]["parameters"]["Kd"]["unit"] = json!("N·m/rad");
    value["analysisParameterSymbols"]["Kd"] = json!("P1.Kp");
    assert!(refused(&value).contains("binds it twice"));
}

#[test]
fn a_parameter_map_whose_bound_unit_differs_from_the_analysis_is_refused_at_open() {
    let mut value = lesson();
    value["parameterBinding"]["parameters"][0]["unit"] = json!("1");
    assert!(refused(&value).contains("does not bind the analysis parameter Kp"));
}
