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
