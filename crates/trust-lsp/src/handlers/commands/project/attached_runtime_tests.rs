//! The runtime that a debug session attached to (specification 25 §6.8).

use super::*;

/// A workspace folder's path on this platform.
#[cfg(windows)]
const FOLDER: &str = r"C:\workspace";
/// A workspace folder's path on this platform.
#[cfg(not(windows))]
const FOLDER: &str = "/workspace";

/// The endpoint kept for the document `name` of [`FOLDER`].
fn attached_for(state: &ServerState, name: &str) -> Option<String> {
    state
        .simulator_controls()
        .attached()
        .for_document(&std::path::Path::new(FOLDER).join(name))
        .map(|runtime| runtime.endpoint)
}

#[test]
fn a_session_s_endpoint_is_kept_for_its_folder_until_the_session_ends() {
    // Specification 25 §6.8: the extension names the session's folder and the endpoint it
    // attached to, trimmed here, and the folder with `endpoint: null` when the session ends.
    // Catches an endpoint kept past its session's end, one kept for a document outside the
    // folder, and the answer of a command that records nothing.
    let state = ServerState::new();
    let answered = answer(
        &state,
        json!({ "folder": FOLDER, "endpoint": " tcp://127.0.0.1:43210 " }),
    )
    .expect("answer");
    assert_eq!(answered, json!({ "ok": true }));
    assert_eq!(
        attached_for(&state, "main.st"),
        Some("tcp://127.0.0.1:43210".to_owned())
    );
    let elsewhere = if cfg!(windows) {
        r"D:\other\main.st"
    } else {
        "/other/main.st"
    };
    assert_eq!(
        state
            .simulator_controls()
            .attached()
            .for_document(std::path::Path::new(elsewhere)),
        None
    );
    answer(&state, json!({ "folder": FOLDER, "endpoint": null })).expect("answer");
    assert_eq!(attached_for(&state, "main.st"), None);
}

#[test]
fn an_endpoint_that_is_not_a_control_endpoint_is_refused() {
    // Specification 11 §6.8.0a: a control endpoint is `tcp://` with an address, or `unix://`
    // with a path on Unix. Catches an endpoint kept that no inline-value fetch could reach.
    let state = ServerState::new();
    let refused = answer(
        &state,
        json!({ "folder": FOLDER, "endpoint": "http://plc:8080" }),
    )
    .expect_err("refused");
    assert!(refused.contains("is not a control endpoint"), "{refused}");
    assert_eq!(attached_for(&state, "main.st"), None);
}
