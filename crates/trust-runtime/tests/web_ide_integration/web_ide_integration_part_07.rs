use super::*;

#[test]
fn web_ide_security_and_path_traversal_contract() {
    let project = make_project("security");
    let state = control_state(source_fixture(), ControlMode::Production, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let (_, viewer_session) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "viewer" })),
        &[],
    );
    let viewer_token = viewer_session
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("viewer token");

    let (status, viewer_write) = request_json(
        "POST",
        &format!("{base}/api/ide/fs/create"),
        Some(json!({ "path": "viewer_blocked.st", "kind": "file" })),
        &[("X-Trust-Ide-Session", viewer_token)],
    );
    assert_eq!(status, 403);
    assert!(viewer_write
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("session role does not allow edits")));

    let (status, _viewer_build) = request_json(
        "POST",
        &format!("{base}/api/ide/build"),
        Some(json!({})),
        &[("X-Trust-Ide-Session", viewer_token)],
    );
    assert_eq!(status, 403);

    let (status, _viewer_validate) = request_json(
        "POST",
        &format!("{base}/api/ide/validate"),
        Some(json!({})),
        &[("X-Trust-Ide-Session", viewer_token)],
    );
    assert_eq!(status, 403);

    let (_, editor_session) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    let editor_token = editor_session
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("editor token");

    let traversal_cases = [
        (
            "POST",
            format!("{base}/api/ide/fs/create"),
            json!({ "path": "../escape.st", "kind": "file" }),
        ),
        (
            "POST",
            format!("{base}/api/ide/fs/rename"),
            json!({ "path": "main.st", "new_path": "../escaped.st" }),
        ),
        (
            "POST",
            format!("{base}/api/ide/fs/delete"),
            json!({ "path": "../main.st" }),
        ),
        (
            "POST",
            format!("{base}/api/ide/file"),
            json!({
                "path": "../main.st",
                "expected_version": 1,
                "content": "PROGRAM Main\nEND_PROGRAM\n"
            }),
        ),
    ];

    for (method, url, payload) in traversal_cases {
        let (status, body) = request_json(
            method,
            &url,
            Some(payload),
            &[("X-Trust-Ide-Session", editor_token)],
        );
        assert_eq!(status, 403);
        assert!(body
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("workspace path escapes project root")));
    }

    let (status, invalid_glob) = request_json(
        "GET",
        &format!("{base}/api/ide/search?q=Main&include=[&limit=20"),
        None,
        &[("X-Trust-Ide-Session", editor_token)],
    );
    assert_eq!(status, 400);
    assert!(invalid_glob
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("invalid include glob pattern")));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn pair_claim_rejects_oversized_json_body() {
    let project = make_project("pair-claim-body-limit");
    let state = control_state(source_fixture(), ControlMode::Production, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);
    let oversized = format!("{{\"code\":\"{}\"}}", "A".repeat(1_100_000));

    let (status, body) = post_raw_json(&format!("{base}/api/pair/claim"), &oversized, &[]);

    assert_eq!(status, 413);
    assert!(body
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("maximum size")));

    let _ = std::fs::remove_dir_all(project);
}
