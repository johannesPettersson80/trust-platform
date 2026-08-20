use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use indexmap::IndexMap;
use smol_str::SmolStr;

use super::*;
use crate::config::MeshRole;
use crate::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

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
            listen: SmolStr::new("127.0.0.1:8080"),
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
            listen: SmolStr::new("127.0.0.1:7447"),
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

fn discovery_entry(
    id: &str,
    name: &str,
    addresses: Vec<IpAddr>,
    last_seen_ms: u64,
) -> DiscoveryEntry {
    DiscoveryEntry {
        id: SmolStr::new(id),
        name: SmolStr::new(name),
        addresses,
        web_port: None,
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        last_seen_ns: last_seen_ms.saturating_mul(1_000_000),
    }
}

#[test]
fn fleet_host_contract_sanitizer_lowercases_ascii_alphanumeric_text() {
    assert_eq!(sanitize_id("LineA42"), "linea42");
}

#[test]
fn fleet_host_contract_sanitizer_replaces_each_non_alphanumeric_character() {
    assert_eq!(sanitize_id("Line A/PLC_1"), "line-a-plc-1");
    assert_eq!(sanitize_id("a::b"), "a--b");
}

#[test]
fn fleet_host_contract_sanitizer_trims_boundary_hyphens() {
    assert_eq!(sanitize_id(" --PLC-- "), "plc");
}

#[test]
fn fleet_host_contract_sanitizer_uses_unknown_for_empty_identity() {
    for input in ["", " ", "---", "___", "åäö"] {
        assert_eq!(sanitize_id(input), "unknown", "{input:?}");
    }
}

#[test]
fn fleet_host_contract_stable_host_id_uses_common_sanitizer() {
    assert_eq!(stable_host_id("Line A.PLC"), "host:line-a-plc");
}

#[test]
fn fleet_host_contract_endpoint_id_uses_runtime_and_protocol() {
    assert_eq!(
        endpoint_id("controller-a", "modbus_tcp"),
        "endpoint:controller-a:modbus_tcp"
    );
}

#[test]
fn fleet_host_contract_first_endpoint_instance_omits_index_suffix() {
    assert_eq!(
        endpoint_instance_id("controller-a", "mqtt", 0),
        "endpoint:controller-a:mqtt"
    );
}

#[test]
fn fleet_host_contract_later_endpoint_instance_uses_zero_based_index() {
    assert_eq!(
        endpoint_instance_id("controller-a", "mqtt", 3),
        "endpoint:controller-a:mqtt:3"
    );
}

#[test]
fn fleet_host_contract_normalized_hostname_trims_space_and_dns_dot() {
    assert_eq!(
        normalized_host_name("  plc-a.example.local.  "),
        Some("plc-a.example.local".to_string())
    );
}

#[test]
fn fleet_host_contract_normalized_hostname_rejects_empty_or_dot_only_values() {
    for value in ["", " ", ".", "...", "  ...  "] {
        assert_eq!(normalized_host_name(value), None, "{value:?}");
    }
}

#[test]
fn fleet_host_contract_hostname_prefers_environment_hostname() {
    assert_eq!(
        host_name_from_sources(Some("env-host"), Some("computer"), Some("os-host")),
        "env-host"
    );
}

#[test]
fn fleet_host_contract_hostname_falls_back_through_empty_sources() {
    assert_eq!(
        host_name_from_sources(Some(" "), Some("computer."), Some("os-host")),
        "computer"
    );
    assert_eq!(
        host_name_from_sources(Some(""), Some(""), Some(" os-host. ")),
        "os-host"
    );
}

#[test]
fn fleet_host_contract_hostname_uses_literal_fallback_when_all_sources_absent() {
    assert_eq!(host_name_from_sources(None, None, None), "local-host");
}

#[test]
fn fleet_host_contract_listen_parser_extracts_ipv4_host() {
    assert_eq!(
        host_from_listen(" 192.0.2.10:8080 "),
        Some("192.0.2.10".to_string())
    );
}

#[test]
fn fleet_host_contract_listen_parser_extracts_dns_host() {
    assert_eq!(
        host_from_listen("plc.local:4840"),
        Some("plc.local".to_string())
    );
}

#[test]
fn fleet_host_contract_listen_parser_unbrackets_ipv6() {
    assert_eq!(
        host_from_listen("[2001:db8::10]:4840"),
        Some("2001:db8::10".to_string())
    );
}

#[test]
fn fleet_host_contract_listen_parser_retains_bare_host() {
    assert_eq!(host_from_listen("plc.local"), Some("plc.local".to_string()));
}

#[test]
fn fleet_host_contract_listen_parser_rejects_empty_and_unclosed_bracket() {
    assert_eq!(host_from_listen("   "), None);
    assert_eq!(host_from_listen("[2001:db8::10"), None);
}

#[test]
fn fleet_host_contract_configured_ips_are_sorted_and_deduplicated() {
    let mut settings = settings();
    settings.web.listen = SmolStr::new("192.0.2.20:8080");
    settings.opcua.listen = SmolStr::new("192.0.2.10:4840");
    settings.mesh.listen = SmolStr::new("192.0.2.20:7447");
    assert_eq!(configured_host_ips(&settings), ["192.0.2.10", "192.0.2.20"]);
}

#[test]
fn fleet_host_contract_configured_ips_exclude_unspecified_addresses() {
    let mut settings = settings();
    settings.web.listen = SmolStr::new("0.0.0.0:8080");
    settings.opcua.listen = SmolStr::new("[::]:4840");
    settings.mesh.listen = SmolStr::new("[2001:db8::2]:7447");
    assert_eq!(configured_host_ips(&settings), ["2001:db8::2"]);
}

#[test]
fn fleet_host_contract_configured_ips_fall_back_to_loopback() {
    let mut settings = settings();
    settings.web.listen = SmolStr::new("0.0.0.0:8080");
    settings.opcua.listen = SmolStr::new("[::]:4840");
    settings.mesh.listen = SmolStr::new("0.0.0.0:7447");
    assert_eq!(configured_host_ips(&settings), ["127.0.0.1"]);
}

#[test]
fn fleet_host_contract_discovery_timestamp_floors_nanoseconds_to_milliseconds() {
    let mut entry = discovery_entry("id", "peer", Vec::new(), 0);
    entry.last_seen_ns = 12_345_678;
    assert_eq!(discovery_last_seen_ms(&entry), 12);
}

#[test]
fn fleet_host_contract_newest_discovery_timestamp_uses_complete_set() {
    let entries = [
        discovery_entry("a", "a", Vec::new(), 10),
        discovery_entry("b", "b", Vec::new(), 30),
        discovery_entry("c", "c", Vec::new(), 20),
    ];
    assert_eq!(newest_discovery_seen_ms(&entries), Some(30));
    assert_eq!(newest_discovery_seen_ms(&[]), None);
}

#[test]
fn fleet_host_contract_future_discovery_timestamp_is_fresh() {
    let entry = discovery_entry(
        "future",
        "future",
        Vec::new(),
        now_ms().saturating_add(60_000),
    );
    assert!(!discovery_is_stale(&entry));
    assert_eq!(discovery_health(&entry), "connected");
}

#[test]
fn fleet_host_contract_old_discovery_timestamp_is_unreachable() {
    let entry = discovery_entry(
        "old",
        "old",
        Vec::new(),
        now_ms().saturating_sub(DISCOVERY_STALE_AFTER_MS + 10_000),
    );
    assert!(discovery_is_stale(&entry));
    assert_eq!(discovery_health(&entry), "runtime_unreachable");
}

#[test]
fn fleet_host_contract_discovery_protocol_order_is_stable() {
    let mut entry = discovery_entry("id", "peer", Vec::new(), 1);
    entry.web_port = Some(8080);
    entry.mesh_port = Some(7447);
    entry.control = Some(SmolStr::new("tcp://192.0.2.10:9900"));
    assert_eq!(
        discovery_protocols(&entry),
        ["discovery", "web", "mesh", "control"]
    );
}

#[test]
fn fleet_host_contract_discovery_protocols_include_only_advertised_surfaces() {
    let mut entry = discovery_entry("id", "peer", Vec::new(), 1);
    entry.mesh_port = Some(7447);
    assert_eq!(discovery_protocols(&entry), ["discovery", "mesh"]);
}

#[test]
fn fleet_host_contract_self_discovery_matches_name_or_id_exactly() {
    let by_name = discovery_entry("service-id", "runtime-a", Vec::new(), 1);
    let by_id = discovery_entry("runtime-a", "other-name", Vec::new(), 1);
    let other = discovery_entry("runtime-a-extra", "Runtime-A", Vec::new(), 1);
    assert!(is_self_discovery_entry("runtime-a", &by_name));
    assert!(is_self_discovery_entry("runtime-a", &by_id));
    assert!(!is_self_discovery_entry("runtime-a", &other));
}

#[test]
fn fleet_host_contract_discovered_runtime_id_uses_runtime_name() {
    let entry = discovery_entry("service-id", "Line A PLC", Vec::new(), 1);
    assert_eq!(discovered_runtime_id(&entry), "runtime:line-a-plc");
}

#[test]
fn fleet_host_contract_discovered_host_id_prefers_host_group() {
    let mut entry = discovery_entry("service-id", "runtime-a", Vec::new(), 1);
    entry.host_group = Some(SmolStr::new("Cell A"));
    assert_eq!(discovered_host_id(&entry), "host:cell-a");
}

#[test]
fn fleet_host_contract_discovered_host_id_falls_back_to_discovery_identity() {
    let entry = discovery_entry("Service ID", "runtime-a", Vec::new(), 1);
    assert_eq!(discovered_host_id(&entry), "host:discovery:service-id");
}

#[test]
fn fleet_host_contract_discovered_external_id_uses_discovery_identity() {
    let entry = discovery_entry("Service ID", "runtime-a", Vec::new(), 1);
    assert_eq!(
        discovered_external_id(&entry),
        "external:discovery:service-id"
    );
}

#[test]
fn fleet_host_contract_same_host_requires_matching_group_on_self_entry() {
    let mut peer = discovery_entry("peer-id", "peer", Vec::new(), 1);
    peer.host_group = Some(SmolStr::new("cell-a"));
    let mut self_entry = discovery_entry("self-id", "runtime-a", Vec::new(), 1);
    self_entry.host_group = Some(SmolStr::new("cell-a"));
    assert!(same_host_by_discovery(
        "runtime-a",
        &peer,
        &[peer.clone(), self_entry]
    ));
}

#[test]
fn fleet_host_contract_same_host_rejects_missing_or_different_group() {
    let peer_without_group = discovery_entry("peer-id", "peer", Vec::new(), 1);
    assert!(!same_host_by_discovery(
        "runtime-a",
        &peer_without_group,
        std::slice::from_ref(&peer_without_group)
    ));

    let mut peer = discovery_entry("peer-id", "peer", Vec::new(), 1);
    peer.host_group = Some(SmolStr::new("cell-a"));
    let mut self_entry = discovery_entry("self-id", "runtime-a", Vec::new(), 1);
    self_entry.host_group = Some(SmolStr::new("cell-b"));
    assert!(!same_host_by_discovery(
        "runtime-a",
        &peer,
        &[peer.clone(), self_entry]
    ));
}

#[test]
fn fleet_host_contract_first_ipv4_address_with_port_is_unambiguous() {
    let entry = discovery_entry(
        "id",
        "peer",
        vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))],
        1,
    );
    assert_eq!(
        first_address_with_port(&entry, 8080),
        Some("192.0.2.10:8080".to_string())
    );
}

#[test]
fn fleet_host_contract_first_ipv6_address_with_port_is_bracketed() {
    let entry = discovery_entry(
        "id",
        "peer",
        vec![IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 10))],
        1,
    );
    assert_eq!(
        first_address_with_port(&entry, 8080),
        Some("[2001:db8::a]:8080".to_string())
    );
}

#[test]
fn fleet_host_contract_missing_address_has_no_socket_authority() {
    let entry = discovery_entry("id", "peer", Vec::new(), 1);
    assert_eq!(first_address_with_port(&entry, 8080), None);
}

#[test]
fn fleet_host_contract_container_detection_recognizes_supported_cgroups() {
    for cgroup in [
        "12:devices:/docker/abcdef",
        "0::/kubepods.slice/pod",
        "0::/system.slice/containerd.service",
        "0::/libpod-123.scope",
    ] {
        assert!(looks_containerized(cgroup), "{cgroup}");
    }
    assert!(!looks_containerized("0::/user.slice/user-1000.slice"));
}

#[test]
fn fleet_host_contract_container_id_reads_bare_hex_component() {
    let cgroup = "0::/docker/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    assert_eq!(parse_container_id(cgroup), Some("0123456789ab".to_string()));
}

#[test]
fn fleet_host_contract_container_id_reads_systemd_docker_scope() {
    let cgroup = "0::/system.slice/docker-fedcba9876543210fedcba9876543210fedcba9876543210.scope";
    assert_eq!(parse_container_id(cgroup), Some("fedcba987654".to_string()));
}

#[test]
fn fleet_host_contract_container_id_rejects_short_or_non_hex_components() {
    for cgroup in [
        "0::/docker/0123456789a",
        "0::/system.slice/docker-not-a-hex-identifier.scope",
        "",
    ] {
        assert_eq!(parse_container_id(cgroup), None, "{cgroup}");
    }
}
