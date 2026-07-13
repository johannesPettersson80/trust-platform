fn connector_fixture(path: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/connectors")
        .join(path);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| {
        panic!("parse fixture {}: {error}", path.display());
    })
}

fn canonical_json(value: &serde_json::Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(value).expect("canonical JSON")
    )
}

fn assert_connector_fixture(actual: &serde_json::Value, fixture: &str) {
    let expected = connector_fixture(fixture);
    assert_eq!(
        canonical_json(actual),
        canonical_json(&expected),
        "fixture mismatch: {fixture}"
    );
}

#[test]
fn phase0_ads_status_matches_disabled_goldens() {
    let state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );

    let response = handle_request_value(json!({"id": 9201, "type": "ads.status"}), &state, None);
    assert!(response.ok, "ads.status failed: {:?}", response.error);
    let result = response.result.expect("ads status result");

    assert_connector_fixture(&result, "phase0/ads/client_disabled.json");

    let response = handle_request_value(
        json!({"id": 92011, "type": "ads.server.status"}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "ads.server.status failed: {:?}",
        response.error
    );
    let result = response.result.expect("ads server status result");

    assert_connector_fixture(&result, "phase0/ads/server_disabled.json");
}

#[test]
fn phase0_opcua_status_matches_capability_goldens() {
    let state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );

    let response = handle_request_value(
        json!({"id": 9202, "type": "comm.capabilities"}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "comm.capabilities failed: {:?}",
        response.error
    );
    let result = response.result.expect("comm.capabilities result");
    let capabilities = result
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .expect("capabilities");
    let opcua = capabilities
        .iter()
        .filter(|capability| {
            matches!(
                capability.get("id").and_then(serde_json::Value::as_str),
                Some("opcua" | "opcua_client")
            )
        })
        .cloned()
        .collect::<Vec<_>>();

    assert_connector_fixture(
        &serde_json::Value::Array(opcua),
        "phase0/opcua/capabilities_not_configured.json",
    );
}

#[test]
fn phase0_io_driver_status_matches_legacy_golden() {
    let state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    state.io_health.lock().expect("io health").extend([
        crate::io::IoDriverStatus {
            name: SmolStr::new("modbus-tcp"),
            health: crate::io::IoDriverHealth::Ok,
        },
        crate::io::IoDriverStatus {
            name: SmolStr::new("mqtt"),
            health: crate::io::IoDriverHealth::Degraded {
                error: SmolStr::new("broker reconnecting"),
            },
        },
        crate::io::IoDriverStatus {
            name: SmolStr::new("ethercat"),
            health: crate::io::IoDriverHealth::Faulted {
                error: SmolStr::new("bus lost"),
            },
        },
    ]);

    let response = handle_request_value(json!({"id": 9203, "type": "status"}), &state, None);
    assert!(response.ok, "status failed: {:?}", response.error);
    let result = response.result.expect("status result");
    let io_drivers = result.get("io_drivers").expect("io_drivers");

    assert_connector_fixture(io_drivers, "phase0/io_driver/status_io_drivers.json");
}

#[test]
fn phase0_missing_or_failed_connectors_do_not_report_healthy() {
    let state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );

    let ads = handle_request_value(json!({"id": 9204, "type": "ads.status"}), &state, None);
    assert!(ads.ok, "ads.status failed: {:?}", ads.error);
    let ads = ads.result.expect("ads.status result");
    assert_eq!(
        ads.pointer("/overall").and_then(serde_json::Value::as_str),
        Some("disabled")
    );
    assert_ne!(
        ads.pointer("/overall").and_then(serde_json::Value::as_str),
        Some("healthy")
    );

    let ads_server = handle_request_value(
        json!({"id": 9205, "type": "ads.server.status"}),
        &state,
        None,
    );
    assert!(
        ads_server.ok,
        "ads.server.status failed: {:?}",
        ads_server.error
    );
    let ads_server = ads_server.result.expect("ads.server.status result");
    assert_eq!(
        ads_server
            .pointer("/status/overall")
            .and_then(serde_json::Value::as_str),
        Some("disabled")
    );
    assert_ne!(
        ads_server
            .pointer("/status/connections/0/state")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );

    let capabilities = handle_request_value(
        json!({"id": 9206, "type": "comm.capabilities"}),
        &state,
        None,
    );
    assert!(
        capabilities.ok,
        "comm.capabilities failed: {:?}",
        capabilities.error
    );
    let capabilities = capabilities.result.expect("comm.capabilities result");
    let capabilities = capabilities
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .expect("capabilities");
    for id in ["opcua", "opcua_client"] {
        let capability = capabilities
            .iter()
            .find(|entry| entry.get("id").and_then(serde_json::Value::as_str) == Some(id))
            .unwrap_or_else(|| panic!("missing capability {id}"));
        assert_eq!(
            capability
                .get("configured")
                .and_then(serde_json::Value::as_bool),
            Some(false),
            "{id} should be unconfigured"
        );
        assert_eq!(
            capability
                .get("operational")
                .and_then(serde_json::Value::as_bool),
            Some(false),
            "{id} should not be operational"
        );
        assert_ne!(
            capability.get("health").and_then(serde_json::Value::as_str),
            Some("ok"),
            "{id} should not report ok health"
        );
    }

    state
        .io_health
        .lock()
        .expect("io health")
        .push(crate::io::IoDriverStatus {
            name: SmolStr::new("modbus-tcp"),
            health: crate::io::IoDriverHealth::Faulted {
                error: SmolStr::new("socket closed"),
            },
        });
    let legacy = handle_request_value(json!({"id": 9207, "type": "status"}), &state, None);
    assert!(legacy.ok, "status failed: {:?}", legacy.error);
    assert_eq!(
        legacy
            .result
            .as_ref()
            .and_then(|value| value.pointer("/io_drivers/0/status"))
            .and_then(serde_json::Value::as_str),
        Some("faulted")
    );

    let connectors = handle_request_value(
        json!({"id": 9208, "type": "connectors.status"}),
        &state,
        None,
    );
    assert!(
        connectors.ok,
        "connectors.status failed: {:?}",
        connectors.error
    );
    let connectors = connectors.result.expect("connectors.status result");
    assert_eq!(
        connectors
            .pointer("/connectors/0/state")
            .and_then(serde_json::Value::as_str),
        Some("faulted")
    );
    assert_eq!(
        connectors
            .pointer("/connectors/0/health")
            .and_then(serde_json::Value::as_str),
        Some("faulted")
    );
}

#[test]
fn phase0_discovery_matches_current_goldens() {
    let mut ads = super::comm_handlers::discover_value(
        json!({
            "protocol": "ads",
            "scope": { "cidr": "127.0.0.1/32", "timeout_ms": 1 },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("ads client discovery no-target baseline");
    // Live candidates plus socket/native-router diagnostics are host-specific.
    // Deterministic ADS candidate and warning contracts are covered by mock-wire tests.
    let ads_object = ads.as_object_mut().expect("ADS discovery response object");
    ads_object.insert("candidates".to_string(), json!([]));
    ads_object.remove("warnings");
    assert_connector_fixture(
        &ads,
        "phase0/discovery/ads_client_no_targets.json",
    );
    assert_connector_fixture(
        &super::comm_handlers::discover_value(
            json!({
                "protocol": "ads_server",
                "origin": "this_host",
                "passive": true
            }),
            None,
        )
        .expect("ads server discovery warning"),
        "phase0/discovery/ads_server_unavailable.json",
    );
    assert_connector_fixture(
        &super::comm_handlers::discover_value(
            json!({
                "protocol": "opcua",
                "scope": { "host": "127.0.0.1:4840" },
                "origin": "this_host",
                "passive": true
            }),
            None,
        )
        .expect("opcua discovery warning"),
        "phase0/discovery/opcua_server_only_warning.json",
    );
    assert_connector_fixture(
        &super::comm_handlers::discover_value(
            json!({
                "protocol": "ethercat",
                "origin": "this_host",
                "passive": true
            }),
            None,
        )
        .expect("ethercat discovery warning"),
        "phase0/discovery/ethercat_this_host_warning.json",
    );

    let (modbus_addr, modbus_handle) = spawn_single_accept_listener();
    let mut modbus = super::comm_handlers::discover_value(
        json!({
            "protocol": "modbus_tcp",
            "scope": { "host": modbus_addr.to_string(), "timeout_ms": 250 },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("modbus discovery");
    normalize_discovery_endpoint(&mut modbus, modbus_addr);
    assert_connector_fixture(
        &modbus,
        "phase0/discovery/modbus_tcp_listener_observed.json",
    );
    modbus_handle.join().expect("join modbus listener");

    let (mqtt_addr, mqtt_handle) = spawn_single_accept_listener();
    let mut mqtt = super::comm_handlers::discover_value(
        json!({
            "protocol": "mqtt",
            "scope": { "host": mqtt_addr.to_string(), "timeout_ms": 250 },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("mqtt discovery");
    normalize_discovery_endpoint(&mut mqtt, mqtt_addr);
    assert_connector_fixture(&mqtt, "phase0/discovery/mqtt_tcp_listener_observed.json");
    mqtt_handle.join().expect("join mqtt listener");
}

fn spawn_single_accept_listener() -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let handle = std::thread::spawn(move || {
        let _ = listener.accept().map(|(mut stream, _)| {
            let mut buffer = [0u8; 1];
            let _ = std::io::Read::read(&mut stream, &mut buffer);
        });
    });
    (addr, handle)
}

fn normalize_discovery_endpoint(value: &mut serde_json::Value, addr: std::net::SocketAddr) {
    let addr = addr.to_string();
    let sanitized = addr
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    replace_json_string(value, addr.as_str(), "$ADDR");
    replace_json_string(value, sanitized.as_str(), "$SANITIZED_ADDR");
}

fn replace_json_string(value: &mut serde_json::Value, needle: &str, replacement: &str) {
    match value {
        serde_json::Value::String(text) => {
            if text.contains(needle) {
                *text = text.replace(needle, replacement);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                replace_json_string(item, needle, replacement);
            }
        }
        serde_json::Value::Object(entries) => {
            for item in entries.values_mut() {
                replace_json_string(item, needle, replacement);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}
