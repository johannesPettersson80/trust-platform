use serde_json::json;
use smol_str::SmolStr;
use trust_ads_core::{PointQuality as AdsPointQuality, PointStatus as AdsPointStatus};
use trust_runtime::ads::diagnostics::{
    AdsConnectionStatus, AdsConnectionStatusState, AdsStatusOverall, AdsStatusReport, DoctorRole,
    TargetIdentity,
};
use trust_runtime::ads::onboarding::ActiveAdsDeviceSnapshot;
use trust_runtime::ads::AdsConnectionState;
use trust_runtime::connectors::adapters::ads::{
    project_active_ads_device_snapshot, project_ads_point_statuses, project_ads_status_report,
    project_ads_status_report_with_default_endpoint,
};
use trust_runtime::connectors::adapters::opcua::project_opcua_client_status_report;
use trust_runtime::connectors::{
    ads_connection_state_status, ads_connection_status_state, ethercat_status, io_driver_status,
    modbus_status, mqtt_session_status, opcua_client_status, opcua_server_snapshot_status,
    ConnectorHealth, ConnectorKind, ConnectorPointMetadata, ConnectorPointStatus,
    ConnectorProtocol, ConnectorState, ConnectorStatusBuilder, DiscoveryConfidence,
    EthercatProjection, ModbusProjection, MqttSessionProjection, OpcUaServerSnapshotState,
    PointDirection, PointQuality, ReconnectPolicy, CONNECTOR_STATUS_SCHEMA_VERSION,
};
use trust_runtime::io::{IoDriverErrorPolicy, IoDriverHealth};
use trust_runtime::opcua::{
    OpcUaClientConnectionState, OpcUaClientConnectionStatus, OpcUaClientPointAccess,
    OpcUaClientPointStatus, OpcUaClientStatusReport, OpcUaDataType,
};

#[test]
fn connector_status_report_serializes_stable_schema() {
    let report = ConnectorStatusBuilder::new(
        "ads:line1",
        ConnectorProtocol::Ads,
        ConnectorKind::SupervisoryClient,
        ConnectorState::Ready,
        ConnectorHealth::Ok,
    )
    .display_name("Line 1 ADS")
    .endpoint("5.23.91.12.1.1:851")
    .confidence(DiscoveryConfidence::Confirmed)
    .reconnect_policy(ReconnectPolicy::ExponentialBackoff)
    .freshness_ms(25)
    .points(vec![
        ConnectorPointStatus {
            metadata: ConnectorPointMetadata {
                name: "line1_temp".to_string(),
                source: Some("MAIN.Temperature".to_string()),
                data_type: Some("REAL".to_string()),
                direction: PointDirection::Read,
            },
            quality: PointQuality::Good,
            last_update_ms: Some(1234),
            detail: None,
        },
        ConnectorPointStatus {
            metadata: ConnectorPointMetadata {
                name: "line1_cmd".to_string(),
                source: Some("MAIN.Command".to_string()),
                data_type: Some("BOOL".to_string()),
                direction: PointDirection::Write,
            },
            quality: PointQuality::WritePending,
            last_update_ms: None,
            detail: Some("queued".to_string()),
        },
    ])
    .build();

    let value = serde_json::to_value(&report).expect("serialize connector report");
    assert_eq!(
        value,
        json!({
            "schema_version": CONNECTOR_STATUS_SCHEMA_VERSION,
            "connector_id": "ads:line1",
            "display_name": "Line 1 ADS",
            "protocol": "ads",
            "kind": "supervisory_client",
            "endpoint": "5.23.91.12.1.1:851",
            "state": "ready",
            "health": "ok",
            "confidence": "confirmed",
            "reconnect_policy": "exponential_backoff",
            "freshness_ms": 25,
            "point_counts": {
                "total": 2,
                "good": 1,
                "degraded": 1,
                "unavailable": 0
            },
            "points": [
                {
                    "metadata": {
                        "name": "line1_temp",
                        "source": "MAIN.Temperature",
                        "data_type": "REAL",
                        "direction": "read"
                    },
                    "quality": "good",
                    "last_update_ms": 1234
                },
                {
                    "metadata": {
                        "name": "line1_cmd",
                        "source": "MAIN.Command",
                        "data_type": "BOOL",
                        "direction": "write"
                    },
                    "quality": "write_pending",
                    "detail": "queued"
                }
            ]
        })
    );
}

#[test]
fn stale_connector_state_and_stale_point_quality_are_distinct_fields() {
    let report = ConnectorStatusBuilder::new(
        "opcua:line1",
        ConnectorProtocol::Opcua,
        ConnectorKind::SupervisoryClient,
        ConnectorState::Stale,
        ConnectorHealth::Degraded,
    )
    .points(vec![ConnectorPointStatus {
        metadata: ConnectorPointMetadata {
            name: "ConveyorRunning".to_string(),
            source: Some("ns=2;i=2".to_string()),
            data_type: Some("BOOL".to_string()),
            direction: PointDirection::Read,
        },
        quality: PointQuality::Stale,
        last_update_ms: Some(99),
        detail: None,
    }])
    .build();

    let value = serde_json::to_value(&report).expect("serialize stale report");
    assert_eq!(value["state"], "stale");
    assert_eq!(value["points"][0]["quality"], "stale");
}

#[test]
fn io_driver_health_mapping_honors_error_policy() {
    let ok = io_driver_status(&IoDriverHealth::Ok, IoDriverErrorPolicy::Fault);
    assert_eq!(ok.state, ConnectorState::Ready);
    assert_eq!(ok.health, ConnectorHealth::Ok);

    let degraded = io_driver_status(
        &IoDriverHealth::Degraded {
            error: SmolStr::new("cycle overrun"),
        },
        IoDriverErrorPolicy::Fault,
    );
    assert_eq!(degraded.state, ConnectorState::Degraded);
    assert_eq!(degraded.health, ConnectorHealth::Degraded);
    assert_eq!(degraded.detail.as_deref(), Some("cycle overrun"));

    let faulted = io_driver_status(
        &IoDriverHealth::Faulted {
            error: SmolStr::new("device lost"),
        },
        IoDriverErrorPolicy::Fault,
    );
    assert_eq!(faulted.state, ConnectorState::Faulted);
    assert_eq!(faulted.health, ConnectorHealth::Faulted);

    let warn_projection = io_driver_status(
        &IoDriverHealth::Faulted {
            error: SmolStr::new("device lost"),
        },
        IoDriverErrorPolicy::Warn,
    );
    assert_eq!(warn_projection.state, ConnectorState::Degraded);
    assert_eq!(warn_projection.health, ConnectorHealth::Degraded);
}

#[test]
fn ads_state_mapping_covers_worker_and_report_states() {
    assert_eq!(
        ads_connection_state_status(AdsConnectionState::Connecting).state,
        ConnectorState::Starting
    );
    assert_eq!(
        ads_connection_state_status(AdsConnectionState::Connected).health,
        ConnectorHealth::Ok
    );
    assert_eq!(
        ads_connection_state_status(AdsConnectionState::Reconnecting).state,
        ConnectorState::Reconnecting
    );
    assert_eq!(
        ads_connection_state_status(AdsConnectionState::Faulted).health,
        ConnectorHealth::Faulted
    );

    let ready = ads_connection_status_state(AdsConnectionStatusState::Connected, 0);
    assert_eq!(ready.state, ConnectorState::Ready);
    assert_eq!(ready.health, ConnectorHealth::Ok);

    let degraded = ads_connection_status_state(AdsConnectionStatusState::Connected, 1);
    assert_eq!(degraded.state, ConnectorState::Degraded);
    assert_eq!(degraded.health, ConnectorHealth::Degraded);

    assert_eq!(
        ads_connection_status_state(AdsConnectionStatusState::Stale, 0).state,
        ConnectorState::Stale
    );
    assert_eq!(
        ads_connection_status_state(AdsConnectionStatusState::Unknown, 0).state,
        ConnectorState::NotReady
    );
    let not_ready = ads_connection_status_state(AdsConnectionStatusState::NotReady, 0);
    assert_eq!(not_ready.state, ConnectorState::NotReady);
    assert_eq!(not_ready.health, ConnectorHealth::Unknown);
}

#[test]
fn ads_status_report_projects_role_endpoint_and_point_counts() {
    let report = AdsStatusReport {
        schema_version: 2,
        role: DoctorRole::Client,
        overall: AdsStatusOverall::Healthy,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: vec![AdsConnectionStatus {
            name: "line1".to_string(),
            target: Some(TargetIdentity {
                name: Some("TwinCAT".to_string()),
                ip: "192.168.77.20".to_string(),
                ams_net_id: "5.23.91.12.1.1".to_string(),
                ams_port: 851,
                tc_version: Some("3.1".to_string()),
            }),
            state: AdsConnectionStatusState::Connected,
            point_count: 4,
            degraded_points: 1,
            last_good_value_ms: Some(1234),
            symbol_version: Some(7),
            summary: "connected".to_string(),
        }],
        summary: "ADS connections healthy.".to_string(),
    };

    let reports = project_ads_status_report(&report);

    assert_eq!(reports.len(), 1);
    let value = serde_json::to_value(&reports[0]).expect("serialize ADS connector");
    assert_eq!(value["connector_id"], "ads:client:5.23.91.12.1.1");
    assert_eq!(value["protocol"], "ads");
    assert_eq!(value["kind"], "supervisory_client");
    assert_eq!(value["endpoint"], "5.23.91.12.1.1:851@192.168.77.20");
    assert_eq!(value["state"], "degraded");
    assert_eq!(value["health"], "degraded");
    assert_eq!(value["confidence"], "confirmed");
    assert_eq!(value["reconnect_policy"], "fixed_delay");
    assert_eq!(
        value["point_counts"],
        json!({
            "total": 4,
            "good": 3,
            "degraded": 1,
            "unavailable": 0
        })
    );

    let disabled = AdsStatusReport {
        schema_version: 2,
        role: DoctorRole::Client,
        overall: AdsStatusOverall::Disabled,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: Vec::new(),
        summary: "ADS is not configured.".to_string(),
    };
    let disabled = project_ads_status_report(&disabled);
    assert_eq!(disabled.len(), 1);
    assert_eq!(disabled[0].connector_id, "ads:client");
    assert_eq!(disabled[0].state, ConnectorState::Disabled);
    assert_eq!(disabled[0].health, ConnectorHealth::Unknown);

    let server = AdsStatusReport {
        schema_version: 2,
        role: DoctorRole::Server,
        overall: AdsStatusOverall::Disabled,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: vec![AdsConnectionStatus {
            name: "ads-server".to_string(),
            target: None,
            state: AdsConnectionStatusState::Disabled,
            point_count: 0,
            degraded_points: 0,
            last_good_value_ms: None,
            symbol_version: None,
            summary: "ADS server exposes 0 symbol(s).".to_string(),
        }],
        summary: "ADS server is disabled.".to_string(),
    };
    let server = project_ads_status_report_with_default_endpoint(
        &server,
        Some("127.0.0.1.1.1:851@127.0.0.1"),
    );
    assert_eq!(server.len(), 1);
    assert_eq!(server[0].kind, ConnectorKind::SupervisoryServer);
    assert_eq!(
        server[0].endpoint.as_deref(),
        Some("127.0.0.1.1.1:851@127.0.0.1")
    );

    let not_ready_server = AdsStatusReport {
        schema_version: 2,
        role: DoctorRole::Server,
        overall: AdsStatusOverall::NotReady,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: vec![AdsConnectionStatus {
            name: "ads-server".to_string(),
            target: None,
            state: AdsConnectionStatusState::NotReady,
            point_count: 0,
            degraded_points: 0,
            last_good_value_ms: None,
            symbol_version: Some(1),
            summary: "ADS server is listening but not ready.".to_string(),
        }],
        summary: "ADS server is listening but not ready to serve symbols.".to_string(),
    };
    let not_ready_server = project_ads_status_report_with_default_endpoint(
        &not_ready_server,
        Some("127.0.0.1.1.1:851@127.0.0.1"),
    );
    assert_eq!(not_ready_server[0].state, ConnectorState::NotReady);
    assert_eq!(not_ready_server[0].health, ConnectorHealth::Unknown);
}

#[test]
fn ads_status_report_projects_reconnect_stale_fault_and_failure_details() {
    let cases = [
        (
            "reconnecting",
            AdsConnectionStatusState::Reconnecting,
            "retrying ADS route",
            ConnectorState::Reconnecting,
            ConnectorHealth::Degraded,
        ),
        (
            "stale",
            AdsConnectionStatusState::Stale,
            "last notification is stale",
            ConnectorState::Stale,
            ConnectorHealth::Degraded,
        ),
        (
            "faulted",
            AdsConnectionStatusState::Faulted,
            "ADS target faulted",
            ConnectorState::Faulted,
            ConnectorHealth::Faulted,
        ),
        (
            "route_failure",
            AdsConnectionStatusState::Faulted,
            "static route missing",
            ConnectorState::Faulted,
            ConnectorHealth::Faulted,
        ),
        (
            "auth_failure",
            AdsConnectionStatusState::Faulted,
            "ADS authentication failed",
            ConnectorState::Faulted,
            ConnectorHealth::Faulted,
        ),
    ];

    for (name, state, summary, expected_state, expected_health) in cases {
        let report = AdsStatusReport {
            schema_version: 2,
            role: DoctorRole::Client,
            overall: if state == AdsConnectionStatusState::Faulted {
                AdsStatusOverall::Faulted
            } else {
                AdsStatusOverall::Degraded
            },
            runtime_identity_hash: None,
            deployed_ads_config_hash: None,
            connections: vec![AdsConnectionStatus {
                name: name.to_string(),
                target: Some(TargetIdentity {
                    name: Some("TwinCAT".to_string()),
                    ip: "192.168.77.20".to_string(),
                    ams_net_id: format!("5.23.91.12.{}.1", name.len()),
                    ams_port: 851,
                    tc_version: None,
                }),
                state,
                point_count: 2,
                degraded_points: 0,
                last_good_value_ms: None,
                symbol_version: None,
                summary: summary.to_string(),
            }],
            summary: summary.to_string(),
        };

        let reports = project_ads_status_report(&report);

        assert_eq!(reports.len(), 1, "{name}");
        assert_eq!(reports[0].state, expected_state, "{name}");
        assert_eq!(reports[0].health, expected_health, "{name}");
        assert_eq!(reports[0].last_error.as_deref(), Some(summary), "{name}");
    }
}

#[test]
fn ads_point_statuses_project_into_connector_point_quality() {
    let statuses = vec![
        AdsPointStatus {
            point_name: "notify_temp".to_string(),
            quality: AdsPointQuality::good(10),
        },
        AdsPointStatus {
            point_name: "cold_read".to_string(),
            quality: AdsPointQuality::stale("waiting for first ADS update"),
        },
        AdsPointStatus {
            point_name: "pending_write".to_string(),
            quality: AdsPointQuality::stale("ADS write pending"),
        },
        AdsPointStatus {
            point_name: "bad_read".to_string(),
            quality: AdsPointQuality::error(20, "ADS read failed"),
        },
        AdsPointStatus {
            point_name: "failed_write".to_string(),
            quality: AdsPointQuality::error(30, "ADS write failed without detail"),
        },
    ];

    let projected = project_ads_point_statuses(&statuses);

    assert_eq!(projected.len(), 5);
    assert_eq!(projected[0].quality, PointQuality::Good);
    assert_eq!(projected[0].last_update_ms, Some(10));
    assert_eq!(projected[1].quality, PointQuality::Stale);
    assert_eq!(projected[2].quality, PointQuality::WritePending);
    assert_eq!(projected[3].quality, PointQuality::Bad);
    assert_eq!(projected[4].quality, PointQuality::WriteFailed);
    assert_eq!(
        projected[4].detail.as_deref(),
        Some("ADS write failed without detail")
    );
}

#[test]
fn active_ads_device_snapshot_projects_point_rows_and_counts() {
    let snapshot = ActiveAdsDeviceSnapshot {
        connection_name: "line1".to_string(),
        target: TargetIdentity {
            name: Some("TwinCAT".to_string()),
            ip: "192.168.77.20".to_string(),
            ams_net_id: "5.23.91.12.1.1".to_string(),
            ams_port: 851,
            tc_version: Some("3.1".to_string()),
        },
        local: None,
        state: AdsConnectionStatusState::Connected,
        point_statuses: vec![
            AdsPointStatus {
                point_name: "notify_temp".to_string(),
                quality: AdsPointQuality::good(10),
            },
            AdsPointStatus {
                point_name: "pending_write".to_string(),
                quality: AdsPointQuality::stale("ADS write pending"),
            },
        ],
        symbol_version: Some(7),
    };

    let report = project_active_ads_device_snapshot(&snapshot);
    let value = serde_json::to_value(report).expect("serialize active ADS connector");

    assert_eq!(value["connector_id"], "ads:client:5.23.91.12.1.1");
    assert_eq!(value["state"], "degraded");
    assert_eq!(value["health"], "degraded");
    assert_eq!(
        value["point_counts"],
        json!({
            "total": 2,
            "good": 1,
            "degraded": 1,
            "unavailable": 0
        })
    );
    assert_eq!(value["points"][0]["quality"], "good");
    assert_eq!(value["points"][1]["quality"], "write_pending");
}

#[test]
fn opcua_mapping_covers_client_and_server_states() {
    let configured = opcua_client_status(OpcUaClientConnectionState::Configured, 0);
    assert_eq!(configured.state, ConnectorState::Configured);
    assert_eq!(configured.health, ConnectorHealth::Unknown);

    let ready = opcua_client_status(OpcUaClientConnectionState::Connected, 0);
    assert_eq!(ready.state, ConnectorState::Ready);
    assert_eq!(ready.health, ConnectorHealth::Ok);

    let degraded = opcua_client_status(OpcUaClientConnectionState::Connected, 2);
    assert_eq!(degraded.state, ConnectorState::Degraded);
    assert_eq!(degraded.health, ConnectorHealth::Degraded);

    assert_eq!(
        opcua_client_status(OpcUaClientConnectionState::Stale, 0).state,
        ConnectorState::Stale
    );
    assert_eq!(
        opcua_server_snapshot_status(OpcUaServerSnapshotState::NoSnapshot).state,
        ConnectorState::NotReady
    );
    assert_eq!(
        opcua_server_snapshot_status(OpcUaServerSnapshotState::SnapshotReady).health,
        ConnectorHealth::Ok
    );
}

#[test]
fn opcua_client_status_projects_point_quality_and_metadata() {
    let reports = project_opcua_client_status_report(&OpcUaClientStatusReport {
        enabled: true,
        deployed_config_hash: Some("abc123".to_string()),
        connections: vec![OpcUaClientConnectionStatus {
            name: SmolStr::new("line1"),
            endpoint_url: "opc.tcp://127.0.0.1:4840/trust".to_string(),
            state: OpcUaClientConnectionState::Connected,
            point_count: 2,
            degraded_points: 1,
            last_seen_ms: Some(1_200),
            detail: "subscription active with one stale point".to_string(),
            points: vec![
                OpcUaClientPointStatus {
                    var: SmolStr::new("line1_temp"),
                    node_id: "ns=2;i=2".to_string(),
                    data_type: OpcUaDataType::Float,
                    access: OpcUaClientPointAccess::Read,
                    state: OpcUaClientConnectionState::Connected,
                    last_seen_ms: Some(1_200),
                    value: None,
                    detail: "fresh subscription update".to_string(),
                },
                OpcUaClientPointStatus {
                    var: SmolStr::new("line1_setpoint"),
                    node_id: "ns=2;i=3".to_string(),
                    data_type: OpcUaDataType::Double,
                    access: OpcUaClientPointAccess::ReadWrite,
                    state: OpcUaClientConnectionState::Stale,
                    last_seen_ms: Some(900),
                    value: None,
                    detail: "server stopped publishing this node".to_string(),
                },
            ],
        }],
    });

    let value = serde_json::to_value(&reports[0]).expect("serialize OPC UA connector");
    assert_eq!(value["connector_id"], "opcua:client:line1");
    assert_eq!(value["protocol"], "opcua");
    assert_eq!(value["kind"], "supervisory_client");
    assert_eq!(value["state"], "degraded");
    assert_eq!(value["health"], "degraded");
    assert_eq!(value["confidence"], "confirmed");
    assert_eq!(value["endpoint"], "opc.tcp://127.0.0.1:4840/trust");
    assert_eq!(
        value["point_counts"],
        json!({
            "total": 2,
            "good": 1,
            "degraded": 1,
            "unavailable": 0
        })
    );
    assert_eq!(value["points"][0]["metadata"]["name"], "line1_temp");
    assert_eq!(value["points"][0]["metadata"]["source"], "ns=2;i=2");
    assert_eq!(value["points"][0]["metadata"]["data_type"], "REAL");
    assert_eq!(value["points"][0]["metadata"]["direction"], "read");
    assert_eq!(value["points"][0]["quality"], "good");
    assert_eq!(value["points"][1]["metadata"]["direction"], "read_write");
    assert_eq!(value["points"][1]["quality"], "stale");
}

#[test]
fn process_image_protocol_mappings_cover_mqtt_modbus_and_ethercat() {
    assert_eq!(
        serde_json::to_value(ConnectorProtocol::Unknown).expect("serialize unknown protocol"),
        json!("unknown")
    );
    assert_eq!(
        mqtt_session_status(MqttSessionProjection::ConnectedFresh).state,
        ConnectorState::Ready
    );
    assert_eq!(
        mqtt_session_status(MqttSessionProjection::ConnectedStale).state,
        ConnectorState::Stale
    );
    assert_eq!(
        modbus_status(ModbusProjection::Timeout).health,
        ConnectorHealth::Degraded
    );
    assert_eq!(
        modbus_status(ModbusProjection::Faulted).state,
        ConnectorState::Faulted
    );
    assert_eq!(
        ethercat_status(EthercatProjection::Operational).health,
        ConnectorHealth::Ok
    );
    assert_eq!(
        ethercat_status(EthercatProjection::Reconnecting).state,
        ConnectorState::Reconnecting
    );
}

#[test]
fn discovery_confidence_serializes_honest_tcp_only_label() {
    let value =
        serde_json::to_value(DiscoveryConfidence::PortReachable).expect("serialize confidence");
    assert_eq!(value, json!("port_reachable"));
}
