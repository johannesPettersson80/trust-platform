use super::*;

#[test]
fn runtime_cloud_dispatch_reads_remote_runtime_status_via_connected_runtime() {
    let project_a = make_project("runtime-cloud-read-remote-a");
    let project_b = make_project("runtime-cloud-read-remote-b");

    let state_b = control_state_named(source_fixture(), "runtime-b");
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

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-read-remote-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/operator-1",
        "action_type": "status_read",
        "dry_run": false,
        "payload": {}
    });
    let response_body = ureq::post(&format!("{base_a}/api/runtime-cloud/actions/dispatch"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud remote read response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud remote read body");
    let response: Value = serde_json::from_str(&response_body).expect("parse remote read response");

    assert_eq!(
        response.get("ok").and_then(Value::as_bool),
        Some(true),
        "expected status_read dispatch success"
    );
    assert_eq!(
        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("response"))
            .and_then(|item| item.get("ok"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("response"))
            .and_then(|item| item.get("result"))
            .and_then(|item| item.get("plc_name"))
            .and_then(Value::as_str),
        Some("runtime-b"),
        "dispatch response should include remote runtime status payload"
    );
    assert!(
        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("audit_id"))
            .and_then(Value::as_str)
            .is_some(),
        "status_read dispatch should return audit_id for UI timeline linking"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}

#[test]
fn runtime_cloud_control_proxy_reads_remote_runtime_status() {
    let project_a = make_project("runtime-cloud-proxy-read-remote-a");
    let project_b = make_project("runtime-cloud-proxy-read-remote-b");

    let state_b = control_state_named(source_fixture(), "runtime-b");
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

    let payload = json!({
        "api_version": "1.0",
        "actor": "spiffe://trust/default-site/operator-1",
        "target_runtime": "runtime-b",
        "control_request": {
            "type": "status",
            "request_id": "req-proxy-status-1"
        }
    });
    let response_body = ureq::post(&format!("{base_a}/api/runtime-cloud/control/proxy"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud control proxy status response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud control proxy body");
    let response: Value =
        serde_json::from_str(&response_body).expect("parse control proxy response");

    assert_eq!(
        response.get("ok").and_then(Value::as_bool),
        Some(true),
        "expected control proxy status success"
    );
    assert_eq!(
        response
            .get("result")
            .and_then(|item| item.get("plc_name"))
            .and_then(Value::as_str),
        Some("runtime-b"),
        "proxy response should include remote runtime status payload"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}

#[test]
fn runtime_cloud_io_config_proxy_reads_remote_runtime_config() {
    let project_a = make_project("runtime-cloud-io-read-remote-a");
    let project_b = make_project("runtime-cloud-io-read-remote-b");

    let state_b = control_state_named(source_fixture(), "runtime-b");
    let base_b = start_test_server(state_b, project_b.clone());
    let port_b = parse_base_port(&base_b);

    let seed_payload = json!({
        "drivers": [{
            "name": "modbus-tcp",
            "params": {
                "address": "192.168.10.5",
                "port": 502,
                "unit_id": 1,
                "timeout_ms": 350
            }
        }]
    });
    let seed_response = ureq::post(&format!("{base_b}/api/io/config"))
        .header("Content-Type", "application/json")
        .send(&seed_payload.to_string())
        .expect("seed remote io config")
        .body_mut()
        .read_to_string()
        .expect("read remote io save response");
    assert!(
        seed_response.contains("I/O config saved"),
        "expected seed save confirmation, got: {seed_response}"
    );

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

    let body = ureq::get(&format!(
        "{base_a}/api/runtime-cloud/io/config?target=runtime-b"
    ))
    .call()
    .expect("runtime cloud io config proxy read")
    .body_mut()
    .read_to_string()
    .expect("read runtime cloud io config proxy body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud io config proxy read");

    assert_eq!(
        response.get("driver").and_then(Value::as_str),
        Some("modbus-tcp"),
        "proxy read should surface remote primary driver"
    );
    assert_eq!(
        response
            .get("params")
            .and_then(|value| value.get("address"))
            .and_then(Value::as_str),
        Some("192.168.10.5"),
        "proxy read should surface remote driver params"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}
