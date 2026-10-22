//! Tests of the number entries of an analysis request (`entries.rs`): text is read as the number it spells, a blank entry is absent, a
//! frequency grid is read whole, and an entry the engine cannot read is refused under its page field.

use serde_json::{json, Value};

use super::{read_entries, read_page_entries};
use crate::json::decode_value;
use crate::messages::AnalyzeRequest;

/// The request of `inputs`, as the page sends it for a frequency tool.
fn request(inputs: &Value) -> Value {
    json!({
        "projectRoot": "/project", "runtimeId": "runtime:local", "loopId": "loop:tank",
        "tool": "frequency", "inputs": inputs
    })
}

/// The field and the sentence of the refusal that `inputs` gets.
fn refusal(inputs: &Value) -> (String, String) {
    let failure = read_entries(&mut request(inputs)).expect_err("the entry is refused");
    (failure.field.unwrap_or_default(), failure.message)
}

#[test]
fn text_is_read_as_the_number_it_spells() {
    let inputs = json!({ "selectedValue": " 0.5 ", "segmentCount": "4", "frequencies": "1, 2.5" });
    let mut value = request(&inputs);
    read_entries(&mut value).expect("the entries are readable");
    assert_eq!(value["inputs"]["selectedValue"], json!(0.5));
    assert_eq!(value["inputs"]["segmentCount"], json!(4));
    assert_eq!(value["inputs"]["frequencies"], json!([1.0, 2.5]));
}

#[test]
fn a_number_already_in_the_request_is_kept() {
    let inputs = json!({ "selectedValue": 2.0, "minimumCoherence": 0.8 });
    let mut value = request(&inputs);
    read_entries(&mut value).expect("the numbers are readable");
    assert_eq!(value["inputs"]["selectedValue"], json!(2.0));
    assert_eq!(value["inputs"]["minimumCoherence"], json!(0.8));
}

#[test]
fn a_blank_entry_is_absent_so_that_the_engine_default_applies() {
    let inputs = json!({ "selectedValue": "  ", "frequencies": "" });
    let mut value = request(&inputs);
    read_entries(&mut value).expect("blank entries are readable");
    assert!(value["inputs"].get("selectedValue").is_none(), "{value}");
    assert!(value["inputs"].get("frequencies").is_none(), "{value}");
}

#[test]
fn an_entry_that_is_no_number_is_refused_under_its_field() {
    assert_eq!(
        refusal(&json!({ "crossoverRadPerSecond": "fast" })),
        (
            "lead-lag-crossover".to_string(),
            "Enter a number.".to_string()
        )
    );
    assert_eq!(
        refusal(&json!({ "selectedValue": "inf" })),
        ("parameter-value".to_string(), "Enter a number.".to_string())
    );
}

#[test]
fn a_count_must_be_a_whole_number() {
    assert_eq!(
        refusal(&json!({ "startupSamples": "2.5" })),
        (
            "periodic-startup".to_string(),
            "Enter a whole number.".to_string()
        )
    );
}

#[test]
fn a_grid_is_read_whole_and_refuses_a_blank_or_unreadable_entry_under_its_field() {
    assert_eq!(
        refusal(
            &json!({ "grid": { "minimumFrequency": "", "maximumFrequency": "10", "points": "64" } })
        ),
        (
            "minimum-frequency".to_string(),
            "Enter a number.".to_string()
        )
    );
    assert_eq!(
        refusal(
            &json!({ "grid": { "minimumFrequency": "0.1", "maximumFrequency": "10", "points": "64.5" } })
        ),
        (
            "frequency-points".to_string(),
            "Enter a whole number.".to_string()
        )
    );
}

#[test]
fn a_grid_whose_entries_are_all_blank_is_absent_and_a_grid_of_numbers_is_kept() {
    let mut blank = request(
        &json!({ "grid": { "minimumFrequency": "", "maximumFrequency": " ", "points": "" } }),
    );
    read_entries(&mut blank).expect("an absent grid is readable");
    assert!(blank["inputs"].get("grid").is_none(), "{blank}");
    let mut numbers = request(
        &json!({ "grid": { "minimumFrequency": 0.1, "maximumFrequency": 10.0, "points": 64 } }),
    );
    read_entries(&mut numbers).expect("a grid of numbers is readable");
    assert_eq!(numbers["inputs"]["grid"]["points"], json!(64));
}

#[test]
fn the_value_of_a_lead_lag_design_is_named_by_its_mode() {
    assert_eq!(
        refusal(&json!({ "mode": "lag", "value": "x" })),
        ("lead-lag-ratio".to_string(), "Enter a number.".to_string())
    );
    assert_eq!(
        refusal(&json!({ "mode": "lead", "value": "x" })),
        ("lead-lag-phase".to_string(), "Enter a number.".to_string())
    );
}

#[test]
fn a_list_with_an_unreadable_or_empty_item_is_refused_under_its_field() {
    assert_eq!(
        refusal(&json!({ "initialState": "1, x" })),
        (
            "response-initial".to_string(),
            "Enter numbers separated by commas.".to_string()
        )
    );
    assert_eq!(
        refusal(&json!({ "input": "1,,2" })),
        (
            "response-input".to_string(),
            "Enter numbers separated by commas.".to_string()
        )
    );
}

#[test]
fn the_page_records_the_entries_it_can_read_and_refuses_the_rest_under_their_field() {
    // The page's record (`controlStudio/page`) carries the inputs of its analysis: they are read as a request's are.
    let mut page = json!({
        "projectRoot": "/project", "runtimeId": "runtime:local", "loopId": "loop:tank",
        "analysis": { "tool": "measuredFrequency", "inputs": { "frequencies": "1, 2", "segmentCount": "4", "startupSamples": "" } }
    });
    read_page_entries(&mut page).expect("the entries are readable");
    assert_eq!(page["analysis"]["inputs"]["frequencies"], json!([1.0, 2.0]));
    assert_eq!(page["analysis"]["inputs"]["segmentCount"], json!(4));
    assert!(
        page["analysis"]["inputs"].get("startupSamples").is_none(),
        "{page}"
    );
    let mut refused =
        json!({ "analysis": { "tool": "measuredFrequency", "inputs": { "segmentCount": "two" } } });
    let failure = read_page_entries(&mut refused).expect_err("the count is refused");
    assert_eq!(failure.field.as_deref(), Some("periodic-segments"));
    assert_eq!(failure.message, "Enter a whole number.");
}

#[test]
fn the_typed_request_decodes_the_entries_that_were_read() {
    let inputs = json!({
        "grid": { "minimumFrequency": "0.1", "maximumFrequency": "10", "points": "64" },
        "mode": "lag", "value": "5"
    });
    let mut value = request(&inputs);
    read_entries(&mut value).expect("the entries are readable");
    let decoded: AnalyzeRequest = decode_value(value).expect("the typed request decodes");
    assert_eq!(decoded.inputs.grid.map(|grid| grid.points), Some(64));
    assert!(decoded
        .inputs
        .value
        .is_some_and(|value| (value - 5.0).abs() < 1e-12));
}
