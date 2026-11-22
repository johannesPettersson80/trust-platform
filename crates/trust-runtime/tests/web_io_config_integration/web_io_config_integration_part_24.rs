//! Runtime Cloud sites end to end (specification 11, "Runtime Cloud sites"): the web servers of
//! two runtimes, `runtime_a` in the default site and `runtime_b` in `site-b`, each discovering the
//! other as it announced itself.

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

fn port_of(base: &str) -> u16 {
    base.rsplit(':')
        .next()
        .and_then(|port| port.parse().ok())
        .expect("the server's port")
}

/// The announcement of `name` of `site` (`None` for the default site) on this computer.
fn announced(name: &str, site: Option<&str>, web_port: u16) -> DiscoveryEntry {
    DiscoveryEntry {
        id: SmolStr::new(format!("{name}-id")),
        name: SmolStr::new(name),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(web_port),
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        site: site.map(SmolStr::new),
        last_seen_ns: 1,
    }
}

/// The topology node of `state` whose ID is `runtime_id`.
fn node<'a>(state: &'a Value, runtime_id: &str) -> &'a Value {
    let nodes = state["topology"]["nodes"].as_array().expect("the nodes");
    nodes
        .iter()
        .find(|node| node["runtime_id"] == runtime_id)
        .unwrap_or_else(|| panic!("no node '{runtime_id}' in {nodes:?}"))
}

/// Dispatches `status_read` to `target` from the runtime at `base`, named `connected_via`.
fn status_read(base: &str, connected_via: &str, target: &str) -> Value {
    post_json(
        &format!("{base}/api/runtime-cloud/actions/dispatch"),
        &json!({
            "api_version": "1.0",
            "request_id": format!("req-site-{connected_via}-{target}"),
            "connected_via": connected_via,
            "target_runtimes": [target],
            "actor": "spiffe://trust/default-site/operator-1",
            "action_type": "status_read",
            "dry_run": false,
            "payload": {}
        }),
    )
}

/// Catches a site that only the configuration knows, and IDs that are not relative to the
/// runtime's own site: each runtime names itself and the runtimes of its own site by their
/// resource names and the other by `<site>/<resource>` from its announcement, a dispatch to that
/// ID reaches the peer while the bare name, a runtime of the own site, reaches nothing, and
/// `config.get` reports the site.
#[test]
fn a_runtime_names_its_peers_relative_to_its_own_site() {
    let project_a = make_project("runtime-cloud-site-a");
    let project_b = make_project("runtime-cloud-site-b");
    let discovery_a = Arc::new(DiscoveryState::new());
    let discovery_b = Arc::new(DiscoveryState::new());
    let base_a = start_test_server_with_discovery(
        control_state_named(source_fixture(), "runtime_a"),
        project_a.clone(),
        Some(discovery_a.clone()),
    );
    let state_b = control_state_named(source_fixture(), "runtime_b");
    state_b
        .settings
        .lock()
        .expect("the settings")
        .runtime_cloud
        .site = Some(SmolStr::new("site-b"));
    let base_b =
        start_test_server_with_discovery(state_b, project_b.clone(), Some(discovery_b.clone()));
    discovery_a.replace_entries(vec![announced(
        "runtime_b",
        Some("site-b"),
        port_of(&base_b),
    )]);
    discovery_b.replace_entries(vec![announced("runtime_a", None, port_of(&base_a))]);

    // A, in the default site, names B by B's site.
    let state = get_json(&format!("{base_a}/api/runtime-cloud/state"));
    assert_eq!(state["context"]["connected_via"], "runtime_a");
    assert_eq!(state["context"]["site_scope"], json!(["default-site"]));
    let peer = node(&state, "site-b/runtime_b");
    assert_eq!(peer["site"], "site-b");
    assert_eq!(peer["display_name"], "runtime_b");

    // B, in site-b, names itself by its resource name and A by A's site.
    let own = get_json(&format!("{base_b}/api/runtime-cloud/state"));
    assert_eq!(own["context"]["connected_via"], "runtime_b");
    assert_eq!(own["context"]["site_scope"], json!(["site-b"]));
    assert_eq!(node(&own, "runtime_b")["site"], "site-b");
    assert_eq!(node(&own, "default-site/runtime_a")["site"], "default-site");

    let reached = status_read(&base_a, "runtime_a", "site-b/runtime_b");
    assert_eq!(reached["ok"], true, "{reached}");
    assert_eq!(reached["results"][0]["runtime_id"], "site-b/runtime_b");
    let bare = status_read(&base_a, "runtime_a", "runtime_b");
    assert_eq!(
        bare["ok"], false,
        "A's bare name is a runtime of A's site: {bare}"
    );
    let back = status_read(&base_b, "runtime_b", "default-site/runtime_a");
    assert_eq!(back["ok"], true, "{back}");
    let bare_back = status_read(&base_b, "runtime_b", "runtime_a");
    assert_eq!(
        bare_back["ok"], false,
        "B's bare name is a runtime of site-b: {bare_back}"
    );

    let config = post_json(
        &format!("{base_b}/api/control"),
        &json!({ "id": 1, "type": "config.get" }),
    );
    assert_eq!(config["result"]["runtime_cloud.site"], "site-b");

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}
