//! Tests of the session rule (`parameterSession.ts`): the one review it retains, its staleness, the applying guard, the review text
//! limit and the session's actions. The first test ports `control-studio-parameter-session.test.ts`.

use serde_json::json;

use super::*;
use crate::commissioning::parameter_application::{
    check_status, review_of, RuntimeMode, RuntimeParameter,
};
use crate::commissioning::parameter_status::{ParameterStatus, RecordState};
use crate::loop_definition::RealType;

fn session_binding() -> RuntimeBinding {
    RuntimeBinding {
        loop_id: "loop".to_owned(),
        task_name: "Task".to_owned(),
        parameters: vec![RuntimeParameter {
            symbol: "P1.Gain".to_owned(),
            iec_type: RealType::Real,
            unit: "1".to_owned(),
            convention: "gain".to_owned(),
            minimum: 0.0,
            maximum: 20.0,
        }],
        mode: RuntimeMode {
            symbol: "P1.Auto".to_owned(),
            manual_value: false,
        },
    }
}

fn scope() -> MessageScope {
    MessageScope {
        project_id: "project".to_owned(),
        editor_session_id: "editor".to_owned(),
        loop_id: "loop".to_owned(),
        loop_revision: "revision".to_owned(),
    }
}

/// A session message of `action` for the scope's loop revision.
fn message(action: &str, payload: Value) -> Value {
    message_for(action, "revision", payload)
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn message_for(action: &str, revision: &str, payload: Value) -> Value {
    json!({
        "schemaVersion": 1,
        "kind": format!("control-studio.parameters.{action}"),
        "messageId": "message",
        "projectId": "project",
        "editorSessionId": "editor",
        "loopId": "loop",
        "expectedLoopRevision": revision,
        "payload": payload,
    })
}

/// A record of the runtime with the given active revision and values, and no transaction.
fn record(active: &str, values: &[(&str, f64)], status: RecordState) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: None,
        program_revision: "program".to_owned(),
        binding_revision: "binding".to_owned(),
        active_revision: active.to_owned(),
        values: values
            .iter()
            .map(|(symbol, value)| ((*symbol).to_owned(), *value))
            .collect(),
        status,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

/// A review of `P1.Gain = 0.5` made against an idle record, retained under `id` on the simulated endpoint.
fn retained(id: &str) -> RetainedReview {
    let observed = check_status(
        &session_binding(),
        record("active", &[("P1.Gain", 1.0)], RecordState::Idle),
    )
    .expect("an idle record is well formed");
    let review = review_of(
        BTreeMap::from([("P1.Gain".to_owned(), 0.5)]),
        observed,
        id.to_owned(),
    )
    .expect("an idle record is reviewed");
    RetainedReview {
        id: id.to_owned(),
        source: ParameterSource::Simulation,
        review,
    }
}

#[test]
fn a_session_keeps_one_review_for_its_endpoint_and_drops_it_after_an_editor_change() {
    // Ports "retains the reviewed endpoint and invalidates review after an editor change".
    let binding = session_binding();
    let scope = scope();
    let mut session = ParameterSession::new(binding.clone());
    let review_message = message("review", json!({ "json": r#"{"P1.Gain":0.1}"# }));
    assert_eq!(
        session_action(&review_message, &scope),
        Some(SessionAction::Review)
    );
    let generation = session.begin_request();
    assert!(matches!(
        session.admit(SessionAction::Review, None),
        Ok(SessionStep::Proceed(SessionAction::Review))
    ));
    let values = session
        .review_values(Some(r#"{"P1.Gain":0.1}"#))
        .expect("the review values are within the binding");
    let observed = check_status(
        &binding,
        record("active", &[("P1.Gain", 1.0)], RecordState::Idle),
    )
    .expect("the record is well formed");
    let review =
        review_of(values, observed, "review-1".to_owned()).expect("the record is reviewed");
    assert_eq!(
        review.values,
        BTreeMap::from([("P1.Gain".to_owned(), f64::from(0.1_f32))]),
        "the session must retain an exact parameter review"
    );
    let retained = RetainedReview {
        id: "review-1".to_owned(),
        source: ParameterSource::Simulation,
        review,
    };
    assert!(
        session.retain_if_current(generation, retained),
        "the reviewed endpoint is retained"
    );

    let invalidate =
        session_action(&message("invalidate", json!({})), &scope).expect("an invalidation");
    assert!(matches!(
        session.admit(invalidate, None),
        Ok(SessionStep::Edited)
    ));
    let apply = session_action(
        &message(
            "apply",
            json!({ "reviewId": "review-1", "physicalApproved": false }),
        ),
        &scope,
    )
    .expect("an apply");
    let refusal = session
        .admit(apply, Some("review-1"))
        .expect_err("invalidated reviews cannot write");
    assert_eq!(refusal.message, "This review expired.");

    let other_revision = message_for("inspect", "another", json!({}));
    assert_eq!(
        session_action(&other_revision, &scope),
        None,
        "a message for another loop revision is not answered"
    );
}

#[test]
fn a_session_discards_the_reply_of_an_older_request() {
    let mut session = ParameterSession::new(session_binding());
    let stale = session.begin_request();
    let newest = session.begin_request();
    assert!(
        !session.is_current(stale),
        "an older request is no longer current"
    );
    assert!(session.is_current(newest));
    assert!(
        !session.retain_if_current(stale, retained("late")),
        "a review of an older request is not retained"
    );
    assert!(session.admit(SessionAction::Apply, Some("late")).is_err());
    session.dispose();
    assert!(
        !session.is_current(newest),
        "a disposed session answers nothing"
    );
}

#[test]
fn a_session_holds_one_review_and_an_apply_in_progress_refuses_other_requests() {
    let mut session = ParameterSession::new(session_binding());
    let generation = session.begin_request();
    assert!(session.retain_if_current(generation, retained("review-1")));
    let wrong = session
        .admit(SessionAction::Apply, Some("other"))
        .expect_err("a review is named by its own identifier");
    assert_eq!(wrong.message, "This review expired.");
    assert!(
        session
            .admit(SessionAction::Apply, Some("review-1"))
            .is_err(),
        "a wrong identifier spends the retained review"
    );

    let generation = session.begin_request();
    assert!(session.retain_if_current(generation, retained("review-2")));
    assert!(matches!(
        session.admit(SessionAction::Apply, Some("review-2")),
        Ok(SessionStep::Apply(_))
    ));
    assert!(session.is_applying());
    let busy = session
        .admit(SessionAction::Inspect, None)
        .expect_err("a transaction awaits its confirmation");
    assert_eq!(
        busy.message,
        "A parameter transaction is awaiting confirmation."
    );
    session.finish_apply();
    assert!(!session.is_applying());
}

#[test]
fn a_review_text_is_limited_to_64_kilobytes_in_utf16_units() {
    let session = ParameterSession::new(session_binding());
    let base = r#"{"P1.Gain":0.1}"#;
    let exact = format!("{base}{}", " ".repeat(MAXIMUM_REVIEW_UNITS - base.len()));
    assert!(
        session.review_values(Some(exact.as_str())).is_ok(),
        "a text of exactly 64 kB is read"
    );
    let too_long = format!("{exact} ");
    let refusal = session
        .review_values(Some(too_long.as_str()))
        .expect_err("a longer text is refused");
    assert_eq!(
        refusal.message,
        "Enter a parameter JSON object of at most 64 kB."
    );
    assert_eq!(
        session
            .review_values(None)
            .expect_err("a missing text")
            .message,
        "Enter a parameter JSON object of at most 64 kB."
    );
    let not_json = session
        .review_values(Some("{"))
        .expect_err("a text that is not JSON");
    assert_eq!(not_json.message, "The parameter values are not valid JSON.");
}

#[test]
fn a_session_action_names_one_of_the_six_actions_of_its_own_scope() {
    let scope = scope();
    assert_eq!(
        session_action(&message("selected-analysis", json!({})), &scope),
        Some(SessionAction::SelectedAnalysis)
    );
    assert_eq!(
        session_action(&message("review-rollback", json!({})), &scope),
        Some(SessionAction::ReviewRollback)
    );
    assert_eq!(session_action(&message("unknown", json!({})), &scope), None);
    let mut other_editor = message("inspect", json!({}));
    other_editor["editorSessionId"] = json!("another");
    assert_eq!(session_action(&other_editor, &scope), None);
    assert_eq!(
        session_action(&json!([1, 2, 3]), &scope),
        None,
        "a message is an object"
    );
}

#[test]
fn a_selected_analysis_and_the_applied_reasons_name_their_mapping_and_outcome() {
    let missing = selected_analysis_missing();
    assert_eq!(
        missing.message,
        "Calculate a selected model candidate with an explicit ST parameter mapping first."
    );
    assert_eq!(missing.next_action, NextAction::Tune);
    assert_eq!(
        applied_reason(true),
        "Previous values restored and observed after ST execution."
    );
    assert_eq!(
        applied_reason(false),
        "Reviewed values observed after ST execution."
    );
}
