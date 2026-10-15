#![allow(clippy::float_cmp)] // The values are the decimals that the record holds and the mapping keeps, compared as written.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use trust_control::analysis::parameter_writes::{writable_parameters, WritableParameter};
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::ParameterWorkRequest;

use super::*;

/// Lesson 45 (root locus): Kp is swept and selected at 16, Kd is fixed at 7, and both are mapped to program symbols.
const LESSON_45: &str = include_str!(
    "../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"
);

fn shaft() -> LoopDefinition {
    LoopDefinition::decode(LESSON_45).expect("the shipped lesson decodes")
}

fn binding(loop_definition: &LoopDefinition) -> RuntimeBinding {
    binding_of(loop_definition).expect("lesson 45 declares its binding")
}

fn writable(loop_definition: &LoopDefinition) -> Vec<WritableParameter> {
    writable_parameters(loop_definition).expect("the mapping of lesson 45 is writable")
}

/// The record of the runtime: the two mapped parameters and one unmapped parameter of the binding.
fn record(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.Kp".to_string(), kp),
        ("P1.Kd".to_string(), kd),
        ("P1.Other".to_string(), 5.0),
    ])
}

/// The binding of lesson 45 has no unmapped parameter, so the record of the test holds the same symbols.
fn record_of_binding(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([("P1.Kp".to_string(), kp), ("P1.Kd".to_string(), kd)])
}

fn status(values: BTreeMap<String, f64>, state: RecordState) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: None,
        program_revision: "program-1".to_string(),
        binding_revision: "binding-1".to_string(),
        active_revision: "active-2".to_string(),
        values,
        status: state,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

fn proposed(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([("P1.Kp".to_string(), kp), ("P1.Kd".to_string(), kd)])
}

#[test]
fn the_review_of_an_apply_names_the_whole_record_it_would_set() {
    let loop_definition = shaft();
    let binding = binding(&loop_definition);
    let writable = writable(&loop_definition);
    let observed = status(record_of_binding(1.0, 7.0), RecordState::Idle);
    let planned = plan_apply(
        &loop_definition,
        &writable,
        &binding,
        observed,
        &proposed(4.0, 7.0),
        "transaction-test".to_string(),
    )
    .expect("4 is inside the analysis range of Kp");
    assert_eq!(planned.review.values["P1.Kp"], 4.0);
    assert_eq!(planned.review.values["P1.Kd"], 7.0);
    assert_eq!(planned.review.transaction_id, "transaction-test");
    assert!(planned.identity.starts_with("sha256:"));
    let kp = planned
        .changes
        .iter()
        .find(|change| change.symbol == "P1.Kp")
        .expect("Kp changes");
    assert_eq!((kp.running, kp.next), (1.0, 4.0));
}

#[test]
fn a_value_outside_the_analysis_range_is_refused_before_the_review_is_made() {
    let loop_definition = shaft();
    let error = plan_apply(
        &loop_definition,
        &writable(&loop_definition),
        &binding(&loop_definition),
        status(record_of_binding(1.0, 7.0), RecordState::Idle),
        &proposed(21.0, 7.0),
        String::new(),
    )
    .err()
    .expect("21 is above the analysis maximum");
    assert!(
        error.message.contains("is outside the range"),
        "{}",
        error.message
    );
}

#[test]
fn a_pending_record_refuses_the_review() {
    let loop_definition = shaft();
    let error = plan_apply(
        &loop_definition,
        &writable(&loop_definition),
        &binding(&loop_definition),
        status(record_of_binding(1.0, 7.0), RecordState::Pending),
        &proposed(4.0, 7.0),
        String::new(),
    )
    .err()
    .expect("a pending record is not reviewed");
    assert!(error.message.contains("pending"), "{}", error.message);
}

#[test]
fn a_review_identity_that_no_longer_matches_refuses_the_write() {
    assert!(confirm_identity("sha256:now", "sha256:now").is_ok());
    let error = confirm_identity("sha256:now", "sha256:when-reviewed")
        .expect_err("the record changed since the review");
    assert_eq!(
        error.detail.as_deref().unwrap_or(error.message.as_str()),
        REVIEW_CHANGED
    );
    assert_eq!(error.cause, Cause::Approval);
    assert_eq!(error.next_action, NextAction::None);
}

#[test]
fn a_restore_reviews_the_values_retained_before_the_last_write() {
    let loop_definition = shaft();
    let writable = writable(&loop_definition);
    let mut retained = status(record_of_binding(4.0, 7.0), RecordState::Applied);
    retained.previous_revision = Some("active-1".to_string());
    retained.previous_values = Some(record_of_binding(1.0, 7.0));
    let planned = plan_restore(
        &loop_definition,
        &writable,
        retained,
        "transaction-restore".to_string(),
    )
    .expect("a confirmed previous record is retained");
    assert_eq!(planned.review.values, record_of_binding(1.0, 7.0));
    assert!(planned.review.is_rollback());
    let kp = planned
        .changes
        .iter()
        .find(|change| change.symbol == "P1.Kp")
        .expect("Kp changes");
    assert_eq!((kp.running, kp.next), (4.0, 1.0));
}

#[test]
fn a_restore_is_refused_when_no_previous_record_is_retained() {
    let loop_definition = shaft();
    let error = plan_restore(
        &loop_definition,
        &writable(&loop_definition),
        status(record_of_binding(4.0, 7.0), RecordState::Applied),
        String::new(),
    )
    .err()
    .expect("nothing is retained to restore");
    assert_eq!(error.cause, Cause::Runtime);
    assert_eq!(error.next_action, NextAction::None);
}

#[test]
fn an_apply_and_a_restore_of_the_same_record_have_different_identities() {
    let loop_definition = shaft();
    let writable = writable(&loop_definition);
    let binding = binding(&loop_definition);
    let mut retained = status(record_of_binding(4.0, 7.0), RecordState::Applied);
    retained.previous_revision = Some("active-1".to_string());
    retained.previous_values = Some(record_of_binding(1.0, 7.0));
    let restore = plan_restore(&loop_definition, &writable, retained.clone(), String::new())
        .expect("restore");
    let apply = plan_apply(
        &loop_definition,
        &writable,
        &binding,
        retained,
        &proposed(1.0, 7.0),
        String::new(),
    )
    .expect("apply");
    assert_ne!(restore.identity, apply.identity);
}

#[test]
fn the_mapped_values_are_the_writable_symbols_of_the_record() {
    let loop_definition = shaft();
    let values = mapped_values(&writable(&loop_definition), &record(4.0, 7.0));
    assert_eq!(
        values,
        BTreeMap::from([("P1.Kd".to_string(), 7.0), ("P1.Kp".to_string(), 4.0)]),
        "the unmapped P1.Other is not offered for a review"
    );
}

#[test]
fn the_review_answer_names_the_identity_the_changes_and_the_target_only() {
    let loop_definition = shaft();
    let planned = plan_apply(
        &loop_definition,
        &writable(&loop_definition),
        &binding(&loop_definition),
        status(record_of_binding(1.0, 7.0), RecordState::Idle),
        &proposed(4.0, 7.0),
        String::new(),
    )
    .expect("the review");
    let answer = review_answer(planned, ApplySource::Simulation).expect("the answer");
    assert_eq!(answer["physical"], json!(false));
    assert!(answer["identity"]
        .as_str()
        .is_some_and(|identity| identity.starts_with("sha256:")));
    assert_eq!(answer["changes"].as_array().map(Vec::len), Some(2));
    assert!(
        answer.get("observed").is_none(),
        "a review writes and reads no record for the page"
    );
}

#[test]
fn the_request_names_its_actions_in_camel_case() {
    let request: ParameterWorkRequest = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "control-root-locus",
        "storageFolder": "",
        "action": "reviewRestore",
        "physicalApproved": false
    }))
    .expect("the request decodes");
    assert_eq!(request.action, ParameterWorkAction::ReviewRestore);
    assert!(request.values.is_empty());
    assert!(request.review.is_none());
    let unknown: Result<ParameterWorkRequest, _> = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "control-root-locus",
        "storageFolder": "",
        "action": "reviewRestore",
        "unknown": true
    }));
    assert!(unknown.is_err(), "an unknown field is refused");
    let value: Value = serde_json::to_value(ParameterWorkAction::Apply).expect("the action");
    assert_eq!(value, json!("apply"));
}

#[test]
fn the_engine_answers_the_parameters_method() {
    assert!(
        crate::commands::METHODS.contains(&crate::commands::PARAMETERS),
        "the engine answers {}",
        crate::commands::PARAMETERS
    );
}
