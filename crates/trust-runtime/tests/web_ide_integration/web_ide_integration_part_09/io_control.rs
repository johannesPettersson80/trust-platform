use super::super::*;

#[test]
fn unified_shell_ide_io_config_route_tracks_active_workspace() {
    let project_a = make_project("ide-io-config-a");
    std::fs::write(
        project_a.join("io.toml"),
        "[io]\ndriver = \"simulated\"\n\n[io.params]\n\n[[io.safe_state]]\naddress = \"%QX0.0\"\nvalue = \"FALSE\"\n",
    )
    .expect("write project A io.toml");

    let project_b = make_project("ide-io-config-b");
    std::fs::write(
        project_b.join("io.toml"),
        "[io]\ndriver = \"mqtt\"\n\n[io.params]\nbroker = \"127.0.0.1:1883\"\ntopic_in = \"trust/in\"\ntopic_out = \"trust/out\"\n\n[[io.safe_state]]\naddress = \"%QX0.0\"\nvalue = \"FALSE\"\n",
    )
    .expect("write project B io.toml");

    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project_a.clone(), WebAuthMode::Local);

    let (_, session) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "viewer" })),
        &[],
    );
    let token = session["result"]["token"]
        .as_str()
        .expect("session token should exist")
        .to_string();
    let headers = [("X-Trust-Ide-Session", token.as_str())];

    let (open_status, open_body) = request_json(
        "POST",
        &format!("{base}/api/ide/project/open"),
        Some(json!({ "path": project_b.to_string_lossy() })),
        &headers,
    );
    assert_eq!(open_status, 200, "project open should succeed: {open_body}");

    let (cfg_status, cfg_body) =
        request_json("GET", &format!("{base}/api/ide/io/config"), None, &headers);
    assert_eq!(
        cfg_status, 200,
        "io config route should succeed: {cfg_body}"
    );
    assert_eq!(
        cfg_body["result"]["driver"].as_str(),
        Some("mqtt"),
        "active workspace io.toml must drive IDE io config payload"
    );
    let drivers = cfg_body["result"]["drivers"]
        .as_array()
        .expect("drivers list should be array");
    assert!(
        drivers
            .iter()
            .any(|entry| entry["name"].as_str() == Some("mqtt")),
        "drivers payload should include mqtt from project B"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}

#[test]
fn unified_shell_control_proxy_supports_runtime_status_forwarding() {
    let local_project = make_project("control-proxy-local");
    let remote_project = make_project("control-proxy-remote");

    let local_state = control_state(source_fixture(), ControlMode::Debug, None);
    let remote_state = control_state(source_fixture(), ControlMode::Debug, None);
    let local_base = start_test_server(local_state, local_project.clone(), WebAuthMode::Local);
    let remote_base = start_test_server(remote_state, remote_project.clone(), WebAuthMode::Local);

    let (status, body) = request_json(
        "POST",
        &format!("{local_base}/api/control/proxy"),
        Some(json!({
            "target": remote_base,
            "control_request": {
                "id": 1,
                "type": "status"
            }
        })),
        &[],
    );
    assert_eq!(status, 200, "proxy status call should succeed: {body}");
    assert_eq!(body["ok"], json!(true));
    assert!(
        body["result"].is_object(),
        "proxied status must include result payload"
    );

    let _ = std::fs::remove_dir_all(local_project);
    let _ = std::fs::remove_dir_all(remote_project);
}

#[test]
fn unified_shell_ide_io_config_post_writes_active_workspace_io_file() {
    let project = make_project("ide-io-config-post");
    std::fs::write(
        project.join("io.toml"),
        "[io]\ndriver = \"simulated\"\n\n[io.params]\n\n[[io.safe_state]]\naddress = \"%QX0.0\"\nvalue = \"FALSE\"\n",
    )
    .expect("write initial io.toml");

    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let (_, session) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    let token = session["result"]["token"]
        .as_str()
        .expect("session token should exist")
        .to_string();
    let headers = [("X-Trust-Ide-Session", token.as_str())];

    let (save_status, save_body) = request_json(
        "POST",
        &format!("{base}/api/ide/io/config"),
        Some(json!({
            "drivers": [
                {
                    "name": "mqtt",
                    "params": {
                        "broker": "10.0.0.10:1883",
                        "topic_in": "factory/in",
                        "topic_out": "factory/out"
                    }
                }
            ],
            "safe_state": [
                {
                    "address": "%QX0.0",
                    "value": "FALSE"
                }
            ],
            "use_system_io": false
        })),
        &headers,
    );
    assert_eq!(
        save_status, 200,
        "io config save must succeed through /api/ide/io/config: {save_body}"
    );
    assert_eq!(save_body["ok"], json!(true));

    let io_text = std::fs::read_to_string(project.join("io.toml")).expect("read io.toml");
    assert!(
        io_text.contains("driver = \"mqtt\""),
        "saved io.toml should contain mqtt driver: {io_text}"
    );
    assert!(
        io_text.contains("broker = \"10.0.0.10:1883\""),
        "saved io.toml should contain updated broker: {io_text}"
    );
    assert!(
        io_text.contains("topic_in = \"factory/in\""),
        "saved io.toml should contain updated topic_in: {io_text}"
    );
    assert!(
        io_text.contains("topic_out = \"factory/out\""),
        "saved io.toml should contain updated topic_out: {io_text}"
    );

    let _ = std::fs::remove_dir_all(project);
}
