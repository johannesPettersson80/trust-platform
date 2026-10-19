//! Exact wire text of the control envelope as trust-runtime writes and reads it today, and the
//! same text written piece by piece into parts within a bound (specification 34, PORT-05).

use alloc::format;
use alloc::string::{String, ToString};

use serde_json::json;
use smol_str::SmolStr;

use super::reply_writer::line;
use super::{ControlAfterResponse, ControlRequest, ControlResponse, ReplyWriter, Written};
use crate::transfer::{reserve_part, room_for, ReplyParts};

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
fn shutdown_and_exit_replies_stopping_and_defers_the_exit_without_serializing_it() {
    // Specification 34 STATE-08: the explicit exit answers as `shutdown` does and defers the end
    // of the process until the reply has been written. Wrong behaviour: the exit shows in the
    // reply, or the deferred action is the plain stop.
    let response = ControlResponse::shutdown_and_exit(41);
    assert_eq!(
        response.after_response,
        Some(ControlAfterResponse::ExitRuntime)
    );
    assert_eq!(
        wire(&response),
        r#"{"id":41,"ok":true,"result":{"status":"stopping"}}"#
    );
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

/// The text of `response` written into the parts of a writer that has no bound and a heap with
/// room, joined into the one line that is sent.
fn written_in_parts(response: &ControlResponse) -> String {
    let mut reply = ReplyWriter::new(usize::MAX, reserve_part);
    response
        .write_to(&mut reply)
        .expect("a writer with no bound and a heap with room takes any text");
    let Written::Line(text) = reply.finish(usize::MAX) else {
        panic!("a reply within the frame is the line that is sent");
    };
    text
}

#[test]
fn every_response_written_in_parts_is_the_line_serialize_writes() {
    // Specification 34 PORT-05: a reply that fits the frame is byte for byte the reply trust-runtime
    // writes, for every combination of members. The texts are worked out from the envelope: `id`, `ok`,
    // then `result`, `error`, `error_code` and `audit_id` when present, in that order. Catches a member
    // written out of order or left out, an error code written without its error, and an audit record id
    // written before the error code.
    let audit = |text| Some(SmolStr::new(text));
    let cases = [
        (
            ControlResponse::ok(7, json!({"status": "running"})),
            r#"{"id":7,"ok":true,"result":{"status":"running"}}"#,
        ),
        (
            ControlResponse::ok(7, json!({"status": "running"})).with_audit_id(audit("audit-1")),
            r#"{"id":7,"ok":true,"result":{"status":"running"},"audit_id":"audit-1"}"#,
        ),
        (
            ControlResponse::ok(u64::MAX, json!(null)),
            r#"{"id":18446744073709551615,"ok":true,"result":null}"#,
        ),
        (
            ControlResponse::error(6, "unsupported request".into()),
            r#"{"id":6,"ok":false,"error":"unsupported request"}"#,
        ),
        (
            ControlResponse::error(5, "unsupported request".into()).with_audit_id(audit("audit-2")),
            r#"{"id":5,"ok":false,"error":"unsupported request","audit_id":"audit-2"}"#,
        ),
        (
            ControlResponse::error_with_code(7702, "debug disabled".into(), "feature_disabled"),
            r#"{"id":7702,"ok":false,"error":"debug disabled","error_code":"feature_disabled"}"#,
        ),
        (
            ControlResponse::error_with_code(
                3,
                "forbidden: requires role admin".into(),
                "insufficient_role",
            )
            .with_audit_id(audit("audit-9")),
            r#"{"id":3,"ok":false,"error":"forbidden: requires role admin","error_code":"insufficient_role","audit_id":"audit-9"}"#,
        ),
    ];
    for (response, expected) in &cases {
        assert_eq!(written_in_parts(response), *expected);
        assert_eq!(wire(response), *expected, "the serializer writes the same");
    }
}

#[test]
fn text_members_are_escaped_as_the_serializer_escapes_them() {
    // RFC 8259 section 7: the quotation mark, the reverse solidus and the control characters below
    // U+0020 are escaped, as `\b`, `\f`, `\n`, `\r` and `\t` or as `\u00` and two lowercase hexadecimal
    // digits, and nothing else is: not DEL (U+007F) and not text beyond ASCII, which stays in UTF-8.
    // Catches a string written with the control characters raw, a DEL escaped, or non-ASCII text
    // escaped as `\u` sequences, in the error and in the audit record id, and in a result.
    let text = "quote\" back\\slash line\nbreak return\r tab\t bell\u{7} backspace\u{8} form\u{c} \
                one\u{1} unit\u{1f} delete\u{7f} é ✓ 𝄞";
    let expected = concat!(
        r#"{"id":1,"ok":false,"error":"quote\" back\\slash line\nbreak return\r tab\t bell\u0007 "#,
        r#"backspace\b form\f one\u0001 unit\u001f delete"#,
        "\u{7f}",
        r#" é ✓ 𝄞","audit_id":"a\"b\\c\nd\u0001é"}"#,
    );
    let response = ControlResponse::error(1, text.into())
        .with_audit_id(Some(SmolStr::new("a\"b\\c\nd\u{1}é")));
    assert_eq!(written_in_parts(&response), expected);
    assert_eq!(wire(&response), expected, "the serializer writes the same");

    let ok = ControlResponse::ok(2, json!({"error": "x\"y\u{1}", "ünï": "✓"}));
    assert_eq!(written_in_parts(&ok), wire(&ok));
    assert_eq!(
        written_in_parts(&ok),
        r#"{"id":2,"ok":true,"result":{"error":"x\"y\u0001","ünï":"✓"}}"#
    );
}

#[test]
fn a_response_longer_than_a_part_is_cut_into_parts_that_join_to_its_serialized_text() {
    // Specification 34 PORT-05: a reply is written in parts of 4,096 bytes, and the parts are bytes: a
    // part may end inside a character. The response below is `{"id":1,"ok":false,"error":"` (28
    // bytes), 4,067 `a`, `é` (the two bytes 0xC3 0xA9), 900 `b` and `"}` (2 bytes): 4,999 bytes. So `é`
    // is at offsets 4,095 and 4,096, the first part is 4,096 bytes and ends with 0xC3, and the second
    // is 4,999 - 4,096 = 903 bytes and begins with 0xA9. Catches a part cut short at a character
    // boundary, which would leave the parts at 4,095 and 904 bytes, and a join that is not the text.
    let error = format!("{}é{}", "a".repeat(4_067), "b".repeat(900));
    let response = ControlResponse::error(1, error);
    let expected = wire(&response);
    assert_eq!(expected.len(), 4_999);
    let mut reply = ReplyWriter::new(usize::MAX, reserve_part);
    response.write_to(&mut reply).expect("a heap with room");
    let Written::Parts(parts) = reply.finish(100) else {
        panic!("a reply of 4,999 bytes is longer than a frame of 100 bytes");
    };
    assert_eq!(parts.part_lengths(), [4_096, 903]);
    assert_eq!(parts.range(4_095, 4_097).as_ref(), "é".as_bytes());
    assert_eq!(
        parts.range(4_090, 4_100).as_ref(),
        &expected.as_bytes()[4_090..4_100]
    );
    assert_eq!(
        parts.into_bytes().expect("a heap with room"),
        expected.as_bytes()
    );
}

#[test]
fn a_reply_that_fits_one_part_takes_one_part_and_one_that_needs_two_takes_two() {
    // Specification 34 PORT-05: each part is taken from the heap when the writing reaches it, never
    // ahead. A heap with room for one part therefore takes a reply of 4,096 bytes, whatever the bound
    // is, and refuses the first byte after them, which needs a second part. Catches a writer that
    // takes its bound up front, or two parts at once.
    let bound = 64 * 1024 * 1024;
    let mut reply = ReplyWriter::new(bound, room_for::<1>);
    reply.push(&"x".repeat(4_096));
    assert!(!reply.refused(), "4,096 bytes are one part");
    let Written::Line(text) = reply.finish(8_192) else {
        panic!("4,096 bytes fit a frame of 8,192");
    };
    assert_eq!(text.len(), 4_096);

    let mut reply = ReplyWriter::new(bound, room_for::<1>);
    reply.push(&"x".repeat(4_097));
    assert!(reply.refused(), "the 4,097th byte needs a second part");
    assert!(matches!(
        reply.finish(8_192),
        Written::NoRoom { written: 4_096 }
    ));
}

#[test]
fn a_writer_that_is_refused_says_whether_the_bound_or_the_heap_refused() {
    // Specification 34 PORT-05: a reply above the capacity and a reply the heap has no room for are
    // refused with different texts, so the writer keeps them apart. A piece that fits the bound
    // exactly is taken; one byte more overflows, and nothing is taken after it. A heap with room for
    // one part refuses the second: 4,096 bytes were written then. Overflow is noticed before the heap
    // is asked, so a piece longer than the bound overflows even where the heap would also refuse.
    let mut reply = ReplyWriter::new(10, reserve_part);
    reply.push("0123456789");
    assert!(!reply.refused(), "ten bytes fill a bound of ten");
    assert!(matches!(reply.finish(10), Written::Line(text) if text == "0123456789"));

    let mut reply = ReplyWriter::new(10, reserve_part);
    reply.push("0123456789");
    reply.push("x");
    reply.push("y");
    assert!(reply.refused());
    assert!(matches!(reply.finish(10), Written::Overflow));

    let mut reply = ReplyWriter::new(usize::MAX, room_for::<1>);
    reply.push(&"x".repeat(5_000));
    assert!(matches!(
        reply.finish(usize::MAX),
        Written::NoRoom { written: 4_096 }
    ));

    let mut reply = ReplyWriter::new(usize::MAX, room_for::<0>);
    reply.push("x");
    assert!(matches!(
        reply.finish(usize::MAX),
        Written::NoRoom { written: 0 }
    ));

    let mut reply = ReplyWriter::new(5_000, room_for::<1>);
    reply.push(&"x".repeat(5_001));
    assert!(matches!(reply.finish(usize::MAX), Written::Overflow));
    let mut reply = ReplyWriter::new(5_000, room_for::<1>);
    reply.push(&"x".repeat(4_999));
    assert!(matches!(
        reply.finish(usize::MAX),
        Written::NoRoom { written: 4_096 }
    ));
}

#[test]
fn a_successful_reply_written_in_parts_is_whole_within_its_bound_and_nothing_beyond_it() {
    // Specification 34 PORT-05, as the event reads write: the start, the result's text and the end of
    // a successful response are its serialized text when they fit the bound, to the byte, and a
    // piece beyond the bound is dropped with every later piece.
    let served = ControlResponse::ok(3, json!(null)).with_audit_id(Some(SmolStr::new("audit-9")));
    let result = json!({"events": []});
    let expected =
        wire(&ControlResponse::ok(3, result.clone()).with_audit_id(Some(SmolStr::new("audit-9"))));
    for bound in [expected.len(), expected.len() + 1, usize::MAX] {
        let mut reply = ReplyWriter::ok(3, bound, reserve_part);
        reply.push_json(&result);
        assert!(!reply.refused(), "bound={bound}");
        served.write_ok_end(&mut reply).expect("the end fits");
        let Written::Line(text) = reply.finish(usize::MAX) else {
            panic!("bound={bound}: the reply fits");
        };
        assert_eq!(text, expected, "bound={bound}");
    }
    let mut short = ReplyWriter::ok(3, expected.len() - 1, reserve_part);
    short.push_json(&result);
    assert!(
        !short.refused(),
        "the result still fits; only the end does not"
    );
    served
        .write_ok_end(&mut short)
        .expect("writing into the writer never fails");
    assert!(short.refused(), "the end does not fit");
    assert!(matches!(short.finish(usize::MAX), Written::Overflow));
    let mut overflowed = ReplyWriter::ok(3, 30, reserve_part);
    overflowed.push("{\"events\":[");
    overflowed.push(&"x".repeat(40));
    assert!(overflowed.refused());
    overflowed.push("]}");
    assert!(
        matches!(overflowed.finish(usize::MAX), Written::Overflow),
        "nothing is written after an overflow"
    );
}

#[test]
fn parts_that_are_not_text_are_reported_and_not_passed_over() {
    // Every piece a writer takes is text, so its parts always join into text. If they did not, the
    // line would be reported as a failure of serialization, not repaired: the first byte that is not
    // text is at offset 1 here. Catches a join that replaces the bytes it cannot read, which would
    // hand a corrupt line to the link.
    let mut parts = ReplyParts::new();
    parts
        .try_write(&[b'a', 0xFF, b'b'])
        .expect("a heap with room");
    let Written::NotText(error) = line(parts) else {
        panic!("0xFF is not UTF-8");
    };
    assert_eq!(error.valid_up_to(), 1);
}
