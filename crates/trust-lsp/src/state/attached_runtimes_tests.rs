//! The runtimes that debug sessions attached to (specification 14 §7.1.2, specification 25 §6.8).

use super::*;

/// A project folder whose `runtime.toml` names the control endpoint `endpoint` with the token
/// `token`, and that has no endpoint file, so that `ctl --project` finds that endpoint and token.
fn project_with_control(endpoint: &str, token: &str) -> PathBuf {
    let folder = trust_test_support::unique_temp_path("trust-lsp-attached-runtime");
    std::fs::create_dir_all(&folder).expect("the project folder");
    let runtime = trust_runtime::bundle_template::render_runtime_toml(&"Attached".into(), 100)
        .replacen(
            "[runtime.control]\n",
            &format!("[runtime.control]\nendpoint = \"{endpoint}\"\nauth_token = \"{token}\"\n"),
            1,
        );
    std::fs::write(folder.join("runtime.toml"), runtime).expect("runtime.toml");
    folder
}

#[test]
fn the_token_is_the_folder_s_own_runtime_s_when_that_is_the_attached_one() {
    // Specification 14 §7.1.2: the server finds the folder's runtime as `ctl --project` does,
    // here through the project's runtime.toml, which names tcp://127.0.0.1:43210 and a token.
    // Attached to that endpoint, the token is the project's; attached to port 43211, or without a
    // folder, there is none. Catches a token sent to a runtime that is not the project's, and an
    // attach session without the token its project has.
    let token = trust_runtime::control::endpoint::generate_control_token().expect("a token");
    let folder = project_with_control("tcp://127.0.0.1:43210", &token);
    let attached = |folder: Option<PathBuf>, endpoint: &str| AttachedRuntime {
        folder,
        endpoint: endpoint.to_owned(),
    };
    assert_eq!(
        attached(Some(folder.clone()), "tcp://127.0.0.1:43210").token(),
        Some(token.clone())
    );
    assert_eq!(
        attached(Some(folder.clone()), "tcp://127.0.0.1:43211").token(),
        None
    );
    assert_eq!(attached(None, "tcp://127.0.0.1:43210").token(), None);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn the_innermost_folder_s_session_serves_a_document_and_one_without_a_folder_the_rest() {
    // Specification 14 §7.1.2: the innermost folder when folders nest. A session of the outer
    // folder and one of the inner folder, and one without a folder: a document of the inner
    // folder takes the inner session, one of the outer folder the outer, one elsewhere the
    // session without a folder; ending the inner session leaves the outer one for its documents.
    // Catches the outer folder winning over the inner, and an ended session still answering.
    let root = if cfg!(windows) { r"C:\work" } else { "/work" };
    let outer = PathBuf::from(root);
    let inner = outer.join("inner");
    let runtimes = AttachedRuntimes::default();
    runtimes.record(
        Some(outer.clone()),
        "outer",
        Some("tcp://127.0.0.1:1001".into()),
    );
    runtimes.record(
        Some(inner.clone()),
        "inner",
        Some("tcp://127.0.0.1:1002".into()),
    );
    runtimes.record(None, "none", Some("tcp://127.0.0.1:1003".into()));
    let endpoint = |document: PathBuf| {
        runtimes
            .for_document(&document)
            .map(|runtime| runtime.endpoint)
    };
    assert_eq!(
        endpoint(inner.join("main.st")),
        Some("tcp://127.0.0.1:1002".into())
    );
    assert_eq!(
        endpoint(outer.join("main.st")),
        Some("tcp://127.0.0.1:1001".into())
    );
    let elsewhere = if cfg!(windows) {
        r"D:\other\main.st"
    } else {
        "/other/main.st"
    };
    assert_eq!(
        endpoint(PathBuf::from(elsewhere)),
        Some("tcp://127.0.0.1:1003".into())
    );
    runtimes.record(Some(inner.clone()), "inner", None);
    assert_eq!(
        endpoint(inner.join("main.st")),
        Some("tcp://127.0.0.1:1001".into())
    );
}

#[test]
fn two_sessions_of_one_folder_keep_their_own_records_and_the_later_one_serves() {
    // Specification 25 §6.8: the extension names each session, so that two attach sessions of one
    // folder keep their own records. Session A attaches, then B: B's endpoint serves the folder's
    // documents; A ends and B's stays; B ends and none is left. Catches the end of one session
    // clearing the record of another that still runs, and the first session winning over a later
    // one.
    let folder = PathBuf::from(if cfg!(windows) { r"C:\work" } else { "/work" });
    let runtimes = AttachedRuntimes::default();
    let endpoint = || {
        runtimes
            .for_document(&folder.join("main.st"))
            .map(|runtime| runtime.endpoint)
    };
    runtimes.record(
        Some(folder.clone()),
        "a",
        Some("tcp://127.0.0.1:2001".into()),
    );
    runtimes.record(
        Some(folder.clone()),
        "b",
        Some("tcp://127.0.0.1:2002".into()),
    );
    assert_eq!(endpoint(), Some("tcp://127.0.0.1:2002".into()));
    runtimes.record(Some(folder.clone()), "a", None);
    assert_eq!(endpoint(), Some("tcp://127.0.0.1:2002".into()));
    runtimes.record(Some(folder.clone()), "b", None);
    assert_eq!(endpoint(), None);
}

#[test]
fn the_token_of_a_project_in_the_folder_s_bundle_folder_is_found() {
    // Specification 25 §6.7: a workspace folder's runtime project is the folder, or its `bundle`
    // folder when only that holds runtime.toml, as the extension resolves the attach endpoint.
    // Here bundle/runtime.toml names tcp://127.0.0.1:43220 and a token: attached to that endpoint,
    // the session gets the token. Catches the token looked for in the workspace folder itself,
    // which has no runtime.toml, so that the runtime refuses the inline values' requests.
    let token = trust_runtime::control::endpoint::generate_control_token().expect("a token");
    let bundle = project_with_control("tcp://127.0.0.1:43220", &token);
    let folder = trust_test_support::unique_temp_path("trust-lsp-attached-bundle");
    std::fs::create_dir_all(&folder).expect("the workspace folder");
    std::fs::rename(&bundle, folder.join("bundle")).expect("the bundle folder");
    let attached = AttachedRuntime {
        folder: Some(folder.clone()),
        endpoint: "tcp://127.0.0.1:43220".to_owned(),
    };
    assert_eq!(attached.token(), Some(token));
    let _ = std::fs::remove_dir_all(&folder);
}
