//! The duration entries of the analysis inputs (`analyze.rs`, `duration_of_entry`): a number of seconds, or the text the page typed.

use serde_json::{json, Value};

use super::AnalysisInputs;
use crate::json::decode_value;

fn inputs(entries: Value) -> AnalysisInputs {
    decode_value(entries).expect("the analysis inputs decode")
}

#[test]
fn a_typed_duration_reads_as_the_seconds_it_names() {
    let typed = inputs(json!({ "stepSeconds": "2 min", "durationSeconds": "1 h 30 min" }));
    let number = inputs(json!({ "stepSeconds": 120.0, "durationSeconds": 5400.0 }));
    assert_eq!(typed.step_seconds, Some(120.0));
    assert_eq!(typed.duration_seconds, Some(5400.0));
    assert_eq!(
        typed, number,
        "a typed time is read as its number of seconds"
    );
}

#[test]
fn a_blank_or_absent_duration_is_absent_so_the_default_applies() {
    assert_eq!(inputs(json!({ "stepSeconds": " " })).step_seconds, None);
    assert_eq!(
        inputs(json!({ "durationSeconds": null })).duration_seconds,
        None
    );
    assert_eq!(inputs(json!({})).duration_seconds, None);
}

#[test]
fn a_text_that_is_no_duration_reaches_the_checker_as_nan() {
    let read = inputs(json!({ "stepSeconds": "soon", "durationSeconds": "12 m" }));
    assert!(
        read.step_seconds.is_some_and(f64::is_nan),
        "`soon` is no duration"
    );
    assert!(
        read.duration_seconds.is_some_and(f64::is_nan),
        "`12 m` is no duration: m is not a unit"
    );
    let other = inputs(json!({ "stepSeconds": true }));
    assert!(
        other.step_seconds.is_some_and(f64::is_nan),
        "a value that is neither a number nor text"
    );
}

#[test]
fn an_answer_writes_a_duration_as_the_engines_words_and_reads_them_back_as_the_same_duration() {
    // B9.9: the page shows a duration as the engine writes it and sends back what the engineer typed. A step of 0.1 s and a duration of
    // 150 s are written `0.1 s` and `2 min 30 s` (150 s is two minutes and thirty seconds), and the engine reads those words as the same
    // seconds, so a page that sends them back unchanged changes nothing.
    let answered = AnalysisInputs {
        step_seconds: Some(0.1),
        duration_seconds: Some(150.0),
        ..AnalysisInputs::default()
    };
    let written = serde_json::to_value(&answered).expect("the inputs are written");
    assert_eq!(
        written,
        json!({ "stepSeconds": "0.1 s", "durationSeconds": "2 min 30 s" })
    );
    assert_eq!(inputs(written), answered);
}

#[test]
fn an_entry_that_is_no_duration_is_written_as_no_value_and_an_absent_one_is_left_out() {
    // A text that no duration reads reaches the checker as NaN, which an answer cannot write as words; it was written as null before the
    // words existed, and the page shows a blank field for it.
    let unreadable = AnalysisInputs {
        step_seconds: Some(f64::NAN),
        ..AnalysisInputs::default()
    };
    assert_eq!(
        serde_json::to_value(&unreadable).expect("the inputs are written"),
        json!({ "stepSeconds": null })
    );
    assert_eq!(
        serde_json::to_value(AnalysisInputs::default()).expect("the inputs are written"),
        json!({})
    );
}
