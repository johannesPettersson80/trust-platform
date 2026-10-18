//! Exact wire text of the control envelope as trust-runtime writes and reads it today, and the
//! same text written piece by piece within a bound (specification 34, PORT-05).

use alloc::string::{String, ToString};

use serde_json::json;
use smol_str::SmolStr;

use super::{BoundedReply, ControlAfterResponse, ControlRequest, ControlResponse};

fn wire(response: &ControlResponse) -> alloc::string::String {
    serde_json::to_string(response).expect("serialize control response")
}

/// The host decodes a request line to a JSON value first, then to the envelope.
fn decode(text: &str) -> Result<ControlRequest, serde_json::Error> {
    serde_json::from_value(serde_json::from_str::<serde_json::Value>(text)?)
}

#[test]
fn responses_serialize_to_the_exact_host_wire_text() {
    assert_eq!(
        wire(&ControlResponse::ok(7, json!({"status": "running"}))),
        r#"{"id":7,"ok":true,"result":{"status":"running"}}"#
    );
    assert_eq!(
        wire(&ControlResponse::error(6, "unsupported request".into())),
        r#"{"id":6,"ok":false,"error":"unsupported request"}"#
    );
    assert_eq!(
        wire(&ControlResponse::error_with_code(
            7702,
            "debug disabled".into(),
            "feature_disabled"
        )),
        r#"{"id":7702,"ok":false,"error":"debug disabled","error_code":"feature_disabled"}"#
    );
}

#[test]
fn audit_id_is_the_last_member_and_absent_without_a_record() {
    let refused = ControlResponse::error_with_code(
        3,
        "forbidden: requires role admin".into(),
        "insufficient_role",
    )
    .with_audit_id(Some(SmolStr::new("audit-1760000000000-1-0000002a")));
    assert_eq!(
        wire(&refused),
        r#"{"id":3,"ok":false,"error":"forbidden: requires role admin","error_code":"insufficient_role","audit_id":"audit-1760000000000-1-0000002a"}"#
    );
    let accepted = ControlResponse::ok(5, json!({"items": []}))
        .with_audit_id(Some(SmolStr::new("audit-1760000000000-2-00000000")));
    assert_eq!(
        wire(&accepted),
        r#"{"id":5,"ok":true,"result":{"items":[]},"audit_id":"audit-1760000000000-2-00000000"}"#
    );
    assert_eq!(
        wire(&ControlResponse::ok(8, json!(null)).with_audit_id(None)),
        r#"{"id":8,"ok":true,"result":null}"#
    );
}

#[test]
fn shutdown_replies_stopping_and_defers_the_stop_without_serializing_it() {
    let response = ControlResponse::shutdown(41);
    assert_eq!(
        response.after_response,
        Some(ControlAfterResponse::StopResource)
    );
    assert_eq!(
        wire(&response),
        r#"{"id":41,"ok":true,"result":{"status":"stopping"}}"#
    );
    for other in [
        ControlResponse::ok(1, json!({})),
        ControlResponse::error(1, "e".into()),
        ControlResponse::error_with_code(1, "e".into(), "code"),
    ] {
        assert_eq!(other.after_response, None);
    }
}

#[test]
fn request_accepts_correlation_id_as_alias_of_request_id() {
    let request = decode(
        r#"{"id":9,"type":"config.set","params":{"log.level":"debug"},"auth":"secret","correlation_id":"cfg-agent-4"}"#,
    )
    .expect("aliased request");
    assert_eq!(request.id, 9);
    assert_eq!(request.r#type, "config.set");
    assert_eq!(request.params, Some(json!({"log.level": "debug"})));
    assert_eq!(request.auth.as_deref(), Some("secret"));
    assert_eq!(request.request_id.as_deref(), Some("cfg-agent-4"));

    let request = decode(r#"{"id":10,"type":"status","request_id":"req-1"}"#).expect("request");
    assert_eq!(request.request_id.as_deref(), Some("req-1"));
}

#[test]
fn request_optional_members_default_and_unknown_members_are_ignored() {
    let request =
        decode(r#"{"id":1,"type":"status","auth":null,"future":true}"#).expect("minimal request");
    assert_eq!(request.id, 1);
    assert_eq!(request.r#type, "status");
    assert_eq!(request.params, None);
    assert_eq!(request.auth, None);
    assert_eq!(request.request_id, None);
}

#[test]
fn request_without_valid_id_or_type_or_with_non_text_auth_is_rejected() {
    for text in [
        "null",
        "[]",
        r#""request""#,
        "{}",
        r#"{"type":"status"}"#,
        r#"{"id":1}"#,
        r#"{"id":"one","type":"status"}"#,
        r#"{"id":-1,"type":"status"}"#,
        r#"{"id":1,"type":null}"#,
        r#"{"id":1,"type":"status","auth":7}"#,
    ] {
        assert!(decode(text).is_err(), "text={text}");
    }
}

#[test]
fn a_successful_response_written_around_its_result_is_its_serialized_text() {
    for id in [0, 7, u64::MAX] {
        for audit_id in [
            None,
            Some("audit-1760000000000-1-0000002a"),
            Some("a\"b\\c\nd\u{1}é"),
        ] {
            for result in [
                json!(null),
                json!({"events": []}),
                json!({"faults": [{"type": "fault", "error": "x\"y", "time_ns": -5}]}),
            ] {
                let response = ControlResponse::ok(id, result.clone())
                    .with_audit_id(audit_id.map(SmolStr::new));
                let mut text = String::new();
                ControlResponse::write_ok_start(&mut text, id).expect("a string takes any text");
                text.push_str(&result.to_string());
                response
                    .write_ok_end(&mut text)
                    .expect("a string takes any text");
                assert_eq!(text, wire(&response), "id={id} audit_id={audit_id:?}");
            }
        }
    }
}

#[test]
fn a_bounded_reply_is_whole_within_its_bound_and_nothing_beyond_it() {
    let served = ControlResponse::ok(3, json!(null)).with_audit_id(Some(SmolStr::new("audit-9")));
    let result = json!({"events": []});
    let expected =
        wire(&ControlResponse::ok(3, result.clone()).with_audit_id(Some(SmolStr::new("audit-9"))));
    for bound in [expected.len(), expected.len() + 1, usize::MAX] {
        let mut reply = BoundedReply::ok(3, bound);
        reply.push_json(&result);
        assert!(!reply.overflowed(), "bound={bound}");
        assert_eq!(
            reply.finish(&served).as_deref(),
            Some(expected.as_str()),
            "bound={bound}"
        );
    }
    let mut short = BoundedReply::ok(3, expected.len() - 1);
    short.push_json(&result);
    assert!(
        !short.overflowed(),
        "the result still fits; only the end does not"
    );
    assert_eq!(short.finish(&served), None, "one byte too long");
    let mut overflowed = BoundedReply::ok(3, 30);
    overflowed.push("{\"events\":[");
    overflowed.push(&"x".repeat(40));
    assert!(overflowed.overflowed());
    overflowed.push("]}");
    assert!(
        overflowed.overflowed(),
        "nothing is written after an overflow"
    );
    assert_eq!(overflowed.finish(&served), None);
}
