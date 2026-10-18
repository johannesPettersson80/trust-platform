//! The `bytecode.reload` rules with today's exact refusal and reply texts (specification 11 section
//! 6.7.1). Each expected text is written out here, not taken from the code under test.

use alloc::string::String;
use alloc::vec;
use core::cell::Cell;

use serde_json::json;
use trust_runtime_core::scheduler::ResourceState;

use super::*;
use crate::tests::fake_host::FakeHost;

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
    assert_eq!(bytes, vec![0x78]);

    // Wrong behaviour: an extra field refuses the request. The params never denied unknown fields.
    let extra = admit(2, Some(json!({"bytes": "eA==", "note": 1})), &running)
        .expect("an extra field is ignored");
    assert_eq!(extra, vec![0x78]);

    // Wrong behaviour: a faulted resource refuses the reload. Its scan thread waits alive in the
    // faulted wait and takes the reload, so only a stopped or a hung resource is refused.
    let faulted = FakeHost::running();
    faulted.state.set(ResourceState::Faulted);
    let taken = admit(3, Some(json!({"bytes": "eA=="})), &faulted)
        .expect("a faulted resource takes the reload");
    assert_eq!(taken, vec![0x78]);
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
