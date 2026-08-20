use super::*;

#[test]
fn runtime_cloud_proxy_planners_reject_malformed_requests_as_bad_requests() {
    let project = make_project("runtime-cloud-proxy-contract-rejection");
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server(state, project.clone());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();

    let blank_control_request_id = json!({
        "api_version": "1.0",
        "actor": "spiffe://trust/default-site/operator-1",
        "target_runtime": "runtime-a",
        "control_request": {
            "type": "status",
            "request_id": " "
        }
    });
    let mut control_response = agent
        .post(&format!("{base}/api/runtime-cloud/control/proxy"))
        .header("Content-Type", "application/json")
        .send(&blank_control_request_id.to_string())
        .expect("blank control request ID response");
    assert_eq!(control_response.status().as_u16(), 400);
    let control_body: Value = serde_json::from_str(
        &control_response
            .body_mut()
            .read_to_string()
            .expect("read control proxy rejection"),
    )
    .expect("parse control proxy rejection");
    assert_eq!(
        control_body.get("denial_code").and_then(Value::as_str),
        Some("contract_violation")
    );

    let breaking_io_request = json!({
        "api_version": "2.0",
        "actor": "spiffe://trust/default-site/engineer-1",
        "target_runtime": "runtime-a",
        "drivers": []
    });
    let mut io_response = agent
        .post(&format!("{base}/api/runtime-cloud/io/config"))
        .header("Content-Type", "application/json")
        .send(&breaking_io_request.to_string())
        .expect("breaking I/O proxy API response");
    assert_eq!(io_response.status().as_u16(), 400);
    let io_body: Value = serde_json::from_str(
        &io_response
            .body_mut()
            .read_to_string()
            .expect("read I/O proxy rejection"),
    )
    .expect("parse I/O proxy rejection");
    assert_eq!(
        io_body.get("denial_code").and_then(Value::as_str),
        Some("contract_violation")
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_io_config_proxy_writes_remote_runtime_config() {
    let project_a = make_project("runtime-cloud-io-write-remote-a");
    let project_b = make_project("runtime-cloud-io-write-remote-b");

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
        last_seen_ns: now_ns(),
    }]);

    let state_a = control_state_named(source_fixture(), "runtime-a");
    let base_a = start_test_server_with_discovery(state_a, project_a.clone(), Some(discovery));

    let payload = json!({
        "api_version": "1.0",
        "actor": "spiffe://trust/default-site/engineer-1",
        "target_runtime": "runtime-b",
        "drivers": [{
            "name": "mqtt",
            "params": {
                "broker": "127.0.0.1:1883",
                "topic_in": "trust/io/in",
                "topic_out": "trust/io/out"
            }
        }]
    });
    let write_body = ureq::post(&format!("{base_a}/api/runtime-cloud/io/config"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud io config proxy write")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud io config proxy write body");
    let write_response: Value =
        serde_json::from_str(&write_body).expect("parse runtime cloud io config write response");

    assert_eq!(
        write_response.get("ok").and_then(Value::as_bool),
        Some(true),
        "proxy write should report success"
    );
    assert!(
        write_response
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("saved"),
        "proxy write should return a save confirmation message"
    );

    let remote_body = ureq::get(&format!("{base_b}/api/io/config"))
        .call()
        .expect("read remote io config after proxy write")
        .body_mut()
        .read_to_string()
        .expect("read remote io config response");
    let remote_response: Value =
        serde_json::from_str(&remote_body).expect("parse remote io config response");
    assert_eq!(
        remote_response.get("driver").and_then(Value::as_str),
        Some("mqtt"),
        "remote runtime should apply forwarded io config"
    );
    assert_eq!(
        remote_response
            .get("params")
            .and_then(|value| value.get("broker"))
            .and_then(Value::as_str),
        Some("127.0.0.1:1883"),
        "remote runtime should persist forwarded driver params"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}

#[test]
fn runtime_cloud_remote_dispatch_emits_audit_for_success_and_failure_paths() {
    let project_a = make_project("runtime-cloud-audit-remote-a");
    let project_b = make_project("runtime-cloud-audit-remote-b");

    let (audit_tx, audit_rx) = std::sync::mpsc::channel::<ControlAuditEvent>();
    let state_b = control_state_named_with_audit(source_fixture(), "runtime-b", Some(audit_tx));
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
        last_seen_ns: now_ns(),
    }]);

    let state_a = control_state_named(source_fixture(), "runtime-a");
    let base_a = start_test_server_with_discovery(state_a, project_a.clone(), Some(discovery));

    let success_request = json!({
        "api_version": "1.0",
        "request_id": "req-audit-ok",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/engineer-1",
        "action_type": "cfg_apply",
        "dry_run": false,
        "payload": { "params": { "log.level": "debug" } }
    });
    let success_body = ureq::post(&format!("{base_a}/api/runtime-cloud/actions/dispatch"))
        .header("Content-Type", "application/json")
        .send(&success_request.to_string())
        .expect("dispatch success request")
        .body_mut()
        .read_to_string()
        .expect("read success dispatch response");
    let success_response: Value =
        serde_json::from_str(&success_body).expect("parse success dispatch");
    assert_eq!(
        success_response.get("ok").and_then(Value::as_bool),
        Some(true)
    );
    let success_audit_id = success_response
        .get("results")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("audit_id"))
        .and_then(Value::as_str)
        .expect("success dispatch audit_id")
        .to_string();

    let failure_request = json!({
        "api_version": "1.0",
        "request_id": "req-audit-fail",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/engineer-1",
        "action_type": "cfg_apply",
        "dry_run": false,
        "payload": { "params": { "runtime.invalid_setting": true } }
    });
    let failure_body = ureq::post(&format!("{base_a}/api/runtime-cloud/actions/dispatch"))
        .header("Content-Type", "application/json")
        .send(&failure_request.to_string())
        .expect("dispatch failure request")
        .body_mut()
        .read_to_string()
        .expect("read failure dispatch response");
    let failure_response: Value =
        serde_json::from_str(&failure_body).expect("parse failure dispatch");
    assert_eq!(
        failure_response.get("ok").and_then(Value::as_bool),
        Some(false)
    );
    let failure_audit_id = failure_response
        .get("results")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("audit_id"))
        .and_then(Value::as_str)
        .expect("failure dispatch audit_id")
        .to_string();

    let first_audit = recv_audit_event(&audit_rx);
    let second_audit = recv_audit_event(&audit_rx);
    let audits = [first_audit, second_audit];

    let ok_event = audits
        .iter()
        .find(|event| event.correlation_id.as_deref() == Some("req-audit-ok"))
        .expect("ok audit event");
    assert_eq!(ok_event.request_type.as_str(), "config.set");
    assert_eq!(ok_event.event_id.as_str(), success_audit_id.as_str());
    assert!(ok_event.ok, "expected successful audit event");
    assert!(
        ok_event.error.is_none(),
        "successful audit must not contain error"
    );

    let fail_event = audits
        .iter()
        .find(|event| event.correlation_id.as_deref() == Some("req-audit-fail"))
        .expect("failure audit event");
    assert_eq!(fail_event.request_type.as_str(), "config.set");
    assert_eq!(fail_event.event_id.as_str(), failure_audit_id.as_str());
    assert!(!fail_event.ok, "expected failure audit event");
    assert!(
        fail_event
            .error
            .as_deref()
            .map(|error| error.contains("unknown config key"))
            .unwrap_or(false),
        "failure audit must include underlying config validation error"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}
