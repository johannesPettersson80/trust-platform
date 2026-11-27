use super::*;

// Specification 11, §6.9.7: a read of the project's files passes the web authentication like any
// other read of runtime state, and only what holds no runtime state answers without it.

/// A token-mode server on a project of its own, with the token of a Viewer pairing.
fn token_server(name: &str) -> (PathBuf, String, String) {
    let project = make_project(name);
    let (pairing, token) = create_pairing_token(project.join("pairings.json"), AccessRole::Viewer);
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_options(
        state,
        project.clone(),
        None,
        Some(pairing),
        WebAuthMode::Token,
    );
    (project, base, token)
}

/// The status and body of `GET <path>`, with `token` in `X-Trust-Token` when there is one.
fn get(base: &str, path: &str, token: Option<&str>) -> (u16, String) {
    let request = ureq::get(&format!("{base}{path}"))
        .config()
        .http_status_as_error(false)
        .build();
    let request = match token {
        Some(token) => request.header("X-Trust-Token", token),
        None => request,
    };
    let mut response = request.call().expect("a response");
    let status = response.status().as_u16();
    (
        status,
        response.body_mut().read_to_string().expect("read the body"),
    )
}

#[test]
fn reads_of_the_project_require_the_web_authentication() {
    // Specification 11, §6.9.7: in token mode, GET /api/program and GET /api/source, which list
    // and read the project's sources in `src`, and GET /api/setup/defaults, which reads its
    // configuration files, answer HTTP 401 with `unauthorized` without a token, and a Viewer's
    // token reads them: the program lists `main.st`, the source is that file's text, and the
    // defaults are a JSON object. Catches the project's sources or configuration read by any
    // network client (27 November 2026: all three answered without a token).
    let (project, base, token) = token_server("project-reads-auth");
    for path in [
        "/api/program",
        "/api/source?file=main.st",
        "/api/setup/defaults",
    ] {
        let (status, body) = get(&base, path, None);
        assert_eq!(status, 401, "{path} without a token: {body}");
        assert_eq!(
            serde_json::from_str::<Value>(&body).ok(),
            Some(json!({ "error": "unauthorized", "ok": false })),
            "{path} without a token"
        );
    }
    let (status, body) = get(&base, "/api/program", Some(token.as_str()));
    assert_eq!(status, 200, "a Viewer lists the sources: {body}");
    let program: Value = serde_json::from_str(&body).expect("the program's JSON");
    assert_eq!(program["sources"], json!(["main.st"]), "{body}");
    let (status, body) = get(&base, "/api/source?file=main.st", Some(token.as_str()));
    assert_eq!(status, 200, "a Viewer reads a source: {body}");
    assert_eq!(
        body,
        "PROGRAM Main\nVAR\nx : INT := 0;\nEND_VAR\nEND_PROGRAM\n"
    );
    let (status, body) = get(&base, "/api/setup/defaults", Some(token.as_str()));
    assert_eq!(status, 200, "a Viewer reads the setup's defaults: {body}");
    assert!(
        serde_json::from_str::<Value>(&body).is_ok_and(|defaults| defaults.is_object()),
        "{body}"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn routes_without_runtime_state_answer_without_a_token() {
    // Specification 11, §6.9.7: in token mode, GET /api/ui/mode answers the interface's mode and
    // GET /api/qr a QR code of the text the request gives, without a token, as neither reads
    // runtime state. Catches a route that the specification names as public refused, which
    // would break the pages that call it before a token is entered.
    let (project, base, _token) = token_server("stateless-routes");
    let (status, body) = get(&base, "/api/ui/mode", None);
    assert_eq!(status, 200, "the interface's mode: {body}");
    let mode: Value = serde_json::from_str(&body).expect("the mode's JSON");
    assert_eq!(mode["ok"], json!(true), "{body}");
    let (status, body) = get(&base, "/api/qr?text=hello", None);
    assert_eq!(status, 200, "a QR code: {body}");
    assert!(body.contains("<svg"), "an SVG image: {body}");
    let _ = std::fs::remove_dir_all(project);
}
