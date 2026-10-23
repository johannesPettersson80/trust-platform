use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor};

use super::*;

#[test]
fn ads_cached_snapshot_returns_tree_and_existing_import_shape() {
    let mut writable = SymbolDescriptor::new(
        "GVL.Setpoint",
        AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        0x4020,
        4,
        4,
    );
    writable.flags.insert(SymbolFlag::Read);
    writable.flags.insert(SymbolFlag::Write);
    let snapshot = SymbolSnapshot::new(
        "line1",
        vec![
            writable,
            SymbolDescriptor::new(
                "MAIN.Temperature",
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                0x4020,
                8,
                4,
            )
            .with_flag(SymbolFlag::Read),
        ],
    );

    let value = browse_symbols_value(
        json!({
            "protocol": "ads",
            "kind": "symbols",
            "connection_name": "line1",
            "snapshot": snapshot,
            "include_patterns": ["setpoint"]
        }),
        None,
        None,
    )
    .expect("browse symbols");
    let tree = value.get("tree").and_then(Value::as_array).expect("tree");
    assert_eq!(tree.len(), 2);
    let gvl = tree
        .iter()
        .find(|node| node.get("name").and_then(Value::as_str) == Some("GVL"))
        .expect("GVL group");
    let setpoint = gvl
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .expect("setpoint child");
    assert_eq!(
        setpoint.get("writable").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(setpoint.get("type").and_then(Value::as_str), Some("REAL"));
    let candidates = value
        .get("ads_import")
        .and_then(|import| import.get("candidates"))
        .and_then(Value::as_array)
        .expect("ads import candidates");
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        candidates[0].get("access").and_then(Value::as_str),
        Some("read")
    );
    assert!(
        serde_json::to_string(&value)
            .expect("json")
            .contains("ads_import"),
        "browse response must expose the existing ADS import shape"
    );
}

#[test]
fn ads_browse_target_preserves_non_plc_port() {
    let target: BrowseTarget = serde_json::from_value(json!({
        "host": "192.168.10.5",
        "target_net_id": "5.23.91.12.1.1",
        "ams_port": 301,
        "name": "line1"
    }))
    .expect("target");

    let identity = target.into_identity().expect("identity");

    assert_eq!(identity.ip, "192.168.10.5");
    assert_eq!(identity.ams_net_id, "5.23.91.12.1.1");
    assert_eq!(identity.ams_port, 301);
    assert_eq!(identity.name.as_deref(), Some("line1"));
}

#[test]
fn ads_browse_target_defaults_to_first_plc_runtime_port() {
    let target: BrowseTarget = serde_json::from_value(json!({
        "host": "192.168.10.5",
        "ams_net_id": "5.23.91.12.1.1"
    }))
    .expect("target");

    assert_eq!(target.into_identity().expect("identity").ams_port, 851);
}

#[test]
fn ads_browse_target_rejects_zero_port() {
    let target: BrowseTarget = serde_json::from_value(json!({
        "host": "192.168.10.5",
        "ams_net_id": "5.23.91.12.1.1",
        "ams_port": 0
    }))
    .expect("target payload");

    assert_eq!(
        target.into_identity().expect_err("port zero must fail"),
        "ADS browse target ams_port must be between 1 and 65535"
    );
}

#[cfg(feature = "ads-wire")]
#[test]
fn ads_browse_errors_distinguish_port_and_symbol_upload_failures() {
    let unavailable = OnboardingWireError::new(
        OnboardingWireErrorKind::NoSymbols,
        "connect ADS target: Connection refused",
    );
    let unsupported = OnboardingWireError::new(
        OnboardingWireErrorKind::NoSymbols,
        "upload ADS symbol table: service is not supported by server",
    );
    let other = OnboardingWireError::new(
        OnboardingWireErrorKind::NoSymbols,
        "upload ADS symbol table: protocol failure",
    );

    assert_eq!(
        classify_ads_browse_error(&unavailable),
        "ads_port_unavailable"
    );
    assert_eq!(
        classify_ads_browse_error(&unsupported),
        "symbol_upload_unsupported"
    );
    assert_eq!(classify_ads_browse_error(&other), "symbol_upload_failed");
}

#[test]
fn opcua_client_browse_leaf_exposes_raw_node_id_and_apply_data_type() {
    let symbol = opcua_node_to_symbol(crate::opcua::OpcUaBrowseNode {
        id: "ns=2;s=MAIN.Temperature".to_string(),
        name: "Temperature".to_string(),
        path: "Objects/Device/Temperature".to_string(),
        data_type_id: "i=11".to_string(),
        data_type: "double".to_string(),
        writable: true,
        children: Vec::new(),
    });
    let value = serde_json::to_value(symbol).expect("serialize OPC UA browse symbol");

    assert_eq!(
        value.get("id").and_then(Value::as_str),
        Some("opcua:node:ns_2_s_MAIN.Temperature"),
        "sanitized id remains a UI key only"
    );
    assert_eq!(
        value.get("node_id").and_then(Value::as_str),
        Some("ns=2;s=MAIN.Temperature"),
        "raw OPC UA NodeId must round-trip into comm.apply"
    );
    assert_eq!(value.get("type").and_then(Value::as_str), Some("i=11"));
    assert_eq!(
        value.get("data_type").and_then(Value::as_str),
        Some("double"),
        "data_type is the apply-friendly scalar type, not the OPC UA type NodeId"
    );
    assert_eq!(value.get("writable").and_then(Value::as_bool), Some(true));
}

#[test]
fn opcua_client_browse_error_response_carries_structured_code() {
    let error = crate::error::RuntimeError::ControlError("OPC UA status: BadNodeIdUnknown".into());
    let value = response_tree_error_value(
        "opcua_client".to_string(),
        "nodes".to_string(),
        crate::opcua::classify_opcua_client_browse_error(&error).as_str(),
        format!("OPC UA node browse failed: {error}"),
    )
    .expect("browse error response");

    assert_eq!(
        value.pointer("/error/code").and_then(Value::as_str),
        Some("browse_denied")
    );
    assert_eq!(
        value
            .pointer("/tree")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

#[cfg(feature = "ads-wire")]
#[test]
fn ads_symbol_upload_timeout_returns_route_missing_response() {
    let route_plan = build_route_plan(RoutePlanRequest {
        role: crate::ads::onboarding::RoutePlanRole::Client,
        route_name: "line1".to_string(),
        target: TargetIdentity {
            name: Some("TwinCAT".to_string()),
            ip: "192.168.77.11".to_string(),
            ams_net_id: "100.67.6.217.1.1".to_string(),
            ams_port: 851,
            tc_version: Some("3.1.4026".to_string()),
        },
        local: LocalIdentity {
            host_name: Some("trust-pi".to_string()),
            chosen_ip: "192.168.77.10".to_string(),
            ams_net_id: "192.168.77.10.1.1".to_string(),
            nic: Some("eth0".to_string()),
            candidates: Vec::new(),
            classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
        },
        channel: CredentialChannelClassification::TrustedSameHost,
    });

    let value = missing_ads_route_browse_response(
        "NoSymbols: upload ADS symbol table: receiving reply (route set?): timed out".to_string(),
        route_plan,
    )
    .expect("route missing browse response");

    assert_eq!(
        value.pointer("/route/status").and_then(Value::as_str),
        Some("missing")
    );
    assert_eq!(
        value.pointer("/route/action").and_then(Value::as_str),
        Some("ads.route_plan")
    );
    assert_eq!(
        value
            .pointer("/tree")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert!(value
        .pointer("/route/detail")
        .and_then(Value::as_str)
        .is_some_and(|detail| detail.contains("timed out")));
    assert!(
        value
            .pointer("/route/route_plan/local/ams_net_id")
            .is_some(),
        "route-missing response must include the route plan the UI uses"
    );
}

#[cfg(feature = "ads-wire")]
#[test]
fn ads_route_check_separates_unavailable_port_from_missing_return_route() {
    let unavailable = OnboardingWireError::new(
        OnboardingWireErrorKind::RouteMissing,
        "connect ADS target: Connection refused",
    );
    assert!(!route_check_failure_implies_missing_route(&unavailable));
    assert_eq!(
        classify_ads_browse_error(&unavailable),
        "ads_port_unavailable"
    );

    let missing_route = OnboardingWireError::new(
        OnboardingWireErrorKind::RouteMissing,
        "target rejected route-back identity",
    );
    assert!(route_check_failure_implies_missing_route(&missing_route));

    for detail in [
        "ADS port not opened",
        "Router: port not registered",
        "Router: port is invalid",
        "Router: port removed",
    ] {
        let error = OnboardingWireError::new(OnboardingWireErrorKind::NoSymbols, detail);
        assert_eq!(classify_ads_browse_error(&error), "ads_port_unavailable");
    }
    for detail in ["Unknown command ID", "Unknown AMS command"] {
        let error = OnboardingWireError::new(OnboardingWireErrorKind::NoSymbols, detail);
        assert_eq!(
            classify_ads_browse_error(&error),
            "symbol_upload_unsupported"
        );
    }
    let exhausted = OnboardingWireError::new(
        OnboardingWireErrorKind::NoSymbols,
        "No more symbols in cache",
    );
    assert_eq!(classify_ads_browse_error(&exhausted), "empty_symbol_table");
}

#[test]
fn local_project_symbol_picker_returns_declared_globals() {
    let root = temp_dir("browse-local-globals");
    write_file(
        &root.join("src/main.st"),
        r#"
VAR_GLOBAL
    Setpoint : REAL;
    PumpRunning : BOOL;
END_VAR

PROGRAM Main
END_PROGRAM
"#,
    );

    let value = browse_symbols_value(
        json!({
            "protocol": "opcua_server",
            "kind": "symbols",
            "target": { "local": true }
        }),
        None,
        Some(root.as_path()),
    )
    .expect("local project symbols");

    assert_eq!(
        value.pointer("/protocol").and_then(Value::as_str),
        Some("opcua_server")
    );
    let children = value
        .pointer("/tree/0/children")
        .and_then(Value::as_array)
        .expect("global children");
    assert!(children.iter().any(|node| {
        node.get("path").and_then(Value::as_str) == Some("global.Setpoint")
            && node.get("type").and_then(Value::as_str) == Some("REAL")
    }));
    assert!(children.iter().any(|node| {
        node.get("path").and_then(Value::as_str) == Some("global.PumpRunning")
            && node.get("type").and_then(Value::as_str) == Some("BOOL")
    }));
}

#[test]
fn ethercat_channel_browse_returns_configured_module_channels() {
    let root = temp_dir("browse-ethercat-channels");
    write_file(
        &root.join("io.toml"),
        r#"
[io]
safe_state = []

[[io.drivers]]
name = "ethercat"
params = { adapter = "mock", modules = [{ model = "EK1100", slot = 0, channels = 1 }, { model = "EL1008", slot = 1, channels = 8 }] }
"#,
    );

    let value = browse_symbols_value(
        json!({
            "protocol": "ethercat",
            "kind": "channels"
        }),
        None,
        Some(root.as_path()),
    )
    .expect("ethercat channels");

    let modules = value
        .pointer("/tree")
        .and_then(Value::as_array)
        .expect("module tree");
    let input = modules
        .iter()
        .find(|node| node.get("name").and_then(Value::as_str) == Some("EL1008 (slot 1)"))
        .expect("EL1008 module");
    assert_eq!(
        input
            .get("children")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(8)
    );
    assert_eq!(
        input.get("type").and_then(Value::as_str),
        Some("field_slave")
    );
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("trust-browse-symbols-{name}-{stamp}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, content).expect("write file");
}
