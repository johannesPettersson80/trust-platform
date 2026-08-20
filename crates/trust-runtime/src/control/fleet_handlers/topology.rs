use super::*;

pub(super) fn topology_shared(runtime_id: &str, io_drivers: &[IoDriverConfig]) -> Vec<FleetShared> {
    let mut brokers = BTreeMap::<String, BTreeSet<String>>::new();
    for driver in io_drivers {
        if protocol_from_driver_name(driver.name.as_str()) != "mqtt" {
            continue;
        }
        if let Some(broker) = driver_endpoint_address(&driver.params) {
            brokers
                .entry(broker)
                .or_default()
                .insert(runtime_id.to_string());
        }
    }
    brokers
        .into_iter()
        .map(|(broker, runtimes)| FleetShared {
            id: shared_mqtt_id(broker.as_str()),
            kind: "broker".to_string(),
            name: "MQTT broker".to_string(),
            address: broker,
            used_by: runtimes.into_iter().collect(),
        })
        .collect()
}

pub(super) fn topology_external(
    runtime_id: &str,
    settings: &RuntimeSettings,
    io_drivers: &[IoDriverConfig],
    ads_client_config: Option<&AdsClientConfig>,
    opcua_client_config: Option<&OpcUaClientConfig>,
    discovery_entries: &[DiscoveryEntry],
) -> Vec<FleetExternal> {
    let mut ids = BTreeSet::new();
    let mut external = Vec::new();
    for (index, target) in settings.mesh.connect.iter().enumerate() {
        let item = FleetExternal {
            id: format!("external:mesh:{index}"),
            kind: "peer".to_string(),
            name: target.to_string(),
            via_protocol: vec!["mesh".to_string()],
            direction: "outbound".to_string(),
            source: Some("config".to_string()),
        };
        ids.insert(item.id.clone());
        external.push(item);
    }
    for (index, rule) in settings
        .runtime_cloud
        .link_preferences
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.target.is_empty())
    {
        let item = FleetExternal {
            id: format!("external:runtime_cloud:{index}"),
            kind: "peer".to_string(),
            name: "runtime cloud target".to_string(),
            via_protocol: vec![rule.transport.as_str().to_string()],
            direction: "policy".to_string(),
            source: Some("config".to_string()),
        };
        ids.insert(item.id.clone());
        external.push(item);
    }
    for entry in discovery_entries {
        if is_self_discovery_entry(runtime_id, entry) {
            continue;
        }
        let item = FleetExternal {
            id: discovered_external_id(entry),
            kind: "runtime".to_string(),
            name: entry.name.to_string(),
            via_protocol: discovery_protocols(entry),
            direction: "observed".to_string(),
            source: Some("discovery".to_string()),
        };
        if ids.insert(item.id.clone()) {
            external.push(item);
        }
    }
    for item in driver_target_externals(io_drivers) {
        if ids.insert(item.id.clone()) {
            external.push(item);
        }
    }
    for item in ads_client_externals(ads_client_config) {
        if ids.insert(item.id.clone()) {
            external.push(item);
        }
    }
    for item in opcua_client_externals(opcua_client_config) {
        if ids.insert(item.id.clone()) {
            external.push(item);
        }
    }
    external
}

pub(super) fn topology_discovered(
    runtime_id: &str,
    entries: &[DiscoveryEntry],
) -> Vec<FleetDiscovered> {
    let mut seen = BTreeSet::new();
    entries
        .iter()
        .filter(|entry| {
            !is_self_discovery_entry(runtime_id, entry)
                && seen.insert(discovered_external_id(entry))
        })
        .map(|entry| FleetDiscovered {
            id: discovered_external_id(entry),
            kind: "runtime".to_string(),
            name: entry.name.to_string(),
            addresses: entry.addresses.iter().map(ToString::to_string).collect(),
            via_protocol: discovery_protocols(entry),
            direction: "observed".to_string(),
            adopted: false,
            control: entry.control.as_ref().map(ToString::to_string),
            web_port: entry.web_port,
            web_tls: Some(entry.web_tls),
            mesh_port: entry.mesh_port,
            host_group: entry.host_group.as_ref().map(ToString::to_string),
            last_seen_ms: Some(discovery_last_seen_ms(entry)),
            source: Some("discovery".to_string()),
        })
        .collect()
}

pub(super) fn discovered_hosts(
    runtime_id: &str,
    entries: &[DiscoveryEntry],
    mesh_evidence: Option<&crate::mesh::MeshTopologyEvidence>,
) -> Vec<FleetHost> {
    let mut hosts = BTreeMap::<String, FleetHost>::new();
    for entry in entries
        .iter()
        .filter(|entry| !is_self_discovery_entry(runtime_id, entry))
    {
        let host_id = discovered_host_id(entry);
        let runtime = discovered_runtime(entry, mesh_evidence);
        let seen_ms = discovery_last_seen_ms(entry);
        let host = hosts.entry(host_id.clone()).or_insert_with(|| FleetHost {
            host_id,
            hostname: entry
                .host_group
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| entry.name.to_string()),
            board: None,
            arch: "unknown".to_string(),
            os: "unknown".to_string(),
            ips: Vec::new(),
            temp_c: None,
            uptime_s: None,
            load: None,
            containers: Vec::new(),
            runtimes: Vec::new(),
            source: Some("discovery".to_string()),
            last_seen_ms: Some(seen_ms),
        });
        host.ips
            .extend(entry.addresses.iter().map(ToString::to_string));
        host.ips.sort();
        host.ips.dedup();
        host.last_seen_ms = Some(host.last_seen_ms.unwrap_or(0).max(seen_ms));
        if !host
            .runtimes
            .iter()
            .any(|existing| existing.runtime_id == runtime.runtime_id)
        {
            host.runtimes.push(runtime);
        }
    }
    hosts.into_values().collect()
}

pub(super) fn discovered_runtime(
    entry: &DiscoveryEntry,
    mesh_evidence: Option<&crate::mesh::MeshTopologyEvidence>,
) -> FleetRuntime {
    let health = discovery_health(entry);
    let mut endpoints = Vec::new();
    if let Some(port) = entry.web_port {
        endpoints.push(FleetEndpoint {
            id: format!("endpoint:{}:web", discovered_runtime_id(entry)),
            kind: "service".to_string(),
            protocol: "web".to_string(),
            name: "Web / IDE".to_string(),
            address: first_address_with_port(entry, port),
            role: Some("server".to_string()),
            health: health.clone(),
            detail: "Advertised by runtime discovery; listener was not probed by this runtime."
                .to_string(),
            live: Some(json!({ "last_seen_ms": discovery_last_seen_ms(entry) })),
            params: None,
            children: Vec::new(),
            owned: false,
            supports_test: false,
            source: Some("discovery".to_string()),
        });
    }
    if let Some(port) = entry.mesh_port {
        let mesh_health = if mesh_peer_is_live(entry, mesh_evidence) {
            "connected".to_string()
        } else {
            "configured_policy".to_string()
        };
        endpoints.push(FleetEndpoint {
            id: format!("endpoint:{}:mesh", discovered_runtime_id(entry)),
            kind: "peer".to_string(),
            protocol: "mesh".to_string(),
            name: "Mesh / Zenoh".to_string(),
            address: first_address_with_port(entry, port),
            role: Some("peer".to_string()),
            health: mesh_health,
            detail: "Mesh endpoint was advertised by runtime discovery.".to_string(),
            live: Some(json!({ "last_seen_ms": discovery_last_seen_ms(entry) })),
            params: None,
            children: Vec::new(),
            owned: false,
            supports_test: false,
            source: Some("discovery".to_string()),
        });
    }

    FleetRuntime {
        runtime_id: discovered_runtime_id(entry),
        name: entry.name.to_string(),
        control_endpoint: entry.control.as_ref().map(ToString::to_string),
        web_listen: entry
            .web_port
            .and_then(|port| first_address_with_port(entry, port)),
        mode: "unknown".to_string(),
        cycle_ms: 0,
        load: None,
        health,
        detail: "Runtime peer observed through local discovery.".to_string(),
        endpoints,
        source: Some("discovery".to_string()),
        last_seen_ms: Some(discovery_last_seen_ms(entry)),
    }
}

pub(super) fn local_host(hostname: &str, settings: &RuntimeSettings) -> FleetHost {
    FleetHost {
        host_id: stable_host_id(hostname),
        hostname: hostname.to_string(),
        board: host_board(),
        arch: std::env::consts::ARCH.to_string(),
        os: std::env::consts::OS.to_string(),
        ips: configured_host_ips(settings),
        temp_c: host_temp_c(),
        uptime_s: host_uptime_s(),
        load: host_load(),
        containers: Vec::new(),
        runtimes: Vec::new(),
        source: Some("self".to_string()),
        last_seen_ms: Some(now_ms()),
    }
}

pub(super) fn current_container(runtime: FleetRuntime) -> FleetContainer {
    let cgroup = fs::read_to_string("/proc/self/cgroup").unwrap_or_default();
    let container_id = parse_container_id(cgroup.as_str()).unwrap_or_else(host_name);
    let name = std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| container_id.clone());
    FleetContainer {
        container_id,
        name,
        image: std::env::var("TRUST_CONTAINER_IMAGE").unwrap_or_else(|_| "unknown".to_string()),
        status: "running".to_string(),
        runtimes: vec![runtime],
        restart_policy: None,
        cpu_limit: None,
        mem_limit: None,
        mounts: None,
        source: Some("self".to_string()),
    }
}

pub(super) fn current_process_is_containerized() -> bool {
    Path::new("/.dockerenv").exists()
        || fs::read_to_string("/proc/self/cgroup")
            .map(|cgroup| looks_containerized(cgroup.as_str()))
            .unwrap_or(false)
}
