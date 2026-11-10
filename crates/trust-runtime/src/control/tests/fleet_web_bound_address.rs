/// A control state whose web interface is configured on `listen`, has recorded `bound_address`
/// and whose listener is `bound` or not.
fn fleet_state_with_web(listen: &str, bound_address: Option<&str>, bound: bool) -> ControlState {
    let mut state = control_test_state("PROGRAM Main\nEND_PROGRAM\n");
    set_project_root(&mut state, &temp_dir("fleet-web-bound"));
    {
        let mut settings = state.settings.lock().expect("settings lock");
        settings.web.enabled = true;
        settings.web.listen = SmolStr::new(listen);
        settings.web_bound_address = bound_address.map(SmolStr::new);
    }
    state
        .web_listener_bound
        .store(bound, std::sync::atomic::Ordering::Relaxed);
    state
}

/// The runtime's own web service endpoint in the `fleet.topology` answer.
fn web_service_endpoint(state: &ControlState) -> serde_json::Value {
    fn find(value: &serde_json::Value) -> Option<&serde_json::Value> {
        match value {
            serde_json::Value::Object(map) => {
                if map.get("protocol") == Some(&json!("web")) && map.get("owned") == Some(&json!(true))
                {
                    return Some(value);
                }
                map.values().find_map(find)
            }
            serde_json::Value::Array(items) => items.iter().find_map(find),
            _ => None,
        }
    }
    let response = handle_request_value(json!({"id": 1, "type": "fleet.topology"}), state, None);
    assert!(response.ok, "fleet.topology failed: {:?}", response.error);
    let result = response.result.expect("topology result");
    find(&result)
        .cloned()
        .unwrap_or_else(|| panic!("no web service endpoint in {result}"))
}

/// Specification 11, fleet topology: once its listener is bound, the web service endpoint reports
/// the address the listener bound, so a configured port 0 shows the port the system chose, and
/// `params.listen` keeps the configured value. Wrong behaviour caught: the endpoint names the
/// configured `127.0.0.1:0`, which no client can open, or the bound address replaces the
/// configured one in `params`.
#[test]
fn fleet_topology_web_endpoint_reports_the_bound_address_and_keeps_the_configured_listen() {
    let state = fleet_state_with_web("127.0.0.1:0", Some("127.0.0.1:43210"), true);

    let web = web_service_endpoint(&state);

    assert_eq!(web["address"], json!("127.0.0.1:43210"));
    assert_eq!(web["params"]["listen"], json!("127.0.0.1:0"));
    assert_eq!(web["health"], json!("connected"));
}

/// Specification 11, fleet topology: before the listener is bound, `address` is the configured
/// `listen`. Wrong behaviour caught: an address that this control state has no bound evidence for
/// shown next to the `configured_policy` health.
#[test]
fn fleet_topology_web_endpoint_names_the_configured_listen_until_the_listener_is_bound() {
    let state = fleet_state_with_web("127.0.0.1:0", Some("127.0.0.1:43210"), false);

    let web = web_service_endpoint(&state);

    assert_eq!(web["address"], json!("127.0.0.1:0"));
    assert_eq!(web["health"], json!("configured_policy"));
}
