use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use serde_json::{json, Value};

use super::*;

fn scope() -> DiscoverScope {
    DiscoverScope::default()
}

fn discover(protocol: &str) -> Result<Value, String> {
    discover_value(
        json!({
            "protocol": protocol,
            "scope": {},
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
}

#[test]
fn request_defaults_are_this_host_and_passive() {
    let value =
        discover_value(json!({ "protocol": "openot" }), None).expect("deferred discovery response");
    assert_eq!(value["schema_version"], DISCOVER_SCHEMA_VERSION);
    assert_eq!(value["protocol"], "openot");
    assert_eq!(value["origin"], "this_host");
    assert_eq!(value["candidates"], json!([]));
    let warnings = value["warnings"].as_array().expect("warnings");
    assert_eq!(warnings.len(), 1);
    assert!(!warnings[0]
        .as_str()
        .expect("warning string")
        .contains("Active write probes"));
}

#[test]
fn active_request_remains_read_only_and_reports_stable_warning() {
    let value = discover_value(
        json!({
            "protocol": "openot",
            "scope": {},
            "origin": "runtime",
            "passive": false
        }),
        None,
    )
    .expect("active request is downgraded safely");
    let warnings = value["warnings"].as_array().expect("warnings");
    assert_eq!(
        warnings[0],
        "Active write probes are not supported; discovery ran in connect/read-only mode."
    );
    assert!(warnings[1]
        .as_str()
        .expect("deferred warning")
        .contains("OpenOT discovery is not available yet"));
}

#[test]
fn request_requires_nonempty_string_protocol() {
    for params in [
        json!({}),
        json!({ "protocol": null }),
        json!({ "protocol": 7 }),
        json!({ "protocol": [] }),
        json!({ "protocol": {} }),
    ] {
        let error = discover_value(params, None).expect_err("malformed protocol accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
    for protocol in ["", " ", "\t\r\n"] {
        let error = discover(protocol).expect_err("blank protocol accepted");
        assert!(error.contains("does not know protocol"), "{error}");
    }
}

#[test]
fn nonobject_scope_and_unknown_keys_are_rejected() {
    for scope in [
        Value::Null,
        json!(7),
        json!("scope"),
        json!([]),
        json!(true),
    ] {
        let error = discover_value(json!({ "protocol": "openot", "scope": scope }), None)
            .expect_err("nonobject scope accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }

    for params in [
        json!({ "protocol": "openot", "pasive": true }),
        json!({
            "protocol": "openot",
            "scope": { "timeot_ms": 50 }
        }),
    ] {
        let error = discover_value(params, None).expect_err("unknown key accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
}

#[test]
fn origin_vocabulary_is_closed() {
    for origin in ["this_host", "runtime"] {
        let value = discover_value(json!({ "protocol": "openot", "origin": origin }), None)
            .expect("valid origin");
        assert_eq!(value["origin"], origin);
    }
    for origin in ["host", "local", "remote", "", "RUNTIME"] {
        let error = discover_value(json!({ "protocol": "openot", "origin": origin }), None)
            .expect_err("unknown origin accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
}

#[test]
fn canonical_protocol_aliases_are_exact() {
    for (alias, expected) in [
        (" MODBUS-TCP ", "modbus_tcp"),
        ("modbus_tcp", "modbus_tcp"),
        ("OPC-UA", "opcua"),
        ("opc_ua_server", "opcua"),
        ("opcua_server", "opcua"),
        ("OPC-UA-CLIENT", "opcua_client"),
        ("opc_ua_client", "opcua_client"),
        ("mqtt_broker", "mqtt"),
        ("ADS-CLIENT", "ads"),
        ("TwinCAT", "ads"),
        ("trust", "discovery"),
        ("trust-runtime", "discovery"),
        ("trust_runtimes", "discovery"),
        ("mDNS", "discovery"),
    ] {
        assert_eq!(canonical_protocol(alias), expected, "alias {alias}");
    }
}

#[test]
fn canonical_protocol_preserves_unknown_normalized_identity() {
    assert_eq!(canonical_protocol(" Custom-Protocol "), "custom_protocol");
    assert_eq!(canonical_protocol("UNKNOWN"), "unknown");
}

#[test]
fn known_deferred_protocols_return_stable_titled_warning() {
    for (protocol, title) in [
        ("simulated", "Simulated I/O"),
        ("loopback", "Loopback"),
        ("openot", "OpenOT"),
        ("mesh", "Mesh"),
        ("runtime_cloud", "Runtime cloud"),
        ("realtime_t0", "Realtime T0"),
        ("ads_server", "ADS server"),
    ] {
        let value = discover(protocol).expect("known deferred protocol");
        assert_eq!(value["protocol"], protocol);
        assert_eq!(value["candidates"], json!([]));
        assert_eq!(
            value["warnings"][0],
            format!("{title} discovery is not available yet.")
        );
    }
}

#[test]
fn known_without_discovery_registry_is_exact() {
    for protocol in [
        "ethercat",
        "gpio",
        "simulated",
        "loopback",
        "openot",
        "opcua",
        "mesh",
        "runtime_cloud",
        "realtime_t0",
        "ads_server",
    ] {
        assert!(
            known_protocol_without_discovery(protocol),
            "missing known protocol {protocol}"
        );
    }
    for protocol in [
        "modbus_tcp",
        "mqtt",
        "opcua_client",
        "ads",
        "discovery",
        "unknown",
    ] {
        assert!(
            !known_protocol_without_discovery(protocol),
            "active/unknown protocol classified as deferred: {protocol}"
        );
    }
}

#[test]
fn protocol_titles_are_stable_for_every_deferred_identity() {
    for (protocol, title) in [
        ("ads_server", "ADS server"),
        ("ethercat", "EtherCAT"),
        ("gpio", "GPIO"),
        ("loopback", "Loopback"),
        ("mesh", "Mesh"),
        ("openot", "OpenOT"),
        ("opcua", "OPC UA server"),
        ("opcua_client", "OPC UA client"),
        ("realtime_t0", "Realtime T0"),
        ("runtime_cloud", "Runtime cloud"),
        ("simulated", "Simulated I/O"),
    ] {
        assert_eq!(protocol_title(protocol), title);
    }
    assert_eq!(protocol_title("unknown"), "Selected protocol");
}

#[test]
fn timeout_defaults_and_clamps_to_closed_bounds() {
    assert_eq!(timeout(&scope()), Duration::from_millis(DEFAULT_TIMEOUT_MS));
    for (requested, expected) in [
        (0, 1),
        (1, 1),
        (150, 150),
        (MAX_TIMEOUT_MS, MAX_TIMEOUT_MS),
        (MAX_TIMEOUT_MS + 1, MAX_TIMEOUT_MS),
        (u64::MAX, MAX_TIMEOUT_MS),
    ] {
        let scope = DiscoverScope {
            timeout_ms: Some(requested),
            ..scope()
        };
        assert_eq!(
            timeout(&scope),
            Duration::from_millis(expected),
            "timeout {requested}"
        );
    }
}

#[test]
fn malformed_timeout_types_are_request_errors() {
    for malformed in [
        json!(-1),
        json!(1.5),
        json!("150"),
        json!(true),
        json!([]),
        json!({}),
    ] {
        let error = discover_value(
            json!({
                "protocol": "openot",
                "scope": { "timeout_ms": malformed }
            }),
            None,
        )
        .expect_err("malformed timeout accepted");
        assert!(
            error.starts_with("invalid comm.discover payload:"),
            "{error}"
        );
    }
}

#[test]
fn modbus_safe_read_defaults_and_bounds_are_exact() {
    let default = modbus_discovery_probe(&scope()).expect("default probe");
    assert_eq!(default.unit_id, 1);
    assert_eq!(default.safe_read, None);

    let with_address = modbus_discovery_probe(&DiscoverScope {
        unit_id: Some(255),
        probe_read_address: Some(400),
        ..scope()
    })
    .expect("safe read defaults");
    assert_eq!(with_address.unit_id, 255);
    assert_eq!(
        with_address.safe_read,
        Some(ModbusSafeReadProbe {
            address: 400,
            quantity: 1
        })
    );

    for quantity in [1, 125] {
        let probe = modbus_discovery_probe(&DiscoverScope {
            probe_read_address: Some(u16::MAX),
            probe_read_quantity: Some(quantity),
            ..scope()
        })
        .expect("boundary quantity");
        assert_eq!(probe.safe_read.expect("safe read").quantity, quantity);
    }
}

#[test]
fn modbus_safe_read_rejects_orphan_or_out_of_range_quantity() {
    let orphan = modbus_discovery_probe(&DiscoverScope {
        probe_read_quantity: Some(1),
        ..scope()
    })
    .expect_err("quantity without address accepted");
    assert!(orphan.contains("probe_read_address is required"));

    for quantity in [0, 126, u16::MAX] {
        let error = modbus_discovery_probe(&DiscoverScope {
            probe_read_address: Some(0),
            probe_read_quantity: Some(quantity),
            ..scope()
        })
        .expect_err("invalid quantity accepted");
        assert!(error.contains("between 1 and 125"), "{error}");
    }
}

#[test]
fn ipv4_cidr_parser_trims_and_accepts_closed_prefix_domain() {
    for (cidr, expected_ip, expected_prefix) in [
        ("10.20.30.40/24", Ipv4Addr::new(10, 20, 30, 40), 24),
        (" 10.20.30.40/31 ", Ipv4Addr::new(10, 20, 30, 40), 31),
        ("192.168.1.7/32", Ipv4Addr::new(192, 168, 1, 7), 32),
        ("0.0.0.0/0", Ipv4Addr::UNSPECIFIED, 0),
    ] {
        assert_eq!(
            parse_ipv4_cidr(cidr).expect("valid CIDR"),
            (expected_ip, expected_prefix)
        );
    }
}

#[test]
fn ipv4_cidr_parser_rejects_malformed_forms() {
    for cidr in [
        "",
        "10.0.0.1",
        "10.0.0.1/",
        "/24",
        "10.0.0.1/not-a-prefix",
        "10.0.0.1/33",
        "300.0.0.1/24",
        "10.0.0.1/24/extra",
        "::1/128",
    ] {
        assert!(
            parse_ipv4_cidr(cidr).is_err(),
            "malformed CIDR accepted: {cidr}"
        );
    }
}

#[test]
fn cidr_enumeration_rejects_ranges_broader_than_slash_24() {
    for cidr in ["0.0.0.0/0", "10.0.0.0/8", "10.0.0.0/16", "10.0.0.0/23"] {
        let error = ipv4_targets_for_cidr(cidr).expect_err("broad CIDR accepted");
        assert!(error.contains("/24 or tighter"), "{error}");
    }
}

#[test]
fn cidr_enumeration_is_canonical_ascending_and_bounded() {
    let cases = [
        (
            "10.0.0.42/24",
            254,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(10, 0, 0, 254),
        ),
        (
            "10.0.0.130/25",
            126,
            Ipv4Addr::new(10, 0, 0, 129),
            Ipv4Addr::new(10, 0, 0, 254),
        ),
        (
            "10.0.0.6/30",
            2,
            Ipv4Addr::new(10, 0, 0, 5),
            Ipv4Addr::new(10, 0, 0, 6),
        ),
        (
            "10.0.0.6/31",
            2,
            Ipv4Addr::new(10, 0, 0, 6),
            Ipv4Addr::new(10, 0, 0, 7),
        ),
        (
            "10.0.0.6/32",
            1,
            Ipv4Addr::new(10, 0, 0, 6),
            Ipv4Addr::new(10, 0, 0, 6),
        ),
    ];
    for (cidr, count, first, last) in cases {
        let targets = ipv4_targets_for_cidr(cidr).expect("CIDR targets");
        assert_eq!(targets.len(), count, "{cidr}");
        assert_eq!(targets.first(), Some(&first), "{cidr}");
        assert_eq!(targets.last(), Some(&last), "{cidr}");
        assert!(
            targets.windows(2).all(|pair| pair[0] < pair[1]),
            "{cidr} is not strictly ascending"
        );
    }
}

#[test]
fn modbus_socket_target_defaults_port_and_preserves_explicit_socket() {
    assert_eq!(
        modbus_socket_target("127.0.0.1").expect("default port"),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MODBUS_PORT)
    );
    assert_eq!(
        modbus_socket_target("127.0.0.1:1502").expect("explicit port"),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1502)
    );
    assert_eq!(
        modbus_socket_target("[::1]:1502").expect("IPv6 socket"),
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1502)
    );
}

#[test]
fn socket_targets_reject_zero_invalid_and_non_authority_ports() {
    for host in [
        "127.0.0.1:0",
        "127.0.0.1:notaport",
        "http://127.0.0.1:502",
        "user@127.0.0.1:502",
        "127.0.0.1:502/path",
        "127.0.0.1:502?query",
        "127.0.0.1:502#fragment",
        "::1",
    ] {
        assert!(
            modbus_socket_target(host).is_err(),
            "invalid Modbus socket target accepted: {host}"
        );
    }
}

#[test]
fn mqtt_missing_host_returns_none_and_one_stable_warning() {
    for host in [None, Some(""), Some(" \t ")] {
        let mut warnings = Vec::new();
        let targets = mqtt_targets(
            &DiscoverScope {
                host: host.map(str::to_string),
                ..scope()
            },
            &mut warnings,
        )
        .expect("missing MQTT host");
        assert_eq!(targets, None);
        assert_eq!(
            warnings,
            vec![
                "MQTT discovery needs a broker host; broad network scans are not run for MQTT."
                    .to_string()
            ]
        );
    }
}

#[test]
fn mqtt_explicit_socket_produces_one_target() {
    for (host, expected) in [
        (
            "127.0.0.1:1884",
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1884),
        ),
        (
            "[::1]:8883",
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 8883),
        ),
    ] {
        let mut warnings = Vec::new();
        let targets = mqtt_targets(
            &DiscoverScope {
                host: Some(host.to_string()),
                ..scope()
            },
            &mut warnings,
        )
        .expect("MQTT target")
        .expect("directed targets");
        assert_eq!(targets, vec![expected]);
        assert!(warnings.is_empty());
    }
}

#[test]
fn mqtt_omitted_port_probes_cleartext_then_tls_without_duplicates() {
    let mut warnings = Vec::new();
    let targets = mqtt_targets(
        &DiscoverScope {
            host: Some("127.0.0.1".to_string()),
            ..scope()
        },
        &mut warnings,
    )
    .expect("MQTT targets")
    .expect("directed targets");
    assert_eq!(
        targets,
        vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MQTT_PORT),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MQTTS_PORT),
        ]
    );
    let unique: std::collections::BTreeSet<_> = targets.iter().copied().collect();
    assert_eq!(unique.len(), targets.len());
}

#[test]
fn mqtt_target_rejects_malformed_authorities_before_probe() {
    for host in [
        "127.0.0.1:0",
        "mqtt://127.0.0.1",
        "mqtts://127.0.0.1",
        "user@127.0.0.1:1883",
        "127.0.0.1:1883/path",
        "127.0.0.1:1883?query",
        "127.0.0.1:1883#fragment",
        "127.0.0.1:notaport",
        "::1",
    ] {
        let mut warnings = Vec::new();
        assert!(
            mqtt_targets(
                &DiscoverScope {
                    host: Some(host.to_string()),
                    ..scope()
                },
                &mut warnings
            )
            .is_err(),
            "invalid MQTT authority accepted: {host}"
        );
    }
}

#[test]
fn opcua_endpoint_requires_nonempty_host() {
    for host in [None, Some(""), Some(" \t ")] {
        let error = opcua_endpoint_url(&DiscoverScope {
            host: host.map(str::to_string),
            ..scope()
        })
        .expect_err("empty OPC UA host accepted");
        assert!(error.contains("scope.host"));
    }
}

#[test]
fn opcua_endpoint_normalizes_valid_authorities() {
    for (host, expected) in [
        ("server.internal", "opc.tcp://server.internal:4840"),
        ("server.internal:4841", "opc.tcp://server.internal:4841"),
        ("server.internal/trust", "opc.tcp://server.internal/trust"),
        (
            "opc.tcp://server.internal:4841/trust",
            "opc.tcp://server.internal:4841/trust",
        ),
        ("127.0.0.1", "opc.tcp://127.0.0.1:4840"),
        ("[::1]", "opc.tcp://[::1]:4840"),
        ("[::1]:4841", "opc.tcp://[::1]:4841"),
    ] {
        let endpoint = opcua_endpoint_url(&DiscoverScope {
            host: Some(format!(" {host} ")),
            ..scope()
        })
        .expect("OPC UA endpoint");
        assert_eq!(endpoint, expected, "{host}");
    }
}

#[test]
fn opcua_endpoint_rejects_malformed_or_unsupported_urls() {
    for host in [
        "http://server.internal:4840",
        "https://server.internal:4840",
        "user@server.internal:4840",
        "server.internal:0",
        "server.internal:notaport",
        "server.internal:4840?query",
        "server.internal:4840#fragment",
        "::1",
    ] {
        assert!(
            opcua_endpoint_url(&DiscoverScope {
                host: Some(host.to_string()),
                ..scope()
            })
            .is_err(),
            "invalid OPC UA endpoint accepted: {host}"
        );
    }
}

#[test]
fn sanitize_id_retains_only_closed_ascii_vocabulary() {
    for (input, expected) in [
        ("alpha-1_beta.two", "alpha-1_beta.two"),
        ("a:b/c d", "a_b_c_d"),
        ("åäö", "___"),
        ("", ""),
    ] {
        assert_eq!(sanitize_id(input), expected);
    }
}

#[test]
fn candidate_serialization_omits_empty_warning_list() {
    let value = serde_json::to_value(DiscoverCandidate {
        id: "mqtt:127.0.0.1_1883".to_string(),
        label: "MQTT broker 127.0.0.1:1883".to_string(),
        source: "tcp_connect",
        confidence: "port_reachable",
        params: json!({ "broker": "127.0.0.1:1883" }),
        warnings: Vec::new(),
    })
    .expect("candidate serialization");
    assert!(value.get("warnings").is_none());
    assert_eq!(value["source"], "tcp_connect");
    assert_eq!(value["confidence"], "port_reachable");
}

#[test]
fn runtime_only_hardware_discovery_rejects_this_host_origin_without_hardware_io() {
    for protocol in ["ethercat", "gpio"] {
        let value = discover_value(
            json!({
                "protocol": protocol,
                "origin": "this_host",
                "scope": { "adapter": "must-not-open" }
            }),
            None,
        )
        .expect("runtime-origin warning");
        assert_eq!(value["candidates"], json!([]));
        assert!(value["warnings"][0]
            .as_str()
            .expect("warning")
            .contains("runtime host"));
    }
}

#[test]
fn opcua_server_discovery_never_calls_client_discovery() {
    let value = discover_value(
        json!({
            "protocol": "opcua",
            "scope": { "host": "must-not-resolve.invalid:4840" },
            "origin": "this_host"
        }),
        None,
    )
    .expect("server setup warning");
    assert_eq!(value["candidates"], json!([]));
    assert!(value["warnings"][0]
        .as_str()
        .expect("warning")
        .contains("OPC UA server setup"));
}
