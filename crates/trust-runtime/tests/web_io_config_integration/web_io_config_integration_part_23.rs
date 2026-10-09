use super::*;

// Specification 11, §6.9.7: the I/O configuration routes pass the web authentication in every
// server mode, and the read route never returns a credential.

const MQTT_IO_TOML: &str = r#"[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883", topic_in = "trust/io/in", topic_out = "trust/io/out", username = "plant", password = "s3cret-mqtt" }
"#;

fn token_server(name: &str, role: AccessRole) -> (PathBuf, String, String) {
    let project = make_project(name);
    std::fs::write(project.join("io.toml"), MQTT_IO_TOML).expect("write io.toml");
    let (pairing, token) = create_pairing_token(project.join("pairings.json"), role);
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

fn read_config(base: &str, token: Option<&str>) -> (u16, String) {
    let request = ureq::get(&format!("{base}/api/io/config"))
        .config()
        .http_status_as_error(false)
        .build();
    let request = match token {
        Some(token) => request.header("X-Trust-Token", token),
        None => request,
    };
    let mut response = request.call().expect("io config response");
    let status = response.status().as_u16();
    (
        status,
        response
            .body_mut()
            .read_to_string()
            .expect("read io config body"),
    )
}

#[test]
fn io_config_read_requires_the_web_authentication() {
    let (project, base, token) = token_server("io-config-read-auth", AccessRole::Viewer);
    let (status, body) = read_config(&base, None);
    assert_eq!(status, 401, "{body}");
    assert!(!body.contains("s3cret-mqtt"), "{body}");
    let (status, _) = read_config(&base, Some(token.as_str()));
    assert_eq!(status, 200, "a Viewer reads the configuration");
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn io_config_read_replaces_secret_parameters_with_a_marker() {
    let (project, base, token) = token_server("io-config-read-redacted", AccessRole::Viewer);
    let (status, body) = read_config(&base, Some(token.as_str()));
    assert_eq!(status, 200, "{body}");
    assert!(
        !body.contains("s3cret-mqtt"),
        "no credential in the answer: {body}"
    );
    let loaded: Value = serde_json::from_str(&body).expect("io config json");
    let params = &loaded["drivers"][0]["params"];
    assert_eq!(params["password"], json!("<redacted>"), "{body}");
    assert_eq!(params["username"], json!("plant"), "other parameters stay");
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn io_config_save_keeps_a_secret_that_comes_back_as_the_marker() {
    let (project, base, token) = token_server("io-config-save-keeps-secret", AccessRole::Engineer);
    let (_, body) = read_config(&base, Some(token.as_str()));
    let mut loaded: Value = serde_json::from_str(&body).expect("io config json");
    // The configuration as it was read, its password as the marker, with one change, goes back.
    loaded["drivers"][0]["params"]["password"] = json!("<redacted>");
    loaded["drivers"][0]["params"]["topic_in"] = json!("trust/io/in2");
    let payload = json!({ "drivers": loaded["drivers"], "use_system_io": false });
    let mut response = ureq::post(&format!("{base}/api/io/config"))
        .config()
        .http_status_as_error(false)
        .build()
        .header("Content-Type", "application/json")
        .header("X-Trust-Token", token.as_str())
        .send(&payload.to_string())
        .expect("save io config");
    let saved = response
        .body_mut()
        .read_to_string()
        .expect("read save response");
    assert!(saved.contains("I/O config saved"), "{saved}");
    let io_toml = std::fs::read_to_string(project.join("io.toml")).expect("read io.toml");
    assert!(
        io_toml.contains("s3cret-mqtt"),
        "the stored password is kept:\n{io_toml}"
    );
    assert!(!io_toml.contains("<redacted>"), "{io_toml}");
    assert!(
        io_toml.contains("trust/io/in2"),
        "the change is saved:\n{io_toml}"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn io_config_save_refuses_the_marker_without_a_stored_secret() {
    let (project, base, token) = token_server("io-config-save-marker-alone", AccessRole::Engineer);
    std::fs::remove_file(project.join("io.toml")).expect("start without io.toml");
    let payload = json!({
        "drivers": [{ "name": "mqtt", "params": { "broker": "127.0.0.1:1883", "topic_in": "a", "topic_out": "b", "password": "<redacted>" } }],
        "use_system_io": false
    });
    let mut response = ureq::post(&format!("{base}/api/io/config"))
        .config()
        .http_status_as_error(false)
        .build()
        .header("Content-Type", "application/json")
        .header("X-Trust-Token", token.as_str())
        .send(&payload.to_string())
        .expect("save io config");
    let saved = response
        .body_mut()
        .read_to_string()
        .expect("read save response");
    assert!(saved.starts_with("error:"), "{saved}");
    assert!(!project.join("io.toml").exists(), "nothing is written");
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn viewer_ide_io_and_generic_file_reads_redact_credentials() {
    let (project, base, token) = token_server("ide-io-credentials", AccessRole::Viewer);
    let mut response = ureq::post(&format!("{base}/api/ide/session"))
        .header("X-Trust-Token", &token)
        .header("Content-Type", "application/json")
        .send("{\"role\":\"viewer\"}")
        .expect("viewer session");
    let session: Value =
        serde_json::from_str(&response.body_mut().read_to_string().unwrap()).unwrap();
    let session = session["result"]["token"].as_str().expect("session token");
    for route in ["/api/ide/io/config", "/api/ide/file?path=io.toml"] {
        let mut response = ureq::get(&format!("{base}{route}"))
            .header("X-Trust-Token", &token)
            .header("X-Trust-Ide-Session", session)
            .call()
            .expect("viewer read");
        let body = response.body_mut().read_to_string().unwrap();
        assert!(body.contains("<redacted>"), "{route}: {body}");
        assert!(!body.contains("s3cret-mqtt"), "{route}: {body}");
    }
    std::fs::write(
        project.join("io.toml"),
        "[io]\npassword = 's3cret-mqtt' invalid",
    )
    .unwrap();
    let (_, body) = read_config(&base, Some(&token));
    assert!(!body.contains("s3cret-mqtt"), "malformed config: {body}");
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn io_json_save_preserves_a_stored_literal_marker() {
    let (project, base, token) = token_server("io-literal-marker", AccessRole::Engineer);
    std::fs::write(
        project.join("io.toml"),
        MQTT_IO_TOML.replace("s3cret-mqtt", "<redacted>"),
    )
    .unwrap();
    let (_, body) = read_config(&base, Some(&token));
    let loaded: Value = serde_json::from_str(&body).unwrap();
    let mut response = ureq::post(&format!("{base}/api/io/config"))
        .header("Content-Type", "application/json")
        .header("X-Trust-Token", &token)
        .send(json!({ "drivers": loaded["drivers"], "use_system_io": false }).to_string())
        .expect("literal marker round trip");
    assert!(response
        .body_mut()
        .read_to_string()
        .unwrap()
        .contains("I/O config saved"));
    let stored = std::fs::read_to_string(project.join("io.toml")).unwrap();
    assert!(stored.contains("<redacted>"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn standalone_config_text_read_and_write_preserve_stored_credentials() {
    let project = make_project("standalone-io-secrets");
    let runtime = super::web_io_config_integration_part_21::runtime_toml("runtime-a", "test", "");
    std::fs::write(project.join("runtime.toml"), runtime).unwrap();
    std::fs::write(project.join("io.toml"), MQTT_IO_TOML).unwrap();
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_config_ui(state, project.clone());
    let mut response = ureq::get(&format!(
        "{base}/api/config-ui/io/config?runtime_id=runtime-a"
    ))
    .call()
    .expect("read standalone io");
    let body = response.body_mut().read_to_string().unwrap();
    assert!(!body.contains("s3cret-mqtt"), "{body}");
    let loaded: Value = serde_json::from_str(&body).unwrap();
    let edited = loaded["text"]
        .as_str()
        .unwrap()
        .replace("trust/io/in", "trust/io/edited");
    let mut response = ureq::post(&format!("{base}/api/config-ui/io/config"))
        .header("Content-Type", "application/json")
        .send(json!({ "runtime_id": "runtime-a", "text": edited, "expected_revision": loaded["revision"] }).to_string())
        .expect("save standalone io");
    let result = response.body_mut().read_to_string().unwrap();
    assert!(result.contains("io.toml saved"), "{result}");
    let saved = std::fs::read_to_string(project.join("io.toml")).unwrap();
    assert!(saved.contains("s3cret-mqtt"));
    assert!(saved.contains("trust/io/edited"));
    let _ = std::fs::remove_dir_all(project);
}
