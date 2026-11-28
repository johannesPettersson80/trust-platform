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
    runtimes.record(Some(outer.clone()), Some("tcp://127.0.0.1:1001".into()));
    runtimes.record(Some(inner.clone()), Some("tcp://127.0.0.1:1002".into()));
    runtimes.record(None, Some("tcp://127.0.0.1:1003".into()));
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
    runtimes.record(Some(inner.clone()), None);
    assert_eq!(
        endpoint(inner.join("main.st")),
        Some("tcp://127.0.0.1:1001".into())
    );
}
