//! Tests of the answers' kinds (specification 35, §0.1a): every answer of a method that answers more than one shape names its kind in a
//! `kind` field that it always carries, with the one word its type fixes, so the page tells the answers apart by the engine's word. The
//! expected words are the ones §0.1a lists for each answer.

use schemars::{schema_for, JsonSchema};
use serde_json::{json, Value};

use super::{AnswerKind, NamedAnswer};
use crate::commissioning::application_records::{ApplicationReview, ApplyOutcome, RestoreOutcome};
use crate::messages::{
    IdentifyResult, ImportedRecording, KeepResult, KeepReviewAnswer, ManualIdentificationAnswer,
    OperationStarted, ProjectTestResult, ResetResult, TuneResult,
};
use crate::tuning::search::SearchAnswer;

/// The `kind` field of the answer type `T`, as its JSON Schema declares it to the page: the field's schema and whether every answer
/// carries it.
fn declared_kind<T: JsonSchema>() -> (Value, bool) {
    let schema = serde_json::to_value(schema_for!(T)).expect("the schema is JSON");
    let field = schema["properties"]["kind"].clone();
    let required = schema["required"]
        .as_array()
        .is_some_and(|required| required.contains(&json!("kind")));
    (field, required)
}

/// Checks that every answer of the type `T` carries `kind` with the word `word` only: the field is required, its schema admits that
/// one word, and the field writes it.
fn assert_named<T: JsonSchema + NamedAnswer>(word: &str) {
    let (field, required) = declared_kind::<T>();
    assert_eq!(field["const"], json!(word), "{field}");
    assert_eq!(field["type"], json!("string"), "{field}");
    assert!(required, "every {word} answer carries its kind");
    assert_eq!(
        serde_json::to_value(AnswerKind::<T>::new()).expect("the kind is JSON"),
        json!(word)
    );
}

/// §0.1a: every answer of `controlStudio/identify` but the review, which only `review` answers, and the final result of its operation
/// name their kind: an identification, a project test that kept no model, the Recorded-data fit's state, a reset, a recording the study
/// keeps, or a started operation. The wrong behaviour caught: a page that takes any answer without `modelType` for a reset, which clears
/// the model and starts a new study on the screen (N12).
#[test]
fn every_identify_answer_names_its_kind() {
    assert_named::<IdentifyResult>("identification");
    assert_named::<ProjectTestResult>("projectTest");
    assert_named::<ManualIdentificationAnswer>("manualIdentification");
    assert_named::<ResetResult>("reset");
    assert_named::<OperationStarted>("operationStarted");
    // A recording the study keeps has no declaration of its own: the page reads it as the engine writes it.
    assert_eq!(ImportedRecording::KIND, "importedRecording");
    assert_eq!(
        serde_json::to_value(AnswerKind::<ImportedRecording>::new()).expect("the kind is JSON"),
        json!("importedRecording")
    );
}

/// §0.1a, B6: every answer of `controlStudio/apply`, `controlStudio/restore` and `controlStudio/keep`, and the final result of their
/// operations, names its kind: the review of an Apply or a Restore, the review of a Keep, and the end of each. The wrong behaviour
/// caught: a page that tells a review from an outcome by the fields it finds (`parameters`, `files`, `checkpointId`, `restored`, `kept`),
/// so that an answer with another field is shown as the wrong one (N12).
#[test]
fn every_application_answer_names_its_kind() {
    assert_named::<ApplicationReview>("applicationReview");
    assert_named::<KeepReviewAnswer>("keepReview");
    assert_named::<ApplyOutcome>("applyOutcome");
    assert_named::<RestoreOutcome>("restoreOutcome");
    assert_named::<KeepResult>("keepResult");
}

/// §0.1a, B4.5, C5: the final result of `controlStudio/tune` names its kind: a candidate, or a finite search's trials. The wrong
/// behaviour caught: a page that takes any answer with `trials` for a search (N12).
#[test]
fn every_tune_answer_names_its_kind() {
    assert_named::<TuneResult>("tuneResult");
    assert_named::<SearchAnswer>("searchAnswer");
}
