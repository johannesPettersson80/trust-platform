use super::*;

#[test]
fn runtime_cloud_link_transport_endpoint_switches_edge_mode_and_checks_same_host() {
    let project = make_project("runtime-cloud-link-transport");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![
        DiscoveryEntry {
            id: SmolStr::new("runtime-b"),
            name: SmolStr::new("runtime-b"),
            addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            web_port: Some(8081),
            web_tls: false,
            mesh_port: Some(5201),
            control: Some(SmolStr::new("tcp://127.0.0.1:5201")),
            host_group: None,
            site: None,
            last_seen_ns: now_ns(),
        },
        DiscoveryEntry {
            id: SmolStr::new("runtime-c"),
            name: SmolStr::new("runtime-c"),
            addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 20, 30, 40))],
            web_port: Some(8082),
            web_tls: false,
            mesh_port: Some(5202),
            control: Some(SmolStr::new("tcp://10.20.30.40:5202")),
            host_group: None,
            site: None,
            last_seen_ns: now_ns(),
        },
    ]);
    let state = control_state_named(source_fixture(), "runtime-a");
    let base = start_test_server_with_discovery(state, project.clone(), Some(discovery));

    let set_realtime_body = ureq::post(&format!("{base}/api/runtime-cloud/links/transport"))
        .header("Content-Type", "application/json")
        .send(
            &json!({
                "api_version": "1.0",
                "actor": "local://engineer",
                "source": "runtime-a",
                "target": "runtime-b",
                "transport": "realtime",
            })
            .to_string(),
        )
        .expect("set runtime-b transport to realtime")
        .body_mut()
        .read_to_string()
        .expect("read realtime set response");
    let set_realtime: Value = serde_json::from_str(&set_realtime_body).expect("parse realtime set");
    assert_eq!(set_realtime.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        set_realtime
            .get("preference")
            .and_then(|value| value.get("transport"))
            .and_then(Value::as_str),
        Some("realtime")
    );

    let state_after_realtime_body = ureq::get(&format!("{base}/api/runtime-cloud/state"))
        .call()
        .expect("load runtime cloud state after realtime switch")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud state after realtime switch");
    let state_after_realtime: Value = serde_json::from_str(&state_after_realtime_body)
        .expect("parse state after realtime switch");
    let edges = state_after_realtime
        .get("topology")
        .and_then(|value| value.get("edges"))
        .and_then(Value::as_array)
        .expect("topology.edges");
    let edge_to_b = edges
        .iter()
        .find(|edge| {
            edge.get("source").and_then(Value::as_str) == Some("runtime-a")
                && edge.get("target").and_then(Value::as_str) == Some("runtime-b")
                && edge.get("channel_type").and_then(Value::as_str) == Some("t0_hard_rt")
        })
        .expect("realtime edge runtime-a->runtime-b");
    assert_eq!(
        edge_to_b.get("channel_type").and_then(Value::as_str),
        Some("t0_hard_rt")
    );
    assert!(
        edges.iter().any(|edge| {
            edge.get("source").and_then(Value::as_str) == Some("runtime-a")
                && edge.get("target").and_then(Value::as_str) == Some("runtime-b")
                && edge.get("channel_type").and_then(Value::as_str) == Some("mesh_t2_ops")
        }),
        "mesh edge should remain when realtime overlay is enabled"
    );

    let set_zenoh_body = ureq::post(&format!("{base}/api/runtime-cloud/links/transport"))
        .header("Content-Type", "application/json")
        .send(
            &json!({
                "api_version": "1.0",
                "actor": "local://engineer",
                "source": "runtime-a",
                "target": "runtime-b",
                "transport": "zenoh",
            })
            .to_string(),
        )
        .expect("set runtime-b transport back to zenoh")
        .body_mut()
        .read_to_string()
        .expect("read zenoh set response");
    let set_zenoh: Value = serde_json::from_str(&set_zenoh_body).expect("parse zenoh set");
    assert_eq!(set_zenoh.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        set_zenoh
            .get("preference")
            .and_then(|value| value.get("transport"))
            .and_then(Value::as_str),
        Some("zenoh")
    );

    let state_after_zenoh_body = ureq::get(&format!("{base}/api/runtime-cloud/state"))
        .call()
        .expect("load runtime cloud state after zenoh switch")
        .body_mut()
        .read_to_string()
        .expect("read runtime cloud state after zenoh switch");
    let state_after_zenoh: Value =
        serde_json::from_str(&state_after_zenoh_body).expect("parse state after zenoh switch");
    let edges_after_zenoh = state_after_zenoh
        .get("topology")
        .and_then(|value| value.get("edges"))
        .and_then(Value::as_array)
        .expect("topology.edges after zenoh switch");
    let edge_to_b_after_zenoh = edges_after_zenoh
        .iter()
        .find(|edge| {
            edge.get("source").and_then(Value::as_str) == Some("runtime-a")
                && edge.get("target").and_then(Value::as_str) == Some("runtime-b")
        })
        .expect("edge runtime-a->runtime-b after zenoh switch");
    assert_eq!(
        edge_to_b_after_zenoh
            .get("channel_type")
            .and_then(Value::as_str),
        Some("mesh_t2_ops")
    );
    assert!(
        !edges_after_zenoh.iter().any(|edge| {
            edge.get("source").and_then(Value::as_str) == Some("runtime-a")
                && edge.get("target").and_then(Value::as_str) == Some("runtime-b")
                && edge.get("channel_type").and_then(Value::as_str) == Some("t0_hard_rt")
        }),
        "realtime edge should be removed when set back to zenoh"
    );

    let mut reject_response = ureq::post(&format!("{base}/api/runtime-cloud/links/transport"))
        .config()
        .http_status_as_error(false)
        .build()
        .header("Content-Type", "application/json")
        .send(
            &json!({
                "api_version": "1.0",
                "actor": "local://engineer",
                "source": "runtime-a",
                "target": "runtime-c",
                "transport": "realtime",
            })
            .to_string(),
        )
        .expect("realtime runtime-c response");
    assert_eq!(reject_response.status().as_u16(), 400);
    let reject_body = reject_response
        .body_mut()
        .read_to_string()
        .expect("read realtime reject body");
    let reject: Value = serde_json::from_str(&reject_body).expect("parse realtime reject");
    assert_eq!(reject.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        reject.get("denial_code").and_then(Value::as_str),
        Some("contract_violation")
    );
    assert!(
        reject
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("same host"),
        "expected same-host rejection error, got: {reject_body}"
    );

    let _ = std::fs::remove_dir_all(project);
}
