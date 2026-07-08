#[test]
fn connectors_status_reports_process_image_drivers_without_mutating_legacy_status() {
    let source = r#"
PROGRAM Main
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    state.io_health.lock().expect("io health").extend([
        crate::io::IoDriverStatus {
            name: SmolStr::new("modbus-tcp"),
            health: crate::io::IoDriverHealth::Ok,
        },
        crate::io::IoDriverStatus {
            name: SmolStr::new("fieldbus"),
            health: crate::io::IoDriverHealth::Faulted {
                error: SmolStr::new("device lost"),
            },
        },
    ]);

    let response = handle_request_value(
        json!({"id": 9101, "type": "connectors.status"}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "connectors.status failed: {:?}",
        response.error
    );
    let result = response.result.expect("connectors result");
    assert_eq!(
        result
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(crate::connectors::CONNECTOR_STATUS_SCHEMA_VERSION as u64)
    );
    let connectors = result
        .get("connectors")
        .and_then(serde_json::Value::as_array)
        .expect("connectors array");
    assert!(
        connectors.len() >= 4,
        "process-image drivers should be reported alongside ADS connector status: {connectors:?}"
    );

    let modbus = connectors
        .iter()
        .find(|entry| {
            entry
                .get("connector_id")
                .and_then(serde_json::Value::as_str)
                == Some("io:modbus-tcp")
        })
        .expect("modbus connector");
    assert_eq!(
        modbus.get("protocol").and_then(serde_json::Value::as_str),
        Some("modbus_tcp")
    );
    assert_eq!(
        modbus.get("kind").and_then(serde_json::Value::as_str),
        Some("process_image")
    );
    assert_eq!(
        modbus.get("state").and_then(serde_json::Value::as_str),
        Some("ready")
    );
    assert_eq!(
        modbus.get("health").and_then(serde_json::Value::as_str),
        Some("ok")
    );
    assert_eq!(
        modbus.get("confidence").and_then(serde_json::Value::as_str),
        Some("confirmed")
    );

    let custom = connectors
        .iter()
        .find(|entry| {
            entry
                .get("connector_id")
                .and_then(serde_json::Value::as_str)
                == Some("io:fieldbus")
        })
        .expect("custom connector");
    assert_eq!(
        custom.get("protocol").and_then(serde_json::Value::as_str),
        Some("unknown")
    );
    assert_eq!(
        custom.get("state").and_then(serde_json::Value::as_str),
        Some("faulted")
    );
    assert_eq!(
        custom.get("last_error").and_then(serde_json::Value::as_str),
        Some("device lost")
    );

    let legacy_status = handle_request_value(json!({"id": 9102, "type": "status"}), &state, None);
    assert!(
        legacy_status.ok,
        "legacy status failed: {:?}",
        legacy_status.error
    );
    let legacy_result = legacy_status.result.expect("legacy status result");
    assert!(legacy_result.get("connectors").is_none());
    assert_eq!(
        legacy_result.get("io_drivers"),
        Some(&json!([
            {
                "name": "modbus-tcp",
                "status": "ok",
            },
            {
                "name": "fieldbus",
                "status": "faulted",
                "error": "device lost",
            }
        ]))
    );
}

#[test]
fn connectors_status_reports_ads_client_and_server_roles() {
    let source = r#"
PROGRAM Main
END_PROGRAM
"#;
    let state = hmi_test_state_with_ads_status(source, connected_ads_status());

    let response = handle_request_value(
        json!({"id": 9103, "type": "connectors.status"}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "connectors.status failed: {:?}",
        response.error
    );
    let result = response.result.expect("connectors result");
    let connectors = result
        .get("connectors")
        .and_then(serde_json::Value::as_array)
        .expect("connectors array");

    let ads_client = connectors
        .iter()
        .find(|entry| {
            entry
                .get("connector_id")
                .and_then(serde_json::Value::as_str)
                == Some("ads:client:5.23.91.12.1.1")
        })
        .expect("ADS client connector");
    assert_eq!(
        ads_client.get("kind").and_then(serde_json::Value::as_str),
        Some("supervisory_client")
    );
    assert_eq!(
        ads_client
            .get("endpoint")
            .and_then(serde_json::Value::as_str),
        Some("5.23.91.12.1.1:851@192.168.77.20")
    );
    assert_eq!(
        ads_client.get("state").and_then(serde_json::Value::as_str),
        Some("ready")
    );
    assert_eq!(
        ads_client
            .get("point_counts")
            .and_then(|counts| counts.get("total"))
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );

    let ads_server = connectors
        .iter()
        .find(|entry| {
            entry
                .get("connector_id")
                .and_then(serde_json::Value::as_str)
                == Some("ads:server:ads-server")
        })
        .expect("ADS server connector");
    assert_eq!(
        ads_server.get("kind").and_then(serde_json::Value::as_str),
        Some("supervisory_server")
    );
    assert_eq!(
        ads_server.get("state").and_then(serde_json::Value::as_str),
        Some("disabled")
    );
    assert_eq!(
        ads_server
            .get("endpoint")
            .and_then(serde_json::Value::as_str),
        Some("127.0.0.1.1.1:851@127.0.0.1")
    );
    assert_eq!(
        ads_server.get("health").and_then(serde_json::Value::as_str),
        Some("unknown")
    );
}

#[test]
fn connectors_status_ads_projection_matches_legacy_ads_status() {
    let source = r#"
PROGRAM Main
END_PROGRAM
"#;
    let state = hmi_test_state_with_ads_status(source, connected_ads_status());

    let legacy = handle_request_value(json!({"id": 9104, "type": "ads.status"}), &state, None);
    assert!(legacy.ok, "ads.status failed: {:?}", legacy.error);
    let legacy = legacy.result.expect("legacy ads.status result");

    let connectors = handle_request_value(
        json!({"id": 9105, "type": "connectors.status"}),
        &state,
        None,
    );
    assert!(
        connectors.ok,
        "connectors.status failed: {:?}",
        connectors.error
    );
    let connectors = connectors.result.expect("connectors result");
    let ads_connector = connectors
        .get("connectors")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| {
            items.iter().find(|entry| {
                entry
                    .get("connector_id")
                    .and_then(serde_json::Value::as_str)
                    == Some("ads:client:5.23.91.12.1.1")
            })
        })
        .expect("ADS client connector");

    assert_eq!(
        legacy
            .pointer("/connections/0/target/ams_net_id")
            .and_then(serde_json::Value::as_str),
        Some("5.23.91.12.1.1")
    );
    assert_eq!(
        ads_connector
            .get("endpoint")
            .and_then(serde_json::Value::as_str),
        Some("5.23.91.12.1.1:851@192.168.77.20")
    );
    assert_eq!(
        ads_connector
            .pointer("/point_counts/total")
            .and_then(serde_json::Value::as_u64),
        legacy
            .pointer("/connections/0/point_count")
            .and_then(serde_json::Value::as_u64)
    );
    assert_eq!(
        ads_connector
            .get("state")
            .and_then(serde_json::Value::as_str),
        Some("ready")
    );
}

#[test]
fn connectors_status_reports_opcua_client_points_with_quality() {
    let source = r#"
PROGRAM Main
END_PROGRAM
"#;
    let state = hmi_test_state_with_opcua_client_status(
        source,
        crate::opcua::OpcUaClientStatusReport {
            enabled: true,
            deployed_config_hash: Some("opcua-config-hash".to_string()),
            connections: vec![crate::opcua::OpcUaClientConnectionStatus {
                name: SmolStr::new("line1"),
                endpoint_url: "opc.tcp://127.0.0.1:4840/trust".to_string(),
                state: crate::opcua::OpcUaClientConnectionState::Connected,
                point_count: 2,
                degraded_points: 1,
                last_seen_ms: Some(2_000),
                detail: "subscription active with one stale point".to_string(),
                points: vec![
                    crate::opcua::OpcUaClientPointStatus {
                        var: SmolStr::new("line1_temp"),
                        node_id: "ns=2;i=2".to_string(),
                        data_type: crate::opcua::OpcUaDataType::Float,
                        access: crate::opcua::OpcUaClientPointAccess::Read,
                        state: crate::opcua::OpcUaClientConnectionState::Connected,
                        last_seen_ms: Some(2_000),
                        value: Some(crate::value::Value::Real(22.5)),
                        detail: "fresh subscription update".to_string(),
                    },
                    crate::opcua::OpcUaClientPointStatus {
                        var: SmolStr::new("line1_setpoint"),
                        node_id: "ns=2;i=3".to_string(),
                        data_type: crate::opcua::OpcUaDataType::Double,
                        access: crate::opcua::OpcUaClientPointAccess::ReadWrite,
                        state: crate::opcua::OpcUaClientConnectionState::Stale,
                        last_seen_ms: Some(1_500),
                        value: Some(crate::value::Value::LReal(12.0)),
                        detail: "server stopped publishing this node".to_string(),
                    },
                ],
            }],
        },
    );

    let response = handle_request_value(
        json!({"id": 9106, "type": "connectors.status"}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "connectors.status failed: {:?}",
        response.error
    );
    let result = response.result.expect("connectors result");
    let opcua = result
        .get("connectors")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| {
            items.iter().find(|entry| {
                entry
                    .get("connector_id")
                    .and_then(serde_json::Value::as_str)
                    == Some("opcua:client:line1")
            })
        })
        .expect("OPC UA client connector");

    assert_eq!(
        opcua.get("kind").and_then(serde_json::Value::as_str),
        Some("supervisory_client")
    );
    assert_eq!(
        opcua.get("endpoint").and_then(serde_json::Value::as_str),
        Some("opc.tcp://127.0.0.1:4840/trust")
    );
    assert_eq!(
        opcua.get("state").and_then(serde_json::Value::as_str),
        Some("degraded")
    );
    assert_eq!(
        opcua
            .pointer("/point_counts/total")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        opcua
            .pointer("/point_counts/degraded")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        opcua
            .pointer("/points/0/metadata/data_type")
            .and_then(serde_json::Value::as_str),
        Some("REAL")
    );
    assert_eq!(
        opcua
            .pointer("/points/0/quality")
            .and_then(serde_json::Value::as_str),
        Some("good")
    );
    assert_eq!(
        opcua
            .pointer("/points/1/metadata/direction")
            .and_then(serde_json::Value::as_str),
        Some("read_write")
    );
    assert_eq!(
        opcua
            .pointer("/points/1/quality")
            .and_then(serde_json::Value::as_str),
        Some("stale")
    );
}

#[test]
fn connectors_status_authz_requires_viewer_and_preserves_local_unix_read() {
    let source = r#"
PROGRAM Main
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);

    let local = handle_request_value(
        json!({"id": 9110, "type": "connectors.status"}),
        &state,
        Some("unix"),
    );
    assert!(
        local.ok,
        "local Unix control should read connector status: {:?}",
        local.error
    );

    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;

    let missing = handle_request_value(
        json!({"id": 9111, "type": "connectors.status"}),
        &state,
        Some("127.0.0.1:55001"),
    );
    assert!(!missing.ok, "missing token should be rejected");
    assert_eq!(missing.error.as_deref(), Some("missing auth token"));
    assert_eq!(missing.error_code.as_deref(), Some("missing_auth_token"));

    let invalid = handle_request_value(
        json!({"id": 9112, "type": "connectors.status", "auth": "bad-token"}),
        &state,
        Some("127.0.0.1:55001"),
    );
    assert!(!invalid.ok, "invalid token should be rejected");
    assert_eq!(invalid.error.as_deref(), Some("invalid auth token"));
    assert_eq!(invalid.error_code.as_deref(), Some("invalid_auth_token"));

    let pairing_path = pairing_file("connectors-status-viewer");
    let store = Arc::new(PairingStore::load(pairing_path.clone()));
    state.pairing = Some(store.clone());
    let code = store.start_pairing();
    let viewer_token = store
        .claim(&code.code, Some(AccessRole::Viewer))
        .expect("viewer token");

    let viewer = handle_request_value(
        json!({"id": 9113, "type": "connectors.status", "auth": viewer_token.clone()}),
        &state,
        Some("127.0.0.1:55001"),
    );
    assert!(
        viewer.ok,
        "viewer token should read connector status: {:?}",
        viewer.error
    );

    let route_add = handle_request_value(
        json!({
            "id": 9114,
            "type": "ads.route_add",
            "auth": viewer_token.clone(),
            "params": ads_route_add_params("trusted_same_host")
        }),
        &state,
        Some("127.0.0.1:55001"),
    );
    assert!(!route_add.ok, "viewer token must not add ADS routes");
    assert_eq!(
        route_add.error.as_deref(),
        Some("forbidden: requires role admin")
    );

    let live_import = handle_request_value(
        json!({
            "id": 9115,
            "type": "ads.import_symbols",
            "auth": viewer_token,
            "params": {
                "connection_name": "line1",
                "target": {
                    "ip": "192.168.10.5",
                    "ams_net_id": "5.23.91.12.1.1",
                    "ams_port": 851
                }
            }
        }),
        &state,
        Some("127.0.0.1:55001"),
    );
    assert!(
        !live_import.ok,
        "viewer token must not import live ADS symbols"
    );
    assert_eq!(
        live_import.error.as_deref(),
        Some("forbidden: requires role engineer")
    );

    let _ = std::fs::remove_file(pairing_path);
}
