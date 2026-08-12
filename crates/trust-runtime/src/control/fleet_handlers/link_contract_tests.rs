use smol_str::SmolStr;

use super::*;
use crate::ads::diagnostics::{
    AdsConnectionStatusState, AdsStatusOverall, DoctorRole, TargetIdentity,
};

fn params(source: &str) -> toml::Value {
    toml::from_str(source).expect("valid test TOML")
}

fn driver_config(name: &str, enabled: bool, params: toml::Value) -> IoDriverConfig {
    IoDriverConfig {
        name: SmolStr::new(name),
        params,
        enabled,
    }
}

fn driver_status(name: &str, health: IoDriverHealth) -> IoDriverStatus {
    IoDriverStatus {
        name: SmolStr::new(name),
        health,
    }
}

fn ads_config() -> AdsClientConfig {
    crate::ads::parse_ads_toml(
        r#"
[[connections]]
name = "line"
target_net_id = "5.23.91.12.1.1"
host = "192.0.2.10"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Ready"
var = "line_ready"
type = "BOOL"
"#,
    )
    .expect("valid ADS config")
}

fn ads_status(state: AdsConnectionStatusState) -> AdsStatusReport {
    AdsStatusReport {
        schema_version: 1,
        role: DoctorRole::Client,
        overall: AdsStatusOverall::Healthy,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: vec![AdsConnectionStatus {
            name: "line".to_string(),
            target: Some(TargetIdentity {
                name: None,
                ip: "192.0.2.10".to_string(),
                ams_net_id: "5.23.91.12.1.1".to_string(),
                ams_port: 851,
                tc_version: None,
            }),
            state,
            point_count: 1,
            degraded_points: 0,
            last_good_value_ms: Some(10),
            symbol_version: None,
            summary: format!("{state:?}"),
        }],
        summary: "status".to_string(),
    }
}

fn opcua_config(policy: &str, mode: &str) -> OpcUaClientConfig {
    crate::opcua::parse_opcua_client_toml(
        format!(
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://192.0.2.10:4840/line"
security_policy = "{policy}"
security_mode = "{mode}"

[[connections.points]]
var = "line_ready"
node_id = "ns=2;s=Ready"
type = "bool"
"#
        )
        .as_str(),
    )
    .expect("valid OPC UA config")
}

fn opcua_status(state: OpcUaClientConnectionState) -> OpcUaClientStatusReport {
    OpcUaClientStatusReport {
        enabled: true,
        deployed_config_hash: None,
        connections: vec![crate::opcua::OpcUaClientConnectionStatus {
            name: SmolStr::new("line"),
            endpoint_url: "opc.tcp://192.0.2.10:4840/line".to_string(),
            state,
            point_count: 1,
            degraded_points: 0,
            last_seen_ms: Some(10),
            detail: format!("{state:?}"),
            points: Vec::new(),
        }],
    }
}

#[test]
fn fleet_link_contract_id_sanitizes_components_in_stable_order() {
    assert_eq!(
        link_id(
            "endpoint:Runtime A:MQTT",
            "shared:mqtt:broker.local:1883",
            "MQTT",
            "publish_subscribe"
        ),
        "link:mqtt:publish-subscribe:endpoint-runtime-a-mqtt:shared-mqtt-broker-local-1883"
    );
}

#[test]
fn fleet_link_contract_projection_retains_unsanitized_fields() {
    let link = fleet_link(
        "endpoint:runtime:mqtt".to_string(),
        "shared:mqtt:broker.local:1883".to_string(),
        "mqtt",
        "publish_subscribe",
        "publish_subscribe",
        false,
        "configured_policy".to_string(),
        true,
        Some("broker.local:1883".to_string()),
    );
    assert_eq!(link.from, "endpoint:runtime:mqtt");
    assert_eq!(link.to, "shared:mqtt:broker.local:1883");
    assert_eq!(link.protocol, "mqtt");
    assert_eq!(link.role, "publish_subscribe");
    assert_eq!(link.direction, "publish_subscribe");
    assert!(!link.same_host);
    assert_eq!(link.status, "configured_policy");
    assert!(link.secure);
    assert_eq!(link.detail.as_deref(), Some("broker.local:1883"));
}

#[test]
fn fleet_link_contract_driver_adapter_is_trimmed() {
    assert_eq!(
        driver_adapter(&params(r#"adapter = " eth0 ""#)),
        Some("eth0".to_string())
    );
}

#[test]
fn fleet_link_contract_driver_adapter_rejects_empty_or_non_string() {
    assert_eq!(driver_adapter(&params(r#"adapter = " ""#)), None);
    assert_eq!(driver_adapter(&params("adapter = 1")), None);
    assert_eq!(driver_adapter(&params("")), None);
}

#[test]
fn fleet_link_contract_external_ids_retain_exact_target_identity() {
    assert_eq!(
        modbus_external_id("192.0.2.10:502"),
        "external:modbus:192.0.2.10:502"
    );
    assert_eq!(ethercat_external_id("enp2s0"), "external:ethercat:enp2s0");
}

#[test]
fn fleet_link_contract_driver_status_without_evidence_is_policy() {
    let config = driver_config("modbus-tcp", true, params(r#"address = "plc:502""#));
    assert_eq!(driver_link_status(&[], 0, &config), "configured_policy");
}

#[test]
fn fleet_link_contract_driver_status_maps_all_health_states() {
    let config = driver_config("modbus-tcp", true, params(r#"address = "plc:502""#));
    assert_eq!(
        driver_link_status(
            &[driver_status("modbus_tcp", IoDriverHealth::Ok)],
            0,
            &config
        ),
        "connected"
    );
    assert_eq!(
        driver_link_status(
            &[driver_status(
                "modbus-tcp",
                IoDriverHealth::Degraded {
                    error: SmolStr::new("slow")
                }
            )],
            0,
            &config
        ),
        "degraded"
    );
    assert_eq!(
        driver_link_status(
            &[driver_status(
                "modbus-tcp",
                IoDriverHealth::Faulted {
                    error: SmolStr::new("lost")
                }
            )],
            0,
            &config
        ),
        "error"
    );
}

#[test]
fn fleet_link_contract_modbus_target_projects_outbound_client_link() {
    let drivers = [driver_config(
        "modbus-tcp",
        true,
        params(
            r#"
address = "192.0.2.10:502"
tls = true
"#,
        ),
    )];
    let links = driver_target_links("runtime", &drivers, &[]);
    assert_eq!(links.len(), 1);
    let link = &links[0];
    assert_eq!(link.from, "endpoint:runtime:modbus_tcp");
    assert_eq!(link.to, "external:modbus:192.0.2.10:502");
    assert_eq!(link.protocol, "modbus_tcp");
    assert_eq!(link.role, "client");
    assert_eq!(link.direction, "outbound");
    assert_eq!(link.status, "configured_policy");
    assert!(link.secure);
    assert_eq!(link.detail.as_deref(), Some("192.0.2.10:502"));
}

#[test]
fn fleet_link_contract_disabled_modbus_link_remains_visible() {
    let drivers = [driver_config(
        "modbus-tcp",
        false,
        params(r#"address = "192.0.2.10:502""#),
    )];
    let links = driver_target_links(
        "runtime",
        &drivers,
        &[driver_status("modbus-tcp", IoDriverHealth::Ok)],
    );
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].status, "disabled");
}

#[test]
fn fleet_link_contract_modbus_without_address_has_no_target_link() {
    let drivers = [driver_config("modbus-tcp", true, params(""))];
    assert!(driver_target_links("runtime", &drivers, &[]).is_empty());
}

#[test]
fn fleet_link_contract_ethercat_target_projects_unsecured_master_link() {
    let drivers = [driver_config(
        "ethercat",
        true,
        params(r#"adapter = " enp2s0 ""#),
    )];
    let links = driver_target_links("runtime", &drivers, &[]);
    assert_eq!(links.len(), 1);
    let link = &links[0];
    assert_eq!(link.from, "endpoint:runtime:ethercat");
    assert_eq!(link.to, "external:ethercat:enp2s0");
    assert_eq!(link.role, "master");
    assert!(!link.secure);
    assert_eq!(link.status, "configured_policy");
}

#[test]
fn fleet_link_contract_non_target_driver_has_no_device_link() {
    for name in ["mqtt", "gpio", "simulated", "loopback"] {
        let drivers = [driver_config(name, true, params(r#"address = "target""#))];
        assert!(
            driver_target_links("runtime", &drivers, &[]).is_empty(),
            "{name}"
        );
    }
}

#[test]
fn fleet_link_contract_driver_target_links_retain_configuration_index() {
    let drivers = [
        driver_config("mqtt", true, params(r#"broker = "broker:1883""#)),
        driver_config("modbus-tcp", true, params(r#"address = "plc:502""#)),
    ];
    let links = driver_target_links("runtime", &drivers, &[]);
    assert_eq!(links[0].from, "endpoint:runtime:modbus_tcp:1");
}

#[test]
fn fleet_link_contract_disabled_driver_does_not_consume_health_ordinal() {
    let drivers = [
        driver_config("modbus-tcp", false, params(r#"address = "disabled:502""#)),
        driver_config("modbus-tcp", true, params(r#"address = "enabled:502""#)),
    ];
    let health = [driver_status("modbus-tcp", IoDriverHealth::Ok)];
    let links = driver_target_links("runtime", &drivers, &health);
    assert_eq!(links[0].status, "disabled");
    assert_eq!(links[1].status, "connected");
}

#[test]
fn fleet_link_contract_driver_target_externals_preserve_supported_order() {
    let drivers = [
        driver_config("mqtt", true, params(r#"broker = "broker:1883""#)),
        driver_config("modbus-tcp", true, params(r#"address = "plc:502""#)),
        driver_config("ethercat", true, params(r#"adapter = "eth0""#)),
    ];
    let external = driver_target_externals(&drivers);
    assert_eq!(external.len(), 2);
    assert_eq!(external[0].id, "external:modbus:plc:502");
    assert_eq!(external[0].kind, "device");
    assert_eq!(external[0].via_protocol, ["modbus_tcp"]);
    assert_eq!(external[1].id, "external:ethercat:eth0");
    assert_eq!(external[1].kind, "fieldbus");
    assert_eq!(external[1].via_protocol, ["ethercat"]);
}

#[test]
fn fleet_link_contract_driver_target_external_omits_empty_target() {
    let drivers = [
        driver_config("modbus-tcp", true, params(r#"address = " ""#)),
        driver_config("ethercat", true, params(r#"adapter = " ""#)),
    ];
    assert!(driver_target_externals(&drivers).is_empty());
}

#[test]
fn fleet_link_contract_ads_external_projection_uses_target_net_id() {
    let config = ads_config();
    let external = ads_client_externals(Some(&config));
    assert_eq!(external.len(), 1);
    assert_eq!(external[0].id, "external:ads:5.23.91.12.1.1");
    assert_eq!(external[0].kind, "plc");
    assert_eq!(external[0].via_protocol, ["ads"]);
    assert_eq!(external[0].direction, "outbound");
}

#[test]
fn fleet_link_contract_ads_absent_config_has_no_external_or_link() {
    assert!(ads_client_externals(None).is_empty());
    assert!(ads_client_links("runtime", None, None).is_empty());
}

#[test]
fn fleet_link_contract_ads_without_status_is_configured_policy() {
    let config = ads_config();
    let links = ads_client_links("runtime", Some(&config), None);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].status, "configured_policy");
    assert_eq!(links[0].from, "endpoint:runtime:ads");
    assert_eq!(links[0].to, "external:ads:5.23.91.12.1.1");
    assert!(!links[0].secure);
}

#[test]
fn fleet_link_contract_ads_link_health_comes_from_matching_status() {
    let config = ads_config();
    for (state, expected) in [
        (AdsConnectionStatusState::Connected, "connected"),
        (AdsConnectionStatusState::Reconnecting, "degraded"),
        (AdsConnectionStatusState::Faulted, "error"),
        (AdsConnectionStatusState::Disabled, "configured_policy"),
    ] {
        let report = ads_status(state);
        let links = ads_client_links("runtime", Some(&config), Some(&report));
        assert_eq!(links[0].status, expected, "{state:?}");
    }
}

#[test]
fn fleet_link_contract_ads_secure_transport_sets_secure_fact() {
    let mut config = ads_config();
    config.connections[0].route.security.transport = trust_ads_core::TransportSecurity::Secure;
    let links = ads_client_links("runtime", Some(&config), None);
    assert!(links[0].secure);
}

#[test]
fn fleet_link_contract_opcua_external_projection_uses_sanitized_endpoint() {
    let config = opcua_config("none", "none");
    let external = opcua_client_externals(Some(&config));
    assert_eq!(external.len(), 1);
    assert_eq!(
        external[0].id,
        "external:opcua:opc-tcp---192-0-2-10-4840-line"
    );
    assert_eq!(external[0].kind, "opcua_server");
    assert_eq!(external[0].via_protocol, ["opcua_client"]);
}

#[test]
fn fleet_link_contract_opcua_absent_config_has_no_external_or_link() {
    assert!(opcua_client_externals(None).is_empty());
    assert!(opcua_client_links("runtime", None, None).is_empty());
}

#[test]
fn fleet_link_contract_opcua_without_status_is_configured_policy() {
    let config = opcua_config("none", "none");
    let links = opcua_client_links("runtime", Some(&config), None);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].status, "configured_policy");
    assert_eq!(links[0].from, "endpoint:runtime:opcua_client");
    assert!(!links[0].secure);
}

#[test]
fn fleet_link_contract_opcua_link_health_comes_from_matching_status() {
    let config = opcua_config("none", "none");
    for (state, expected) in [
        (OpcUaClientConnectionState::Connected, "connected"),
        (OpcUaClientConnectionState::Reconnecting, "degraded"),
        (OpcUaClientConnectionState::Faulted, "error"),
        (OpcUaClientConnectionState::Configured, "configured_policy"),
    ] {
        let report = opcua_status(state);
        let links = opcua_client_links("runtime", Some(&config), Some(&report));
        assert_eq!(links[0].status, expected, "{state:?}");
    }
}

#[test]
fn fleet_link_contract_opcua_non_none_policy_sets_secure_fact() {
    let config = opcua_config("basic256sha256", "sign_and_encrypt");
    let links = opcua_client_links("runtime", Some(&config), None);
    assert!(links[0].secure);
}

#[test]
fn fleet_link_contract_mesh_without_evidence_is_configured_policy() {
    assert_eq!(
        configured_mesh_link_status("peer-a", None),
        "configured_policy"
    );
}

#[test]
fn fleet_link_contract_mesh_ready_matching_peer_is_connected() {
    let evidence = crate::mesh::MeshTopologyEvidence::for_test(true, &["peer-a"], 10);
    assert_eq!(
        configured_mesh_link_status("peer-a", Some(&evidence)),
        "connected"
    );
    assert_eq!(
        configured_mesh_link_status("tcp/peer-a:7447", Some(&evidence)),
        "connected"
    );
}

#[test]
fn fleet_link_contract_mesh_ready_missing_peer_is_degraded() {
    let evidence = crate::mesh::MeshTopologyEvidence::for_test(true, &["peer-a"], 10);
    assert_eq!(
        configured_mesh_link_status("peer-b", Some(&evidence)),
        "degraded"
    );
}

#[test]
fn fleet_link_contract_mesh_not_ready_does_not_claim_live_peer() {
    let evidence = crate::mesh::MeshTopologyEvidence::for_test(false, &["peer-a"], 10);
    assert_eq!(
        configured_mesh_link_status("peer-a", Some(&evidence)),
        "configured_policy"
    );
}
