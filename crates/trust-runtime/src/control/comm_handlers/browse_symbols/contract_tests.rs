use std::collections::BTreeSet;

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag, SymbolSnapshot,
    SYMBOL_SNAPSHOT_SCHEMA_VERSION,
};

use super::*;

fn browse_target(value: Value) -> BrowseTarget {
    serde_json::from_value(value).expect("browse target")
}

fn browse_request(
    protocol: &str,
    kind: &str,
    target: Option<BrowseTarget>,
) -> BrowseSymbolsRequest {
    BrowseSymbolsRequest {
        protocol: protocol.to_string(),
        target,
        instance_id: None,
        kind: kind.to_string(),
        connection_name: None,
        include_patterns: Vec::new(),
        name_prefix: None,
        snapshot: None,
        credential_channel: None,
    }
}

fn ads_symbol(name: &str, byte_size: u32, writable: bool) -> SymbolDescriptor {
    let mut symbol = SymbolDescriptor::new(
        name,
        AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        0x4020,
        0,
        byte_size,
    )
    .with_flag(SymbolFlag::Read);
    if writable {
        symbol.flags.insert(SymbolFlag::Write);
    }
    symbol
}

fn io_driver(name: &str) -> crate::config::IoDriverConfig {
    crate::config::IoDriverConfig {
        name: SmolStr::new(name),
        params: toml::Value::Table(toml::map::Map::new()),
        enabled: true,
    }
}

fn opcua_error(target: &BrowseTarget) -> String {
    match target.opcua_auth() {
        Ok(_) => panic!("invalid OPC UA auth accepted"),
        Err(error) => error,
    }
}

#[test]
fn request_requires_string_protocol_and_object_target() {
    for params in [
        json!({}),
        json!({ "protocol": null }),
        json!({ "protocol": 7 }),
        json!({ "protocol": [] }),
        json!({ "protocol": "ads", "target": 7 }),
        json!({ "protocol": "ads", "target": [] }),
    ] {
        let error = browse_symbols_value(params, None, None)
            .expect_err("malformed browse request accepted");
        assert!(
            error.starts_with("invalid comm.browse_symbols payload:"),
            "{error}"
        );
    }
}

#[test]
fn unknown_request_and_target_keys_are_rejected() {
    for params in [
        json!({ "protocol": "ads", "knd": "symbols" }),
        json!({
            "protocol": "ads",
            "target": {
                "host": "127.0.0.1",
                "ams_net_id": "1.2.3.4.5.6",
                "ams_prt": 851
            }
        }),
    ] {
        let error =
            browse_symbols_value(params, None, None).expect_err("unknown browse key accepted");
        assert!(
            error.starts_with("invalid comm.browse_symbols payload:"),
            "{error}"
        );
    }
}

#[test]
fn canonical_protocol_aliases_are_exact() {
    for (alias, expected) in [
        (" ADS ", "ads"),
        ("ads-client", "ads"),
        ("TwinCAT", "ads"),
        ("ads-server", "ads_server"),
        ("OPC-UA", "opcua_server"),
        ("opc_ua_server", "opcua_server"),
        ("opcua-client", "opcua_client"),
        ("open_ot", "openot"),
        ("ether-cat", "ethercat"),
        ("ECAT", "ethercat"),
    ] {
        assert_eq!(canonical_protocol(alias), expected, "alias {alias}");
    }
}

#[test]
fn canonical_protocol_preserves_unknown_normalized_identity() {
    assert_eq!(canonical_protocol(" Custom-Protocol "), "custom_protocol");
    assert_eq!(canonical_protocol(""), "");
}

#[test]
fn kind_defaults_to_symbols_and_normalizes_before_dispatch() {
    assert_eq!(default_kind(), "symbols");
    for kind in [" symbols ", "SYMBOLS", "symbols"] {
        let error = browse_symbols_value(json!({ "protocol": "openot", "kind": kind }), None, None)
            .expect_err("missing project root must reject local browse");
        assert!(
            error.contains("local symbol browsing needs"),
            "kind was not canonicalized before dispatch: {error}"
        );
    }
}

#[test]
fn supported_protocol_kind_pairs_are_closed() {
    for (protocol, kind, expected_error) in [
        (
            "ads",
            "nodes",
            "ADS comm.browse_symbols supports kind='symbols'",
        ),
        ("opcua_client", "symbols", "does not support protocol"),
        ("ethercat", "symbols", "does not support protocol"),
        ("openot", "nodes", "does not support protocol"),
        ("unknown", "symbols", "does not support protocol"),
    ] {
        let error = browse_symbols_value(json!({ "protocol": protocol, "kind": kind }), None, None)
            .expect_err("unsupported protocol/kind accepted");
        assert!(error.contains(expected_error), "{protocol}/{kind}: {error}");
    }
}

#[test]
fn local_picker_protocol_and_kind_matrix_is_exact() {
    for protocol in ["opcua_server", "ads_server", "openot"] {
        assert!(is_local_symbol_picker(&browse_request(
            protocol, "symbols", None
        )));
        assert!(!is_local_symbol_picker(&browse_request(
            protocol, "nodes", None
        )));
    }
    for protocol in ["ads", "opcua_client", "ethercat", "unknown"] {
        assert!(!is_local_symbol_picker(&browse_request(
            protocol, "symbols", None
        )));
    }
}

#[test]
fn local_picker_respects_explicit_local_and_remote_targets() {
    for protocol in ["opcua_server", "ads_server", "openot"] {
        let explicit_local = browse_target(json!({ "local": true, "host": "remote.invalid" }));
        assert!(is_local_symbol_picker(&browse_request(
            protocol,
            "symbols",
            Some(explicit_local)
        )));

        let implicit_local = browse_target(json!({ "local": false, "host": " " }));
        assert!(is_local_symbol_picker(&browse_request(
            protocol,
            "symbols",
            Some(implicit_local)
        )));

        let remote = browse_target(json!({ "local": false, "host": "remote.internal" }));
        assert!(!is_local_symbol_picker(&browse_request(
            protocol,
            "symbols",
            Some(remote)
        )));
    }
}

#[test]
fn response_tree_value_uses_stable_shape_and_omits_empty_optionals() {
    let value = response_tree_value(
        "ethercat".to_string(),
        "channels".to_string(),
        Vec::new(),
        Vec::new(),
    )
    .expect("tree response");
    assert_eq!(value["schema_version"], BROWSE_SYMBOLS_SCHEMA_VERSION);
    assert_eq!(value["protocol"], "ethercat");
    assert_eq!(value["kind"], "channels");
    assert_eq!(value["tree"], json!([]));
    for omitted in ["error", "route", "ads_import", "warnings"] {
        assert!(value.get(omitted).is_none(), "unexpected {omitted}");
    }
}

#[test]
fn response_tree_error_has_empty_tree_and_no_success_payloads() {
    let value = response_tree_error_value(
        "opcua_client".to_string(),
        "nodes".to_string(),
        "browse_denied",
        "browse failed".to_string(),
    )
    .expect("tree error response");
    assert_eq!(value["schema_version"], BROWSE_SYMBOLS_SCHEMA_VERSION);
    assert_eq!(value["tree"], json!([]));
    assert_eq!(value["error"]["code"], "browse_denied");
    assert_eq!(value["error"]["message"], "browse failed");
    assert!(value.get("route").is_none());
    assert!(value.get("ads_import").is_none());
    assert!(value.get("warnings").is_none());
}

#[test]
fn tree_node_serialization_omits_absent_protocol_specific_metadata() {
    let value = serde_json::to_value(SymbolTreeNode {
        id: "group".to_string(),
        name: "group".to_string(),
        path: "group".to_string(),
        type_label: "group".to_string(),
        node_id: None,
        data_type: None,
        size: None,
        writable: None,
        children: Vec::new(),
    })
    .expect("serialize tree node");
    assert_eq!(
        value
            .as_object()
            .expect("node object")
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "id".to_string(),
            "name".to_string(),
            "path".to_string(),
            "type".to_string(),
        ])
    );
    assert!(value.get("children").is_none());
}

#[test]
fn ads_symbol_tree_is_sorted_at_every_level() {
    let tree = symbol_tree(&[
        ads_symbol("Z.Root", 4, false),
        ads_symbol("A.Zeta", 4, false),
        ads_symbol("A.Alpha", 4, false),
        ads_symbol("M", 4, false),
    ]);
    let root_names: Vec<_> = tree.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(root_names, vec!["A", "M", "Z"]);
    let child_names: Vec<_> = tree[0]
        .children
        .iter()
        .map(|node| node.name.as_str())
        .collect();
    assert_eq!(child_names, vec!["Alpha", "Zeta"]);
}

#[test]
fn ads_groups_and_leaves_have_exact_identity_and_metadata() {
    let tree = symbol_tree(&[ads_symbol("GVL.Pump.Setpoint", 4, true)]);
    let gvl = &tree[0];
    assert_eq!(gvl.id, "ads:group:GVL");
    assert_eq!(gvl.path, "GVL");
    assert_eq!(gvl.type_label, "group");
    assert_eq!(gvl.size, None);
    assert_eq!(gvl.writable, None);

    let pump = &gvl.children[0];
    assert_eq!(pump.id, "ads:group:GVL.Pump");
    assert_eq!(pump.path, "GVL.Pump");

    let leaf = &pump.children[0];
    assert_eq!(leaf.id, "ads:symbol:GVL.Pump.Setpoint");
    assert_eq!(leaf.name, "Setpoint");
    assert_eq!(leaf.path, "GVL.Pump.Setpoint");
    assert_eq!(leaf.type_label, "REAL");
    assert_eq!(leaf.size, Some(4));
    assert_eq!(leaf.writable, Some(true));
    assert!(leaf.children.is_empty());
}

#[test]
fn ads_leaf_writable_flag_depends_only_on_remote_write_capability() {
    let tree = symbol_tree(&[
        ads_symbol("A.ReadOnly", 4, false),
        ads_symbol("A.Writable", 4, true),
    ]);
    let children = &tree[0].children;
    assert_eq!(children[0].name, "ReadOnly");
    assert_eq!(children[0].writable, Some(false));
    assert_eq!(children[1].name, "Writable");
    assert_eq!(children[1].writable, Some(true));
}

#[test]
fn cached_ads_snapshot_is_canonicalized_for_tree_and_import() {
    let snapshot = SymbolSnapshot {
        schema_version: SYMBOL_SNAPSHOT_SCHEMA_VERSION,
        route_name: "line-a".to_string(),
        symbols: vec![
            ads_symbol("Z.Last", 4, false),
            ads_symbol("A.First", 4, true),
        ],
    };
    let value = browse_symbols_value(
        json!({
            "protocol": "ads",
            "snapshot": snapshot,
            "connection_name": "line-a"
        }),
        None,
        None,
    )
    .expect("cached ADS browse");
    assert_eq!(value["tree"][0]["name"], "A");
    assert_eq!(value["tree"][1]["name"], "Z");
    assert_eq!(
        value["ads_import"]["snapshot"]["symbols"][0]["name"],
        "A.First"
    );
    assert_eq!(
        value["ads_import"]["snapshot"]["symbols"][1]["name"],
        "Z.Last"
    );
}

#[test]
fn cached_ads_tree_and_import_share_exact_descriptor_denominator() {
    let snapshot = SymbolSnapshot::new(
        "line-a",
        vec![
            ads_symbol("A.One", 4, false),
            ads_symbol("A.Two", 4, true),
            ads_symbol("B.Three", 4, false),
        ],
    );
    let value = browse_symbols_value(
        json!({ "protocol": "ads", "snapshot": snapshot }),
        None,
        None,
    )
    .expect("cached ADS browse");
    fn leaf_count(nodes: &[Value]) -> usize {
        nodes
            .iter()
            .map(
                |node| match node.get("children").and_then(Value::as_array) {
                    Some(children) if !children.is_empty() => leaf_count(children),
                    _ => 1,
                },
            )
            .sum()
    }
    let tree_count = leaf_count(value["tree"].as_array().expect("tree"));
    let snapshot_count = value["ads_import"]["snapshot"]["symbols"]
        .as_array()
        .expect("snapshot symbols")
        .len();
    assert_eq!(tree_count, snapshot_count);
    assert_eq!(tree_count, 3);
}

#[test]
fn cached_ads_snapshot_rejects_wrong_schema_or_blank_route() {
    for snapshot in [
        json!({
            "schema_version": SYMBOL_SNAPSHOT_SCHEMA_VERSION + 1,
            "route_name": "line-a",
            "symbols": []
        }),
        json!({
            "schema_version": SYMBOL_SNAPSHOT_SCHEMA_VERSION,
            "route_name": "",
            "symbols": []
        }),
        json!({
            "schema_version": SYMBOL_SNAPSHOT_SCHEMA_VERSION,
            "route_name": " ",
            "symbols": []
        }),
    ] {
        let error = browse_symbols_value(
            json!({ "protocol": "ads", "snapshot": snapshot }),
            None,
            None,
        )
        .expect_err("invalid cached snapshot accepted");
        assert!(
            error.contains("snapshot") || error.contains("route"),
            "{error}"
        );
    }
}

#[test]
fn cached_ads_snapshot_rejects_invalid_or_duplicate_symbol_paths() {
    let invalid_names = ["", ".", ".Leading", "Trailing.", "A..B", " A.B", "A.B "];
    for name in invalid_names {
        let snapshot = SymbolSnapshot::new("line-a", vec![ads_symbol(name, 4, false)]);
        assert!(
            browse_symbols_value(
                json!({ "protocol": "ads", "snapshot": snapshot }),
                None,
                None
            )
            .is_err(),
            "invalid ADS symbol path accepted: {name:?}"
        );
    }

    let duplicate = SymbolSnapshot::new(
        "line-a",
        vec![
            ads_symbol("A.Duplicate", 4, false),
            ads_symbol("A.Duplicate", 4, true),
        ],
    );
    assert!(
        browse_symbols_value(
            json!({ "protocol": "ads", "snapshot": duplicate }),
            None,
            None
        )
        .is_err(),
        "duplicate ADS symbol path accepted"
    );
}

#[test]
fn ads_cached_browse_requires_no_target_or_live_state() {
    let snapshot = SymbolSnapshot::new("offline", vec![ads_symbol("A.Value", 4, false)]);
    let value = browse_symbols_value(
        json!({
            "protocol": "ads",
            "target": {
                "host": "must-not-resolve.invalid",
                "ams_net_id": "1.2.3.4.5.6"
            },
            "snapshot": snapshot
        }),
        None,
        None,
    )
    .expect("cached browse must remain offline");
    assert_eq!(value["protocol"], "ads");
    assert_eq!(value["tree"][0]["name"], "A");
    assert!(value.get("route").is_none());
}

#[test]
fn ads_live_browse_requires_target_or_snapshot() {
    let error = browse_symbols_value(json!({ "protocol": "ads", "kind": "symbols" }), None, None)
        .expect_err("missing ADS target accepted");
    assert!(
        error.contains("requires target or cached snapshot"),
        "{error}"
    );
}

#[test]
fn ads_instance_without_target_rejects_before_wire_activity() {
    let error = browse_symbols_value(
        json!({
            "protocol": "ads",
            "kind": "symbols",
            "instance_id": "ads:0"
        }),
        None,
        None,
    )
    .expect_err("instance-only ADS browse accepted");
    assert!(
        error.contains("instance_id needs the UI to pass target"),
        "{error}"
    );
}

#[test]
fn ads_target_aliases_and_defaults_project_exact_identity() {
    for target in [
        json!({
            "host": " 192.168.10.5 ",
            "ams_net_id": " 5.23.91.12.1.1 ",
            "name": "line-a"
        }),
        json!({
            "ip": " 192.168.10.5 ",
            "target_net_id": " 5.23.91.12.1.1 ",
            "name": "line-a"
        }),
    ] {
        let identity = browse_target(target).into_identity().expect("ADS identity");
        assert_eq!(identity.ip, "192.168.10.5");
        assert_eq!(identity.ams_net_id, "5.23.91.12.1.1");
        assert_eq!(identity.ams_port, 851);
        assert_eq!(identity.name.as_deref(), Some("line-a"));
    }
}

#[test]
fn ads_target_requires_host_and_six_octet_ams_net_id() {
    for target in [
        json!({ "ams_net_id": "1.2.3.4.5.6" }),
        json!({ "host": "127.0.0.1" }),
        json!({ "host": "127.0.0.1", "ams_net_id": "" }),
        json!({ "host": "127.0.0.1", "ams_net_id": "1.2.3.4.5" }),
        json!({ "host": "127.0.0.1", "ams_net_id": "1.2.3.4.5.6.7" }),
        json!({ "host": "127.0.0.1", "ams_net_id": "1.2.3.4.5.256" }),
        json!({ "host": "127.0.0.1", "ams_net_id": "1.2.3.4.5.x" }),
    ] {
        assert!(
            browse_target(target).into_identity().is_err(),
            "invalid ADS target accepted"
        );
    }
}

#[test]
fn ads_target_rejects_non_authority_host_and_zero_port() {
    for target in [
        json!({
            "host": "http://127.0.0.1",
            "ams_net_id": "1.2.3.4.5.6"
        }),
        json!({
            "host": "user@127.0.0.1",
            "ams_net_id": "1.2.3.4.5.6"
        }),
        json!({
            "host": "127.0.0.1/path",
            "ams_net_id": "1.2.3.4.5.6"
        }),
        json!({
            "host": "127.0.0.1",
            "ams_net_id": "1.2.3.4.5.6",
            "ams_port": 0
        }),
    ] {
        assert!(
            browse_target(target).into_identity().is_err(),
            "invalid ADS authority accepted"
        );
    }
}

#[test]
fn conflicting_ads_target_aliases_fail_deserialization() {
    for target in [
        json!({
            "ip": "192.168.1.10",
            "host": "192.168.1.11",
            "ams_net_id": "1.2.3.4.5.6"
        }),
        json!({
            "host": "192.168.1.10",
            "ams_net_id": "1.2.3.4.5.6",
            "target_net_id": "6.5.4.3.2.1"
        }),
    ] {
        assert!(
            serde_json::from_value::<BrowseTarget>(target).is_err(),
            "conflicting aliases accepted"
        );
    }
}

#[test]
fn opcua_endpoint_target_precedence_and_normalization_are_exact() {
    for (target, expected) in [
        (
            json!({
                "endpoint_url": " opc.tcp://primary:4841/path ",
                "host": "ignored:4842"
            }),
            "opc.tcp://primary:4841/path",
        ),
        (
            json!({ "host": "server.internal" }),
            "opc.tcp://server.internal:4840",
        ),
        (
            json!({ "host": "server.internal:4841" }),
            "opc.tcp://server.internal:4841",
        ),
        (json!({ "host": "[::1]" }), "opc.tcp://[::1]:4840"),
        (json!({ "host": "[::1]:4841" }), "opc.tcp://[::1]:4841"),
    ] {
        assert_eq!(
            browse_target(target)
                .opcua_endpoint_url()
                .expect("OPC UA endpoint"),
            expected
        );
    }
}

#[test]
fn opcua_endpoint_rejects_missing_or_malformed_authority() {
    for target in [
        json!({}),
        json!({ "endpoint_url": "" }),
        json!({ "host": "" }),
        json!({ "endpoint_url": "http://server:4840" }),
        json!({ "host": "http://server:4840" }),
        json!({ "host": "server:0" }),
        json!({ "host": "server:notaport" }),
        json!({ "host": "user@server:4840" }),
        json!({ "host": "server:4840?query" }),
        json!({ "host": "server:4840#fragment" }),
        json!({ "host": "::1" }),
    ] {
        assert!(
            browse_target(target).opcua_endpoint_url().is_err(),
            "invalid OPC UA endpoint accepted"
        );
    }
}

#[test]
fn opcua_security_profile_normalizes_policy_mode_and_auth_together() {
    for (auth, expected_anonymous) in [
        ("anonymous", true),
        (" ANONYMOUS ", true),
        ("username", false),
        (" USER ", false),
    ] {
        let target = browse_target(json!({
            "security_policy": " Basic256-Sha256 ",
            "security_mode": " Sign-And-Encrypt ",
            "auth": auth,
            "username": "operator",
            "password": "credential"
        }));
        let profile = target.opcua_security_profile().expect("security profile");
        assert_eq!(profile.policy.as_config_value(), "basic256sha256");
        assert_eq!(profile.mode.as_config_value(), "sign_and_encrypt");
        assert_eq!(profile.allow_anonymous, expected_anonymous, "auth {auth}");
    }
}

#[test]
fn opcua_security_rejects_unknown_policy_and_mode() {
    let policy = browse_target(json!({ "security_policy": "unknown" }));
    assert!(policy.opcua_security_profile().is_err());
    let mode = browse_target(json!({ "security_mode": "unknown" }));
    assert!(mode.opcua_security_profile().is_err());
}

#[test]
fn opcua_auth_defaults_anonymous_and_normalizes_username_aliases() {
    let anonymous = browse_target(json!({}))
        .opcua_auth()
        .expect("anonymous auth");
    assert!(matches!(
        anonymous,
        crate::opcua::OpcUaClientAuthConfig::Anonymous
    ));

    for auth in ["username", "user_name", "user", " USER "] {
        let target = browse_target(json!({
            "auth": auth,
            "username": " operator ",
            "password": " credential "
        }));
        match target.opcua_auth().expect("username auth") {
            crate::opcua::OpcUaClientAuthConfig::UserName { username, password } => {
                assert_eq!(username.as_str(), "operator");
                assert_eq!(password.as_str(), "credential");
            }
            crate::opcua::OpcUaClientAuthConfig::Anonymous => {
                panic!("username auth alias selected anonymous")
            }
        }
    }
}

#[test]
fn opcua_username_auth_requires_both_credentials_and_closed_mode() {
    for target in [
        json!({ "auth": "username", "password": "credential" }),
        json!({ "auth": "username", "username": "operator" }),
        json!({
            "auth": "username",
            "username": " ",
            "password": "credential"
        }),
        json!({
            "auth": "username",
            "username": "operator",
            "password": " "
        }),
        json!({ "auth": "certificate" }),
    ] {
        assert!(!opcua_error(&browse_target(target)).is_empty());
    }
}

#[test]
fn opcua_node_projection_recurses_without_losing_raw_identity() {
    let node = crate::opcua::OpcUaBrowseNode {
        id: "ns=2;s=Root".to_string(),
        name: "Root".to_string(),
        path: "Objects/Root".to_string(),
        data_type_id: "i=22".to_string(),
        data_type: "structure".to_string(),
        writable: false,
        children: vec![crate::opcua::OpcUaBrowseNode {
            id: "ns=2;s=Root.Value".to_string(),
            name: "Value".to_string(),
            path: "Objects/Root/Value".to_string(),
            data_type_id: "i=11".to_string(),
            data_type: "double".to_string(),
            writable: true,
            children: Vec::new(),
        }],
    };
    let projected = opcua_node_to_symbol(node);
    assert_eq!(projected.id, "opcua:node:ns_2_s_Root");
    assert_eq!(projected.node_id.as_deref(), Some("ns=2;s=Root"));
    assert_eq!(projected.path, "Objects/Root");
    assert_eq!(projected.type_label, "i=22");
    assert_eq!(projected.data_type.as_deref(), Some("structure"));
    assert_eq!(projected.writable, Some(false));
    assert_eq!(projected.children.len(), 1);
    let child = &projected.children[0];
    assert_eq!(child.id, "opcua:node:ns_2_s_Root.Value");
    assert_eq!(child.node_id.as_deref(), Some("ns=2;s=Root.Value"));
    assert_eq!(child.data_type.as_deref(), Some("double"));
    assert_eq!(child.writable, Some(true));
}

#[test]
fn sanitize_id_uses_closed_ascii_vocabulary() {
    for (input, expected) in [
        ("A-1_b.c", "A-1_b.c"),
        ("ns=2;s=A/B C", "ns_2_s_A_B_C"),
        ("åäö", "___"),
        ("", ""),
    ] {
        assert_eq!(sanitize_id(input), expected);
    }
}

#[test]
fn ethercat_module_projection_preserves_slot_channel_order_and_shape() {
    let node = ethercat_module_channel_node(&crate::io::EthercatModuleInfo {
        model: "EL1008".to_string(),
        slot: 3,
        channels: 3,
    });
    assert_eq!(node.id, "ethercat:module:slot3");
    assert_eq!(node.name, "EL1008 (slot 3)");
    assert_eq!(node.path, "ethercat.slot3");
    assert_eq!(node.type_label, "field_slave");
    assert_eq!(node.size, Some(3));
    assert_eq!(node.children.len(), 3);
    for (index, channel) in node.children.iter().enumerate() {
        assert_eq!(channel.id, format!("ethercat:channel:slot3:{index}"));
        assert_eq!(channel.name, format!("Channel {index}"));
        assert_eq!(channel.path, format!("ethercat.slot3.channel{index}"));
        assert_eq!(channel.type_label, "BOOL");
        assert_eq!(channel.size, Some(1));
        assert_eq!(channel.writable, None);
    }
}

#[test]
fn ethercat_driver_selection_retains_original_io_driver_index() {
    let drivers = vec![
        io_driver("mqtt"),
        io_driver("ethercat"),
        io_driver("loopback"),
        io_driver("ether-cat"),
    ];
    assert_eq!(
        select_ethercat_driver(&drivers, None)
            .expect("first EtherCAT")
            .name
            .as_str(),
        "ethercat"
    );
    assert_eq!(
        select_ethercat_driver(&drivers, Some("ethercat:1"))
            .expect("first identity")
            .name
            .as_str(),
        "ethercat"
    );
    assert_eq!(
        select_ethercat_driver(&drivers, Some("endpoint:ethercat:3"))
            .expect("second identity")
            .name
            .as_str(),
        "ether-cat"
    );
}

#[test]
fn ethercat_driver_selection_rejects_missing_stale_and_cross_protocol_ids() {
    let drivers = vec![io_driver("mqtt"), io_driver("ethercat")];
    for instance_id in [
        "ethercat:0",
        "ethercat:2",
        "mqtt:1",
        "endpoint:mqtt:1",
        "ethercat:not-an-index",
        "",
    ] {
        assert!(
            select_ethercat_driver(&drivers, Some(instance_id)).is_err(),
            "invalid EtherCAT instance accepted: {instance_id}"
        );
    }
    assert!(select_ethercat_driver(&[io_driver("mqtt")], None).is_err());
}
