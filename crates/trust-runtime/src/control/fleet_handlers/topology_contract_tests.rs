use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use indexmap::IndexMap;
use serde_json::json;
use smol_str::SmolStr;

use super::*;
use crate::config::{
    MeshRole, RuntimeCloudLinkPreferenceRule, RuntimeCloudPreferredTransport, RuntimeCloudProfile,
    RuntimeCloudWanAllowRule,
};
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

fn driver(name: &str, enabled: bool, source: &str) -> IoDriverConfig {
    IoDriverConfig {
        name: SmolStr::new(name),
        params: toml::from_str(source).expect("valid test TOML"),
        enabled,
    }
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
fn fleet_topology_contract_schema_version_is_four_and_empty_discovered_is_omitted() {
    let response = FleetTopologyResponse {
        schema_version: FLEET_TOPOLOGY_SCHEMA_VERSION,
        hosts: Vec::new(),
        links: Vec::new(),
        shared: Vec::new(),
        external: Vec::new(),
        discovered: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(response).expect("topology response"),
        json!({
            "schema_version": 4,
            "hosts": [],
            "links": [],
            "shared": [],
            "external": [],
        })
    );
}

#[test]
fn fleet_topology_contract_nonempty_discovered_is_serialized() {
    let response = FleetTopologyResponse {
        schema_version: 4,
        hosts: Vec::new(),
        links: Vec::new(),
        shared: Vec::new(),
        external: Vec::new(),
        discovered: vec![FleetDiscovered {
            id: "external:discovery:peer".to_string(),
            kind: "runtime".to_string(),
            name: "peer".to_string(),
            addresses: vec!["192.0.2.10".to_string()],
            via_protocol: vec!["discovery".to_string()],
            direction: "observed".to_string(),
            adopted: false,
            control: None,
            web_port: None,
            web_tls: Some(false),
            mesh_port: None,
            host_group: None,
            last_seen_ms: Some(10),
            source: Some("discovery".to_string()),
        }],
    };
    let value = serde_json::to_value(response).expect("topology response");
    assert_eq!(value["discovered"].as_array().map(Vec::len), Some(1));
}

#[test]
fn fleet_topology_contract_runtime_cloud_dev_without_rules_is_not_configured() {
    assert!(!runtime_cloud_configured(&settings()));
}

#[test]
fn fleet_topology_contract_runtime_cloud_non_dev_profile_is_configured() {
    for profile in [RuntimeCloudProfile::Plant, RuntimeCloudProfile::Wan] {
        let mut settings = settings();
        settings.runtime_cloud.profile = profile;
        assert!(runtime_cloud_configured(&settings), "{profile:?}");
    }
}

#[test]
fn fleet_topology_contract_runtime_cloud_rules_configure_dev_profile() {
    let mut allow = settings();
    allow
        .runtime_cloud
        .wan_allow_write
        .push(RuntimeCloudWanAllowRule {
            action: SmolStr::new("write"),
            target: SmolStr::new("peer"),
        });
    assert!(runtime_cloud_configured(&allow));

    let mut links = settings();
    links
        .runtime_cloud
        .link_preferences
        .push(RuntimeCloudLinkPreferenceRule {
            source: SmolStr::new(""),
            target: SmolStr::new(""),
            transport: RuntimeCloudPreferredTransport::Mesh,
        });
    assert!(runtime_cloud_configured(&links));
}

#[test]
fn fleet_topology_contract_shared_mqtt_ignores_other_protocols_and_empty_brokers() {
    let drivers = [
        driver("modbus-tcp", true, r#"address = "plc:502""#),
        driver("mqtt", true, r#"broker = " ""#),
        driver("gpio", true, ""),
    ];
    assert!(topology_shared("runtime", &drivers).is_empty());
}

#[test]
fn fleet_topology_contract_shared_mqtt_is_grouped_sorted_and_deduplicated() {
    let drivers = [
        driver("mqtt", true, r#"broker = "z.example:1883""#),
        driver("MQTT", false, r#"broker = "a.example:1883""#),
        driver("mqtt", true, r#"broker = "z.example:1883""#),
    ];
    let shared = topology_shared("runtime", &drivers);
    assert_eq!(shared.len(), 2);
    assert_eq!(shared[0].address, "a.example:1883");
    assert_eq!(shared[0].used_by, ["runtime"]);
    assert_eq!(shared[1].address, "z.example:1883");
    assert_eq!(shared[1].used_by, ["runtime"]);
}

#[test]
fn fleet_topology_contract_shared_mqtt_id_uses_sanitized_broker() {
    assert_eq!(
        shared_mqtt_id("Broker.Example:1883"),
        "shared:mqtt:broker-example-1883"
    );
}

#[test]
fn fleet_topology_contract_external_mesh_targets_keep_configuration_order() {
    let mut settings = settings();
    settings.mesh.connect = vec![
        SmolStr::new("tcp/peer-a:7447"),
        SmolStr::new("tcp/peer-b:7447"),
    ];
    let external = topology_external("runtime-a", &settings, &[], None, None, &[]);
    assert_eq!(external.len(), 2);
    assert_eq!(external[0].id, "external:mesh:0");
    assert_eq!(external[0].name, "tcp/peer-a:7447");
    assert_eq!(external[1].id, "external:mesh:1");
    assert_eq!(external[1].name, "tcp/peer-b:7447");
}

#[test]
fn fleet_topology_contract_unresolved_cloud_target_gets_stable_placeholder() {
    let mut settings = settings();
    settings
        .runtime_cloud
        .link_preferences
        .push(RuntimeCloudLinkPreferenceRule {
            source: SmolStr::new("runtime"),
            target: SmolStr::new(""),
            transport: RuntimeCloudPreferredTransport::OpcUa,
        });
    let external = topology_external("runtime-a", &settings, &[], None, None, &[]);
    assert_eq!(external.len(), 1);
    assert_eq!(external[0].id, "external:runtime_cloud:0");
    assert_eq!(external[0].name, "runtime cloud target");
    assert_eq!(external[0].via_protocol, ["opcua"]);
    assert_eq!(external[0].direction, "policy");
}

#[test]
fn fleet_topology_contract_explicit_cloud_target_does_not_fabricate_external_node() {
    let mut settings = settings();
    settings
        .runtime_cloud
        .link_preferences
        .push(RuntimeCloudLinkPreferenceRule {
            source: SmolStr::new("runtime-a"),
            target: SmolStr::new("runtime-b"),
            transport: RuntimeCloudPreferredTransport::Realtime,
        });
    assert!(topology_external("runtime-a", &settings, &[], None, None, &[]).is_empty());
}

#[test]
fn fleet_topology_contract_external_projection_orders_source_classes() {
    let mut settings = settings();
    settings.mesh.connect.push(SmolStr::new("peer"));
    settings
        .runtime_cloud
        .link_preferences
        .push(RuntimeCloudLinkPreferenceRule {
            source: SmolStr::new(""),
            target: SmolStr::new(""),
            transport: RuntimeCloudPreferredTransport::Mesh,
        });
    let discovery = discovery_entry(
        "discovery",
        "observed",
        vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 30))],
        10,
    );
    let drivers = [
        driver("modbus-tcp", true, r#"address = "plc:502""#),
        driver("ethercat", true, r#"adapter = "eth0""#),
    ];
    let external = topology_external("runtime-a", &settings, &drivers, None, None, &[discovery]);
    assert_eq!(
        external
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        [
            "external:mesh:0",
            "external:runtime_cloud:0",
            "external:discovery:discovery",
            "external:modbus:plc:502",
            "external:ethercat:eth0",
        ]
    );
}

#[test]
fn fleet_topology_contract_external_projection_excludes_self_discovery() {
    let settings = settings();
    let self_entry = discovery_entry("service", "runtime-a", Vec::new(), 10);
    assert!(topology_external("runtime-a", &settings, &[], None, None, &[self_entry]).is_empty());
}

#[test]
fn fleet_topology_contract_duplicate_discovery_external_identity_collapses_first() {
    let settings = settings();
    let first = discovery_entry("same", "first", Vec::new(), 10);
    let second = discovery_entry("same", "second", Vec::new(), 20);
    let external = topology_external("runtime-a", &settings, &[], None, None, &[first, second]);
    assert_eq!(external.len(), 1);
    assert_eq!(external[0].name, "first");
}

#[test]
fn fleet_topology_contract_discovered_inventory_excludes_name_or_id_self() {
    let entries = [
        discovery_entry("service-a", "runtime-a", Vec::new(), 10),
        discovery_entry("runtime-a", "alias", Vec::new(), 10),
        discovery_entry("peer", "peer", Vec::new(), 10),
    ];
    let discovered = topology_discovered("runtime-a", &entries);
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].name, "peer");
}

#[test]
fn fleet_topology_contract_discovered_inventory_preserves_observation_fields() {
    let mut entry = discovery_entry(
        "peer-id",
        "peer-a",
        vec![
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ],
        123,
    );
    entry.web_port = Some(8080);
    entry.web_tls = true;
    entry.mesh_port = Some(7447);
    entry.control = Some(SmolStr::new("tcp://192.0.2.10:9900"));
    entry.host_group = Some(SmolStr::new("cell-a"));
    let discovered = topology_discovered("runtime", &[entry]);
    let item = &discovered[0];
    assert_eq!(item.id, "external:discovery:peer-id");
    assert_eq!(item.kind, "runtime");
    assert_eq!(item.name, "peer-a");
    assert_eq!(item.addresses, ["192.0.2.10", "::1"]);
    assert_eq!(item.via_protocol, ["discovery", "web", "mesh", "control"]);
    assert_eq!(item.direction, "observed");
    assert!(!item.adopted);
    assert_eq!(item.control.as_deref(), Some("tcp://192.0.2.10:9900"));
    assert_eq!(item.web_port, Some(8080));
    assert_eq!(item.web_tls, Some(true));
    assert_eq!(item.mesh_port, Some(7447));
    assert_eq!(item.host_group.as_deref(), Some("cell-a"));
    assert_eq!(item.last_seen_ms, Some(123));
    assert_eq!(item.source.as_deref(), Some("discovery"));
}

#[test]
fn fleet_topology_contract_duplicate_discovered_identity_collapses_first() {
    let entries = [
        discovery_entry("same", "first", Vec::new(), 10),
        discovery_entry("same", "second", Vec::new(), 20),
    ];
    let discovered = topology_discovered("runtime", &entries);
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].name, "first");
}

#[test]
fn fleet_topology_contract_discovered_runtime_projects_web_endpoint_without_probe_claim() {
    let mut entry = discovery_entry(
        "peer",
        "peer-a",
        vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))],
        now_ms(),
    );
    entry.web_port = Some(8080);
    let runtime = discovered_runtime(&entry, None);
    assert_eq!(runtime.health, "connected");
    assert_eq!(runtime.web_listen.as_deref(), Some("192.0.2.10:8080"));
    assert_eq!(runtime.endpoints.len(), 1);
    let endpoint = &runtime.endpoints[0];
    assert_eq!(endpoint.protocol, "web");
    assert_eq!(endpoint.health, "connected");
    assert!(!endpoint.owned);
    assert!(!endpoint.supports_test);
    assert!(endpoint.detail.contains("not probed"));
}

#[test]
fn fleet_topology_contract_discovered_runtime_brackets_ipv6_web_authority() {
    let mut entry = discovery_entry(
        "peer",
        "peer-a",
        vec![IpAddr::V6(Ipv6Addr::LOCALHOST)],
        now_ms(),
    );
    entry.web_port = Some(8080);
    let runtime = discovered_runtime(&entry, None);
    assert_eq!(runtime.web_listen.as_deref(), Some("[::1]:8080"));
    assert_eq!(runtime.endpoints[0].address.as_deref(), Some("[::1]:8080"));
}

#[test]
fn fleet_topology_contract_stale_discovered_runtime_and_web_are_unreachable() {
    let mut entry = discovery_entry(
        "peer",
        "peer-a",
        Vec::new(),
        now_ms().saturating_sub(DISCOVERY_STALE_AFTER_MS + 10_000),
    );
    entry.web_port = Some(8080);
    let runtime = discovered_runtime(&entry, None);
    assert_eq!(runtime.health, "runtime_unreachable");
    assert_eq!(runtime.endpoints[0].health, "runtime_unreachable");
}

#[test]
fn fleet_topology_contract_discovered_mesh_without_evidence_is_policy() {
    let mut entry = discovery_entry("peer", "peer-a", Vec::new(), now_ms());
    entry.mesh_port = Some(7447);
    let runtime = discovered_runtime(&entry, None);
    assert_eq!(runtime.endpoints[0].protocol, "mesh");
    assert_eq!(runtime.endpoints[0].health, "configured_policy");
}

#[test]
fn fleet_topology_contract_discovered_mesh_requires_ready_matching_evidence() {
    let mut entry = discovery_entry("peer", "peer-a", Vec::new(), now_ms());
    entry.mesh_port = Some(7447);

    let ready = crate::mesh::MeshTopologyEvidence::for_test(true, &["peer-a"], 10);
    assert_eq!(
        discovered_runtime(&entry, Some(&ready)).endpoints[0].health,
        "connected"
    );

    let not_ready = crate::mesh::MeshTopologyEvidence::for_test(false, &["peer-a"], 10);
    assert_eq!(
        discovered_runtime(&entry, Some(&not_ready)).endpoints[0].health,
        "configured_policy"
    );
}

#[test]
fn fleet_topology_contract_discovered_host_without_group_uses_discovery_identity() {
    let entry = discovery_entry(
        "service-id",
        "peer-a",
        vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))],
        10,
    );
    let hosts = discovered_hosts("runtime", &[entry], None);
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].host_id, "host:discovery:service-id");
    assert_eq!(hosts[0].hostname, "peer-a");
    assert_eq!(hosts[0].ips, ["192.0.2.10"]);
    assert_eq!(hosts[0].runtimes.len(), 1);
    assert_eq!(hosts[0].source.as_deref(), Some("discovery"));
    assert_eq!(hosts[0].last_seen_ms, Some(10));
}

#[test]
fn fleet_topology_contract_shared_host_group_coalesces_runtime_children() {
    let mut first = discovery_entry(
        "first",
        "runtime-a",
        vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 20))],
        10,
    );
    first.host_group = Some(SmolStr::new("cell-a"));
    let mut second = discovery_entry(
        "second",
        "runtime-b",
        vec![
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)),
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 20)),
        ],
        20,
    );
    second.host_group = Some(SmolStr::new("cell-a"));

    let hosts = discovered_hosts("runtime", &[first, second], None);
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].host_id, "host:cell-a");
    assert_eq!(hosts[0].hostname, "cell-a");
    assert_eq!(hosts[0].ips, ["192.0.2.10", "192.0.2.20"]);
    assert_eq!(
        hosts[0]
            .runtimes
            .iter()
            .map(|runtime| runtime.name.as_str())
            .collect::<Vec<_>>(),
        ["runtime-a", "runtime-b"]
    );
    assert_eq!(hosts[0].last_seen_ms, Some(20));
}

#[test]
fn fleet_topology_contract_duplicate_runtime_observation_is_not_duplicated_in_host() {
    let mut first = discovery_entry("first", "runtime-a", Vec::new(), 10);
    first.host_group = Some(SmolStr::new("cell-a"));
    let mut second = discovery_entry("second", "runtime-a", Vec::new(), 20);
    second.host_group = Some(SmolStr::new("cell-a"));
    let hosts = discovered_hosts("runtime", &[first, second], None);
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].runtimes.len(), 1);
    assert_eq!(hosts[0].last_seen_ms, Some(20));
}

#[test]
fn fleet_topology_contract_discovered_hosts_exclude_all_self_aliases() {
    let entries = [
        discovery_entry("service", "runtime", Vec::new(), 10),
        discovery_entry("runtime", "alias", Vec::new(), 10),
    ];
    assert!(discovered_hosts("runtime", &entries, None).is_empty());
}
