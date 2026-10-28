//! Tests of the scenario form (specification 35, B4.3): the numbers the page types as text are read as their numbers, the study's own
//! object form is read as it is, and each rule that the list breaks is refused under `scenarios` with its sentence.

#![allow(clippy::float_cmp)] // The times and values are the exact decimal texts the tests give.

use serde_json::{json, Value};

use super::scenarios_of_form;
use crate::validation::run_validation::RunCheck;
use crate::validation::scenario_contract::EngineerScenario;

/// A scenario as the page sends it: its initial values and timed changes as given, and one bounds check whose limits are text.
fn scenario(id: &str, initial: &Value, events: &Value) -> Value {
    json!({
        "id": id,
        "initialValues": initial,
        "events": events,
        "requirements": [{
            "id": "r1", "channel": "SP", "window": { "start": 0.0, "end": 4.0 },
            "kind": "bounds", "minimum": "0", "maximum": " 10 "
        }]
    })
}

/// The message and the field of the refusal that a list receives.
fn refusal(list: &Value) -> (String, Option<String>) {
    let failure = scenarios_of_form(list).expect_err("the list is refused");
    (failure.message, failure.field)
}

#[test]
fn the_numbers_the_page_types_as_text_are_read_as_their_numbers() {
    let list = json!([scenario(
        " step ",
        &json!([{ "symbol": " SP ", "value": " 2.5 " }]),
        &json!([{ "atMs": "1500", "symbol": " SP ", "value": "3" }]),
    )]);
    let scenarios = scenarios_of_form(&list).expect("the list is read");
    let read: &EngineerScenario = &scenarios[0];
    assert_eq!(read.id, "step", "a name is read trimmed");
    assert_eq!(read.initial_values.get("SP"), Some(&2.5));
    assert_eq!(read.events[0].at_ms, 1500.0);
    assert_eq!(read.events[0].symbol, "SP");
    assert_eq!(read.events[0].value, 3.0);
    assert!(matches!(
        read.requirements[0].check,
        RunCheck::Bounds { minimum, maximum } if minimum == 0.0 && maximum == 10.0
    ));
}

#[test]
fn the_object_form_the_study_keeps_is_read_as_it_is() {
    let list = json!([scenario(
        "kept",
        &json!({ "SP": 2.0 }),
        &json!([{ "atMs": 0, "symbol": "SP", "value": 1.0 }]),
    )]);
    let scenarios = scenarios_of_form(&list).expect("the list is read");
    assert_eq!(scenarios[0].initial_values.get("SP"), Some(&2.0));
    assert_eq!(scenarios[0].events[0].at_ms, 0.0);
}

#[test]
fn an_empty_list_declares_that_there_are_no_scenarios() {
    let scenarios = scenarios_of_form(&json!([])).expect("an empty list is read");
    assert_eq!(scenarios, [] as [EngineerScenario; 0]);
}

#[test]
fn a_list_that_is_not_a_list_is_refused_under_the_scenarios_field() {
    assert_eq!(
        refusal(&json!({ "id": "a" })),
        (
            "The verification scenarios are not a list.".to_owned(),
            Some("scenarios".to_owned())
        )
    );
}

#[test]
fn a_repeated_name_is_refused() {
    let repeated = json!([
        scenario("a", &json!([]), &json!([])),
        scenario(" a ", &json!([]), &json!([])),
    ]);
    assert_eq!(
        refusal(&repeated),
        (
            "Scenario names must be unique.".to_owned(),
            Some("scenarios".to_owned())
        )
    );
}

#[test]
fn a_scenario_without_a_name_takes_the_first_identity_no_other_scenario_holds() {
    // B4.3: the page adds a scenario with no name; a name left blank (or only spaces, or missing) is given by the engine, in the list's
    // order, and never one that another scenario of the list holds, wherever that scenario stands.
    let mut missing = scenario("", &json!([]), &json!([]));
    missing.as_object_mut().expect("an object").remove("id");
    let list = json!([
        scenario("", &json!([]), &json!([])),
        scenario(" scenario-1 ", &json!([]), &json!([])),
        scenario("   ", &json!([]), &json!([])),
        missing,
        scenario("scenario-3", &json!([]), &json!([])),
    ]);
    let names: Vec<String> = scenarios_of_form(&list)
        .expect("the list is read")
        .into_iter()
        .map(|read| read.id)
        .collect();
    assert_eq!(
        names,
        [
            "scenario-2",
            "scenario-1",
            "scenario-4",
            "scenario-5",
            "scenario-3"
        ]
    );
}

#[test]
fn a_signal_given_two_initial_values_is_refused() {
    let list = json!([scenario(
        "a",
        &json!([{ "symbol": "SP", "value": "1" }, { "symbol": " SP", "value": "2" }]),
        &json!([]),
    )]);
    assert_eq!(refusal(&list).0, "Signal SP already has an initial value.");
}

#[test]
fn a_number_that_no_finite_number_reads_is_refused() {
    for text in ["abc", "", "NaN", "inf"] {
        let list = json!([scenario(
            "a",
            &json!([{ "symbol": "SP", "value": text }]),
            &json!([])
        )]);
        assert_eq!(
            refusal(&list).0,
            "Enter a finite number.",
            "{text:?} is not a finite number"
        );
    }
    let timed = json!([scenario(
        "a",
        &json!([]),
        &json!([{ "atMs": "soon", "symbol": "SP", "value": "1" }])
    )]);
    assert_eq!(refusal(&timed).0, "Enter a finite number.");
}

#[test]
fn a_blank_signal_name_is_refused_in_an_initial_value_and_in_a_timed_change() {
    let initial = json!([scenario(
        "a",
        &json!([{ "symbol": " ", "value": "1" }]),
        &json!([])
    )]);
    assert_eq!(refusal(&initial).0, "Enter a signal name.");
    let timed = json!([scenario(
        "a",
        &json!([]),
        &json!([{ "atMs": "0", "symbol": "", "value": "1" }])
    )]);
    assert_eq!(refusal(&timed).0, "Enter a signal name.");
}

#[test]
fn a_timed_change_before_its_run_starts_is_refused() {
    let list = json!([scenario(
        "a",
        &json!([]),
        &json!([{ "atMs": "-1", "symbol": "SP", "value": "1" }])
    )]);
    assert_eq!(
        refusal(&list).0,
        "A scenario event starts at zero or later."
    );
}

/// The advanced text of a scenario: the JSON object of the fields the form does not edit, here a requirement and no effects.
const ADVANCED: &str = r#"{
    "requirements": [{ "id": "r2", "channel": "PV", "window": { "start": 0.0, "end": 2.0 },
        "kind": "bounds", "minimum": "1", "maximum": 5 }],
    "effects": []
}"#;

/// The sentence of advanced fields that are not a JSON object.
const NOT_AN_OBJECT: &str = "Advanced fields must be a valid JSON object.";

#[test]
fn the_advanced_text_replaces_the_scenarios_other_fields_and_leaves_its_rows_as_they_are() {
    let mut typed = scenario(
        "typed",
        &json!([{ "symbol": "SP", "value": "2" }]),
        &json!([{ "atMs": "100", "symbol": "SP", "value": "3" }]),
    );
    typed["advanced"] = json!(ADVANCED);
    let scenarios = scenarios_of_form(&json!([typed])).expect("the list is read");
    let read: &EngineerScenario = &scenarios[0];
    assert_eq!(read.requirements.len(), 1);
    assert_eq!(
        read.requirements[0].id, "r2",
        "the typed requirements replace the ones the form held"
    );
    assert!(matches!(
        read.requirements[0].check,
        RunCheck::Bounds { minimum, maximum } if minimum == 1.0 && maximum == 5.0
    ));
    assert_eq!(read.effects, Some(Vec::new()));
    assert_eq!(read.id, "typed");
    assert_eq!(read.initial_values.get("SP"), Some(&2.0));
    assert_eq!(read.events[0].at_ms, 100.0);
}

#[test]
fn advanced_text_that_is_not_a_json_object_is_refused_and_no_text_leaves_the_scenario_as_it_is() {
    for text in ["{", "[]", "3", "\"x\"", ""] {
        let mut typed = scenario("a", &json!([]), &json!([]));
        typed["advanced"] = json!(text);
        assert_eq!(
            refusal(&json!([typed])),
            (NOT_AN_OBJECT.to_owned(), Some("scenarios".to_owned())),
            "{text:?} is no JSON object"
        );
    }
    let mut number = scenario("a", &json!([]), &json!([]));
    number["advanced"] = json!(3);
    assert_eq!(refusal(&json!([number])).0, NOT_AN_OBJECT);
    let mut none = scenario("a", &json!([]), &json!([]));
    none["advanced"] = Value::Null;
    assert!(
        scenarios_of_form(&json!([none])).is_ok(),
        "a null is no typed text"
    );
}

#[test]
fn advanced_text_that_holds_the_name_the_initial_values_or_the_timed_changes_is_refused() {
    for key in ["id", "initialValues", "events"] {
        let mut typed = scenario("a", &json!([]), &json!([]));
        typed["advanced"] = json!(format!("{{ \"{key}\": [] }}"));
        assert_eq!(
            refusal(&json!([typed])).0,
            "Advanced fields hold the scenario's other fields; its name, initial values and timed changes are edited above them.",
            "{key} is edited above the advanced fields"
        );
    }
}
