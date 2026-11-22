use super::*;

#[test]
fn runtime_cloud_dispatch_keeps_local_cfg_apply_operational_when_peer_is_partitioned() {
    let project = make_project("runtime-cloud-local-operational-partition");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("runtime-b"),
        name: SmolStr::new("runtime-b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(0),
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        site: None,
        last_seen_ns: 1,
    }]);
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_discovery(state, project.clone(), Some(discovery));

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-local-operational-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-a"],
        "actor": "spiffe://trust/default-site/engineer-1",
        "action_type": "cfg_apply",
        "dry_run": false,
        "payload": {
            "params": { "log.level": "debug" }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/dispatch"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud local dispatch response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud local dispatch response");
    let response: Value = serde_json::from_str(&body).expect("parse runtime cloud local dispatch");
    assert_eq!(response.get("ok").and_then(Value::as_bool), Some(true));

    let config_body = ureq::post(&format!("{base}/api/control"))
        .header("Content-Type", "application/json")
        .send(r#"{"id":211,"type":"config.get"}"#)
        .expect("runtime cloud local config.get response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud local config.get response");
    let config: Value = serde_json::from_str(&config_body).expect("parse local config.get");
    assert_eq!(
        config
            .get("result")
            .and_then(|value| value.get("log.level"))
            .and_then(Value::as_str),
        Some("debug"),
        "local config writes should remain operational when peer discovery is partitioned"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_preflight_marks_partial_partition_target_as_stale() {
    let target_runtime = "runtime-partition-target-15";
    let project = make_project("runtime-cloud-preflight-partial-partition");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new(target_runtime),
        name: SmolStr::new(target_runtime),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(0),
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        site: None,
        last_seen_ns: now_ns(),
    }]);
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_discovery(state, project.clone(), Some(discovery));

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-partial-partition-1",
        "connected_via": "runtime-a",
        "target_runtimes": [target_runtime],
        "actor": "spiffe://trust/default-site/operator-1",
        "action_type": "status_read",
        "dry_run": true,
        "payload": {}
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud partition preflight response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud partition preflight response");
    let response: Value = serde_json::from_str(&body).expect("parse partition preflight");

    assert_eq!(
        response.get("allowed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("stale_data")
    );
    assert_eq!(
        response
            .get("decisions")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|decision| decision.get("denial_code"))
            .and_then(Value::as_str),
        Some("stale_data")
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_preflight_denies_cfg_apply_for_viewer_with_deterministic_acl_code() {
    let project = make_project("runtime-cloud-preflight-viewer-acl");
    let pairing_path = project.join("pairings.json");
    let (pairing, viewer_token) = create_pairing_token(pairing_path, AccessRole::Viewer);

    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("runtime-b"),
        name: SmolStr::new("runtime-b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(8080),
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        site: None,
        last_seen_ns: now_ns(),
    }]);

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_options(
        state,
        project.clone(),
        Some(discovery),
        Some(pairing),
        WebAuthMode::Token,
    );

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-preflight-acl-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/viewer-1",
        "action_type": "cfg_apply",
        "dry_run": true,
        "payload": {
            "params": { "log.level": "debug" }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .header("X-Trust-Token", viewer_token.as_str())
        .send(&payload.to_string())
        .expect("runtime cloud preflight viewer response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud preflight viewer body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud preflight viewer");

    assert_eq!(
        response.get("allowed").and_then(Value::as_bool),
        Some(false),
        "viewer must not be allowed to run cfg_apply"
    );
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("acl_denied_cfg_write")
    );
    assert_eq!(
        response
            .get("decisions")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("denial_code"))
            .and_then(Value::as_str),
        Some("acl_denied_cfg_write")
    );

    let _ = std::fs::remove_dir_all(project);
}
