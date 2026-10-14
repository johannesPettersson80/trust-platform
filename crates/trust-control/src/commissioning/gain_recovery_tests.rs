#![allow(clippy::float_cmp)] // The gains are compared exactly: they are the values the campaign wrote.

use super::*;

use serde_json::{json, Value};

use super::super::parameter_status::RecordState;

/// The stored context of a campaign that moved `Main.Kp` from 1 to 36.5 (the workflow integration test's values).
fn stored_context() -> Value {
    json!({
        "origin": {
            "program_revision": "sha256:program",
            "binding_revision": "sha256:binding",
            "active_revision": "sha256:original",
            "values": { "Main.Kp": 1.0, "Main.Ki": 0.25 }
        },
        "candidate": { "Main.Kp": 36.5, "Main.Ki": 0.5 }
    })
}

/// A parameter record that the campaign left: the candidate applied over the original set, whose revision is the previous one.
fn record_after_apply(previous_values: Option<BTreeMap<String, f64>>) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("apply-1".to_owned()),
        program_revision: "sha256:program".to_owned(),
        binding_revision: "sha256:binding".to_owned(),
        active_revision: "sha256:candidate".to_owned(),
        values: BTreeMap::from([("Main.Kp".to_owned(), 36.5), ("Main.Ki".to_owned(), 0.5)]),
        status: RecordState::Applied,
        previous_revision: Some("sha256:original".to_owned()),
        previous_values,
        reason: None,
    }
}

fn original_values() -> BTreeMap<String, f64> {
    BTreeMap::from([("Main.Kp".to_owned(), 1.0), ("Main.Ki".to_owned(), 0.25)])
}

#[test]
fn a_stored_context_decodes_only_in_the_shape_the_campaign_writes() {
    let context =
        decode_gain_recovery_context(stored_context()).expect("the campaign's own context decodes");
    assert_eq!(context.origin.values["Main.Kp"], 1.0);
    assert_eq!(context.candidate["Main.Kp"], 36.5);
    assert_eq!(context.origin.active_revision, "sha256:original");
}

#[test]
fn a_gain_held_as_text_is_refused() {
    let mut corrupt = stored_context();
    corrupt["origin"]["values"]["Main.Kp"] = json!("1");
    assert!(decode_gain_recovery_context(corrupt).is_none());
}

#[test]
fn an_extra_field_of_the_origin_is_refused() {
    let mut corrupt = stored_context();
    corrupt["origin"]["extra"] = json!(true);
    assert!(decode_gain_recovery_context(corrupt).is_none());
}

#[test]
fn a_candidate_missing_a_parameter_of_the_origin_is_refused() {
    let mut corrupt = stored_context();
    corrupt["candidate"]
        .as_object_mut()
        .expect("an object")
        .remove("Main.Kp");
    assert!(decode_gain_recovery_context(corrupt).is_none());
}

#[test]
fn a_candidate_naming_a_parameter_the_origin_lacks_is_refused() {
    let mut corrupt = stored_context();
    corrupt["candidate"]["Main.Kd"] = json!(0.0);
    assert!(decode_gain_recovery_context(corrupt).is_none());
}

#[test]
fn a_revision_of_no_text_is_refused() {
    let mut corrupt = stored_context();
    corrupt["origin"]["program_revision"] = json!("");
    assert!(decode_gain_recovery_context(corrupt).is_none());
}

#[test]
fn the_running_record_matches_only_the_record_the_campaign_left() {
    let context = decode_gain_recovery_context(stored_context()).expect("decodes");
    let left = record_after_apply(Some(original_values()));
    assert!(matches_gain_recovery(&left, Some(&context)));
    assert!(
        !matches_gain_recovery(&left, None),
        "no context matches nothing"
    );

    let mut other_previous = record_after_apply(Some(original_values()));
    other_previous.previous_revision = Some("sha256:other".to_owned());
    assert!(
        !matches_gain_recovery(&other_previous, Some(&context)),
        "the previous revision must be the origin's"
    );

    let mut other_candidate = record_after_apply(Some(original_values()));
    other_candidate.values.insert("Main.Kp".to_owned(), 2.0);
    assert!(
        !matches_gain_recovery(&other_candidate, Some(&context)),
        "the values in force must be the candidate"
    );

    let no_previous = record_after_apply(None);
    assert!(
        !matches_gain_recovery(&no_previous, Some(&context)),
        "a record without its previous values does not match"
    );
}
