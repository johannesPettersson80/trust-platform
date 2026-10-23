use super::*;
use std::path::Path;

pub(super) fn runtime_toml(runtime_id: &str, host_group: &str, links: &str) -> String {
    format!(
        r#"
[bundle]
version = 1

[resource]
name = "{runtime_id}"
cycle_interval_ms = 20

[runtime.control]
endpoint = "unix:///tmp/{runtime_id}.sock"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"

[runtime.web]
enabled = true
listen = "127.0.0.1:0"
auth = "local"
tls = false

[runtime.discovery]
enabled = true
service_name = "{runtime_id}"
advertise = true
interfaces = ["lo"]
host_group = "{host_group}"

[runtime.mesh]
enabled = true
role = "peer"
listen = "127.0.0.1:0"
connect = []
tls = false
publish = []
subscribe = {{}}

[runtime.cloud]
profile = "dev"

[runtime.cloud.wan]
allow_write = []

[runtime.cloud.links]
transports = [{links}]
"#,
    )
}

fn io_toml(driver_name: &str, params: &str) -> String {
    format!(
        r#"
[io]
drivers = [
  {{ name = "{driver_name}", params = {params} }}
]
"#
    )
}

fn create_runtime_project(root: &Path, folder: &str, runtime_text: &str, io_text: &str) {
    let runtime_root = root.join(folder);
    std::fs::create_dir_all(runtime_root.join("src")).expect("create runtime src");
    std::fs::write(runtime_root.join("runtime.toml"), runtime_text).expect("write runtime.toml");
    std::fs::write(runtime_root.join("io.toml"), io_text).expect("write io.toml");
    std::fs::write(
        runtime_root.join("src/main.st"),
        "PROGRAM Main\nVAR\n  x : INT := 0;\nEND_VAR\nEND_PROGRAM\n",
    )
    .expect("write src/main.st");
}

#[test]
fn config_ui_mode_serves_project_state_and_topology_projection() {
    let workspace = make_project("config-ui-project-state");
    create_runtime_project(
        &workspace,
        "runtime-a",
        runtime_toml(
            "runtime-a",
            "cell-a-edge-ipc",
            r#"{ source = "runtime-a", target = "runtime-b", transport = "zenoh" }"#,
        )
        .as_str(),
        io_toml("mqtt", r#"{ broker = "10.24.10.50:1883" }"#).as_str(),
    );
    create_runtime_project(
        &workspace,
        "runtime-b",
        runtime_toml(
            "runtime-b",
            "cell-b-edge-ipc",
            r#"{ source = "runtime-b", target = "runtime-a", transport = "realtime" }"#,
        )
        .as_str(),
        io_toml(
            "modbus-tcp",
            r#"{ address = "10.42.0.20", port = 1502, unit_id = 2 }"#,
        )
        .as_str(),
    );

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_config_ui(state, workspace.clone());

    let mode_body = ureq::get(&format!("{base}/api/ui/mode"))
        .call()
        .expect("ui mode response")
        .body_mut()
        .read_to_string()
        .expect("read ui mode body");
    let mode: Value = serde_json::from_str(&mode_body).expect("parse ui mode body");
    assert_eq!(
        mode.get("mode").and_then(Value::as_str),
        Some("standalone-ide")
    );

    let state_body = ureq::get(&format!("{base}/api/config-ui/project/state"))
        .call()
        .expect("project state response")
        .body_mut()
        .read_to_string()
        .expect("read project state body");
    let project_state: Value = serde_json::from_str(&state_body).expect("parse project state");
    assert_eq!(project_state.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        project_state
            .get("runtimes")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(2)
    );

    let topology_body = ureq::get(&format!("{base}/api/runtime-cloud/state"))
        .call()
        .expect("runtime cloud state response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud state body");
    let topology: Value = serde_json::from_str(&topology_body).expect("parse runtime cloud state");
    assert_eq!(
        topology.get("api_version").and_then(Value::as_str),
        Some("1.0")
    );
    assert_eq!(
        topology
            .get("topology")
            .and_then(|value| value.get("nodes"))
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(2)
    );
    assert_eq!(
        topology
            .get("topology")
            .and_then(|value| value.get("host_groups"))
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(2)
    );
    let node_states = topology
        .get("topology")
        .and_then(|value| value.get("nodes"))
        .and_then(Value::as_array)
        .expect("topology nodes");
    assert!(
        node_states.iter().all(|node| {
            node.get("lifecycle_state").and_then(Value::as_str) == Some("offline")
                && node.get("health_state").and_then(Value::as_str) == Some("degraded")
        }),
        "config-ui topology should render planned runtimes as offline/degraded",
    );
    let edge_states = topology
        .get("topology")
        .and_then(|value| value.get("edges"))
        .and_then(Value::as_array)
        .expect("topology edges");
    assert!(
        edge_states
            .iter()
            .all(|edge| edge.get("state").and_then(Value::as_str) == Some("failed")),
        "config-ui topology should render links as failed until live connect",
    );

    let config_body = ureq::get(&format!("{base}/api/runtime-cloud/config"))
        .call()
        .expect("runtime cloud config response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud config body");
    let config: Value = serde_json::from_str(&config_body).expect("parse runtime cloud config");
    assert_eq!(
        config.get("api_version").and_then(Value::as_str),
        Some("1.0")
    );
    assert_eq!(
        config.get("runtime_id").and_then(Value::as_str),
        Some("runtime-a")
    );

    let rollouts_body = ureq::get(&format!("{base}/api/runtime-cloud/rollouts"))
        .call()
        .expect("runtime cloud rollouts response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud rollouts body");
    let rollouts: Value =
        serde_json::from_str(&rollouts_body).expect("parse runtime cloud rollouts");
    assert_eq!(
        rollouts.get("api_version").and_then(Value::as_str),
        Some("1.0")
    );
    assert_eq!(
        rollouts
            .get("items")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(0)
    );

    let io_body = ureq::get(&format!(
        "{base}/api/runtime-cloud/io/config?target={}",
        urlencoding::encode("runtime-b")
    ))
    .call()
    .expect("runtime cloud io response")
    .body_mut()
    .read_to_string()
    .expect("read runtime cloud io body");
    let io_value: Value = serde_json::from_str(&io_body).expect("parse runtime cloud io body");
    assert_eq!(
        io_value
            .get("drivers")
            .and_then(Value::as_array)
            .and_then(|drivers| drivers.first())
            .and_then(|driver| driver.get("name"))
            .and_then(Value::as_str),
        Some("modbus-tcp")
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn config_ui_runtime_config_write_conflict_is_reported() {
    let workspace = make_project("config-ui-runtime-write");
    create_runtime_project(
        &workspace,
        "runtime-a",
        runtime_toml("runtime-a", "cell-a-edge-ipc", "").as_str(),
        io_toml("mqtt", r#"{ broker = "10.24.10.50:1883" }"#).as_str(),
    );

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_config_ui(state, workspace.clone());

    let get_body = ureq::get(&format!(
        "{base}/api/config-ui/runtime/config?runtime_id={}",
        urlencoding::encode("runtime-a")
    ))
    .call()
    .expect("get runtime config")
    .body_mut()
    .read_to_string()
    .expect("read runtime config body");
    let get_value: Value = serde_json::from_str(&get_body).expect("parse runtime config body");
    let revision = get_value
        .get("revision")
        .and_then(Value::as_str)
        .expect("revision");
    let text = get_value
        .get("text")
        .and_then(Value::as_str)
        .expect("runtime text");

    let stale_payload = json!({
        "runtime_id": "runtime-a",
        "text": text,
        "expected_revision": "stale-revision",
    });
    let stale_client_config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build();
    let stale_client: ureq::Agent = stale_client_config.into();
    let mut stale_response = stale_client
        .post(&format!("{base}/api/config-ui/runtime/config"))
        .header("Content-Type", "application/json")
        .send(&stale_payload.to_string())
        .expect("post stale runtime config");
    assert_eq!(stale_response.status(), 409);
    let stale = stale_response
        .body_mut()
        .read_to_string()
        .expect("read stale runtime config body");
    let stale_value: Value = serde_json::from_str(&stale).expect("parse stale runtime config");
    assert_eq!(stale_value.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        stale_value.get("error_code").and_then(Value::as_str),
        Some("conflict")
    );

    let next_text = text.replace("cycle_interval_ms = 20", "cycle_interval_ms = 25");
    let write_payload = json!({
        "runtime_id": "runtime-a",
        "text": next_text,
        "expected_revision": revision,
    });
    let write_body = ureq::post(&format!("{base}/api/config-ui/runtime/config"))
        .header("Content-Type", "application/json")
        .send(&write_payload.to_string())
        .expect("write runtime config")
        .body_mut()
        .read_to_string()
        .expect("read write runtime config body");
    let write_value: Value = serde_json::from_str(&write_body).expect("parse write runtime config");
    assert_eq!(write_value.get("ok").and_then(Value::as_bool), Some(true));

    let saved = std::fs::read_to_string(workspace.join("runtime-a/runtime.toml"))
        .expect("read updated runtime.toml");
    assert!(saved.contains("cycle_interval_ms = 25"));

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn config_ui_st_file_write_and_validate_roundtrip() {
    let workspace = make_project("config-ui-st-write");
    create_runtime_project(
        &workspace,
        "runtime-a",
        runtime_toml("runtime-a", "cell-a-edge-ipc", "").as_str(),
        io_toml("mqtt", r#"{ broker = "10.24.10.50:1883" }"#).as_str(),
    );

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_config_ui(state, workspace.clone());

    let files_body = ureq::get(&format!(
        "{base}/api/config-ui/st/files?runtime_id={}",
        urlencoding::encode("runtime-a")
    ))
    .call()
    .expect("list st files")
    .body_mut()
    .read_to_string()
    .expect("read st files body");
    let files_value: Value = serde_json::from_str(&files_body).expect("parse st files body");
    let file_entry = files_value
        .get("files")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("path").and_then(Value::as_str) == Some("main.st"))
        })
        .expect("main.st entry");
    let revision = file_entry
        .get("revision")
        .and_then(Value::as_str)
        .expect("main.st revision")
        .to_string();

    let write_payload = json!({
        "runtime_id": "runtime-a",
        "path": "main.st",
        "text": "PROGRAM Main\nVAR\n  x : INT := 42;\nEND_VAR\nEND_PROGRAM\n",
        "expected_revision": revision,
    });
    let write_body = ureq::post(&format!("{base}/api/config-ui/st/file"))
        .header("Content-Type", "application/json")
        .send(&write_payload.to_string())
        .expect("write st file")
        .body_mut()
        .read_to_string()
        .expect("read st write body");
    let write_value: Value = serde_json::from_str(&write_body).expect("parse st write body");
    assert_eq!(write_value.get("ok").and_then(Value::as_bool), Some(true));

    let validate_ok_payload = json!({
        "runtime_id": "runtime-a"
    });
    let validate_ok = ureq::post(&format!("{base}/api/config-ui/st/validate"))
        .header("Content-Type", "application/json")
        .send(&validate_ok_payload.to_string())
        .expect("validate st")
        .body_mut()
        .read_to_string()
        .expect("read st validate body");
    let validate_ok_value: Value =
        serde_json::from_str(&validate_ok).expect("parse st validate body");
    assert_eq!(
        validate_ok_value.get("ok").and_then(Value::as_bool),
        Some(true)
    );

    let validate_err_payload = json!({
        "runtime_id": "runtime-a",
        "path": "main.st",
        "text": "PROGRAM Main\nVAR\n  x : INT := ;\nEND_VAR\nEND_PROGRAM\n"
    });
    let validate_err_client_config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build();
    let validate_err_client: ureq::Agent = validate_err_client_config.into();
    let mut validate_err_response = validate_err_client
        .post(&format!("{base}/api/config-ui/st/validate"))
        .header("Content-Type", "application/json")
        .send(&validate_err_payload.to_string())
        .expect("post broken st validate payload");
    assert_eq!(validate_err_response.status(), 400);
    let validate_err = validate_err_response
        .body_mut()
        .read_to_string()
        .expect("read broken st validate body");
    let validate_err_value: Value =
        serde_json::from_str(&validate_err).expect("parse broken st validate body");
    assert_eq!(
        validate_err_value.get("ok").and_then(Value::as_bool),
        Some(false)
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn config_ui_runtime_create_and_delete_roundtrip() {
    let workspace = make_project("config-ui-runtime-create-delete");
    create_runtime_project(
        &workspace,
        "runtime-a",
        runtime_toml("runtime-a", "cell-a-edge-ipc", "").as_str(),
        io_toml("mqtt", r#"{ broker = "10.24.10.50:1883" }"#).as_str(),
    );

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_config_ui(state, workspace.clone());

    let create_payload = json!({
        "runtime_id": "runtime-b",
        "host_group": "cell-b-edge-ipc"
    });
    let create_body = ureq::post(&format!("{base}/api/config-ui/runtime/create"))
        .header("Content-Type", "application/json")
        .send(&create_payload.to_string())
        .expect("create runtime")
        .body_mut()
        .read_to_string()
        .expect("read create runtime body");
    let create_value: Value =
        serde_json::from_str(&create_body).expect("parse create runtime body");
    assert_eq!(create_value.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        create_value.get("runtime_id").and_then(Value::as_str),
        Some("runtime-b")
    );

    let created_runtime_toml = workspace.join("runtime-b/runtime.toml");
    assert!(created_runtime_toml.is_file());
    let runtime_toml_text =
        std::fs::read_to_string(&created_runtime_toml).expect("read created runtime.toml");
    assert!(runtime_toml_text.contains("name = \"runtime-b\""));
    assert!(runtime_toml_text.contains("host_group = \"cell-b-edge-ipc\""));

    let state_after_create = ureq::get(&format!("{base}/api/config-ui/project/state"))
        .call()
        .expect("project state after create")
        .body_mut()
        .read_to_string()
        .expect("read project state after create");
    let state_after_create_value: Value =
        serde_json::from_str(&state_after_create).expect("parse project state after create");
    assert_eq!(
        state_after_create_value
            .get("runtimes")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(2)
    );

    let delete_payload = json!({ "runtime_id": "runtime-b" });
    let delete_body = ureq::post(&format!("{base}/api/config-ui/runtime/delete"))
        .header("Content-Type", "application/json")
        .send(&delete_payload.to_string())
        .expect("delete runtime")
        .body_mut()
        .read_to_string()
        .expect("read delete runtime body");
    let delete_value: Value =
        serde_json::from_str(&delete_body).expect("parse delete runtime body");
    assert_eq!(delete_value.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        delete_value.get("runtime_id").and_then(Value::as_str),
        Some("runtime-b")
    );
    assert!(!workspace.join("runtime-b").exists());

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn config_ui_live_targets_and_live_state_endpoints_roundtrip() {
    let workspace = make_project("config-ui-live-targets");
    create_runtime_project(
        &workspace,
        "runtime-a",
        runtime_toml("runtime-a", "cell-a-edge-ipc", "").as_str(),
        io_toml("mqtt", r#"{ broker = "10.24.10.50:1883" }"#).as_str(),
    );

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_config_ui(state, workspace.clone());

    let initial_targets = ureq::get(&format!("{base}/api/config-ui/live/targets"))
        .call()
        .expect("get initial live targets")
        .body_mut()
        .read_to_string()
        .expect("read initial live targets body");
    let initial_targets_value: Value =
        serde_json::from_str(&initial_targets).expect("parse initial live targets body");
    assert_eq!(
        initial_targets_value.get("ok").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        initial_targets_value
            .get("profiles")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(0)
    );

    let upsert_payload = json!({
        "target": "http://127.0.0.1:65530",
        "label": "lab-a",
    });
    let upsert_body = ureq::post(&format!("{base}/api/config-ui/live/targets"))
        .header("Content-Type", "application/json")
        .send(&upsert_payload.to_string())
        .expect("upsert live target")
        .body_mut()
        .read_to_string()
        .expect("read upsert live target body");
    let upsert_value: Value =
        serde_json::from_str(&upsert_body).expect("parse upsert live target body");
    assert_eq!(upsert_value.get("ok").and_then(Value::as_bool), Some(true));

    let connect_payload = json!({
        "target": "http://127.0.0.1:65530",
    });
    let connect_body = ureq::post(&format!("{base}/api/config-ui/live/connect"))
        .header("Content-Type", "application/json")
        .send(&connect_payload.to_string())
        .expect("connect live target")
        .body_mut()
        .read_to_string()
        .expect("read connect live target body");
    let connect_value: Value =
        serde_json::from_str(&connect_body).expect("parse connect live target body");
    assert_eq!(
        connect_value.get("ok").and_then(Value::as_bool),
        Some(true),
        "connect endpoint should respond with transport status",
    );
    assert_eq!(
        connect_value.get("connected").and_then(Value::as_bool),
        Some(false),
        "unreachable target should report disconnected",
    );
    assert!(
        connect_value
            .get("last_error")
            .and_then(Value::as_str)
            .is_some(),
        "connect failure should include last_error",
    );

    let state_body = ureq::get(&format!("{base}/api/config-ui/live/state"))
        .call()
        .expect("get live state")
        .body_mut()
        .read_to_string()
        .expect("read live state body");
    let state_value: Value = serde_json::from_str(&state_body).expect("parse live state body");
    assert_eq!(state_value.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        state_value.get("connected").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        state_value.get("active_target").and_then(Value::as_str),
        Some("http://127.0.0.1:65530")
    );

    let remove_payload = json!({ "target": "http://127.0.0.1:65530" });
    let remove_body = ureq::post(&format!("{base}/api/config-ui/live/targets/remove"))
        .header("Content-Type", "application/json")
        .send(&remove_payload.to_string())
        .expect("remove live target")
        .body_mut()
        .read_to_string()
        .expect("read remove live target body");
    let remove_value: Value =
        serde_json::from_str(&remove_body).expect("parse remove live target body");
    assert_eq!(remove_value.get("ok").and_then(Value::as_bool), Some(true));

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn config_ui_runtime_lifecycle_endpoints_report_workspace_runtimes() {
    let workspace = make_project("config-ui-runtime-lifecycle");
    create_runtime_project(
        &workspace,
        "runtime-a",
        runtime_toml("runtime-a", "cell-a-edge-ipc", "").as_str(),
        io_toml("mqtt", r#"{ broker = "10.24.10.50:1883" }"#).as_str(),
    );

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_config_ui(state, workspace.clone());

    let lifecycle_body = ureq::get(&format!("{base}/api/config-ui/runtime/lifecycle"))
        .call()
        .expect("get runtime lifecycle")
        .body_mut()
        .read_to_string()
        .expect("read runtime lifecycle body");
    let lifecycle_value: Value =
        serde_json::from_str(&lifecycle_body).expect("parse runtime lifecycle body");
    assert_eq!(
        lifecycle_value.get("ok").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        lifecycle_value
            .get("items")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(1)
    );
    let runtime_item = lifecycle_value
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .expect("runtime lifecycle item");
    assert_eq!(
        runtime_item.get("runtime_id").and_then(Value::as_str),
        Some("runtime-a")
    );
    assert_eq!(
        runtime_item.get("managed").and_then(Value::as_bool),
        Some(false)
    );

    let status_payload = json!({
        "runtime_id": "runtime-a",
        "action": "status",
    });
    let status_body = ureq::post(&format!("{base}/api/config-ui/runtime/lifecycle"))
        .header("Content-Type", "application/json")
        .send(&status_payload.to_string())
        .expect("post runtime lifecycle status")
        .body_mut()
        .read_to_string()
        .expect("read runtime lifecycle status body");
    let status_value: Value =
        serde_json::from_str(&status_body).expect("parse runtime lifecycle status body");
    assert_eq!(status_value.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        status_value
            .get("result")
            .and_then(|value| value.get("result"))
            .and_then(Value::as_str),
        Some("stopped")
    );

    let _ = std::fs::remove_dir_all(workspace);
}
