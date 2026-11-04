//! When Live mode's operating view ends, and why (specification 39, §11 item 6). Every expected
//! text is the specification's.

use serde_json::{json, Value};

use super::*;

const DEPLOYED: &str = "0123456789abcdef";

fn operation(event: Value) -> Value {
    live_operation_value(vec![event])
}

/// The verdict of a known event: whether it goes on, and the reason operation ends.
fn verdict(event: Value) -> (bool, Value) {
    let answer = operation(event);
    assert_eq!(answer["ok"], true, "{answer}");
    (
        answer["proceed"].as_bool().expect("proceed"),
        answer["end"].clone(),
    )
}

fn reply(method: &str, status: u16, body: Value) -> Value {
    json!({ "event": "reply", "method": method, "status": status, "body": body, "bundle": DEPLOYED })
}

// Catches a command sent from an HMI that is no longer the one deployed: unsaved changes end
// operation before the request goes out.
#[test]
fn unsaved_changes_end_operation_before_a_request() {
    assert_eq!(
        verdict(json!({ "event": "request", "dirty": true })),
        (
            false,
            json!("The HMI changed. Save and deploy it before operating again.")
        )
    );
    assert_eq!(
        verdict(json!({ "event": "request", "dirty": false })),
        (true, Value::Null)
    );
}

// Catches operation that goes on after an HMI file changed while operating: the editor no longer
// shows the deployed HMI, so operation ends at once with the same reason.
#[test]
fn an_edit_while_operating_ends_operation_at_once() {
    assert_eq!(
        verdict(json!({ "event": "edited" })),
        (
            false,
            json!("The HMI changed. Save and deploy it before operating again.")
        )
    );
}

// Catches a page from another deployment shown in the operating view: a page read answered from
// another bundle ends operation before it shows, also when the bundle it names is not text.
#[test]
fn a_page_read_from_another_bundle_ends_before_it_shows() {
    let ended = (
        false,
        json!("The deployed HMI changed. Check and deploy the matching project before operating again."),
    );
    for body in [
        json!({ "bundle": "fedcba9876543210", "page": { "name": "overview" } }),
        json!({ "bundle": null }),
    ] {
        assert_eq!(verdict(reply("GET", 200, body.clone())), ended, "{body}");
    }
}

// Catches operation ended by an answer that names no bundle, or by a command's answer, which never
// says which bundle serves the page: only a page read that names another bundle ends it.
#[test]
fn other_answers_are_shown_and_operation_goes_on() {
    for (method, body) in [
        (
            "GET",
            json!({ "bundle": DEPLOYED, "page": { "name": "overview" } }),
        ),
        ("GET", json!({ "user": "operator" })),
        (
            "POST",
            json!({ "bundle": "fedcba9876543210", "outcome": "applied" }),
        ),
    ] {
        assert_eq!(
            verdict(reply(method, 200, body.clone())),
            (true, Value::Null),
            "{method} {body}"
        );
    }
}

// Catches a 409 hidden from the operating view, or one after which operation goes on: the named
// session's target changed or is gone, so the answer is shown and then operation ends.
#[test]
fn a_409_is_shown_and_then_ends_operation() {
    assert_eq!(
        verdict(reply("POST", 409, json!({ "error": "session target changed" }))),
        (
            true,
            json!("The run target changed or disconnected. Any unfinished command has an unknown outcome; none will be resent.")
        )
    );
}

// Catches a lost stream, runtime or run target that leaves the operating view running, and a
// reason that does not say that unfinished commands are not resent.
#[test]
fn each_lost_connection_ends_operation_with_its_reason() {
    for (event, reason) in [
        ("streamUnavailable", "The runtime stream is unavailable. Reopen Live mode to connect."),
        ("streamClosed", "The runtime disconnected. Any unfinished command has an unknown outcome; none will be resent."),
        ("unreachable", "The runtime cannot be reached. Any unfinished command has an unknown outcome; none will be resent."),
        ("targetChanged", "The selected run target changed. Reopen Live mode to connect."),
        ("targetUnreachable", "The selected run target cannot be reached. Reopen Live mode to connect."),
    ] {
        assert_eq!(
            verdict(json!({ "event": event })),
            (false, json!(reason)),
            "{event}"
        );
    }
}

// Catches an event the language server does not know taken as one it does, and an event with a
// field it does not take, the events without facts among them.
#[test]
fn an_unknown_event_or_field_is_refused() {
    for event in [
        json!({ "event": "pause" }),
        json!({ "event": "request", "dirty": false, "page": "overview" }),
        json!({ "event": "request" }),
        json!({ "event": "streamClosed", "dirty": true }),
        json!({ "event": "edited", "page": "overview" }),
    ] {
        let answer = operation(event.clone());
        assert_eq!(answer["ok"], false, "{event}: {answer}");
        assert!(answer["error"].is_string(), "{answer}");
    }
}
