//! The gate of a declared analysis (specification 35, §0.1a and C12): a C12 observation or open-loop loop with its analysis is served
//! by the engine, and any other loop with the analysis keys stays parked on them (stage V7).

use serde_json::{json, Value};

use super::*;

/// Lessons 44 (Routh-Hurwitz), 45 (root locus), 47 (Nyquist) and 48 (sensitivity): project-scripted observation loops with a declared
/// analysis, which the engine serves (B6).
const SHIPPED_ANALYSIS_LESSONS: [&str; 4] = [
    include_str!(
        "../../../../examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz/control-loop.json"
    ),
    include_str!(
        "../../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"
    ),
    include_str!(
        "../../../../examples/control/07-classical-stability-and-frequency-design/47-nyquist-stability/control-loop.json"
    ),
    include_str!(
        "../../../../examples/control/07-classical-stability-and-frequency-design/48-sensitivity-functions/control-loop.json"
    ),
];

fn raw(text: &str) -> Value {
    serde_json::from_str(text).expect("the shipped lesson is JSON")
}

#[test]
fn the_shipped_analysis_lessons_are_served_by_the_engine() {
    for text in SHIPPED_ANALYSIS_LESSONS {
        let loop_definition = LoopDefinition::decode(text).expect("the shipped lesson decodes");
        assert!(
            loop_definition.is_observation_or_open_loop(),
            "{} is a C12 observation loop",
            loop_definition.id
        );
        assert!(
            loop_definition.rust_engine_support().is_ok(),
            "{} is served",
            loop_definition.id
        );
        assert_eq!(
            parked_feature_of(&raw(text)),
            None,
            "{}",
            loop_definition.id
        );
    }
}

#[test]
fn an_analysis_on_a_loop_that_is_not_a_c12_loop_parks_it() {
    let mut value = raw(SHIPPED_ANALYSIS_LESSONS[1]);
    value["controller"]["design"] = json!("pid");
    let loop_definition = LoopDefinition::decode(&value.to_string()).expect("the loop decodes");
    assert!(loop_definition.declares_parked_analysis());
    let failure = loop_definition
        .rust_engine_support()
        .expect_err("a loop that is not a C12 kind stays parked");
    assert!(
        failure.message.contains("a declared analysis"),
        "{}",
        failure.message
    );
    assert_eq!(failure.cause, Cause::ControllerCapability);
    assert_eq!(
        parked_feature_of(&value),
        Some("a declared analysis".to_owned())
    );
}

#[test]
fn a_mapping_alone_parks_a_loop_that_is_not_a_c12_loop() {
    let mut value = raw(SHIPPED_ANALYSIS_LESSONS[0]);
    value["controller"]["design"] = json!("pid");
    value.as_object_mut().expect("the loop").remove("analysis");
    assert_eq!(
        parked_feature_of(&value),
        Some("a declared analysis".to_owned()),
        "the mapping keys park a file that is not a C12 kind"
    );
}

#[test]
fn a_c12_loop_without_analysis_keys_has_no_analysis_to_park_it() {
    let mut value = raw(SHIPPED_ANALYSIS_LESSONS[0]);
    let object = value.as_object_mut().expect("the loop");
    object.remove("analysis");
    object.remove("analysisParameterSymbols");
    assert_eq!(parked_feature_of(&value), None);
    let loop_definition = LoopDefinition::decode(&value.to_string()).expect("the loop decodes");
    assert!(!loop_definition.declares_parked_analysis());
}
