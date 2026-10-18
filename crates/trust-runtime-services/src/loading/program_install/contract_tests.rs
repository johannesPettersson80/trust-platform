//! The `program.install` rules (specification 34 §11.2; decisions Q11 and G13). Each expected text and
//! value is written out here, from the specification and from the requests below, not taken from the
//! code under test.

use alloc::string::{String, ToString};
use alloc::vec;
use core::cell::RefCell;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::json;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::scheduler::ResourceState;

use super::*;
use crate::access::AccessRole;
use crate::dispatch::dispatch_value;
use crate::loading::reload;
use crate::loading::tests::{transfer_of, PROGRAM};
use crate::ports::RecordsPort;
use crate::tests::fake_host::FakeHost;
use crate::transfer::{TransferSession, PART_BYTES};

const HUNG: &str = "resource scan thread did not return; the runtime process must be restarted";

/// Asserts a refusal with its text and its stable code, which is `None` when it has none.
fn assert_refusal(response: &ControlResponse, error: &str, code: Option<&str>) {
    assert!(!response.ok, "a refusal is not ok: {response:?}");
    assert_eq!(response.error.as_deref(), Some(error));
    assert_eq!(response.error_code.as_deref(), code);
}

/// A stopped platform with a program store that commits what the checks admit. Its commit gives the
/// program sequence 3, the SHA-256 of 32 bytes of `0x11` and the fixture's 6,412 bytes.
fn store_host() -> FakeHost {
    let mut host = FakeHost::running();
    host.state.set(ResourceState::Stopped);
    host.has_program_store = true;
    host.transfers = Some(RefCell::new(TransferSession::new(7, 65_536, 65_536)));
    host.commit_identity = Some(ProgramIdentity {
        sequence: 3,
        program_sha256: [0x11; 32],
        bytes: 6412,
    });
    host
}

/// Uploads `container` as one complete transfer of `host` in parts of at most a part's length, and
/// returns its id as the request names it.
fn upload(host: &FakeHost, container: &[u8]) -> String {
    let id = host
        .with_transfers(|session| session.begin(container.len() as u64, sha256(container)))
        .expect("the platform gives out its session")
        .expect("the upload begins");
    for (index, part) in container.chunks(PART_BYTES).enumerate() {
        let offset = (index * PART_BYTES) as u64;
        let crc = crc32::checksum(part);
        host.with_transfers(|session| session.part(id, offset, &STANDARD.encode(part), crc))
            .expect("the platform gives out its session")
            .expect("the part is stored");
    }
    id.to_string()
}

/// `program.install {transfer}` through the shared rule, for the upload `transfer`.
fn install_of(host: &FakeHost, id: u64, transfer: &str) -> ControlResponse {
    request(id, Some(&json!({ "transfer": transfer })), host)
}

#[test]
fn program_install_requires_admin() {
    // Specification 11 §6.8.1 and 34 §11.2: `program.install` is an Admin operation. An Operator's
    // request is refused by the role check before anything else, and nothing is stored. Wrong
    // behaviour: an Operator's install reaches the store.
    let mut host = store_host();
    host.control_token = Some("secret");
    host.requires_auth = true;
    host.pairing = Some(("paired", AccessRole::Operator));
    let id = upload(&host, &transfer_of(PROGRAM));
    let refused = dispatch_value(
        json!({"id": 1, "type": "program.install", "auth": "paired", "params": {"transfer": id}}),
        &host,
        None,
    );
    assert_refusal(
        &refused,
        "forbidden: requires role admin",
        Some("insufficient_role"),
    );
    assert!(
        host.commits.borrow().is_empty(),
        "an Operator's install stores nothing"
    );
}

#[test]
fn program_install_while_running_replies_resource_not_running() {
    // Specification 34 §11.2 (decision Q11): a program is installed only while the resource is Stopped.
    // A running resource refuses it with `resource_not_running` and its state, and nothing is stored.
    // Wrong behaviour: the container is committed while the program runs.
    let host = store_host();
    host.state.set(ResourceState::Running);
    let id = upload(&host, &transfer_of(PROGRAM));
    let refused = install_of(&host, 1, &id);
    assert_refusal(
        &refused,
        "resource is running and its scheduler does not accept commands",
        Some("resource_not_running"),
    );
    assert!(
        host.commits.borrow().is_empty(),
        "a running resource stores nothing"
    );
}

#[test]
fn a_refused_state_leaves_the_transfer_open() {
    // Specification 34 §11.2: the transfer is taken only once the state and store checks have passed,
    // so a refusal for the state leaves the complete upload open, and it installs once the resource is
    // Stopped. Wrong behaviour: a refused install uses up its upload, and the client must upload again.
    let host = store_host();
    host.state.set(ResourceState::Running);
    let id = upload(&host, &transfer_of(PROGRAM));
    let refused = install_of(&host, 1, &id);
    assert!(!refused.ok, "a running resource refuses the install");
    host.state.set(ResourceState::Stopped);
    let installed = install_of(&host, 2, &id);
    assert!(
        installed.ok,
        "the complete upload installs once stopped: {installed:?}"
    );
}

#[test]
fn a_platform_without_a_program_store_answers_unsupported_request_and_takes_nothing() {
    // Specification 34 §11.2: a platform without a program store answers `unsupported request` once its
    // state check has passed, and takes nothing: the complete upload is still there afterwards. Wrong
    // behaviour: the upload is taken and lost, or the request is answered as if it were stored.
    let mut host = store_host();
    host.has_program_store = false;
    let id = upload(&host, &transfer_of(PROGRAM));
    let refused = install_of(&host, 1, &id);
    assert_refusal(&refused, "unsupported request", None);
    assert!(host.commits.borrow().is_empty(), "nothing is stored");
    host.has_program_store = true;
    let installed = install_of(&host, 2, &id);
    assert!(
        installed.ok,
        "the upload was not taken by the refusal: {installed:?}"
    );
}

#[test]
fn an_install_that_cannot_restore_its_retained_values_is_refused_before_any_write() {
    // Specification 11 §6.7.1 and 34 §11.2 (decision G13): the retained-value check runs before the
    // commit, and its refusal is the reply, with nothing written. The text is the warm start's own
    // error: the subrange refusal of spec 11 §6.7.1 for the retained INT(0..10) value 100. Wrong
    // behaviour: the commit runs before the check, or the refusal is dropped and the install commits.
    let mut host = store_host();
    host.retained_refusal =
        Some("invalid retained value for 'r': value 100 outside declared subrange 0..10");
    let id = upload(&host, &transfer_of(PROGRAM));
    let refused = install_of(&host, 1, &id);
    assert_refusal(
        &refused,
        "the retained values cannot be restored: retain migration error 'invalid retained value for 'r': value 100 outside declared subrange 0..10'",
        None,
    );
    assert!(
        host.commits.borrow().is_empty(),
        "a refused check writes nothing"
    );
}

#[test]
fn program_install_commits_the_checked_container_and_replies_its_identity() {
    // Specification 34 §11.2: an install that passes its checks commits the complete container and
    // replies `installed` with the identity the store gives the program. Wrong behaviour: a container
    // other than the uploaded one is committed, or the reply drops the sequence or writes the SHA-256
    // in another form.
    let host = store_host();
    let container = transfer_of(PROGRAM);
    let id = upload(&host, &container);
    let installed = install_of(&host, 1, &id);
    assert!(installed.ok, "a checked install commits: {installed:?}");
    assert_eq!(
        installed.result,
        Some(json!({
            "status": "installed",
            "sequence": 3,
            "program_sha256": "11".repeat(32),
            "bytes": 6412,
        }))
    );
    assert_eq!(*host.commits.borrow(), vec![container]);
}

#[test]
fn program_install_names_its_params_and_a_hung_resource_refusal() {
    // Specification 34 PORT-05 and 11 §6.8.1: an install without params, or without `transfer`, is
    // refused with the params' text, and a hung resource is refused as `bytecode.reload` refuses it.
    // Wrong behaviour: the request reaches the store with no transfer, or a hung scan is asked to stop.
    let host = store_host();
    let missing = request(1, None, &host);
    assert_refusal(&missing, "missing params", None);
    let no_transfer = request(2, Some(&json!({})), &host);
    assert_refusal(
        &no_transfer,
        "invalid params: missing field `transfer`",
        None,
    );

    let mut hung = store_host();
    hung.hung = true;
    let refused = install_of(&hung, 3, "0000000700000001");
    assert_refusal(&refused, HUNG, Some("resource_not_running"));
}

#[test]
fn bytecode_reload_while_running_is_still_allowed() {
    // Specification 34 §11.2 and 11 §6.7.1: online change stays allowed while the resource runs, and the
    // Stopped rule of `program.install` does not reach it. The request is admitted and its bytes are
    // queued. Wrong behaviour: a running program refuses its online change.
    let running = FakeHost::running();
    let bytes = reload::admit(1, Some(json!({"bytes": "eA=="})), &running)
        .expect("a running resource takes an online change");
    assert_eq!(bytes, b"x".to_vec());
}
