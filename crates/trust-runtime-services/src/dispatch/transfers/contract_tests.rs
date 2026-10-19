//! The transfer operations and `program.active` through the dispatcher (specification 34, PORT-05 and
//! §11.2). Each expected value is a literal worked out from the specification, the requests below,
//! and the SHA-256 and CRC-32 of the five bytes `hello`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::sha256;

use crate::access::AccessRole;
use crate::dispatch::{prepare_line, prepare_line_within};
use crate::events::RuntimeEvent;
use crate::ports::{ProgramIdentities, ProgramIdentity};
use crate::protocol::ControlAfterResponse;
use crate::tests::fake_host::FakeHost;
use crate::transfer::wire::hex_text;
use crate::transfer::{room_for, TransferSession};

/// The boot number of the session of these tests. Its first transfer id is `0000000700000001`: the
/// boot number, then sequence 1 (specification 34, PORT-05).
const BOOT: u32 = 7;
const FIRST_ID: &str = "0000000700000001";
/// The SHA-256 of `hello`, as 64 hexadecimal digits.
const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
/// The CRC-32 of `hello`, as 8 hexadecimal digits.
const HELLO_CRC32: &str = "3610a686";
/// `hello` in standard base64 (RFC 4648).
const HELLO_BASE64: &str = "aGVsbG8=";
/// The frame of a device's link: 8,192 bytes (specification 34, PORT-05).
const FRAME: usize = 8_192;
/// The reply part length: 4,096 bytes (specification 34, PORT-05).
const PART: usize = 4_096;
/// The upload limit of the ESP32-C6's session: one program slot, 3,056 KiB (specification 34,
/// Appendix D.2).
const C6_SLOT_BYTES: usize = 3_129_344;
/// The reply capacity of the ESP32-C6's session: its heap, 72 KiB + 64 KiB = 139,264 bytes
/// (specification 34, Appendix D.2).
const C6_HEAP_BYTES: usize = 139_264;
/// One event of the store [`fill_events`] fills, as `events.tail` writes it: `{"feature":"debug",
/// "request_type":null,"time_ns":null,"type":"feature_disabled"}` is 80 bytes, whatever order the
/// members come in.
const EVENT_BYTES: usize = 80;
/// The text before the first event of the reply to request 7: `{"id":7,"ok":true,"result":
/// {"events":[` is 38 bytes.
const REPLY_START: &str = r#"{"id":7,"ok":true,"result":{"events":["#;
/// The text after the last event of the reply to request 7 when its audit record is the first:
/// `]},"audit_id":"audit-1"}` is 24 bytes.
const REPLY_END: &str = r#"]},"audit_id":"audit-1"}"#;

/// A running runtime whose transfer session takes uploads of `max` bytes and holds replies of
/// `capacity` bytes.
fn host_with_session(max: usize, capacity: usize) -> FakeHost {
    let mut host = FakeHost::running();
    host.transfers = Some(RefCell::new(TransferSession::new(BOOT, max, capacity)));
    host
}

/// The answer to `request` through the unbounded pipeline, parsed.
fn answer(host: &FakeHost, request: &Value) -> Value {
    let line = prepare_line(&request.to_string(), host, None).response;
    serde_json::from_str(&line).expect("every reply is JSON")
}

/// Fills the event store with `count` feature-disabled events, oldest first.
fn fill_events(host: &FakeHost, count: usize) {
    let mut events = host.events.borrow_mut();
    for _ in 0..count {
        events.push_back(RuntimeEvent::FeatureDisabled {
            feature: SmolStr::new("debug"),
            request_type: None,
            time: None,
        });
    }
}

/// Serves `events.tail` for the whole store through the link of [`FRAME`] bytes.
fn events_tail_line() -> String {
    json!({"id": 7, "type": "events.tail", "params": {"limit": 1000}}).to_string()
}

/// Serves `events.tail` for the newest `limit` events.
fn events_tail_of(limit: usize) -> String {
    json!({"id": 7, "type": "events.tail", "params": {"limit": limit}}).to_string()
}

/// A host with the ESP32-C6's session and `count` events in its store.
fn c6_host_with_events(count: usize) -> FakeHost {
    let host = host_with_session(C6_SLOT_BYTES, C6_HEAP_BYTES);
    fill_events(&host, count);
    host
}

/// The reply that trust-runtime writes to `events.tail` of `count` events, request 7: the whole
/// reply of the unbounded pipeline on a host of its own.
fn whole_reply_of(count: usize) -> String {
    let host = host_with_session(C6_SLOT_BYTES, C6_HEAP_BYTES);
    fill_events(&host, count);
    prepare_line(&events_tail_of(count), &host, None).response
}

/// The answer to `transfer.get` of `len` bytes at `offset`, served as a chip serves it.
fn get_through_the_link(host: &FakeHost, transfer: &str, offset: usize, len: usize) -> Value {
    let request = json!({"id": 8, "type": "transfer.get", "params": {"transfer": transfer,
        "offset": offset, "len": len}});
    let line = prepare_line_within(&request.to_string(), host, None, FRAME).response;
    serde_json::from_str(&line).expect("every reply is JSON")
}

/// The bytes of a successful `transfer.get` answer.
fn bytes_of(answer: &Value) -> Vec<u8> {
    STANDARD
        .decode(answer["result"]["data"].as_str().expect("part data"))
        .expect("part data is base64")
}

#[test]
fn transfer_operations_require_admin() {
    // Specification 34 PORT-05 with the operation registry: the transfer operations are Admin. An
    // Operator's `transfer.begin` must be refused; a wrong registry entry of Operator would let it
    // open an upload.
    let mut operator = host_with_session(1024, 1024);
    operator.requires_auth = true;
    operator.pairing = Some(("operator-token", AccessRole::Operator));
    let refused = answer(
        &operator,
        &json!({"id": 1, "type": "transfer.begin", "auth": "operator-token",
            "params": {"bytes": 5, "sha256": HELLO_SHA256}}),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["error"], json!("forbidden: requires role admin"));
    assert_eq!(refused["error_code"], json!("insufficient_role"));

    let mut admin = host_with_session(1024, 1024);
    admin.requires_auth = true;
    admin.pairing = Some(("admin-token", AccessRole::Admin));
    let accepted = answer(
        &admin,
        &json!({"id": 1, "type": "transfer.begin", "auth": "admin-token",
            "params": {"bytes": 5, "sha256": HELLO_SHA256}}),
    );
    assert_eq!(accepted["result"]["transfer"], json!(FIRST_ID));
}

#[test]
fn a_transfer_runs_begin_parts_status_and_abort_through_the_dispatcher() {
    // Specification 34 PORT-05, transfer begin, part, status, transfer completion and abort. The
    // five bytes are one last part at offset 0, with the CRC-32 of `hello`. A dispatcher that does
    // not route the operation answers `unsupported request`; a part whose data is not stored
    // leaves `next_offset` at 0.
    let host = host_with_session(1024, 1024);
    let begin = answer(
        &host,
        &json!({"id": 1, "type": "transfer.begin", "params": {"bytes": 5, "sha256": HELLO_SHA256}}),
    );
    assert_eq!(
        begin["result"],
        json!({"transfer": FIRST_ID, "part_bytes": 4096, "next_offset": 0})
    );
    let part = answer(
        &host,
        &json!({"id": 2, "type": "transfer.part", "params": {"transfer": FIRST_ID, "offset": 0,
            "data": HELLO_BASE64, "crc32": HELLO_CRC32}}),
    );
    assert_eq!(part["result"], json!({"next_offset": 5}));
    let status = answer(
        &host,
        &json!({"id": 3, "type": "transfer.status", "params": {"transfer": FIRST_ID}}),
    );
    assert_eq!(
        status["result"],
        json!({"state": "complete", "next_offset": 5, "bytes": 5})
    );
    let abort = answer(
        &host,
        &json!({"id": 4, "type": "transfer.abort", "params": {"transfer": FIRST_ID}}),
    );
    assert_eq!(abort["result"], json!({}));
    let gone = answer(
        &host,
        &json!({"id": 5, "type": "transfer.status", "params": {"transfer": FIRST_ID}}),
    );
    assert_eq!(gone["error"], json!("unknown transfer"));
}

#[test]
fn a_host_without_a_session_answers_unsupported_for_transfers() {
    // Specification 34 PORT-05 names no session for a platform that has none: the operations are
    // unsupported there, as before the platform gave out its session. A default that opened an
    // upload would answer `ok`.
    let host = FakeHost::running();
    let begin = answer(
        &host,
        &json!({"id": 1, "type": "transfer.begin", "params": {"bytes": 5, "sha256": HELLO_SHA256}}),
    );
    assert_eq!(begin["ok"], json!(false));
    assert_eq!(begin["error"], json!("unsupported request"));
}

#[test]
fn transfer_params_refuse_a_fractional_number_with_one_text() {
    // Spec 11 `invalid params`: a fractional number in a whole-number member is refused with the same
    // text on a computer and on a chip. serde_json reads `1.50` differently when its
    // arbitrary_precision feature is on, so a serde message would differ between the two.
    let host = host_with_session(1024, 1024);
    let begin = answer(
        &host,
        &json!({"id": 1, "type": "transfer.begin", "params": {"bytes": 1.50, "sha256": HELLO_SHA256}}),
    );
    assert_eq!(
        begin["error"],
        json!("invalid params: bytes must be a whole number")
    );
}

#[test]
fn program_active_reports_the_active_and_the_previous_slot() {
    // Specification 34 §11.2 and design §2.1: `program.active` reports the selected program and the
    // one before it. The values differ, so a dispatcher that swaps the slots reports the wrong one.
    let mut host = FakeHost::running();
    host.program_identities = Some(ProgramIdentities {
        active: Some(ProgramIdentity {
            sequence: 2,
            program_sha256: [0x22; 32],
            bytes: 300,
        }),
        previous: Some(ProgramIdentity {
            sequence: 1,
            program_sha256: [0x11; 32],
            bytes: 280,
        }),
    });
    let reply = answer(&host, &json!({"id": 1, "type": "program.active"}));
    assert_eq!(
        reply["result"],
        json!({
            "active": {"sequence": 2, "program_sha256": "2222222222222222222222222222222222222222222222222222222222222222", "bytes": 300},
            "previous": {"sequence": 1, "program_sha256": "1111111111111111111111111111111111111111111111111111111111111111", "bytes": 280},
        })
    );
}

#[test]
fn program_active_reports_null_for_a_slot_without_a_program() {
    // Each identity is `null` when the store holds no such program. A store with one program and
    // no previous one must not report the active program twice.
    let mut host = FakeHost::running();
    host.program_identities = Some(ProgramIdentities {
        active: Some(ProgramIdentity {
            sequence: 1,
            program_sha256: [0x22; 32],
            bytes: 300,
        }),
        previous: None,
    });
    let reply = answer(&host, &json!({"id": 1, "type": "program.active"}));
    assert_eq!(reply["result"]["previous"], Value::Null);
    assert_eq!(reply["result"]["active"]["sequence"], json!(1));
}

#[test]
fn program_active_without_a_store_is_unsupported() {
    // Until a platform wires its store (S4-4, S4-6), `program.active` answers `unsupported request`.
    let host = FakeHost::running();
    let reply = answer(&host, &json!({"id": 1, "type": "program.active"}));
    assert_eq!(reply["error"], json!("unsupported request"));
}

#[test]
fn a_reply_longer_than_the_frame_is_answered_with_its_head_and_fetched_whole() {
    // Specification 34 PORT-05, reply head and get: a reply longer than the frame is held whole and
    // answered with its head. The parts fetched with `transfer.get` give the reply, and their
    // SHA-256 is the head's. A head that is not given, or a reply cut at the frame, fails here.
    let host = host_with_session(1024, 1024 * 1024);
    fill_events(&host, 1000);
    let head_reply = prepare_line_within(&events_tail_line(), &host, None, FRAME);
    let head: Value = serde_json::from_str(&head_reply.response).expect("the head is JSON");
    assert_eq!(head["id"], json!(7));
    assert_eq!(head["ok"], json!(true));
    let head_result = &head["result"];
    assert_eq!(head_result["part_bytes"], json!(PART));
    let transfer = head_result["reply_transfer"]
        .as_str()
        .expect("a reply transfer id");
    let bytes = usize::try_from(head_result["bytes"].as_u64().expect("the reply's length"))
        .expect("the length fits a usize");
    assert!(bytes > FRAME, "the reply must be longer than the frame");

    let mut whole: Vec<u8> = Vec::new();
    while whole.len() < bytes {
        let offset = whole.len();
        let get = answer(
            &host,
            &json!({"id": 8, "type": "transfer.get", "params": {"transfer": transfer,
                "offset": offset, "len": PART}}),
        );
        let data = STANDARD
            .decode(get["result"]["data"].as_str().expect("part data"))
            .expect("part data is base64");
        assert_eq!(
            get["result"]["crc32"],
            json!(hex_text(&crc32::checksum(&data).to_be_bytes()))
        );
        whole.extend_from_slice(&data);
    }
    assert_eq!(whole.len(), bytes);
    assert_eq!(head_result["sha256"], json!(hex_text(&sha256(&whole))));
    let reply: Value = serde_json::from_slice(&whole).expect("the whole reply is JSON");
    assert_eq!(reply["id"], json!(7));
    assert_eq!(reply["audit_id"], head["audit_id"]);
    assert_eq!(
        reply["result"]["events"].as_array().map(Vec::len),
        Some(1000)
    );
}

#[test]
fn events_tail_beyond_the_frame_is_whole_through_the_reply_head() {
    // Specification 34 PORT-05: `events.tail` is not cut at the frame. Fetched whole, the held reply
    // is byte for byte the reply the unbounded pipeline writes for the same store.
    let host = host_with_session(1024, 1024 * 1024);
    fill_events(&host, 1000);
    let head: Value = serde_json::from_str(
        &prepare_line_within(&events_tail_line(), &host, None, FRAME).response,
    )
    .expect("the head is JSON");
    let transfer = head["result"]["reply_transfer"]
        .as_str()
        .expect("a reply transfer id");
    let bytes =
        usize::try_from(head["result"]["bytes"].as_u64().expect("length")).expect("fits a usize");
    let mut whole: Vec<u8> = Vec::new();
    while whole.len() < bytes {
        let offset = whole.len();
        let get = answer(
            &host,
            &json!({"id": 8, "type": "transfer.get", "params": {"transfer": transfer,
                "offset": offset, "len": PART}}),
        );
        whole.extend(
            STANDARD
                .decode(get["result"]["data"].as_str().expect("part data"))
                .expect("part data is base64"),
        );
    }
    let unbounded_host = host_with_session(1024, 1024 * 1024);
    fill_events(&unbounded_host, 1000);
    let unbounded = prepare_line(&events_tail_line(), &unbounded_host, None).response;
    assert_eq!(whole, unbounded.as_bytes());
}

#[test]
fn a_reply_above_the_buffer_is_refused_not_cut() {
    // Specification 34 PORT-05: a reply longer than the reply buffer is refused whole, with its id
    // and audit record id, and never cut. The buffer here holds 1,000 bytes; the reply is longer.
    let host = host_with_session(1024, 1000);
    fill_events(&host, 1000);
    let reply: Value = serde_json::from_str(
        &prepare_line_within(&events_tail_line(), &host, None, FRAME).response,
    )
    .expect("the refusal is JSON");
    assert_eq!(reply["id"], json!(7));
    assert_eq!(reply["ok"], json!(false));
    assert_eq!(reply["error"], json!("reply longer than 1000 bytes"));
    assert!(reply.get("result").is_none());
    assert!(reply.get("audit_id").is_some());
}

#[test]
fn a_chip_holds_two_hundred_events_in_parts_and_answers_them_whole() {
    // Specification 34 PORT-05, reply head and get, reply capacity and parts; Appendix D.2, long
    // replies. A chip with the C6's session (a reply capacity of 139,264 bytes) holds a read of its
    // store's maximum, 200 events, which the bench found refused at the 8,192-byte frame. By hand:
    // the reply is 38 bytes of start, 200 events of 80 bytes, 199 commas and 24 bytes of end,
    // 38 + 16,000 + 199 + 24 = 16,261 bytes, which is three parts of 4,096 and a fourth of
    // 16,261 - 12,288 = 3,973. The head names that length and the SHA-256 of the whole reply as
    // trust-runtime writes it, and `transfer.get` from offset 0 in parts of 4,096 gives exactly those
    // bytes. Catches a chip that still refuses at the frame, a head whose digest or length belongs to
    // other bytes, a part read from the wrong place and a reply that is cut.
    let whole = whole_reply_of(200);
    assert_eq!(whole.len(), 16_261);
    assert_eq!(
        whole.len(),
        REPLY_START.len() + 200 * EVENT_BYTES + 199 + REPLY_END.len()
    );
    let host = c6_host_with_events(200);
    let head_line = prepare_line_within(&events_tail_of(200), &host, None, FRAME).response;
    let head: Value = serde_json::from_str(&head_line).expect("the head is JSON");
    assert_eq!(head["id"], json!(7));
    assert_eq!(head["ok"], json!(true));
    assert_eq!(head["audit_id"], json!("audit-1"));
    let result = &head["result"];
    assert_eq!(result["bytes"], json!(16_261));
    assert_eq!(result["part_bytes"], json!(PART));
    assert_eq!(result["sha256"], json!(hex_text(&sha256(whole.as_bytes()))));
    let transfer = result["reply_transfer"]
        .as_str()
        .expect("a reply transfer id");
    assert_eq!(transfer, FIRST_ID, "the first transfer of the boot");

    let mut read: Vec<u8> = Vec::new();
    let mut lengths = Vec::new();
    while read.len() < 16_261 {
        let answer = get_through_the_link(&host, transfer, read.len(), PART);
        assert_eq!(answer["ok"], json!(true), "offset {}", read.len());
        assert_eq!(answer["result"]["offset"], json!(read.len()));
        let part = bytes_of(&answer);
        assert_eq!(
            answer["result"]["crc32"],
            json!(hex_text(&crc32::checksum(&part).to_be_bytes()))
        );
        lengths.push(part.len());
        read.extend(part);
    }
    assert_eq!(lengths, [4_096, 4_096, 4_096, 3_973]);
    assert_eq!(read, whole.as_bytes());
    assert!(read.starts_with(REPLY_START.as_bytes()));
    assert!(read.ends_with(REPLY_END.as_bytes()));
}

#[test]
fn a_reply_of_exactly_the_frame_is_one_line_and_one_byte_more_is_held() {
    // Specification 34 PORT-05: a reply that fits the frame is still one line, byte for byte the reply
    // trust-runtime writes; a longer one is held. By hand, 100 events are 38 + 8,000 + 99 + 24 =
    // 8,161 bytes, written in two parts of 4,096 and 8,161 - 4,096 = 4,065 bytes: a frame of 8,161
    // bytes takes them as one line, joined from the two parts, and a frame of 8,160 holds them.
    // Catches a comparison that is off by one, and a reply of two parts that is held although it
    // fits the frame.
    let whole = whole_reply_of(100);
    assert_eq!(whole.len(), 8_161);
    let host = c6_host_with_events(100);
    let line = prepare_line_within(&events_tail_of(100), &host, None, 8_161).response;
    assert_eq!(line, whole, "a reply of exactly the frame is the line");

    let host = c6_host_with_events(100);
    let long = prepare_line_within(&events_tail_of(100), &host, None, 8_160).response;
    let head: Value = serde_json::from_str(&long).expect("the head is JSON");
    assert_eq!(head["result"]["bytes"], json!(8_161));
    assert_eq!(head["result"]["reply_transfer"], json!(FIRST_ID));
}

#[test]
fn reading_the_last_byte_frees_the_held_reply() {
    // Specification 34 PORT-05, held reply freed: after the last part is read, the reply's id is
    // `unknown transfer` for `transfer.get` and for `transfer.abort`, and reading a middle part twice
    // still works. The reply is 16,261 bytes, so its last part is the fourth, at offset 12,288.
    // Catches a reply that is never freed, one freed by the first or by a middle get, and one whose
    // last part is refused because the reply was freed before the answer was made.
    let host = c6_host_with_events(200);
    let head: Value = serde_json::from_str(
        &prepare_line_within(&events_tail_of(200), &host, None, FRAME).response,
    )
    .expect("the head is JSON");
    let transfer = head["result"]["reply_transfer"]
        .as_str()
        .expect("a reply transfer id");
    let middle = get_through_the_link(&host, transfer, 4_096, PART);
    let again = get_through_the_link(&host, transfer, 4_096, PART);
    assert_eq!(middle["ok"], json!(true));
    assert_eq!(
        again["result"], middle["result"],
        "a middle part reads again"
    );
    assert_eq!(bytes_of(&middle).len(), 4_096);
    assert_eq!(
        get_through_the_link(&host, transfer, 0, PART)["ok"],
        json!(true)
    );

    let last = get_through_the_link(&host, transfer, 12_288, PART);
    assert_eq!(last["ok"], json!(true));
    assert_eq!(bytes_of(&last).len(), 3_973);
    for operation in ["transfer.get", "transfer.abort"] {
        let params = if operation == "transfer.get" {
            json!({"transfer": transfer, "offset": 0, "len": 1})
        } else {
            json!({"transfer": transfer})
        };
        let refused = answer(
            &host,
            &json!({"id": 9, "type": operation, "params": params}),
        );
        assert_eq!(refused["ok"], json!(false), "{operation}");
        assert_eq!(refused["error"], json!("unknown transfer"), "{operation}");
    }
}

#[test]
fn a_reply_the_heap_has_no_room_for_is_refused_and_audited_as_served() {
    // Specification 34 PORT-05, reply refusals: a reply for which the heap has no room is refused
    // with `no room for a reply longer than <n> bytes`, where n is the bytes written when the heap
    // had no room, never cut and never an abort; the refusal carries the request's id and audit
    // record id; the request was served and is audited as served; the work after the reply stays.
    // A heap with room for two parts gives 2 x 4,096 = 8,192 bytes of the 16,261-byte reply of 200
    // events, so n is 8,192. A heap with room for none refuses a `shutdown` after 0 bytes, and the
    // stop that follows its reply is kept. Catches a refusal that names another number, one that
    // loses the audit record, a request audited as failed, a refusal that drops the stop, and a
    // reply that is held although its parts were never all taken.
    let session =
        TransferSession::new(BOOT, C6_SLOT_BYTES, C6_HEAP_BYTES).with_part_reserve(room_for::<2>);
    let mut host = FakeHost::running();
    host.transfers = Some(RefCell::new(session));
    fill_events(&host, 200);
    let refused = prepare_line_within(&events_tail_of(200), &host, None, FRAME);
    assert_eq!(
        refused.response,
        r#"{"id":7,"ok":false,"error":"no room for a reply longer than 8192 bytes","audit_id":"audit-1"}"#
    );
    assert_eq!(refused.after_response, None);
    let audits = host.audits();
    assert_eq!(audits.len(), 1);
    assert!(
        audits[0].ok,
        "the read was served; only its reply has no room"
    );
    let nothing_held = get_through_the_link(&host, FIRST_ID, 0, 1);
    assert_eq!(nothing_held["error"], json!("unknown transfer"));

    let session =
        TransferSession::new(BOOT, C6_SLOT_BYTES, C6_HEAP_BYTES).with_part_reserve(room_for::<0>);
    let mut host = FakeHost::running();
    host.transfers = Some(RefCell::new(session));
    let stopped = prepare_line_within(r#"{"id":41,"type":"shutdown"}"#, &host, None, FRAME);
    assert_eq!(
        stopped.response,
        r#"{"id":41,"ok":false,"error":"no room for a reply longer than 0 bytes","audit_id":"audit-1"}"#
    );
    assert_eq!(
        stopped.after_response,
        Some(ControlAfterResponse::StopResource),
        "the work after the reply stays"
    );
    assert!(host.audits()[0].ok);
}

#[test]
fn a_heap_with_room_for_one_part_still_answers_a_short_request() {
    // Specification 34 PORT-05, reply capacity and parts: each part is taken when the writing reaches
    // it, so a reply of under 4,096 bytes takes one part, whatever the session's capacity is. Here the
    // heap has room for one part only and the reply of `resume` is 69 bytes. Catches a writer that
    // takes more than the part it needs, which a heap this tight would refuse.
    let session =
        TransferSession::new(BOOT, C6_SLOT_BYTES, C6_HEAP_BYTES).with_part_reserve(room_for::<1>);
    let mut host = FakeHost::running();
    host.transfers = Some(RefCell::new(session));
    let reply = prepare_line_within(r#"{"id":4,"type":"resume"}"#, &host, None, FRAME).response;
    assert_eq!(
        reply,
        r#"{"id":4,"ok":true,"result":{"status":"running"},"audit_id":"audit-1"}"#
    );
}

#[test]
fn a_reply_that_is_not_an_event_read_is_held_in_parts_too() {
    // Specification 34 PORT-05, reply head and get: every reply longer than the frame is held, not
    // only the event reads. `capabilities` is answered as a whole response and written member by
    // member into the parts; on a link of 300-byte frames its reply is longer than a frame. The head
    // names the length and SHA-256 of the reply that trust-runtime writes for the same request, and
    // `transfer.get` gives those bytes. Catches a whole response that is refused or cut at the frame
    // because only the event reads know how to be held.
    let request = r#"{"id":5,"type":"capabilities"}"#;
    let whole = prepare_line(request, &host_with_session(1024, 65_536), None).response;
    assert!(whole.len() > 300, "the reply is longer than the frame");
    assert!(whole.len() < PART, "and fits one part");
    let host = host_with_session(1024, 65_536);
    let head: Value =
        serde_json::from_str(&prepare_line_within(request, &host, None, 300).response)
            .expect("the head is JSON");
    assert_eq!(head["id"], json!(5));
    assert_eq!(head["audit_id"], json!("audit-1"));
    assert_eq!(head["result"]["bytes"], json!(whole.len()));
    assert_eq!(
        head["result"]["sha256"],
        json!(hex_text(&sha256(whole.as_bytes())))
    );
    let transfer = head["result"]["reply_transfer"]
        .as_str()
        .expect("a reply transfer id");
    let part = get_through_the_link(&host, transfer, 0, PART);
    assert_eq!(bytes_of(&part), whole.as_bytes());
}
