use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{json, Value};
use smol_str::SmolStr;

use super::*;
use crate::ads::diagnostics::DoctorRole;
use crate::config::{MeshRole, RuntimeCloudProfile};
use crate::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

fn serialized_string(value: impl Serialize) -> String {
    serde_json::to_value(value)
        .expect("contract value must serialize")
        .as_str()
        .expect("contract identifier must serialize as a string")
        .to_string()
}

fn status_value(status: &RuntimeCapabilityStatus) -> Value {
    serde_json::to_value(status).expect("capability status must serialize")
}

fn action_kind(status: &RuntimeCapabilityStatus) -> String {
    status_value(status)["next_action"]["kind"]
        .as_str()
        .expect("next action kind")
        .to_string()
}

fn driver(name: &str, health: IoDriverHealth) -> IoDriverStatus {
    IoDriverStatus {
        name: SmolStr::new(name),
        health,
    }
}

fn io_status(name: &str, health: &[IoDriverStatus]) -> RuntimeCapabilityStatus {
    io_driver_capability(CommId::ModbusTcp, name, true, None, health)
}

fn settings() -> RuntimeSettings {
    RuntimeSettings::new(
        crate::value::Duration::from_millis(10),
        BaseSettings {
            log_level: SmolStr::new("info"),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::SafeHalt,
            retain_mode: RetainMode::None,
            retain_save_interval: None,
        },
        WebSettings {
            enabled: false,
            listen: SmolStr::new("127.0.0.1:0"),
            auth: SmolStr::new("local"),
            tls: false,
        },
        DiscoverySettings {
            enabled: false,
            service_name: SmolStr::new("truST"),
            advertise: false,
            interfaces: Vec::new(),
            host_group: None,
        },
        MeshSettings {
            enabled: false,
            role: MeshRole::Peer,
            listen: SmolStr::new("127.0.0.1:0"),
            connect: Vec::new(),
            tls: false,
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
            zenohd_version: SmolStr::new("1.7.2"),
            plugin_versions: IndexMap::new(),
        },
        SimulationSettings {
            enabled: false,
            time_scale: 1,
            mode_label: SmolStr::new("production"),
            warning: SmolStr::new(""),
        },
    )
}

fn ads_report(overall: AdsStatusOverall) -> AdsStatusReport {
    AdsStatusReport {
        schema_version: 1,
        role: DoctorRole::Client,
        overall,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: Vec::new(),
        summary: format!("ADS status: {overall:?}"),
    }
}

fn opcua_client_config() -> crate::opcua::OpcUaClientConfig {
    crate::opcua::OpcUaClientConfig {
        connections: vec![crate::opcua::OpcUaClientConnectionConfig {
            name: SmolStr::new("line"),
            endpoint_url: "opc.tcp://127.0.0.1:4840".to_string(),
            security: crate::opcua::OpcUaSecurityProfile::default(),
            auth: crate::opcua::OpcUaClientAuthConfig::Anonymous,
            trust_server_certificate: false,
            poll_interval_ms: 250,
            timeout_ms: 500,
            points: Vec::new(),
        }],
    }
}

#[cfg(feature = "opcua-wire")]
fn opcua_connection(
    name: &str,
    state: crate::opcua::OpcUaClientConnectionState,
    degraded_points: usize,
) -> crate::opcua::OpcUaClientConnectionStatus {
    crate::opcua::OpcUaClientConnectionStatus {
        name: SmolStr::new(name),
        endpoint_url: format!("opc.tcp://{name}:4840"),
        state,
        point_count: degraded_points.max(1),
        degraded_points,
        last_seen_ms: None,
        detail: String::new(),
        points: Vec::new(),
    }
}

#[cfg(feature = "opcua-wire")]
fn opcua_status(
    connections: Vec<crate::opcua::OpcUaClientConnectionStatus>,
) -> crate::opcua::OpcUaClientStatusReport {
    crate::opcua::OpcUaClientStatusReport {
        enabled: true,
        deployed_config_hash: None,
        connections,
    }
}

fn assert_status_coherent(status: &RuntimeCapabilityStatus) {
    if status.operational {
        assert!(status.built, "operational capability must be built");
        assert!(
            status.configured,
            "operational capability must be configured"
        );
        assert_eq!(status.health, CommHealth::Connected);
    }
    if status.health == CommHealth::Connected {
        assert!(status.built);
        assert!(status.configured);
        assert!(status.operational);
    }
    if status.health == CommHealth::NotConfigured {
        assert!(!status.configured);
        assert!(!status.operational);
    }
    if status.health == CommHealth::NotInBuild {
        assert!(!status.built);
        assert!(!status.operational);
    }
    if status.health == CommHealth::ConfiguredPolicy {
        assert!(status.configured);
        assert!(!status.operational);
    }
}

#[test]
fn capability_contract_serializes_exact_protocol_id_vocabulary_and_order() {
    let ids = [
        CommId::Ads,
        CommId::AdsServer,
        CommId::Opcua,
        CommId::OpcuaClient,
        CommId::ModbusTcp,
        CommId::Mqtt,
        CommId::Openot,
        CommId::Discovery,
        CommId::Mesh,
        CommId::RealtimeT0,
        CommId::RuntimeCloud,
        CommId::Ethercat,
        CommId::Gpio,
        CommId::Simulated,
        CommId::Loopback,
    ];
    assert_eq!(
        ids.map(serialized_string),
        [
            "ads",
            "ads_server",
            "opcua",
            "opcua_client",
            "modbus_tcp",
            "mqtt",
            "openot",
            "discovery",
            "mesh",
            "realtime_t0",
            "runtime_cloud",
            "ethercat",
            "gpio",
            "simulated",
            "loopback",
        ]
    );
}

#[test]
fn capability_contract_serializes_exact_health_vocabulary() {
    let values = [
        CommHealth::NotInBuild,
        CommHealth::NotConfigured,
        CommHealth::Simulate,
        CommHealth::RuntimeUnreachable,
        CommHealth::Connected,
        CommHealth::Degraded,
        CommHealth::Error,
        CommHealth::ConfiguredPolicy,
    ];
    assert_eq!(
        values.map(serialized_string),
        [
            "not_in_build",
            "not_configured",
            "simulate",
            "runtime_unreachable",
            "connected",
            "degraded",
            "error",
            "configured_policy",
        ]
    );
}

#[test]
fn capability_contract_serializes_exact_platform_vocabulary() {
    assert_eq!(
        [
            serialized_string(CommPlatform::Unix),
            serialized_string(CommPlatform::Linux),
        ],
        ["unix", "linux"]
    );
}

#[test]
fn capability_contract_serializes_exact_action_vocabulary_including_none() {
    let actions = [
        CommNextActionKind::None,
        CommNextActionKind::Setup,
        CommNextActionKind::OpenRuntimePane,
        CommNextActionKind::TestConnection,
        CommNextActionKind::DiagnoseAds,
        CommNextActionKind::OpenDocs,
        CommNextActionKind::ApplyConfig,
        CommNextActionKind::SwitchToOnline,
        CommNextActionKind::GetBuildWithFeature,
    ];
    assert_eq!(
        actions.map(serialized_string),
        [
            "none",
            "setup",
            "open_runtime_pane",
            "test_connection",
            "diagnose_ads",
            "open_docs",
            "apply_config",
            "switch_to_online",
            "get_build_with_feature",
        ]
    );
}

#[test]
fn capability_contract_response_uses_schema_version_four() {
    let response = CommCapabilitiesResponse {
        schema_version: COMM_SCHEMA_VERSION,
        capabilities: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(response).expect("response json"),
        json!({
            "schema_version": 4,
            "capabilities": [],
        })
    );
}

#[test]
fn capability_contract_serializes_required_fields_and_platform() {
    let status = capability(
        CommId::Ethercat,
        true,
        true,
        true,
        Some(CommPlatform::Unix),
        CommHealth::Connected,
        "healthy",
        action(CommNextActionKind::None, "Status"),
    );
    assert_eq!(
        status_value(&status),
        json!({
            "id": "ethercat",
            "built": true,
            "configured": true,
            "operational": true,
            "platform": "unix",
            "health": "connected",
            "detail": "healthy",
            "next_action": {"kind": "none", "label": "Status"},
        })
    );
}

#[test]
fn capability_contract_omits_absent_platform() {
    let status = capability(
        CommId::Mqtt,
        true,
        false,
        false,
        None,
        CommHealth::NotConfigured,
        "not configured",
        action(CommNextActionKind::Setup, "Set up"),
    );
    assert!(status_value(&status).get("platform").is_none());
}

#[test]
fn capability_contract_action_mapping_is_deterministic_for_non_ads() {
    let cases = [
        (CommHealth::Connected, "none"),
        (CommHealth::NotInBuild, "get_build_with_feature"),
        (CommHealth::Simulate, "switch_to_online"),
        (CommHealth::RuntimeUnreachable, "open_runtime_pane"),
        (CommHealth::NotConfigured, "setup"),
        (CommHealth::ConfiguredPolicy, "setup"),
        (CommHealth::Degraded, "setup"),
        (CommHealth::Error, "setup"),
    ];
    for (health, expected) in cases {
        let status = capability(
            CommId::Mqtt,
            health != CommHealth::NotInBuild,
            !matches!(health, CommHealth::NotConfigured | CommHealth::NotInBuild),
            health == CommHealth::Connected,
            None,
            health,
            "detail",
            action_for_health(health, false),
        );
        assert_eq!(action_kind(&status), expected, "health {health:?}");
        assert!(
            !status.next_action.label.trim().is_empty(),
            "health {health:?}"
        );
    }
}

#[test]
fn capability_contract_action_mapping_uses_ads_diagnostics_for_failures() {
    for health in [CommHealth::Degraded, CommHealth::Error] {
        let status = capability(
            CommId::Ads,
            true,
            true,
            false,
            None,
            health,
            "detail",
            action_for_health(health, true),
        );
        assert_eq!(action_kind(&status), "diagnose_ads");
    }
}

#[test]
fn capability_contract_io_not_built_is_never_configured_or_operational() {
    let status = io_driver_capability(
        CommId::Ethercat,
        "ethercat",
        false,
        Some(CommPlatform::Unix),
        &[driver("ethercat", IoDriverHealth::Ok)],
    );
    assert_eq!(status.health, CommHealth::NotInBuild);
    assert!(!status.built);
    assert!(!status.configured);
    assert!(!status.operational);
    assert_eq!(status_value(&status)["platform"], "unix");
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_io_missing_driver_is_not_configured() {
    let status = io_status("modbus-tcp", &[]);
    assert_eq!(status.health, CommHealth::NotConfigured);
    assert_eq!(action_kind(&status), "setup");
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_io_healthy_driver_is_connected() {
    let status = io_status("modbus-tcp", &[driver("modbus-tcp", IoDriverHealth::Ok)]);
    assert_eq!(status.health, CommHealth::Connected);
    assert!(status.operational);
    assert_eq!(action_kind(&status), "none");
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_io_driver_match_is_ascii_case_insensitive() {
    let status = io_status("modbus-tcp", &[driver("MoDbUs-TcP", IoDriverHealth::Ok)]);
    assert_eq!(status.health, CommHealth::Connected);
}

#[test]
fn capability_contract_io_driver_accepts_schema_spelling_alias() {
    let status = io_status("modbus-tcp", &[driver("modbus_tcp", IoDriverHealth::Ok)]);
    assert_eq!(status.health, CommHealth::Connected);
}

#[test]
fn capability_contract_io_unrelated_driver_does_not_configure_protocol() {
    let status = io_status("modbus-tcp", &[driver("mqtt", IoDriverHealth::Ok)]);
    assert_eq!(status.health, CommHealth::NotConfigured);
}

#[test]
fn capability_contract_io_degraded_driver_is_configured_but_not_operational() {
    let status = io_status(
        "modbus-tcp",
        &[driver(
            "modbus-tcp",
            IoDriverHealth::Degraded {
                error: SmolStr::new("slow"),
            },
        )],
    );
    assert_eq!(status.health, CommHealth::Degraded);
    assert!(status.configured);
    assert!(!status.operational);
    assert!(status.detail.contains("slow"));
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_io_faulted_driver_is_error() {
    let status = io_status(
        "modbus-tcp",
        &[driver(
            "modbus-tcp",
            IoDriverHealth::Faulted {
                error: SmolStr::new("wire lost"),
            },
        )],
    );
    assert_eq!(status.health, CommHealth::Error);
    assert!(status.configured);
    assert!(!status.operational);
    assert!(status.detail.contains("wire lost"));
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_io_fault_dominates_earlier_healthy_instance() {
    let status = io_status(
        "modbus-tcp",
        &[
            driver("modbus-tcp", IoDriverHealth::Ok),
            driver(
                "MODBUS_TCP",
                IoDriverHealth::Faulted {
                    error: SmolStr::new("second instance fault"),
                },
            ),
        ],
    );
    assert_eq!(status.health, CommHealth::Error);
    assert!(status.detail.contains("second instance fault"));
}

#[test]
fn capability_contract_io_fault_dominates_earlier_degraded_instance() {
    let status = io_status(
        "modbus-tcp",
        &[
            driver(
                "modbus-tcp",
                IoDriverHealth::Degraded {
                    error: SmolStr::new("first slow"),
                },
            ),
            driver(
                "modbus-tcp",
                IoDriverHealth::Faulted {
                    error: SmolStr::new("second fault"),
                },
            ),
        ],
    );
    assert_eq!(status.health, CommHealth::Error);
}

#[test]
fn capability_contract_io_degraded_dominates_earlier_healthy_instance() {
    let status = io_status(
        "modbus-tcp",
        &[
            driver("modbus-tcp", IoDriverHealth::Ok),
            driver(
                "modbus-tcp",
                IoDriverHealth::Degraded {
                    error: SmolStr::new("second slow"),
                },
            ),
        ],
    );
    assert_eq!(status.health, CommHealth::Degraded);
}

#[test]
fn capability_contract_io_all_matching_instances_must_be_healthy_for_connected() {
    let status = io_status(
        "modbus-tcp",
        &[
            driver(
                "mqtt",
                IoDriverHealth::Faulted {
                    error: SmolStr::new("unrelated"),
                },
            ),
            driver("modbus-tcp", IoDriverHealth::Ok),
            driver("MODBUS_TCP", IoDriverHealth::Ok),
        ],
    );
    assert_eq!(status.health, CommHealth::Connected);
    assert!(status.operational);
}

#[test]
fn capability_contract_openot_platform_and_build_state_are_explicit() {
    let status = openot_capability();
    assert_eq!(status_value(&status)["platform"], "unix");
    assert_eq!(status.built, cfg!(unix));
    assert_eq!(
        status.health,
        if cfg!(unix) {
            CommHealth::NotConfigured
        } else {
            CommHealth::NotInBuild
        }
    );
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_discovery_absent_settings_is_not_configured() {
    let status = discovery_capability(None);
    assert_eq!(status.health, CommHealth::NotConfigured);
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_discovery_enabled_is_configured_but_not_operational() {
    let mut settings = settings();
    settings.discovery.enabled = true;
    let status = discovery_capability(Some(&settings));
    assert_eq!(status.health, CommHealth::Degraded);
    assert!(status.configured);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_mesh_absent_settings_is_not_configured() {
    let status = mesh_capability(None);
    assert_eq!(status.health, CommHealth::NotConfigured);
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_mesh_connect_target_counts_as_configuration() {
    let mut settings = settings();
    settings.mesh.connect.push(SmolStr::new("tcp/peer:7447"));
    let status = mesh_capability(Some(&settings));
    assert_eq!(status.health, CommHealth::Degraded);
    assert!(status.configured);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_runtime_cloud_absent_settings_is_not_configured() {
    let status = runtime_cloud_capability(None);
    assert_eq!(status.health, CommHealth::NotConfigured);
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_runtime_cloud_policy_is_not_a_live_connection() {
    let mut settings = settings();
    settings.runtime_cloud.profile = RuntimeCloudProfile::Wan;
    let status = runtime_cloud_capability(Some(&settings));
    assert_eq!(status.health, CommHealth::ConfiguredPolicy);
    assert!(status.configured);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_opcua_server_absent_settings_is_not_configured_when_built() {
    let status = opcua_capability(None);
    assert_eq!(status.built, cfg!(feature = "opcua-wire"));
    assert_eq!(
        status.health,
        if cfg!(feature = "opcua-wire") {
            CommHealth::NotConfigured
        } else {
            CommHealth::NotInBuild
        }
    );
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_opcua_server_preserves_configured_fact_without_wire_build() {
    let mut settings = settings();
    settings.opcua.enabled = true;
    let status = opcua_capability(Some(&settings));
    assert!(status.configured);
    assert!(!status.operational);
    assert_eq!(
        status.health,
        if cfg!(feature = "opcua-wire") {
            CommHealth::Degraded
        } else {
            CommHealth::NotInBuild
        }
    );
    assert_status_coherent(&status);
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn capability_contract_ads_without_wire_build_is_not_operational() {
    let status = ads_client_capability(Some(&ads_report(AdsStatusOverall::Healthy)));
    assert_eq!(status.health, CommHealth::NotInBuild);
    assert!(!status.built);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[cfg(feature = "ads-wire")]
#[test]
fn capability_contract_ads_missing_report_is_degraded() {
    let status = ads_client_capability(None);
    assert_eq!(status.health, CommHealth::Degraded);
    assert!(!status.configured);
    assert!(!status.operational);
}

#[cfg(feature = "ads-wire")]
#[test]
fn capability_contract_ads_healthy_report_is_connected() {
    let status = ads_client_capability(Some(&ads_report(AdsStatusOverall::Healthy)));
    assert_eq!(status.health, CommHealth::Connected);
    assert_status_coherent(&status);
}

#[cfg(feature = "ads-wire")]
#[test]
fn capability_contract_ads_faulted_report_is_error() {
    let status = ads_client_capability(Some(&ads_report(AdsStatusOverall::Faulted)));
    assert_eq!(status.health, CommHealth::Error);
    assert_eq!(action_kind(&status), "diagnose_ads");
    assert_status_coherent(&status);
}

#[cfg(feature = "ads-wire")]
#[test]
fn capability_contract_ads_disabled_report_is_not_configured() {
    let status = ads_client_capability(Some(&ads_report(AdsStatusOverall::Disabled)));
    assert_eq!(status.health, CommHealth::NotConfigured);
    assert_status_coherent(&status);
}

#[cfg(feature = "ads-wire")]
#[test]
fn capability_contract_ads_incomplete_reports_are_degraded() {
    for overall in [
        AdsStatusOverall::Degraded,
        AdsStatusOverall::NotReady,
        AdsStatusOverall::Unknown,
    ] {
        let status = ads_client_capability(Some(&ads_report(overall)));
        assert_eq!(status.health, CommHealth::Degraded, "{overall:?}");
        assert_status_coherent(&status);
    }
}

#[cfg(not(feature = "opcua-wire"))]
#[test]
fn capability_contract_opcua_client_without_wire_retains_configured_fact() {
    let config = opcua_client_config();
    let status = opcua_client_capability(Some(&config), None);
    assert_eq!(status.health, CommHealth::NotInBuild);
    assert!(status.configured);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[cfg(feature = "opcua-wire")]
#[test]
fn capability_contract_opcua_client_without_connections_is_not_configured() {
    let config = crate::opcua::OpcUaClientConfig {
        connections: Vec::new(),
    };
    let status = opcua_client_capability(Some(&config), None);
    assert_eq!(status.health, CommHealth::NotConfigured);
    assert_status_coherent(&status);
}

#[cfg(feature = "opcua-wire")]
#[test]
fn capability_contract_opcua_client_missing_status_is_degraded() {
    let config = opcua_client_config();
    let status = opcua_client_capability(Some(&config), None);
    assert_eq!(status.health, CommHealth::Degraded);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[cfg(feature = "opcua-wire")]
#[test]
fn capability_contract_opcua_client_before_first_read_is_configured_policy() {
    let config = opcua_client_config();
    let status_report = opcua_status(Vec::new());
    let status = opcua_client_capability(Some(&config), Some(&status_report));
    assert_eq!(status.health, CommHealth::ConfiguredPolicy);
    assert_status_coherent(&status);
}

#[cfg(feature = "opcua-wire")]
#[test]
fn capability_contract_opcua_client_all_live_connections_are_connected() {
    let config = opcua_client_config();
    let status_report = opcua_status(vec![
        opcua_connection("a", crate::opcua::OpcUaClientConnectionState::Connected, 0),
        opcua_connection("b", crate::opcua::OpcUaClientConnectionState::Connected, 0),
    ]);
    let status = opcua_client_capability(Some(&config), Some(&status_report));
    assert_eq!(status.health, CommHealth::Connected);
    assert_status_coherent(&status);
}

#[cfg(feature = "opcua-wire")]
#[test]
fn capability_contract_opcua_client_degraded_points_prevent_connected() {
    let config = opcua_client_config();
    let status_report = opcua_status(vec![opcua_connection(
        "a",
        crate::opcua::OpcUaClientConnectionState::Connected,
        1,
    )]);
    let status = opcua_client_capability(Some(&config), Some(&status_report));
    assert_eq!(status.health, CommHealth::Degraded);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[cfg(feature = "opcua-wire")]
#[test]
fn capability_contract_opcua_client_any_fault_is_error() {
    let config = opcua_client_config();
    let status_report = opcua_status(vec![
        opcua_connection(
            "healthy",
            crate::opcua::OpcUaClientConnectionState::Connected,
            0,
        ),
        opcua_connection(
            "faulted",
            crate::opcua::OpcUaClientConnectionState::Faulted,
            0,
        ),
    ]);
    let status = opcua_client_capability(Some(&config), Some(&status_report));
    assert_eq!(status.health, CommHealth::Error);
    assert!(!status.operational);
    assert_status_coherent(&status);
}

#[test]
fn capability_contract_readonly_audit_removes_direct_secrets() {
    let details = audit_details_for_comm_request(
        "comm.discover",
        Some(&json!({
            "protocol": "mqtt",
            "password": "direct-password",
            "auth_token": "direct-auth",
            "token": "direct-token",
            "credential": "direct-credential",
            "credentials": {"password": "nested"},
            "secret": "direct-secret",
            "client_secret": "direct-client-secret",
        })),
    )
    .expect("audit details");
    assert_eq!(details, json!({"protocol": "mqtt"}));
}

#[test]
fn capability_contract_readonly_audit_removes_target_secrets() {
    let details = audit_details_for_comm_request(
        "comm.browse_symbols",
        Some(&json!({
            "protocol": "opcua_client",
            "target": {
                "endpoint_url": "opc.tcp://plc:4840",
                "username": "operator",
                "password": "target-password",
                "token": "target-token",
                "client_secret": "target-client-secret",
            }
        })),
    )
    .expect("audit details");
    assert_eq!(
        details,
        json!({
            "protocol": "opcua_client",
            "target": {
                "endpoint_url": "opc.tcp://plc:4840",
                "username": "operator",
            }
        })
    );
}

#[test]
fn capability_contract_readonly_audit_removes_secrets_at_arbitrary_object_depth() {
    let details = audit_details_for_comm_request(
        "comm.discover",
        Some(&json!({
            "protocol": "mqtt",
            "scope": {
                "directed": {
                    "broker": {
                        "host": "plc.local",
                        "password": "deep-password",
                        "auth_token": "deep-token",
                    }
                }
            }
        })),
    )
    .expect("audit details");
    assert_eq!(
        details,
        json!({
            "protocol": "mqtt",
            "scope": {"directed": {"broker": {"host": "plc.local"}}}
        })
    );
}

#[test]
fn capability_contract_readonly_audit_removes_secrets_inside_arrays() {
    let details = audit_details_for_comm_request(
        "comm.browse_symbols",
        Some(&json!({
            "protocol": "ads",
            "routes": [
                {"name": "one", "credentials": {"password": "one"}},
                {"name": "two", "secret": "two"},
            ]
        })),
    )
    .expect("audit details");
    assert_eq!(
        details,
        json!({
            "protocol": "ads",
            "routes": [{"name": "one"}, {"name": "two"}],
        })
    );
}

#[test]
fn capability_contract_readonly_audit_secret_key_matching_is_case_insensitive() {
    let details = audit_details_for_comm_request(
        "comm.discover",
        Some(&json!({
            "protocol": "mqtt",
            "PASSWORD": "upper",
            "Auth_Token": "mixed",
            "Client_Secret": "mixed-client",
            "nested": {"ToKeN": "mixed-token", "host": "broker"},
        })),
    )
    .expect("audit details");
    assert_eq!(
        details,
        json!({"protocol": "mqtt", "nested": {"host": "broker"}})
    );
}

#[test]
fn capability_contract_readonly_audit_preserves_public_selection_metadata() {
    let params = json!({
        "protocol": "ads",
        "kind": "symbols",
        "instance": "ads:line",
        "target": {
            "host": "192.0.2.5",
            "ams_net_id": "192.0.2.5.1.1",
            "ams_port": 851,
        },
        "scope": {"cidr": "192.0.2.0/30"},
        "snapshot": {"schema_version": 2, "route_name": "line"},
    });
    let details = audit_details_for_comm_request("comm.browse_symbols", Some(&params))
        .expect("audit details");
    assert_eq!(details, params);
}

#[test]
fn capability_contract_readonly_audit_preserves_non_object_values() {
    for value in [Value::Null, json!("mqtt"), json!(42), json!([1, 2, 3])] {
        let details =
            audit_details_for_comm_request("comm.discover", Some(&value)).expect("audit details");
        assert_eq!(details, value);
    }
}

#[test]
fn capability_contract_unrelated_audit_kind_returns_no_details() {
    assert!(audit_details_for_comm_request("status", Some(&json!({"token": "secret"}))).is_none());
}

#[test]
fn capability_contract_absent_params_return_no_audit_details() {
    for kind in [
        "comm.apply",
        "comm.test",
        "comm.discover",
        "comm.browse_symbols",
    ] {
        assert!(
            audit_details_for_comm_request(kind, None).is_none(),
            "{kind}"
        );
    }
}
