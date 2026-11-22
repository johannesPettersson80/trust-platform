//! Runtime Cloud sites end to end (specification 11, "Runtime Cloud sites"): the web servers of
//! two runtimes, `runtime_a` in the default site and `runtime_b` in `site-b`, the first
//! discovering the second as it announced itself.

use super::*;

fn get_json(url: &str) -> Value {
    let body = ureq::get(url)
        .call()
        .expect("GET answers")
        .body_mut()
        .read_to_string()
        .expect("read the answer");
    serde_json::from_str(&body).expect("the answer is JSON")
}

fn post_json(url: &str, payload: &Value) -> Value {
    let body = ureq::post(url)
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("POST answers")
        .body_mut()
        .read_to_string()
        .expect("read the answer");
    serde_json::from_str(&body).expect("the answer is JSON")
}

/// Catches a site that only the configuration knows: runtime A's state names its peer by the
/// cloud ID that the peer's announced site gives, a dispatch to that ID reaches the peer while
/// the bare name reaches nothing, and the peer answers as `site-b/runtime_b` in `site-b`, with
/// `config.get` reporting its site.
#[test]
fn runtimes_are_named_by_their_site_and_reached_by_their_cloud_id() {
    let project_b = make_project("runtime-cloud-site-b");
    let state_b = control_state_named(source_fixture(), "runtime_b");
    state_b
        .settings
        .lock()
        .expect("the settings")
        .runtime_cloud
        .site = Some(SmolStr::new("site-b"));
    let base_b = start_test_server(state_b, project_b.clone());
    let port_b: u16 = base_b
        .rsplit(':')
        .next()
        .and_then(|port| port.parse().ok())
        .expect("runtime B's port");

    let project_a = make_project("runtime-cloud-site-a");
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("runtime-b-id"),
        name: SmolStr::new("runtime_b"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(port_b),
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        site: Some(SmolStr::new("site-b")),
        last_seen_ns: 1,
    }]);
    let base_a = start_test_server_with_discovery(
        control_state_named(source_fixture(), "runtime_a"),
        project_a.clone(),
        Some(discovery),
    );

    let state = get_json(&format!("{base_a}/api/runtime-cloud/state"));
    assert_eq!(state["context"]["connected_via"], "runtime_a");
    assert_eq!(state["context"]["site_scope"], json!(["default-site"]));
    let nodes = state["topology"]["nodes"].as_array().expect("the nodes");
    let peer = nodes
        .iter()
        .find(|node| node["runtime_id"] == "site-b/runtime_b")
        .unwrap_or_else(|| panic!("the peer by its cloud ID: {nodes:?}"));
    assert_eq!(peer["site"], "site-b");
    assert_eq!(peer["display_name"], "runtime_b");

    let dispatch = |target: &str| {
        post_json(
            &format!("{base_a}/api/runtime-cloud/actions/dispatch"),
            &json!({
                "api_version": "1.0",
                "request_id": format!("req-site-{target}"),
                "connected_via": "runtime_a",
                "target_runtimes": [target],
                "actor": "spiffe://trust/default-site/operator-1",
                "action_type": "status_read",
                "dry_run": false,
                "payload": {}
            }),
        )
    };
    let reached = dispatch("site-b/runtime_b");
    assert_eq!(reached["ok"], true, "{reached}");
    assert_eq!(reached["results"][0]["runtime_id"], "site-b/runtime_b");
    let bare = dispatch("runtime_b");
    assert_eq!(
        bare["ok"], false,
        "the bare name is a default-site runtime: {bare}"
    );

    let own = get_json(&format!("{base_b}/api/runtime-cloud/state"));
    assert_eq!(own["context"]["connected_via"], "site-b/runtime_b");
    assert_eq!(own["context"]["site_scope"], json!(["site-b"]));
    let own_nodes = own["topology"]["nodes"].as_array().expect("B's nodes");
    let itself = own_nodes
        .iter()
        .find(|node| node["runtime_id"] == "site-b/runtime_b")
        .unwrap_or_else(|| panic!("B's own node: {own_nodes:?}"));
    assert_eq!(itself["site"], "site-b");
    assert_eq!(
        itself["display_name"], "runtime_b",
        "a runtime is shown by its resource name, as its peers show it"
    );
    let config = post_json(
        &format!("{base_b}/api/control"),
        &json!({ "id": 1, "type": "config.get" }),
    );
    assert_eq!(config["result"]["runtime_cloud.site"], "site-b");

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}
