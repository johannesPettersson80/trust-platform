use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;
use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, PointAccess, TransportSecurity};

use super::*;
use crate::ads::diagnostics::{
    AdsConnectionStatusState, AdsStatusOverall, DoctorRole, TargetIdentity,
};

fn ads_config() -> AdsClientConfig {
    crate::ads::parse_ads_toml(
        r#"
[[connections]]
name = "line-a"
target_net_id = "5.23.91.12.1.1"
host = "192.0.2.10"
ams_port = 851
local_net_id = "192.0.2.100.1.1"
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Ready"
var = "line_a_ready"
type = "BOOL"
access = "read"

[[connections.points]]
index_group = 16416
index_offset = 4
size = 2
var = "line_a_command"
type = "WORD"
access = "write"

[[connections]]
name = "line-b"
target_net_id = "5.23.91.13.1.1"
host = "192.0.2.11"
ams_port = 852
auto_add_route = true

[[connections.points]]
symbol = "MAIN.Speed"
var = "line_b_speed"
type = "REAL"
access = "read_write"
"#,
    )
    .expect("valid ADS topology fixture")
}

fn ads_connection_status(
    name: &str,
    target_net_id: &str,
    ams_port: u16,
    state: AdsConnectionStatusState,
    degraded_points: usize,
    last_seen_ms: Option<u64>,
) -> AdsConnectionStatus {
    AdsConnectionStatus {
        name: name.to_string(),
        target: Some(TargetIdentity {
            name: None,
            ip: "192.0.2.20".to_string(),
            ams_net_id: target_net_id.to_string(),
            ams_port,
            tc_version: None,
        }),
        state,
        point_count: 3,
        degraded_points,
        last_good_value_ms: last_seen_ms,
        symbol_version: None,
        summary: format!("{state:?}"),
    }
}

fn ads_status(connections: Vec<AdsConnectionStatus>) -> AdsStatusReport {
    AdsStatusReport {
        schema_version: 1,
        role: DoctorRole::Client,
        overall: AdsStatusOverall::Healthy,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections,
        summary: "test status".to_string(),
    }
}

fn opcua_config() -> OpcUaClientConfig {
    crate::opcua::parse_opcua_client_toml(
        r#"
[[connections]]
name = "line-a"
endpoint_url = "opc.tcp://192.0.2.10:4840/a"
security_policy = "none"
security_mode = "none"
auth = "anonymous"
poll_interval_ms = 250
timeout_ms = 2000

[[connections.points]]
var = "line_a_ready"
node_id = "ns=2;s=Ready"
type = "bool"
access = "read"

[[connections]]
name = "line-b"
endpoint_url = "opc.tcp://192.0.2.11:4840/b"
security_policy = "basic256sha256"
security_mode = "sign_and_encrypt"
auth = "username"
username = "operator"
password = "topology-must-not-leak"
trust_server_certificate = true
poll_interval_ms = 500
timeout_ms = 3000

[[connections.points]]
var = "line_b_command"
node_id = "ns=3;s=Command"
type = "int32"
access = "read_write"
"#,
    )
    .expect("valid OPC UA topology fixture")
}

fn opcua_connection_status(
    name: &str,
    endpoint_url: &str,
    state: OpcUaClientConnectionState,
    degraded_points: usize,
    last_seen_ms: Option<u64>,
) -> crate::opcua::OpcUaClientConnectionStatus {
    crate::opcua::OpcUaClientConnectionStatus {
        name: SmolStr::new(name),
        endpoint_url: endpoint_url.to_string(),
        state,
        point_count: 2,
        degraded_points,
        last_seen_ms,
        detail: format!("{state:?}"),
        points: Vec::new(),
    }
}

fn opcua_status(
    connections: Vec<crate::opcua::OpcUaClientConnectionStatus>,
) -> OpcUaClientStatusReport {
    OpcUaClientStatusReport {
        enabled: true,
        deployed_config_hash: None,
        connections,
    }
}

#[test]
fn fleet_protocol_contract_ads_access_labels_are_closed() {
    assert_eq!(ads_access_name(PointAccess::Read), "read");
    assert_eq!(ads_access_name(PointAccess::Write), "write");
    assert_eq!(ads_access_name(PointAccess::ReadWrite), "read_write");
}

#[test]
fn fleet_protocol_contract_ads_symbol_reference_keeps_symbol_only() {
    let address = crate::ads::AdsPointAddress::Symbol("MAIN.Ready".to_string());
    assert_eq!(
        ads_point_external_ref(&address),
        (Some("MAIN.Ready".to_string()), None)
    );
}

#[test]
fn fleet_protocol_contract_ads_index_reference_keeps_exact_address_only() {
    let address = crate::ads::AdsPointAddress::Index {
        index_group: 0x4020,
        index_offset: 0x10,
        size: 4,
    };
    assert_eq!(
        ads_point_external_ref(&address),
        (None, Some("index 0x4020:0x10 · 4 bytes".to_string()))
    );
}

#[test]
fn fleet_protocol_contract_ads_type_prefers_source_name() {
    let descriptor = AdsDataTypeDescriptor::scalar("MY_REAL_ALIAS", IecDataType::Real);
    assert_eq!(ads_type_name(&descriptor), "MY_REAL_ALIAS");
}

#[test]
fn fleet_protocol_contract_ads_type_falls_back_to_uppercase_iec_name() {
    let descriptor = AdsDataTypeDescriptor::scalar("", IecDataType::Dint);
    assert_eq!(ads_type_name(&descriptor), "DINT");
}

#[test]
fn fleet_protocol_contract_ads_params_preserve_connection_order_and_identity() {
    let value = ads_client_params(&ads_config());
    assert_eq!(value["connections"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["connections"][0]["name"], "line-a");
    assert_eq!(value["connections"][0]["target_net_id"], "5.23.91.12.1.1");
    assert_eq!(value["connections"][0]["host"], "192.0.2.10");
    assert_eq!(value["connections"][0]["ams_port"], 851);
    assert_eq!(value["connections"][0]["local_net_id_set"], true);
    assert_eq!(value["connections"][0]["transport"], "plain");
    assert_eq!(value["connections"][1]["name"], "line-b");
    assert_eq!(value["connections"][1]["transport"], "secure");
    assert_eq!(value["connections"][1]["auto_add_route"], true);
}

#[test]
fn fleet_protocol_contract_ads_params_keep_symbol_and_index_points_distinct() {
    let value = ads_client_params(&ads_config());
    let points = value["connections"][0]["points"]
        .as_array()
        .expect("ADS points");
    assert_eq!(
        points[0],
        json!({
            "var": "line_a_ready",
            "symbol": "MAIN.Ready",
            "address": null,
            "type": "BOOL",
            "access": "read",
        })
    );
    assert_eq!(points[1]["var"], "line_a_command");
    assert_eq!(points[1]["symbol"], JsonValue::Null);
    assert_eq!(points[1]["address"], "index 0x4020:0x4 · 2 bytes");
    assert_eq!(points[1]["type"], "WORD");
    assert_eq!(points[1]["access"], "write");
}

#[test]
fn fleet_protocol_contract_ads_params_do_not_serialize_route_credentials() {
    let payload = ads_client_params(&ads_config()).to_string();
    for forbidden in ["password", "credential", "auth_token", "secret"] {
        assert!(
            !payload.to_ascii_lowercase().contains(forbidden),
            "{payload}"
        );
    }
}

#[test]
fn fleet_protocol_contract_ads_local_net_id_uses_first_configured_identity() {
    assert_eq!(
        ads_client_local_net_id(&ads_config()),
        Some("192.0.2.100.1.1".to_string())
    );
}

#[test]
fn fleet_protocol_contract_ads_local_net_id_is_absent_when_unconfigured() {
    let mut config = ads_config();
    for connection in &mut config.connections {
        connection.route.local_net_id = None;
    }
    assert_eq!(ads_client_local_net_id(&config), None);
}

#[test]
fn fleet_protocol_contract_ads_status_matches_exact_connection_name() {
    let config = ads_config();
    let report = ads_status(vec![ads_connection_status(
        "line-a",
        "1.2.3.4.5.6",
        1,
        AdsConnectionStatusState::Connected,
        0,
        Some(10),
    )]);
    assert_eq!(
        ads_status_for_connection(Some(&report), &config.connections[0])
            .map(|item| item.name.as_str()),
        Some("line-a")
    );
}

#[test]
fn fleet_protocol_contract_ads_status_falls_back_to_target_identity() {
    let config = ads_config();
    let report = ads_status(vec![ads_connection_status(
        "renamed-runtime-row",
        "5.23.91.12.1.1",
        851,
        AdsConnectionStatusState::Connected,
        0,
        Some(10),
    )]);
    assert!(ads_status_for_connection(Some(&report), &config.connections[0]).is_some());
}

#[test]
fn fleet_protocol_contract_ads_status_rejects_partial_target_identity() {
    let config = ads_config();
    let wrong_port = ads_status(vec![ads_connection_status(
        "other",
        "5.23.91.12.1.1",
        852,
        AdsConnectionStatusState::Connected,
        0,
        Some(10),
    )]);
    let wrong_net_id = ads_status(vec![ads_connection_status(
        "other",
        "5.23.91.99.1.1",
        851,
        AdsConnectionStatusState::Connected,
        0,
        Some(10),
    )]);
    assert!(ads_status_for_connection(Some(&wrong_port), &config.connections[0]).is_none());
    assert!(ads_status_for_connection(Some(&wrong_net_id), &config.connections[0]).is_none());
}

#[test]
fn fleet_protocol_contract_ads_exact_name_precedes_conflicting_target_match() {
    let config = ads_config();
    let report = ads_status(vec![
        ads_connection_status(
            "stale-row",
            "5.23.91.12.1.1",
            851,
            AdsConnectionStatusState::Faulted,
            0,
            Some(1),
        ),
        ads_connection_status(
            "line-a",
            "9.9.9.9.9.9",
            999,
            AdsConnectionStatusState::Connected,
            0,
            Some(2),
        ),
    ]);
    let matched =
        ads_status_for_connection(Some(&report), &config.connections[0]).expect("matched row");
    assert_eq!(matched.name, "line-a");
}

#[test]
fn fleet_protocol_contract_ads_connection_health_maps_connected() {
    let status = ads_connection_status(
        "line",
        "5.23.91.12.1.1",
        851,
        AdsConnectionStatusState::Connected,
        0,
        Some(1),
    );
    assert_eq!(ads_connection_status_health(&status), "connected");
}

#[test]
fn fleet_protocol_contract_ads_connection_health_degrades_connected_row_with_bad_points() {
    let status = ads_connection_status(
        "line",
        "5.23.91.12.1.1",
        851,
        AdsConnectionStatusState::Connected,
        1,
        Some(1),
    );
    assert_eq!(ads_connection_status_health(&status), "degraded");
}

#[test]
fn fleet_protocol_contract_ads_connection_health_maps_transitional_states() {
    for state in [
        AdsConnectionStatusState::Reconnecting,
        AdsConnectionStatusState::NotReady,
        AdsConnectionStatusState::Stale,
    ] {
        let status = ads_connection_status("line", "5.23.91.12.1.1", 851, state, 0, Some(1));
        assert_eq!(
            ads_connection_status_health(&status),
            "degraded",
            "{state:?}"
        );
    }
}

#[test]
fn fleet_protocol_contract_ads_connection_health_maps_fault_and_policy_states() {
    let faulted = ads_connection_status(
        "line",
        "5.23.91.12.1.1",
        851,
        AdsConnectionStatusState::Faulted,
        0,
        Some(1),
    );
    assert_eq!(ads_connection_status_health(&faulted), "error");
    for state in [
        AdsConnectionStatusState::Disabled,
        AdsConnectionStatusState::Unknown,
    ] {
        let status = ads_connection_status("line", "5.23.91.12.1.1", 851, state, 0, Some(1));
        assert_eq!(
            ads_connection_status_health(&status),
            "configured_policy",
            "{state:?}"
        );
    }
}

#[test]
fn fleet_protocol_contract_ads_aggregate_without_report_is_configured_policy() {
    let config = ads_config();
    let (health, detail) = ads_client_endpoint_health_and_detail(&config, None);
    assert_eq!(health, "configured_policy");
    assert!(detail.contains("2 ADS connection(s) configured"));
}

#[test]
fn fleet_protocol_contract_ads_aggregate_without_matching_rows_is_configured_policy() {
    let config = ads_config();
    let report = ads_status(vec![ads_connection_status(
        "other",
        "9.9.9.9.9.9",
        999,
        AdsConnectionStatusState::Connected,
        0,
        Some(1),
    )]);
    assert_eq!(
        ads_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "configured_policy"
    );
}

#[test]
fn fleet_protocol_contract_ads_partial_match_is_degraded() {
    let config = ads_config();
    let report = ads_status(vec![ads_connection_status(
        "line-a",
        "5.23.91.12.1.1",
        851,
        AdsConnectionStatusState::Connected,
        0,
        Some(1),
    )]);
    assert_eq!(
        ads_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "degraded"
    );
}

#[test]
fn fleet_protocol_contract_ads_aggregate_fault_dominates() {
    let config = ads_config();
    let report = ads_status(vec![
        ads_connection_status(
            "line-a",
            "5.23.91.12.1.1",
            851,
            AdsConnectionStatusState::Connected,
            0,
            Some(1),
        ),
        ads_connection_status(
            "line-b",
            "5.23.91.13.1.1",
            852,
            AdsConnectionStatusState::Faulted,
            0,
            Some(2),
        ),
    ]);
    assert_eq!(
        ads_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "error"
    );
}

#[test]
fn fleet_protocol_contract_ads_aggregate_all_matching_healthy_is_connected() {
    let config = ads_config();
    let report = ads_status(vec![
        ads_connection_status(
            "line-a",
            "5.23.91.12.1.1",
            851,
            AdsConnectionStatusState::Connected,
            0,
            Some(1),
        ),
        ads_connection_status(
            "line-b",
            "5.23.91.13.1.1",
            852,
            AdsConnectionStatusState::Connected,
            0,
            Some(2),
        ),
    ]);
    assert_eq!(
        ads_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "connected"
    );
}

#[test]
fn fleet_protocol_contract_ads_live_keeps_configured_denominator_and_order() {
    let config = ads_config();
    let report = ads_status(vec![ads_connection_status(
        "line-b",
        "5.23.91.13.1.1",
        852,
        AdsConnectionStatusState::Connected,
        0,
        Some(20),
    )]);
    let live = ads_client_live(&config, Some(&report)).expect("live projection");
    assert_eq!(live["value"]["total"], 2);
    assert_eq!(live["value"]["connections"][0]["name"], "line-a");
    assert_eq!(live["value"]["connections"][0]["state"], JsonValue::Null);
    assert_eq!(live["value"]["connections"][1]["name"], "line-b");
    assert_eq!(live["value"]["connections"][1]["state"], "connected");
}

#[test]
fn fleet_protocol_contract_ads_live_ignores_unconfigured_status_rows_in_count_and_freshness() {
    let config = ads_config();
    let report = ads_status(vec![
        ads_connection_status(
            "line-a",
            "5.23.91.12.1.1",
            851,
            AdsConnectionStatusState::Connected,
            0,
            Some(10),
        ),
        ads_connection_status(
            "line-b",
            "5.23.91.13.1.1",
            852,
            AdsConnectionStatusState::Connected,
            0,
            Some(20),
        ),
        ads_connection_status(
            "stale-extra",
            "9.9.9.9.9.9",
            999,
            AdsConnectionStatusState::Connected,
            0,
            Some(999),
        ),
    ]);
    let live = ads_client_live(&config, Some(&report)).expect("live projection");
    assert_eq!(live["value"]["connected"], 2);
    assert_eq!(live["last_seen_ms"], 20);
}

#[test]
fn fleet_protocol_contract_ads_live_is_absent_without_report() {
    assert_eq!(ads_client_live(&ads_config(), None), None);
}

#[test]
fn fleet_protocol_contract_ads_route_security_follows_transport() {
    let config = ads_config();
    assert!(!ads_route_secure(&config.connections[0]));
    assert_eq!(
        config.connections[1].route.security.transport,
        TransportSecurity::Secure
    );
    assert!(ads_route_secure(&config.connections[1]));
}

#[test]
fn fleet_protocol_contract_opcua_params_preserve_connections_and_security() {
    let value = opcua_client_params(&opcua_config());
    assert_eq!(value["connections"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["connections"][0]["name"], "line-a");
    assert_eq!(value["connections"][0]["security_policy"], "none");
    assert_eq!(value["connections"][0]["security_mode"], "none");
    assert_eq!(value["connections"][0]["auth"], "anonymous");
    assert_eq!(value["connections"][0]["username_set"], false);
    assert_eq!(value["connections"][1]["security_policy"], "basic256sha256");
    assert_eq!(value["connections"][1]["security_mode"], "sign_and_encrypt");
    assert_eq!(value["connections"][1]["auth"], "username");
    assert_eq!(value["connections"][1]["username_set"], true);
    assert_eq!(value["connections"][1]["trust_server_certificate"], true);
}

#[test]
fn fleet_protocol_contract_opcua_params_preserve_point_access_and_writability() {
    let value = opcua_client_params(&opcua_config());
    assert_eq!(value["connections"][0]["points"][0]["access"], "read");
    assert_eq!(value["connections"][0]["points"][0]["writable"], false);
    assert_eq!(value["connections"][1]["points"][0]["access"], "read_write");
    assert_eq!(value["connections"][1]["points"][0]["writable"], true);
}

#[test]
fn fleet_protocol_contract_opcua_params_never_serialize_username_or_password() {
    let payload = opcua_client_params(&opcua_config()).to_string();
    assert!(!payload.contains("operator"), "{payload}");
    assert!(!payload.contains("topology-must-not-leak"), "{payload}");
    assert!(!payload.contains("\"password\""), "{payload}");
}

#[test]
fn fleet_protocol_contract_opcua_primary_endpoint_is_first_configured() {
    assert_eq!(
        opcua_client_primary_endpoint(&opcua_config()),
        Some("opc.tcp://192.0.2.10:4840/a".to_string())
    );
    assert_eq!(
        opcua_client_primary_endpoint(&OpcUaClientConfig {
            connections: Vec::new()
        }),
        None
    );
}

#[test]
fn fleet_protocol_contract_opcua_state_labels_are_closed() {
    let cases = [
        (OpcUaClientConnectionState::Disabled, "disabled"),
        (OpcUaClientConnectionState::Configured, "configured_policy"),
        (OpcUaClientConnectionState::Connecting, "connecting"),
        (OpcUaClientConnectionState::Connected, "connected"),
        (OpcUaClientConnectionState::Reconnecting, "reconnecting"),
        (OpcUaClientConnectionState::Stale, "stale"),
        (OpcUaClientConnectionState::Faulted, "error"),
    ];
    for (state, expected) in cases {
        assert_eq!(opcua_client_state_label(state), expected, "{state:?}");
    }
}

#[test]
fn fleet_protocol_contract_opcua_connection_health_maps_all_states() {
    let cases = [
        (OpcUaClientConnectionState::Disabled, 0, "configured_policy"),
        (
            OpcUaClientConnectionState::Configured,
            0,
            "configured_policy",
        ),
        (OpcUaClientConnectionState::Connecting, 0, "degraded"),
        (OpcUaClientConnectionState::Connected, 0, "connected"),
        (OpcUaClientConnectionState::Connected, 1, "degraded"),
        (OpcUaClientConnectionState::Reconnecting, 0, "degraded"),
        (OpcUaClientConnectionState::Stale, 0, "degraded"),
        (OpcUaClientConnectionState::Faulted, 0, "error"),
    ];
    for (state, degraded, expected) in cases {
        let status = opcua_connection_status("line", "opc.tcp://plc:4840", state, degraded, None);
        assert_eq!(
            opcua_client_connection_health(&status),
            expected,
            "{state:?}/{degraded}"
        );
    }
}

#[test]
fn fleet_protocol_contract_opcua_status_matches_exact_name() {
    let config = opcua_config();
    let report = opcua_status(vec![opcua_connection_status(
        "line-a",
        "opc.tcp://wrong:4840",
        OpcUaClientConnectionState::Connected,
        0,
        Some(1),
    )]);
    assert!(opcua_client_status_for_connection(Some(&report), &config.connections[0]).is_some());
}

#[test]
fn fleet_protocol_contract_opcua_status_falls_back_to_endpoint() {
    let config = opcua_config();
    let report = opcua_status(vec![opcua_connection_status(
        "renamed",
        "opc.tcp://192.0.2.10:4840/a",
        OpcUaClientConnectionState::Connected,
        0,
        Some(1),
    )]);
    assert!(opcua_client_status_for_connection(Some(&report), &config.connections[0]).is_some());
}

#[test]
fn fleet_protocol_contract_opcua_exact_name_precedes_conflicting_endpoint_match() {
    let config = opcua_config();
    let report = opcua_status(vec![
        opcua_connection_status(
            "stale",
            "opc.tcp://192.0.2.10:4840/a",
            OpcUaClientConnectionState::Faulted,
            0,
            Some(1),
        ),
        opcua_connection_status(
            "line-a",
            "opc.tcp://different:4840",
            OpcUaClientConnectionState::Connected,
            0,
            Some(2),
        ),
    ]);
    let matched = opcua_client_status_for_connection(Some(&report), &config.connections[0])
        .expect("matched status");
    assert_eq!(matched.name, "line-a");
}

#[test]
fn fleet_protocol_contract_opcua_aggregate_without_report_or_match_is_policy() {
    let config = opcua_config();
    assert_eq!(
        opcua_client_endpoint_health_and_detail(&config, None).0,
        "configured_policy"
    );
    let report = opcua_status(vec![opcua_connection_status(
        "other",
        "opc.tcp://other:4840",
        OpcUaClientConnectionState::Connected,
        0,
        Some(1),
    )]);
    assert_eq!(
        opcua_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "configured_policy"
    );
}

#[test]
fn fleet_protocol_contract_opcua_partial_match_is_degraded() {
    let config = opcua_config();
    let report = opcua_status(vec![opcua_connection_status(
        "line-a",
        "opc.tcp://192.0.2.10:4840/a",
        OpcUaClientConnectionState::Connected,
        0,
        Some(1),
    )]);
    assert_eq!(
        opcua_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "degraded"
    );
}

#[test]
fn fleet_protocol_contract_opcua_fault_dominates_aggregate() {
    let config = opcua_config();
    let report = opcua_status(vec![
        opcua_connection_status(
            "line-a",
            "opc.tcp://192.0.2.10:4840/a",
            OpcUaClientConnectionState::Connected,
            0,
            Some(1),
        ),
        opcua_connection_status(
            "line-b",
            "opc.tcp://192.0.2.11:4840/b",
            OpcUaClientConnectionState::Faulted,
            0,
            Some(2),
        ),
    ]);
    assert_eq!(
        opcua_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "error"
    );
}

#[test]
fn fleet_protocol_contract_opcua_all_matching_healthy_is_connected() {
    let config = opcua_config();
    let report = opcua_status(vec![
        opcua_connection_status(
            "line-a",
            "opc.tcp://192.0.2.10:4840/a",
            OpcUaClientConnectionState::Connected,
            0,
            Some(1),
        ),
        opcua_connection_status(
            "line-b",
            "opc.tcp://192.0.2.11:4840/b",
            OpcUaClientConnectionState::Connected,
            0,
            Some(2),
        ),
    ]);
    assert_eq!(
        opcua_client_endpoint_health_and_detail(&config, Some(&report)).0,
        "connected"
    );
}

#[test]
fn fleet_protocol_contract_opcua_live_keeps_configured_denominator_and_null_unmatched_row() {
    let config = opcua_config();
    let report = opcua_status(vec![opcua_connection_status(
        "line-b",
        "opc.tcp://192.0.2.11:4840/b",
        OpcUaClientConnectionState::Connected,
        0,
        Some(20),
    )]);
    let live = opcua_client_live(&config, Some(&report)).expect("live projection");
    assert_eq!(live["value"]["total"], 2);
    assert_eq!(live["value"]["connections"][0]["name"], "line-a");
    assert_eq!(live["value"]["connections"][0]["state"], JsonValue::Null);
    assert_eq!(live["value"]["connections"][1]["name"], "line-b");
    assert_eq!(live["value"]["connections"][1]["state"], "connected");
}

#[test]
fn fleet_protocol_contract_opcua_live_ignores_unconfigured_rows_in_count_and_freshness() {
    let config = opcua_config();
    let report = opcua_status(vec![
        opcua_connection_status(
            "line-a",
            "opc.tcp://192.0.2.10:4840/a",
            OpcUaClientConnectionState::Connected,
            0,
            Some(10),
        ),
        opcua_connection_status(
            "line-b",
            "opc.tcp://192.0.2.11:4840/b",
            OpcUaClientConnectionState::Connected,
            0,
            Some(20),
        ),
        opcua_connection_status(
            "stale-extra",
            "opc.tcp://old:4840",
            OpcUaClientConnectionState::Connected,
            0,
            Some(999),
        ),
    ]);
    let live = opcua_client_live(&config, Some(&report)).expect("live projection");
    assert_eq!(live["value"]["connected"], 2);
    assert_eq!(live["last_seen_ms"], 20);
}

#[test]
fn fleet_protocol_contract_opcua_live_is_absent_without_report() {
    assert_eq!(opcua_client_live(&opcua_config(), None), None);
}

#[test]
fn fleet_protocol_contract_realtime_errors_dominate_warnings_and_activity() {
    let mut status = LinuxRtRuntimeStatus::from_config(Default::default());
    status.active = true;
    status.warnings.push(SmolStr::new("warning"));
    status.errors.push(SmolStr::new("failure"));
    let (health, detail) = realtime_health_and_detail(&status);
    assert_eq!(health, "error");
    assert!(detail.contains("failure"));
}

#[test]
fn fleet_protocol_contract_realtime_inactive_or_warning_state_is_degraded() {
    let inactive = LinuxRtRuntimeStatus::from_config(Default::default());
    assert_eq!(realtime_health_and_detail(&inactive).0, "degraded");

    let mut warning = LinuxRtRuntimeStatus::from_config(Default::default());
    warning.active = true;
    warning.warnings.push(SmolStr::new("mlock unavailable"));
    assert_eq!(realtime_health_and_detail(&warning).0, "degraded");
}

#[test]
fn fleet_protocol_contract_realtime_active_without_findings_is_connected() {
    let mut status = LinuxRtRuntimeStatus::from_config(Default::default());
    status.active = true;
    status.warnings.clear();
    status.errors.clear();
    assert_eq!(
        realtime_health_and_detail(&status),
        (
            "connected".to_string(),
            "Realtime posture evidence is active.".to_string()
        )
    );
}
