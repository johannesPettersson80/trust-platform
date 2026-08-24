use super::*;

#[test]
fn runtime_cloud_ha_split_brain_preflight_denies_dual_active_candidates() {
    let project = make_project("runtime-cloud-ha-split-brain");
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
    let base = start_test_server_with_discovery(state, project.clone(), Some(discovery));

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-ha-split-brain-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-a", "runtime-b"],
        "actor": "spiffe://trust/default-site/operator-1",
        "action_type": "cmd_invoke",
        "dry_run": true,
        "payload": {
            "command": "status",
            "ha": {
                "profile": "dual_host",
                "group_id": "line-1-ha",
                "active_namespace": true,
                "command_seq": 41,
                "targets": {
                    "runtime-a": {
                        "role": "active",
                        "lease_consistent_authority": true,
                        "lease_available": true,
                        "lease_owner_runtime_id": "runtime-a",
                        "fencing_token": "fence-a",
                        "fence_valid": true
                    },
                    "runtime-b": {
                        "role": "active",
                        "lease_consistent_authority": true,
                        "lease_available": true,
                        "lease_owner_runtime_id": "runtime-b",
                        "fencing_token": "fence-b",
                        "fence_valid": true
                    }
                }
            }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud HA split-brain preflight response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud HA split-brain body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud HA split-brain preflight");

    assert_eq!(
        response.get("allowed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("lease_unavailable")
    );
    let decisions = response
        .get("decisions")
        .and_then(Value::as_array)
        .expect("preflight decisions");
    assert_eq!(decisions.len(), 2);
    assert!(decisions.iter().all(
        |decision| decision.get("denial_code").and_then(Value::as_str) == Some("lease_unavailable")
    ));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_ha_split_brain_preflight_checks_candidates_outside_action_subset() {
    let project = make_project("runtime-cloud-ha-split-brain-subset");
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server(state, project.clone());

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-ha-split-brain-subset-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-a"],
        "actor": "spiffe://trust/default-site/operator-1",
        "action_type": "cmd_invoke",
        "dry_run": true,
        "payload": {
            "command": "status",
            "ha": {
                "profile": "dual_host",
                "group_id": "line-subset-ha",
                "active_namespace": true,
                "command_seq": 42,
                "targets": {
                    "runtime-a": {
                        "role": "active",
                        "lease_consistent_authority": true,
                        "lease_available": true,
                        "lease_owner_runtime_id": "runtime-a",
                        "fencing_token": "fence-a",
                        "fence_valid": true
                    },
                    "runtime-b": {
                        "role": "active",
                        "lease_consistent_authority": true,
                        "lease_available": true,
                        "lease_owner_runtime_id": "runtime-b",
                        "fencing_token": "fence-b",
                        "fence_valid": true
                    }
                }
            }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud HA subset split-brain preflight response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud HA subset split-brain body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud HA subset split-brain preflight");

    assert_eq!(
        response.get("allowed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("lease_unavailable")
    );
    let decisions = response
        .get("decisions")
        .and_then(Value::as_array)
        .expect("preflight decisions");
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].get("runtime_id").and_then(Value::as_str),
        Some("runtime-a")
    );
    assert_eq!(
        decisions[0].get("denial_code").and_then(Value::as_str),
        Some("lease_unavailable")
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_ha_lease_expiry_demotes_active_runtime_preflight() {
    let project = make_project("runtime-cloud-ha-lease-expiry");
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server(state, project.clone());

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-ha-lease-expiry-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-a"],
        "actor": "spiffe://trust/default-site/operator-1",
        "action_type": "cmd_invoke",
        "dry_run": true,
        "payload": {
            "command": "status",
            "ha": {
                "profile": "dual_host",
                "group_id": "line-2-ha",
                "active_namespace": true,
                "command_seq": 3,
                "targets": {
                    "runtime-a": {
                        "role": "active",
                        "lease_consistent_authority": true,
                        "lease_available": false,
                        "lease_owner_runtime_id": "runtime-a",
                        "fencing_token": "fence-a",
                        "fence_valid": true
                    }
                }
            }
        }
    });
    let body = ureq::post(&format!("{base}/api/runtime-cloud/actions/preflight"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud HA lease expiry preflight response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud HA lease expiry body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud HA lease expiry preflight");
    assert_eq!(
        response.get("allowed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        response.get("denial_code").and_then(Value::as_str),
        Some("lease_unavailable")
    );
    assert!(response
        .get("denial_reason")
        .and_then(Value::as_str)
        .map(|value| value.contains("demoted_safe"))
        .unwrap_or(false));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn runtime_cloud_ha_dual_output_prevention_blocks_standby_dispatch() {
    let project_a = make_project("runtime-cloud-ha-standby-deny-a");
    let project_b = make_project("runtime-cloud-ha-standby-deny-b");
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

    let payload = json!({
        "api_version": "1.0",
        "request_id": "req-ha-standby-deny-1",
        "connected_via": "runtime-a",
        "target_runtimes": ["runtime-b"],
        "actor": "spiffe://trust/default-site/operator-1",
        "action_type": "cmd_invoke",
        "dry_run": false,
        "payload": {
            "command": "status",
            "ha": {
                "profile": "dual_host",
                "group_id": "line-3-ha",
                "active_namespace": true,
                "command_seq": 51,
                "targets": {
                    "runtime-b": {
                        "role": "standby",
                        "lease_consistent_authority": true,
                        "lease_available": true,
                        "lease_owner_runtime_id": "runtime-a",
                        "fencing_token": "fence-a",
                        "fence_valid": true
                    }
                }
            }
        }
    });
    let body = ureq::post(&format!("{base_a}/api/runtime-cloud/actions/dispatch"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("runtime cloud HA standby dispatch response")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud HA standby dispatch body");
    let response: Value =
        serde_json::from_str(&body).expect("parse runtime cloud HA standby dispatch");

    assert_eq!(response.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("denial_code"))
            .and_then(Value::as_str),
        Some("permission_denied")
    );
    assert!(
        audit_rx.recv_timeout(Duration::from_millis(250)).is_err(),
        "standby runtime should not receive any protected command dispatch in dual-output prevention path"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}
