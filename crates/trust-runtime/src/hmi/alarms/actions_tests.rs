//! Specification 40, §4 and §5.1: each operator action's request field, value, role and
//! confirmation.

use serde_json::json;

use super::actions::alarm_action;
use super::board::EventKind;
use crate::hmi::commands::write::WriteAction;
use crate::security::AccessRole;

#[test]
fn acknowledge_writes_its_request_for_an_operator_and_is_confirmed_by_unacknowledged_false() {
    let action = alarm_action("acknowledge", None).expect("an action");
    assert_eq!(action.kind, EventKind::Acknowledged);
    assert_eq!(action.field, "Acknowledge");
    assert_eq!(action.write, WriteAction::Set(json!(true)));
    assert_eq!(action.role, AccessRole::Operator);
    assert_eq!(action.confirm, ("Unacknowledged", json!(false)));
    assert_eq!(action.detail, None);
}

#[test]
fn shelve_takes_1_to_86400_seconds_and_records_them() {
    let action = alarm_action("shelve", Some(&json!(600))).expect("an action");
    assert_eq!((action.kind, action.field), (EventKind::Shelved, "Shelve"));
    assert_eq!(action.write, WriteAction::Set(json!(600.0)));
    assert_eq!(action.confirm, ("Shelved", json!(true)));
    assert_eq!(
        action.detail.as_deref(),
        Some("10 min"),
        "durations read as people say them"
    );
    for (seconds, said) in [
        (90, "90 s"),
        (3_600, "1 h"),
        (5_400, "90 min"),
        (86_400, "24 h"),
    ] {
        let action = alarm_action("shelve", Some(&json!(seconds))).expect("an action");
        assert_eq!(action.detail.as_deref(), Some(said), "{seconds} s");
    }
    for wrong in [
        json!(0),
        json!(86_401),
        json!(-5),
        json!("long"),
        json!(null),
    ] {
        let error = alarm_action("shelve", Some(&wrong)).expect_err("refused");
        assert!(error.contains("1 to 86400 seconds"), "{wrong}: {error}");
    }
    assert!(alarm_action("shelve", None).is_err());
}

#[test]
fn unshelve_and_reset_are_confirmed_by_shelved_and_active_false() {
    let unshelve = alarm_action("unshelve", None).expect("an action");
    assert_eq!(
        (unshelve.field, unshelve.confirm),
        ("Unshelve", ("Shelved", json!(false)))
    );
    let reset = alarm_action("reset", None).expect("an action");
    assert_eq!((reset.kind, reset.field), (EventKind::Reset, "Reset"));
    assert_eq!(reset.confirm, ("Active", json!(false)));
    assert_eq!(reset.role, AccessRole::Operator);
}

#[test]
fn a_comment_has_1_to_120_characters_and_is_confirmed_when_the_instance_took_it() {
    let action = alarm_action("comment", Some(&json!("Checked on site"))).expect("an action");
    assert_eq!((action.kind, action.field), (EventKind::Comment, "Comment"));
    assert_eq!(action.write, WriteAction::Set(json!("Checked on site")));
    assert_eq!(action.confirm, ("Request.Comment", json!("")));
    assert_eq!(action.detail.as_deref(), Some("Checked on site"));
    for wrong in [json!(""), json!("   "), json!("x".repeat(121)), json!(5)] {
        let error = alarm_action("comment", Some(&wrong)).expect_err("refused");
        assert!(error.contains("1 to 120 characters"), "{wrong}: {error}");
    }
}

#[test]
fn out_of_service_and_in_service_need_an_engineer() {
    let out = alarm_action("remove-from-service", None).expect("an action");
    assert_eq!(
        (out.kind, out.field),
        (EventKind::OutOfService, "RemoveFromService")
    );
    assert_eq!(out.role, AccessRole::Engineer);
    assert_eq!(out.confirm, ("OutOfService", json!(true)));
    let back = alarm_action("return-to-service", None).expect("an action");
    assert_eq!(
        (back.kind, back.field),
        (EventKind::InService, "ReturnToService")
    );
    assert_eq!(back.role, AccessRole::Engineer);
    assert_eq!(back.confirm, ("OutOfService", json!(false)));
}

#[test]
fn an_unknown_action_or_a_value_where_none_belongs_is_refused() {
    let error = alarm_action("silence", None).expect_err("refused");
    assert!(error.contains("acknowledge, shelve, unshelve, reset, comment, remove-from-service or return-to-service"), "{error}");
    let error = alarm_action("acknowledge", Some(&json!(true))).expect_err("refused");
    assert!(error.contains("takes no value"), "{error}");
}
