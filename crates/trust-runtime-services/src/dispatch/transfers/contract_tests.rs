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
use crate::tests::fake_host::FakeHost;
use crate::transfer::wire::hex_text;
use crate::transfer::TransferSession;

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
