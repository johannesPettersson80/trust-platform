use super::*;

#[test]
fn runtime_cloud_preflight_denies_cmd_invoke_for_viewer_with_permission_denied_code() {
    let project = make_project("runtime-cloud-preflight-viewer-cmd");
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
        "request_id": "req-preflight-cmd-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/viewer-1",
        "action_type": "cmd_invoke",
        "dry_run": true,
        "payload": {
            "command": "restart",
            "params": { "mode": "warm" }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .header("X-Trust-Token", viewer_token.as_str())
        .send(&payload.to_string())
        .expect("runtime cloud preflight cmd viewer response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud preflight cmd viewer body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud preflight cmd viewer");

    assert_eq!(
        response.get("allowed").and_then(Value::as_bool),
        Some(false),
        "viewer must not be allowed to invoke protected command actions"
    );
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("permission_denied")
    );
    assert_eq!(
        response
            .get("decisions")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("denial_code"))
            .and_then(Value::as_str),
        Some("permission_denied")
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_dispatch_routes_cfg_apply_to_local_runtime() {
    let project = make_project("runtime-cloud-dispatch");
    let state = control_state(source_fixture());
    let base = start_test_server(state, project.clone());

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-dispatch-1",
        "connected_via": "RESOURCE",
        "target_runtimes": ["RESOURCE"],
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
        .expect("runtime cloud dispatch response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud dispatch body");
    let response: Value = serde_json::from_str(&body).expect("parse runtime cloud dispatch");

    assert_eq!(
        response.get("ok").and_then(Value::as_bool),
        Some(true),
        "local cfg_apply dispatch should succeed for local auth admin role"
    );
    assert_eq!(
        response
            .get("preflight")
            .and_then(|item| item.get("allowed"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("audit_id"))
            .and_then(Value::as_str)
            .is_some(),
        "dispatch response must surface per-target audit_id"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_dispatch_reaches_remote_runtime_and_propagates_audit_correlation_id() {
    let project_a = make_project("runtime-cloud-dispatch-remote-a");
    let project_b = make_project("runtime-cloud-dispatch-remote-b");

    let state_b_source = source_fixture();
    let (audit_tx, audit_rx) = std::sync::mpsc::channel::<ControlAuditEvent>();
    let state_b = control_state_named_with_audit(state_b_source, "runtime-b", Some(audit_tx));
    let base_b = start_test_server(state_b, project_b.clone());
    let port_b = parse_base_port(&base_b);

    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("runtime-b"),
        name: SmolStr::new("runtime-b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(port_b),
        web_tls: false,
        mesh_port: Some(5200),
        control: Some(SmolStr::new("tcp://127.0.0.1:5201")),
        host_group: None,
        site: None,
        last_seen_ns: now_ns(),
    }]);

    let state_a = control_state_named(source_fixture(), "runtime-a");
    let base_a = start_test_server_with_discovery(state_a, project_a.clone(), Some(discovery));

    let request_id = "req-dispatch-remote-1";
    let payload = json!({
        "api_version": "1.0",
        "request_id": request_id,
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/engineer-1",
        "action_type": "cfg_apply",
        "dry_run": false,
        "payload": {
            "params": { "log.level": "debug" }
        }
    });
    let response_body = ureq::post(&format!("{base_a}/api/runtime-cloud/actions/dispatch"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud remote dispatch response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud remote dispatch body");
    let response: Value = serde_json::from_str(&response_body).expect("parse dispatch response");

    assert_eq!(
        response.get("ok").and_then(Value::as_bool),
        Some(true),
        "expected remote dispatch success"
    );
    assert_eq!(
        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("runtime_id"))
            .and_then(Value::as_str),
        Some("runtime-b")
    );
    let response_audit_id = response
        .get("results")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("audit_id"))
        .and_then(Value::as_str)
        .expect("remote dispatch response audit_id")
        .to_string();

    let remote_audit = recv_audit_event(&audit_rx);
    assert_eq!(remote_audit.request_type.as_str(), "config.set");
    assert_eq!(remote_audit.event_id.as_str(), response_audit_id.as_str());
    assert_eq!(
        remote_audit.correlation_id.as_deref(),
        Some(request_id),
        "runtime-cloud request_id must propagate into control audit correlation_id"
    );

    let remote_config_body = ureq::post(&format!("{base_b}/api/control"))
        .header("Content-Type", "application/json")
        .send(r#"{"id":11,"type":"config.get"}"#)
        .expect("query remote config")
        .body_mut()
        .read_to_string()
        .expect("read remote config response");
    let remote_config: Value = serde_json::from_str(&remote_config_body).expect("parse config.get");
    assert!(remote_config
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or(false));
    assert_eq!(
        remote_config
            .get("result")
            .and_then(|value| value.get("log.level"))
            .and_then(Value::as_str),
        Some("debug"),
        "remote runtime should apply forwarded cfg_apply settings"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}
