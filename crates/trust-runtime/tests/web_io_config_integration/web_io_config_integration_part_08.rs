use super::*;

#[test]
fn runtime_cloud_state_marks_fresh_mesh_disconnect_as_stale_before_partitioned() {
    let project = make_project("runtime-cloud-state-stale-before-partition");
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
        last_seen_ns: now_ns(),
    }]);

    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_discovery(state, project.clone(), Some(discovery));

    let body = ureq::get(&format!("{base}/api/runtime-cloud/state"))
        .call()
        .expect("load runtime cloud state for stale peer")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud stale state body");
    let payload: Value = serde_json::from_str(&body).expect("parse runtime cloud stale json");

    let nodes = payload
        .get("topology")
        .and_then(|value| value.get("nodes"))
        .and_then(Value::as_array)
        .expect("topology.nodes");
    let runtime_b = nodes
        .iter()
        .find(|node| node.get("runtime_id").and_then(Value::as_str) == Some("runtime-b"))
        .expect("runtime-b node");
    assert_eq!(
        runtime_b.get("lifecycle_state").and_then(Value::as_str),
        Some("stale")
    );

    let edges = payload
        .get("topology")
        .and_then(|value| value.get("edges"))
        .and_then(Value::as_array)
        .expect("topology.edges");
    let edge = edges
        .iter()
        .find(|item| item.get("target").and_then(Value::as_str) == Some("runtime-b"))
        .expect("runtime-b edge");
    assert_eq!(edge.get("state").and_then(Value::as_str), Some("stale"));
    assert_eq!(edge.get("stale").and_then(Value::as_bool), Some(true));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_discovery_endpoint_exposes_secure_transport_metadata() {
    let project = make_project("runtime-cloud-discovery-web-tls");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("runtime-b"),
        name: SmolStr::new("runtime-b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(8080),
        web_tls: true,
        mesh_port: Some(5200),
        control: Some(SmolStr::new("tcp://127.0.0.1:5201")),
        host_group: None,
        last_seen_ns: now_ns(),
    }]);

    let state = control_state_named(source_fixture(), "runtime-a");
    let (pairing, token) = create_pairing_token(project.join("pairings.json"), AccessRole::Viewer);
    let base = start_test_server_with_options(
        state,
        project.clone(),
        Some(discovery),
        Some(pairing),
        WebAuthMode::Token,
    );
    // The runtime discovery route passes the web authentication (specification 11, runtime
    // discovery).
    let body = ureq::get(&format!("{base}/api/discovery"))
        .header("X-Trust-Token", token.as_str())
        .call()
        .expect("load discovery endpoint")
        .body_mut()
        .read_to_string()
        .expect("read discovery endpoint body");
    let payload: Value = serde_json::from_str(&body).expect("parse discovery endpoint json");
    assert_eq!(
        payload
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("web_tls"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        payload
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("last_seen_ns"))
            .and_then(Value::as_u64)
            .is_some(),
        "discovery endpoint should expose last_seen_ns for stale/partition transitions"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_config_agent_reconciles_desired_reported_meta_and_status() {
    let project = make_project("runtime-cloud-config-agent-reconcile");
    let state = control_state(source_fixture());
    let base = start_test_server(state, project.clone());

    let initial_body = ureq::get(&format!("{base}/api/runtime-cloud/config"))
        .call()
        .expect("load runtime cloud config")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud config body");
    let initial: Value =
        serde_json::from_str(&initial_body).expect("parse initial config snapshot");
    let expected_revision = initial
        .get("meta")
        .and_then(|value| value.get("desired_revision"))
        .and_then(Value::as_u64)
        .expect("initial desired revision");
    let expected_etag = initial
        .get("meta")
        .and_then(|value| value.get("desired_etag"))
        .and_then(Value::as_str)
        .expect("initial desired etag")
        .to_string();

    let desired_write = json!({
        "api_version": "1.0",
        "actor": "spiffe://trust/default-site/engineer-1",
        "expected_revision": expected_revision,
        "expected_etag": expected_etag,
        "desired": {
            "log.level": "debug"
        }
    });
    let write_body = ureq::post(&format!("{base}/api/runtime-cloud/config/desired"))
        .header("Content-Type", "application/json")
        .send(&desired_write.to_string())
        .expect("write desired config")
        .body_mut()
        .read_to_string()
        .expect("read desired write response");
    let write_response: Value = serde_json::from_str(&write_body).expect("parse desired write");
    assert_eq!(
        write_response.get("ok").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        write_response
            .get("status")
            .and_then(|value| value.get("state"))
            .and_then(Value::as_str),
        Some("pending")
    );

    let reconciled_body = ureq::get(&format!("{base}/api/runtime-cloud/config"))
        .call()
        .expect("load reconciled config")
        .body_mut()
        .read_to_string()
        .expect("read reconciled config");
    let reconciled: Value =
        serde_json::from_str(&reconciled_body).expect("parse reconciled config");
    assert_eq!(
        reconciled
            .get("status")
            .and_then(|value| value.get("state"))
            .and_then(Value::as_str),
        Some("in_sync")
    );
    assert_eq!(
        reconciled
            .get("meta")
            .and_then(|value| value.get("desired_revision"))
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        reconciled
            .get("meta")
            .and_then(|value| value.get("reported_revision"))
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        reconciled
            .get("desired")
            .and_then(|value| value.get("log.level"))
            .and_then(Value::as_str),
        Some("debug")
    );
    assert_eq!(
        reconciled
            .get("reported")
            .and_then(|value| value.get("log.level"))
            .and_then(Value::as_str),
        Some("debug")
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_config_desired_write_enforces_revision_and_etag_conflict() {
    let project = make_project("runtime-cloud-config-agent-conflict");
    let state = control_state(source_fixture());
    let base = start_test_server(state, project.clone());

    let payload = json!({
        "api_version": "1.0",
        "actor": "spiffe://trust/default-site/engineer-1",
        "expected_revision": 99,
        "expected_etag": "sha256:invalid",
        "desired": {
            "log.level": "debug"
        }
    });
    let mut response = ureq::post(&format!("{base}/api/runtime-cloud/config/desired"))
        .header("Content-Type", "application/json")
        .config()
        .http_status_as_error(false)
        .build()
        .send(&payload.to_string())
        .expect("stale expected_revision response");
    assert_eq!(
        response.status().as_u16(),
        409,
        "stale expected_revision should conflict"
    );
    let body = response
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud conflict body");
    let response: Value = serde_json::from_str(&body).expect("parse conflict response");
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("revision_conflict")
    );
    assert_eq!(
        response
            .get("status")
            .and_then(|value| value.get("state"))
            .and_then(Value::as_str),
        Some("blocked")
    );

    let _ = std::fs::remove_dir_all(project);
}
