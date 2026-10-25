//! The `bytecode.reload` rules with today's exact refusal and reply texts (specification 11 section
//! 6.7.1). Each expected text is written out here, not taken from the code under test.

use alloc::string::String;
use alloc::vec;
use core::cell::{Cell, RefCell};

use serde_json::json;
use trust_runtime_core::scheduler::ResourceState;

use super::*;
use crate::ports::RecordsPort;
use crate::tests::fake_host::FakeHost;
use crate::transfer::TransferSession;

const HUNG: &str = "resource scan thread did not return; the runtime process must be restarted";
const BAD_SYMBOL: &str = "invalid bytecode: Invalid symbol 33, offset 0.";

/// Asserts a refusal with its text and its stable code, which is `None` when it has none.
fn assert_refusal(response: &ControlResponse, error: &str, code: Option<&str>) {
    assert!(!response.ok, "a refusal is not ok: {response:?}");
    assert_eq!(response.error.as_deref(), Some(error));
    assert_eq!(response.error_code.as_deref(), code);
}

#[test]
fn each_reload_refusal_keeps_its_text() {
    let running = FakeHost::running();

    // Wrong behaviour: a request without params is queued, or it gets another text.
    let missing = admit(1, None, &running).expect_err("a request without params is refused");
    assert_refusal(&missing, "missing params", None);

    // Wrong behaviour: params without `bytes` are accepted and the reload is queued with no bytes.
    let no_bytes =
        admit(2, Some(json!({})), &running).expect_err("params without bytes are refused");
    assert_refusal(&no_bytes, "invalid params: missing field `bytes`", None);

    // Wrong behaviour: text that is not base64 is passed on to the scan thread. `!` is byte 33 at
    // offset 0, as the base64 decoder names it.
    let not_base64 = admit(3, Some(json!({"bytes": "!!!"})), &running)
        .expect_err("text that is not base64 is refused");
    assert_refusal(&not_base64, BAD_SYMBOL, None);

    // Wrong behaviour: a hung resource is asked to take the reload and never answers.
    let mut hung = FakeHost::running();
    hung.hung = true;
    let hung_reply =
        admit(4, Some(json!({"bytes": "eA=="})), &hung).expect_err("a hung resource is refused");
    assert_refusal(&hung_reply, HUNG, Some("resource_not_running"));

    // Wrong behaviour: a stopped resource is queued, and its caller waits for a reply that cannot come.
    let stopped = FakeHost::running();
    stopped.state.set(ResourceState::Stopped);
    let stopped_reply = admit(5, Some(json!({"bytes": "eA=="})), &stopped)
        .expect_err("a stopped resource is refused");
    assert_refusal(
        &stopped_reply,
        "resource is stopped",
        Some("resource_not_running"),
    );

    // Wrong behaviour: the resource is checked before the params, so bad bytes on a stopped resource
    // get the state text instead of the decoder's text.
    let params_first =
        admit(6, Some(json!({"bytes": "!!!"})), &stopped).expect_err("the params are read first");
    assert_refusal(&params_first, BAD_SYMBOL, None);

    // Wrong behaviour: a stopped resource that is also hung is named as stopped, so the hung text
    // would be lost.
    let mut hung_and_stopped = FakeHost::running();
    hung_and_stopped.hung = true;
    hung_and_stopped.state.set(ResourceState::Stopped);
    let hung_first = admit(7, Some(json!({"bytes": "eA=="})), &hung_and_stopped)
        .expect_err("a hung resource is refused before its state is read");
    assert_refusal(&hung_first, HUNG, Some("resource_not_running"));
}

#[test]
fn admitted_reload_queues_the_decoded_bytes_and_a_faulted_resource_takes_it() {
    let running = FakeHost::running();
    // "eA==" is one byte, 0x78 (`x`): `e` is 30 and `A` is 0, so the bits 011110 000000 give 01111000.
    // Wrong behaviour: the base64 text is queued instead of the bytes it encodes.
    let bytes = admit(1, Some(json!({"bytes": "eA=="})), &running)
        .expect("a running resource takes the reload");
    assert_eq!(bytes.bytes, vec![0x78]);
    assert_eq!(
        bytes.prehash, None,
        "bytes in the request are not an upload's parts"
    );

    // Wrong behaviour: an extra field refuses the request. The params never denied unknown fields.
    let extra = admit(2, Some(json!({"bytes": "eA==", "note": 1})), &running)
        .expect("an extra field is ignored");
    assert_eq!(extra.bytes, vec![0x78]);

    // Wrong behaviour: a faulted resource refuses the reload. Its scan thread waits alive in the
    // faulted wait and takes the reload, so only a stopped or a hung resource is refused.
    let faulted = FakeHost::running();
    faulted.state.set(ResourceState::Faulted);
    let taken = admit(3, Some(json!({"bytes": "eA=="})), &faulted)
        .expect("a faulted resource takes the reload");
    assert_eq!(taken.bytes, vec![0x78]);
}

#[test]
fn each_reload_outcome_gives_its_reply() {
    // Wrong behaviour: an applied reload is not published, or it is published after its reply.
    let published = Cell::new(None);
    let applied = reply(1, ReloadOutcome::Applied(7_u32), |metadata| {
        published.set(Some(metadata));
        Ok(())
    });
    assert_eq!(published.get(), Some(7), "the metadata is published once");
    assert!(applied.ok, "an applied reload is ok: {applied:?}");
    assert_eq!(applied.result, Some(json!({"status": "reloaded"})));
    assert_eq!(applied.error_code, None);

    // Wrong behaviour: a refresh that fails after the reload is still reported as `reloaded`.
    let refresh = reply(2, ReloadOutcome::Applied(7_u32), |_| {
        Err(String::from("ads refresh failed"))
    });
    assert_refusal(&refresh, "ads refresh failed", None);

    // Wrong behaviour: a reload the scan thread refused is published, or its text is lost.
    let failed_published = Cell::new(false);
    let failed = reply(
        3,
        ReloadOutcome::<u32>::Failed(String::from("program is not valid")),
        |_| {
            failed_published.set(true);
            Ok(())
        },
    );
    assert_refusal(&failed, "program is not valid", None);
    assert!(
        !failed_published.get(),
        "a refused reload publishes nothing"
    );

    // Wrong behaviour: a withdrawn reload is reported as applied, or with another text.
    let withdrawn_published = Cell::new(false);
    let withdrawn = reply(4, ReloadOutcome::<u32>::Withdrawn, |_| {
        withdrawn_published.set(true);
        Ok(())
    });
    assert_refusal(&withdrawn, "reload timeout", None);
    assert!(
        !withdrawn_published.get(),
        "a withdrawn reload publishes nothing"
    );

    // Wrong behaviour: a reload whose scan thread stopped loses the stable code `resource_not_running`.
    let stopped = reply(5, ReloadOutcome::<u32>::Stopped, |_| Ok(()));
    assert_refusal(
        &stopped,
        "resource stopped before the reload was applied",
        Some("resource_not_running"),
    );

    // Wrong behaviour: a commit with no result is reported as withdrawn, so a caller retries a
    // reload that may already be applied.
    let unknown = reply(6, ReloadOutcome::<u32>::Unknown, |_| Ok(()));
    assert_refusal(&unknown, "reload commit outcome is unknown", None);
}

/// The first transfer id of the session of these tests: boot 7, then sequence 1 (specification 34,
/// PORT-05).
const FIRST_ID: &str = "0000000700000001";

/// The SHA-256 of `hello`, the five bytes these tests upload, as its 32 bytes:
/// `2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824`.
const HELLO_DIGEST: [u8; 32] = [
    0x2c, 0xf2, 0x4d, 0xba, 0x5f, 0xb0, 0xa3, 0x0e, 0x26, 0xe8, 0x3b, 0x2a, 0xc5, 0xb9, 0xe2, 0x9e,
    0x1b, 0x16, 0x1e, 0x5c, 0x1f, 0xa7, 0x42, 0x5e, 0x73, 0x04, 0x33, 0x62, 0x93, 0x8b, 0x98, 0x24,
];

#[test]
fn bytecode_reload_applies_only_a_complete_transfer() {
    // Specification 34 PORT-05 and design S4 section 2.3: `bytecode.reload {transfer}` takes the bytes
    // of a complete transfer and of no other. An upload with no bytes is refused with the session's
    // own text and stays open; once its last part is in, the same request gives its five bytes, and
    // the transfer is then ended. Wrong behaviour: a partial container is queued and runs, or a refused
    // request ends the upload.
    let mut host = FakeHost::running();
    host.transfers = Some(RefCell::new(TransferSession::new(7, 1024, 1024)));
    let id = host
        .with_transfers(|session| session.begin(5, HELLO_DIGEST))
        .expect("the platform gives out its session")
        .expect("the upload begins");
    let partial = admit(1, Some(json!({"transfer": FIRST_ID})), &host)
        .expect_err("an upload with no bytes is not applied");
    assert_refusal(
        &partial,
        "transfer is receiving: next offset 0 of 5 bytes",
        None,
    );
    // `hello` is one part at offset 0, with the CRC-32 of its bytes, 0x3610a686.
    host.with_transfers(|session| session.part(id, 0, "aGVsbG8=", 0x3610_a686))
        .expect("the platform gives out its session")
        .expect("the part is stored");
    let applied = admit(2, Some(json!({"transfer": FIRST_ID})), &host)
        .expect("a complete transfer is applied");
    assert_eq!(applied.bytes, b"hello".to_vec());
    assert_eq!(
        applied.prehash, None,
        "an upload that is no image keeps no prehash"
    );
    let taken = admit(3, Some(json!({"transfer": FIRST_ID})), &host)
        .expect_err("a transfer that the reload took is unknown");
    assert_refusal(&taken, "unknown transfer", None);
}

#[test]
fn invalid_params_text_is_the_same_with_and_without_arbitrary_precision() {
    // Specification 11 `invalid params`, and the S4-2b follow-up: a fractional number in `bytes` is
    // refused with one text on every runtime. serde_json reads `1.50` differently when its
    // arbitrary_precision feature is on (the computer's build, through opcua) and off (a chip's), so
    // serde's own message would differ between the two. Wrong behaviour: the chip's serde text, or the
    // computer's, is given.
    let running = FakeHost::running();
    let refusal = admit(1, Some(json!({"bytes": 1.50})), &running)
        .expect_err("a fractional number is refused");
    assert_refusal(&refusal, "invalid params: bytes must be a string", None);
    // The same holds when the whole params are one fractional number.
    let whole = admit(2, Some(json!(1.50)), &running).expect_err("a fractional params is refused");
    assert_refusal(&whole, "invalid params: parameters must be an object", None);
}
