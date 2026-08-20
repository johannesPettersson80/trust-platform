use super::*;

#[test]
fn runtime_cloud_preflight_wan_requires_secure_profile_preconditions() {
    let project = make_project("runtime-cloud-preflight-wan-preconditions");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("runtime-b"),
        name: SmolStr::new("runtime-b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(8080),
        web_tls: false,
        mesh_port: Some(5200),
        control: None,
        host_group: None,
        last_seen_ns: now_ns(),
    }]);
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_options_and_profile(
        state,
        project.clone(),
        Some(discovery),
        None,
        WebAuthMode::Local,
        RuntimeCloudProfile::Wan,
    );

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-preflight-wan-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/operator-1",
        "action_type": "cfg_apply",
        "dry_run": true,
        "payload": {
            "params": { "log.level": "debug" }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud wan preflight response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud wan preflight body");
    let response: Value = serde_json::from_str(&body).expect("parse runtime cloud wan preflight");
    assert_eq!(
        response.get("allowed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("not_configured")
    );
    assert!(
        response
            .get("denial_reason")
            .and_then(Value::as_str)
            .map(|value| value.contains("runtime.web.auth='token'"))
            .unwrap_or(false),
        "expected WAN secure profile requirement reason, got: {response:?}"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_preflight_dev_allows_cross_site_cfg_apply_without_allowlist() {
    let project = make_project("runtime-cloud-preflight-dev-cross-site-allow");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("site-b/runtime-b"),
        name: SmolStr::new("site-b/runtime-b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(8080),
        web_tls: false,
        mesh_port: Some(5200),
        control: Some(SmolStr::new("tcp://127.0.0.1:5201")),
        host_group: None,
        last_seen_ns: now_ns(),
    }]);
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_discovery(state, project.clone(), Some(discovery));

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-dev-cross-site-allow-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["site-b/runtime-b"],
        "actor": "spiffe://trust/site-a/operator-1",
        "action_type": "cfg_apply",
        "dry_run": true,
        "payload": {
            "params": { "log.level": "debug" }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud dev cross-site preflight response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud dev cross-site preflight body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud dev cross-site preflight");

    assert_eq!(response.get("allowed").and_then(Value::as_bool), Some(true));
    assert!(response
        .get("denial_reason")
        .and_then(Value::as_str)
        .is_none());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_preflight_allows_cross_site_cfg_apply_with_allowlist() {
    let project = make_project("runtime-cloud-preflight-cross-site-allow");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("site-b/runtime-b"),
        name: SmolStr::new("site-b/runtime-b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(8080),
        web_tls: false,
        mesh_port: Some(5200),
        control: Some(SmolStr::new("tcp://127.0.0.1:5201")),
        host_group: None,
        last_seen_ns: now_ns(),
    }]);
    let state = control_state_named(source_fixture(), "runtime-a");
    if let Ok(mut settings) = state.settings.lock() {
        settings.runtime_cloud.wan_allow_write = vec![RuntimeCloudWanAllowRule {
            action: SmolStr::new("cfg_apply"),
            target: SmolStr::new("site-b/*"),
        }];
    }
    let base = start_test_server_with_discovery(state, project.clone(), Some(discovery));

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-cross-site-allow-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["site-b/runtime-b"],
        "actor": "spiffe://trust/site-a/operator-1",
        "action_type": "cfg_apply",
        "dry_run": true,
        "payload": {
            "params": { "log.level": "debug" }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud cross-site allow preflight response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud cross-site allow preflight body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud cross-site allow preflight");

    assert_eq!(response.get("allowed").and_then(Value::as_bool), Some(true));
    assert!(response
        .get("denial_code")
        .and_then(Value::as_str)
        .is_none());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_wan_allowlist_policy_change_is_audited() {
    let project = make_project("runtime-cloud-wan-allowlist-audit");
    let (audit_tx, audit_rx) = std::sync::mpsc::channel::<ControlAuditEvent>();
    let state = control_state_named_with_audit(source_fixture(), "runtime-a", Some(audit_tx));
    let base = start_test_server(state, project.clone());

    let update_payload = json!({
        "id": 401,
        "type": "config.set",
        "request_id": "req-federation-policy-1",
        "params": {
            "runtime_cloud.wan.allow_write": [
                {
                    "action": "cfg_apply",
                    "target": "site-b/*"
                }
            ]
        }
    });
    let body = ureq::post(&format!("{base}/api/control"))
        .header("Content-Type", "application/json")
        .send(&update_payload.to_string())
        .expect("federation policy update response")
        .body_mut()
        .read_to_string()
        .expect("read federation policy update response");
    let response: Value = serde_json::from_str(&body).expect("parse federation policy update");
    assert_eq!(response.get("ok").and_then(Value::as_bool), Some(true));

    let audit = recv_audit_event(&audit_rx);
    assert_eq!(audit.request_type.as_str(), "config.set");
    assert_eq!(
        audit.correlation_id.as_deref(),
        Some("req-federation-policy-1")
    );
    assert!(audit.ok);

    let config_body = ureq::post(&format!("{base}/api/control"))
        .header("Content-Type", "application/json")
        .send(r#"{"id":402,"type":"config.get"}"#)
        .expect("federation policy config.get response")
        .body_mut()
        .read_to_string()
        .expect("read federation policy config.get response");
    let config: Value = serde_json::from_str(&config_body).expect("parse config.get");
    let first_rule = config
        .get("result")
        .and_then(|value| value.get("runtime_cloud.wan.allow_write"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .expect("runtime_cloud.wan.allow_write first rule");
    assert_eq!(
        first_rule.get("action").and_then(Value::as_str),
        Some("cfg_apply")
    );
    assert_eq!(
        first_rule.get("target").and_then(Value::as_str),
        Some("site-b/*")
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_link_transport_preferences_change_is_audited_and_roundtrips() {
    let project = make_project("runtime-cloud-link-transport-audit");
    let (audit_tx, audit_rx) = std::sync::mpsc::channel::<ControlAuditEvent>();
    let state = control_state_named_with_audit(source_fixture(), "runtime-a", Some(audit_tx));
    let base = start_test_server(state, project.clone());

    let update_payload = json!({
        "id": 501,
        "type": "config.set",
        "request_id": "req-link-transport-1",
        "params": {
            "runtime_cloud.links.transports": [
                {
                    "source": "runtime-a",
                    "target": "runtime-b",
                    "transport": "realtime"
                },
                {
                    "source": "runtime-c",
                    "target": "runtime-d",
                    "transport": "zenoh"
                },
                {
                    "source": "runtime-e",
                    "target": "runtime-f",
                    "transport": "mqtt"
                }
            ]
        }
    });
    let body = ureq::post(&format!("{base}/api/control"))
        .header("Content-Type", "application/json")
        .send(&update_payload.to_string())
        .expect("runtime cloud link transport update response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud link transport update response");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud link transport update");
    assert_eq!(response.get("ok").and_then(Value::as_bool), Some(true));

    let audit = recv_audit_event(&audit_rx);
    assert_eq!(audit.request_type.as_str(), "config.set");
    assert_eq!(
        audit.correlation_id.as_deref(),
        Some("req-link-transport-1")
    );
    assert!(audit.ok);

    let config_body = ureq::post(&format!("{base}/api/control"))
        .header("Content-Type", "application/json")
        .send(r#"{"id":502,"type":"config.get"}"#)
        .expect("runtime cloud link transport config.get response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud link transport config.get response");
    let config: Value = serde_json::from_str(&config_body).expect("parse config.get");
    let rules = config
        .get("result")
        .and_then(|value| value.get("runtime_cloud.links.transports"))
        .and_then(Value::as_array)
        .expect("runtime_cloud.links.transports rules");
    assert_eq!(rules.len(), 3);
    assert_eq!(
        rules[0].get("source").and_then(Value::as_str),
        Some("runtime-a")
    );
    assert_eq!(
        rules[0].get("target").and_then(Value::as_str),
        Some("runtime-b")
    );
    assert_eq!(
        rules[0].get("transport").and_then(Value::as_str),
        Some("realtime")
    );
    assert_eq!(
        rules[1].get("source").and_then(Value::as_str),
        Some("runtime-c")
    );
    assert_eq!(
        rules[1].get("target").and_then(Value::as_str),
        Some("runtime-d")
    );
    assert_eq!(
        rules[1].get("transport").and_then(Value::as_str),
        Some("zenoh")
    );
    assert_eq!(
        rules[2].get("source").and_then(Value::as_str),
        Some("runtime-e")
    );
    assert_eq!(
        rules[2].get("target").and_then(Value::as_str),
        Some("runtime-f")
    );
    assert_eq!(
        rules[2].get("transport").and_then(Value::as_str),
        Some("mqtt")
    );

    let _ = std::fs::remove_dir_all(project);
}
