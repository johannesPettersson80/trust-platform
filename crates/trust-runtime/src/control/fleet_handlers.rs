use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;

use crate::ads::diagnostics::{AdsConnectionStatus, AdsConnectionStatusState, AdsStatusReport};
use crate::ads::{AdsClientConfig, AdsConnectionConfig};
use crate::config::{IoConfig, IoDriverConfig};
use crate::discovery::DiscoveryEntry;
use crate::io::{IoAddress, IoDriverHealth, IoDriverStatus, IoSnapshot, IoSnapshotEntry};
use crate::linux_rt::LinuxRtRuntimeStatus;
use crate::memory::IoArea;
use crate::scheduler::ResourceCommand;
use crate::settings::RuntimeSettings;

use super::{ControlResponse, ControlState};

mod offline;

const FLEET_TOPOLOGY_SCHEMA_VERSION: u32 = 4;
const DISCOVERY_STALE_AFTER_MS: u64 = 120_000;
const ADS_STATUS_TIMEOUT: Duration = Duration::from_millis(250);

pub(super) fn handle_fleet_topology(id: u64, state: &ControlState) -> ControlResponse {
    let response = match build_fleet_topology(state) {
        Ok(response) => response,
        Err(error) => return ControlResponse::error(id, error),
    };
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => {
            ControlResponse::error(id, format!("fleet topology serialization failed: {error}"))
        }
    }
}

pub(super) fn build_project_fleet_topology_value(
    project_root: &Path,
) -> Result<serde_json::Value, String> {
    let response = offline::build_project_fleet_topology(project_root)?;
    serde_json::to_value(response)
        .map_err(|error| format!("fleet topology serialization failed: {error}"))
}

fn build_fleet_topology(state: &ControlState) -> Result<FleetTopologyResponse, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings unavailable".to_string())?
        .clone();
    let io_health = state
        .io_health
        .lock()
        .map_err(|_| "I/O health unavailable".to_string())?
        .clone();
    let io_snapshot = state
        .io_snapshot
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let io_snapshot_seen_ms = state.io_snapshot_seen_ms.load(Ordering::Relaxed);
    let realtime = state.realtime_status.lock().ok().map(|guard| guard.clone());
    let mesh_evidence = state
        .mesh_topology
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let discovery_entries = state.discovery.snapshot();
    let io_drivers = configured_io_drivers(state);
    let ads_client_config = state
        .ads_client_config
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let ads_status = ads_status_report(state);

    let runtime_id = state.resource_name.to_string();
    let hostname = host_name();
    let runtime_inputs = FleetRuntimeInputs {
        state,
        settings: &settings,
        io_health: &io_health,
        io_drivers: &io_drivers,
        io_snapshot: io_snapshot.as_ref(),
        io_snapshot_seen_ms,
        realtime: realtime.as_ref(),
        mesh_evidence: mesh_evidence.as_ref(),
        discovery_entries: &discovery_entries,
        ads_client_config: ads_client_config.as_ref(),
        ads_status: ads_status.as_ref(),
    };
    let runtime = runtime_node(&runtime_id, &runtime_inputs);
    let mut local_host = local_host(&hostname, &settings);
    if current_process_is_containerized() {
        local_host.containers.push(current_container(runtime));
    } else {
        local_host.runtimes.push(runtime);
    }

    let mut hosts = vec![local_host];
    hosts.extend(discovered_hosts(
        &runtime_id,
        discovery_entries.as_slice(),
        mesh_evidence.as_ref(),
    ));

    let mut links = topology_links(&runtime_id, &runtime_inputs);
    links.sort_by(|left, right| left.id.cmp(&right.id));
    links.dedup_by(|left, right| left.id == right.id);

    let shared = topology_shared(&runtime_id, io_drivers.as_slice());
    let external = topology_external(
        &settings,
        io_drivers.as_slice(),
        ads_client_config.as_ref(),
        discovery_entries.as_slice(),
    );
    let discovered = topology_discovered(&runtime_id, discovery_entries.as_slice());

    Ok(FleetTopologyResponse {
        schema_version: FLEET_TOPOLOGY_SCHEMA_VERSION,
        hosts,
        links,
        shared,
        external,
        discovered,
    })
}

struct FleetRuntimeInputs<'a> {
    state: &'a ControlState,
    settings: &'a RuntimeSettings,
    io_health: &'a [IoDriverStatus],
    io_drivers: &'a [IoDriverConfig],
    io_snapshot: Option<&'a IoSnapshot>,
    io_snapshot_seen_ms: u64,
    realtime: Option<&'a LinuxRtRuntimeStatus>,
    mesh_evidence: Option<&'a crate::mesh::MeshTopologyEvidence>,
    discovery_entries: &'a [DiscoveryEntry],
    ads_client_config: Option<&'a AdsClientConfig>,
    ads_status: Option<&'a AdsStatusReport>,
}

fn runtime_node(runtime_id: &str, inputs: &FleetRuntimeInputs<'_>) -> FleetRuntime {
    let mut endpoints = io_endpoints(
        runtime_id,
        inputs.io_health,
        inputs.io_drivers,
        inputs.io_snapshot,
        inputs.io_snapshot_seen_ms,
    );
    endpoints.extend(service_endpoints(runtime_id, inputs));

    let metrics = inputs
        .state
        .metrics
        .lock()
        .ok()
        .map(|guard| guard.snapshot())
        .unwrap_or_default();
    let load = if inputs.settings.cycle_interval.as_millis() > 0
        && metrics.cycle.last_ms.is_finite()
    {
        Some((metrics.cycle.last_ms / inputs.settings.cycle_interval.as_millis() as f64).min(1.0))
    } else {
        None
    };
    let health = runtime_health(inputs.state, inputs.settings, endpoints.as_slice());

    FleetRuntime {
        runtime_id: runtime_id.to_string(),
        name: inputs.state.resource_name.to_string(),
        control_endpoint: None,
        web_listen: inputs
            .settings
            .web
            .enabled
            .then(|| inputs.settings.web.listen.to_string()),
        mode: if inputs.settings.simulation.enabled {
            "simulate".to_string()
        } else {
            "online".to_string()
        },
        cycle_ms: inputs.settings.cycle_interval.as_millis() as u64,
        load,
        health,
        detail: runtime_detail(inputs.state),
        endpoints,
        source: Some("self".to_string()),
        last_seen_ms: Some(now_ms()),
    }
}

fn runtime_health(
    state: &ControlState,
    settings: &RuntimeSettings,
    endpoints: &[FleetEndpoint],
) -> String {
    if state.resource.last_error().is_some() {
        return "error".to_string();
    }
    if endpoints.iter().any(|endpoint| endpoint.health == "error") {
        return "error".to_string();
    }
    if endpoints
        .iter()
        .any(|endpoint| endpoint.health == "degraded" || endpoint.health == "runtime_unreachable")
    {
        return "degraded".to_string();
    }
    if settings.simulation.enabled {
        return "simulate".to_string();
    }
    "connected".to_string()
}

fn runtime_detail(state: &ControlState) -> String {
    if let Some(error) = state.resource.last_error() {
        return format!("Runtime reported a fault: {error}");
    }
    "Runtime answered fleet.topology from its control channel.".to_string()
}

fn io_endpoints(
    runtime_id: &str,
    io_health: &[IoDriverStatus],
    io_drivers: &[IoDriverConfig],
    io_snapshot: Option<&IoSnapshot>,
    io_snapshot_seen_ms: u64,
) -> Vec<FleetEndpoint> {
    if !io_drivers.is_empty() {
        return io_drivers
            .iter()
            .enumerate()
            .map(|(index, driver)| {
                let health = driver_health_for_config(io_health, index, driver.name.as_str());
                endpoint_from_driver_config(
                    runtime_id,
                    index,
                    driver,
                    health,
                    io_snapshot,
                    io_snapshot_seen_ms,
                )
            })
            .collect();
    }

    io_health
        .iter()
        .enumerate()
        .map(|(index, driver)| {
            endpoint_from_driver_health(runtime_id, index, driver, io_snapshot, io_snapshot_seen_ms)
        })
        .collect()
}

fn endpoint_from_driver_config(
    runtime_id: &str,
    index: usize,
    driver: &IoDriverConfig,
    health: Option<&IoDriverStatus>,
    io_snapshot: Option<&IoSnapshot>,
    io_snapshot_seen_ms: u64,
) -> FleetEndpoint {
    let protocol = protocol_from_driver_name(driver.name.as_str());
    let (health_value, detail) = health.map(driver_health).unwrap_or_else(|| {
        (
            "configured_policy".to_string(),
            "Configured in io.toml; no live driver health has been reported yet.".to_string(),
        )
    });
    FleetEndpoint {
        id: endpoint_instance_id(runtime_id, protocol.as_str(), index),
        kind: "field".to_string(),
        protocol: protocol.clone(),
        name: driver_display_name(protocol.as_str(), index, &driver.params),
        address: driver_endpoint_address(&driver.params),
        role: Some(driver_role(protocol.as_str()).to_string()),
        health: health_value,
        detail,
        live: io_snapshot_live(io_snapshot, io_snapshot_seen_ms),
        params: Some(redacted_toml_params(&driver.params)),
        children: ethercat_endpoint_children(protocol.as_str(), &driver.params),
        owned: true,
        supports_test: matches!(driver.name.as_str(), "modbus-tcp" | "mqtt"),
        source: Some("self".to_string()),
    }
}

fn endpoint_from_driver_health(
    runtime_id: &str,
    index: usize,
    driver: &IoDriverStatus,
    io_snapshot: Option<&IoSnapshot>,
    io_snapshot_seen_ms: u64,
) -> FleetEndpoint {
    let protocol = protocol_from_driver_name(driver.name.as_str());
    let role = driver_role(protocol.as_str()).to_string();
    let (health, detail) = driver_health(driver);
    FleetEndpoint {
        id: endpoint_instance_id(runtime_id, protocol.as_str(), index),
        kind: "field".to_string(),
        protocol,
        name: driver.name.to_string(),
        address: None,
        role: Some(role),
        health,
        detail,
        live: io_snapshot_live(io_snapshot, io_snapshot_seen_ms),
        params: None,
        children: Vec::new(),
        owned: true,
        supports_test: matches!(driver.name.as_str(), "modbus-tcp" | "mqtt"),
        source: Some("self".to_string()),
    }
}

fn driver_health(driver: &IoDriverStatus) -> (String, String) {
    match &driver.health {
        IoDriverHealth::Ok => ("connected".to_string(), "Driver is healthy.".to_string()),
        IoDriverHealth::Degraded { error } => (
            "degraded".to_string(),
            format!("Driver is degraded: {error}"),
        ),
        IoDriverHealth::Faulted { error } => {
            ("error".to_string(), format!("Driver faulted: {error}"))
        }
    }
}

fn service_endpoints(runtime_id: &str, inputs: &FleetRuntimeInputs<'_>) -> Vec<FleetEndpoint> {
    let mut endpoints = Vec::new();
    let state = inputs.state;
    let settings = inputs.settings;
    if settings.web.enabled {
        let bound = state.web_listener_bound.load(Ordering::Relaxed);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "web"),
            kind: "service".to_string(),
            protocol: "web".to_string(),
            name: "Web / IDE".to_string(),
            address: Some(settings.web.listen.to_string()),
            role: Some("server".to_string()),
            health: if bound {
                "connected"
            } else {
                "configured_policy"
            }
            .to_string(),
            detail: if bound {
                "Web listener successfully bound during runtime startup.".to_string()
            } else {
                "Web is configured, but this control state has no listener-bound evidence."
                    .to_string()
            },
            live: bound.then(|| json!({ "last_seen_ms": now_ms() })),
            params: Some(json!({
                "enabled": settings.web.enabled,
                "listen": settings.web.listen.to_string(),
                "auth": settings.web.auth.to_string(),
                "tls": settings.web.tls,
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.opcua.enabled || !settings.opcua.expose.is_empty() {
        let bound = state.opcua_server_bound.load(Ordering::Relaxed);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "opcua"),
            kind: "service".to_string(),
            protocol: "opcua".to_string(),
            name: "OPC UA server".to_string(),
            address: Some(settings.opcua.listen.to_string()),
            role: Some("server".to_string()),
            health: if bound {
                "connected"
            } else if settings.opcua.enabled {
                "configured_policy"
            } else {
                "not_configured"
            }
            .to_string(),
            detail: if bound {
                "OPC UA server startup probe succeeded.".to_string()
            } else if settings.opcua.enabled {
                "OPC UA is configured, but no bound server evidence is available.".to_string()
            } else {
                "OPC UA exposure exists but the server is disabled.".to_string()
            },
            live: bound.then(|| json!({ "last_seen_ms": now_ms() })),
            params: Some(json!({
                "enabled": settings.opcua.enabled,
                "listen": settings.opcua.listen.to_string(),
                "endpoint_path": settings.opcua.endpoint_path.to_string(),
                "namespace_uri": settings.opcua.namespace_uri.to_string(),
                "publish_interval_ms": settings.opcua.publish_interval_ms,
                "max_nodes": settings.opcua.max_nodes,
                "expose": settings.opcua.expose.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "security_policy": settings.opcua.security_policy.to_string(),
                "security_mode": settings.opcua.security_mode.to_string(),
                "allow_anonymous": settings.opcua.allow_anonymous,
                "username_set": settings.opcua.username_set,
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.discovery.enabled {
        let peer_count = inputs
            .discovery_entries
            .iter()
            .filter(|entry| entry.name.as_str() != runtime_id)
            .count();
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "discovery"),
            kind: "peer".to_string(),
            protocol: "discovery".to_string(),
            name: "Runtime discovery".to_string(),
            address: Some(settings.discovery.service_name.to_string()),
            role: Some(if settings.discovery.advertise {
                "advertise_browse".to_string()
            } else {
                "browse".to_string()
            }),
            health: if peer_count > 0 {
                "connected"
            } else {
                "configured_policy"
            }
            .to_string(),
            detail: if peer_count > 0 {
                format!("Discovery has observed {peer_count} runtime peer(s).")
            } else {
                "Discovery is configured; no peers have been observed yet.".to_string()
            },
            live: Some(json!({
                "value": { "observed_peers": peer_count },
                "last_seen_ms": newest_discovery_seen_ms(inputs.discovery_entries),
            })),
            params: Some(json!({
                "enabled": settings.discovery.enabled,
                "service_name": settings.discovery.service_name.to_string(),
                "advertise": settings.discovery.advertise,
                "interfaces": settings.discovery.interfaces.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "host_group": settings.discovery.host_group.as_ref().map(ToString::to_string),
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.mesh.enabled || !settings.mesh.connect.is_empty() {
        let mesh_snapshot = inputs
            .mesh_evidence
            .map(|evidence| evidence.liveliness_snapshot());
        let peer_count = mesh_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.peers.len());
        let health = match inputs.mesh_evidence {
            Some(evidence) if evidence.is_ready() => "connected",
            Some(_) => "degraded",
            None => "configured_policy",
        };
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "mesh"),
            kind: "peer".to_string(),
            protocol: "mesh".to_string(),
            name: "Mesh / Zenoh".to_string(),
            address: Some(settings.mesh.listen.to_string()),
            role: Some(settings.mesh.role.as_str().to_string()),
            health: health.to_string(),
            detail: mesh_detail(inputs.mesh_evidence, peer_count),
            live: mesh_snapshot.map(|snapshot| {
                json!({
                    "value": { "peers": snapshot.peers },
                    "last_seen_ms": snapshot.history.iter().map(|event| event.timestamp_ns / 1_000_000).max(),
                })
            }),
            params: Some(json!({
                "enabled": settings.mesh.enabled,
                "role": settings.mesh.role.as_str(),
                "listen": settings.mesh.listen.to_string(),
                "connect": settings.mesh.connect.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "tls": settings.mesh.tls,
                "publish": settings.mesh.publish.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "subscribe": settings.mesh.subscribe.iter().map(|(key, value)| {
                    json!({ "topic": key.to_string(), "target": value.to_string() })
                }).collect::<Vec<_>>(),
                "zenohd_version": settings.mesh.zenohd_version.to_string(),
                "plugin_versions": settings.mesh.plugin_versions.iter().map(|(key, value)| {
                    json!({ "plugin": key.to_string(), "version": value.to_string() })
                }).collect::<Vec<_>>(),
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if runtime_cloud_configured(settings) {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "runtime_cloud"),
            kind: "peer".to_string(),
            protocol: "runtime_cloud".to_string(),
            name: "Runtime cloud / federation".to_string(),
            address: None,
            role: Some("policy".to_string()),
            health: "configured_policy".to_string(),
            detail: "Federation policy is configured; it is not a live transport by itself."
                .to_string(),
            live: None,
            params: Some(json!({
                "profile": settings.runtime_cloud.profile.as_str(),
                "wan_allow_write": settings.runtime_cloud.wan_allow_write.iter().map(|rule| {
                    json!({
                        "action": rule.action.to_string(),
                        "target": rule.target.to_string(),
                    })
                }).collect::<Vec<_>>(),
                "link_preferences": settings.runtime_cloud.link_preferences.iter().map(|rule| {
                    json!({
                        "source": rule.source.to_string(),
                        "target": rule.target.to_string(),
                        "transport": rule.transport.as_str(),
                    })
                }).collect::<Vec<_>>(),
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.realtime.enabled {
        let (health, detail) = inputs
            .realtime
            .map(realtime_health_and_detail)
            .unwrap_or_else(|| {
                (
                    "configured_policy".to_string(),
                    "Realtime posture is configured, but no runtime posture evidence is available."
                        .to_string(),
                )
            });
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "realtime_t0"),
            kind: "peer".to_string(),
            protocol: "realtime_t0".to_string(),
            name: "Realtime T0".to_string(),
            address: None,
            role: Some("same_host".to_string()),
            health,
            detail,
            live: inputs.realtime.map(|status| {
                json!({
                    "value": {
                        "active": status.active,
                        "scheduler": status.active_scheduler.map(|scheduler| scheduler.as_str()),
                        "priority": status.active_priority,
                    },
                    "last_seen_ms": now_ms(),
                })
            }),
            params: Some(json!({
                "enabled": settings.realtime.enabled,
                "require_preempt_rt_kernel": settings.realtime.require_preempt_rt_kernel,
                "lock_memory": settings.realtime.lock_memory,
                "scheduler": settings.realtime.scheduler.as_str(),
                "priority": settings.realtime.priority,
                "cpu_affinity": settings.realtime.cpu_affinity.clone(),
                "strict": settings.realtime.strict,
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if let Some(config) = inputs
        .ads_client_config
        .filter(|config| !config.connections.is_empty())
    {
        let (health, detail) = ads_client_endpoint_health_and_detail(config, inputs.ads_status);
        let live = ads_client_live(config, inputs.ads_status);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "ads"),
            kind: "peer".to_string(),
            protocol: "ads".to_string(),
            name: "ADS client".to_string(),
            address: ads_client_local_net_id(config),
            role: Some("client".to_string()),
            health,
            detail,
            live,
            params: Some(ads_client_params(config)),
            children: Vec::new(),
            owned: true,
            supports_test: true,
            source: Some("self".to_string()),
        });
    }
    if let Some(config) = state
        .ads_server_config
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .filter(|config| config.enabled || !config.expose.is_empty())
    {
        let connected_clients = ads_server_connected_clients(state);
        let active = connected_clients.is_some();
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "ads_server"),
            kind: "service".to_string(),
            protocol: "ads_server".to_string(),
            name: "ADS server".to_string(),
            address: config.listen.as_ref().map(ToString::to_string),
            role: Some("server".to_string()),
            health: if active {
                "connected"
            } else if config.enabled {
                "configured_policy"
            } else {
                "not_configured"
            }
            .to_string(),
            detail: if active {
                "ADS server runtime is active and listening.".to_string()
            } else if config.enabled {
                "ADS server is configured, but no active server runtime handle is available."
                    .to_string()
            } else {
                "ADS server exposure exists but the server is disabled.".to_string()
            },
            live: active.then(|| {
                json!({
                    "value": { "connected_clients": connected_clients.unwrap_or(0) },
                    "last_seen_ms": now_ms(),
                })
            }),
            params: Some(ads_server_params(&config)),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    endpoints
}

fn topology_links(runtime_id: &str, inputs: &FleetRuntimeInputs<'_>) -> Vec<FleetLink> {
    let settings = inputs.settings;
    let mesh_links = settings
        .mesh
        .connect
        .iter()
        .enumerate()
        .map(|(index, target)| {
            fleet_link(
                endpoint_id(runtime_id, "mesh"),
                format!("external:mesh:{index}"),
                "mesh",
                "peer",
                "outbound",
                false,
                configured_mesh_link_status(target.as_str(), inputs.mesh_evidence),
                settings.mesh.tls,
                Some(target.to_string()),
            )
        });
    let cloud_links = settings
        .runtime_cloud
        .link_preferences
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let from = if rule.source.is_empty() {
                runtime_id.to_string()
            } else {
                rule.source.to_string()
            };
            let to = if rule.target.is_empty() {
                format!("external:runtime_cloud:{index}")
            } else {
                rule.target.to_string()
            };
            fleet_link(
                from,
                to,
                rule.transport.as_str(),
                "peer",
                "policy",
                matches!(
                    rule.transport,
                    crate::config::RuntimeCloudPreferredTransport::Realtime
                ),
                "configured_policy".to_string(),
                !matches!(
                    rule.transport,
                    crate::config::RuntimeCloudPreferredTransport::ModbusTcp
                        | crate::config::RuntimeCloudPreferredTransport::Mqtt
                ),
                Some("runtime.cloud link preference".to_string()),
            )
        });
    let shared_links = inputs
        .io_drivers
        .iter()
        .enumerate()
        .filter_map(|(index, driver)| {
            let protocol = protocol_from_driver_name(driver.name.as_str());
            if protocol != "mqtt" {
                return None;
            }
            let broker = driver_endpoint_address(&driver.params)?;
            Some(fleet_link(
                endpoint_instance_id(runtime_id, protocol.as_str(), index),
                shared_mqtt_id(broker.as_str()),
                protocol,
                "publish_subscribe",
                "publish_subscribe",
                false,
                "configured_policy".to_string(),
                bool_param(&driver.params, "tls"),
                Some("MQTT broker referenced by io.toml".to_string()),
            ))
        });
    let discovery_links = inputs.discovery_entries.iter().filter_map(|entry| {
        if is_self_discovery_entry(runtime_id, entry) {
            return None;
        }
        Some(fleet_link(
            endpoint_id(runtime_id, "discovery"),
            discovered_runtime_id(entry),
            "discovery",
            "peer",
            "observed",
            same_host_by_discovery(runtime_id, entry, inputs.discovery_entries),
            discovery_health(entry),
            false,
            Some(format!(
                "mDNS last seen {} ms",
                discovery_last_seen_ms(entry)
            )),
        ))
    });
    mesh_links
        .chain(cloud_links)
        .chain(shared_links)
        .chain(discovery_links)
        .chain(driver_target_links(
            runtime_id,
            inputs.io_drivers,
            inputs.io_health,
        ))
        .chain(ads_client_links(
            runtime_id,
            inputs.ads_client_config,
            inputs.ads_status,
        ))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn fleet_link(
    from: String,
    to: String,
    protocol: impl Into<String>,
    role: impl Into<String>,
    direction: impl Into<String>,
    same_host: bool,
    status: String,
    secure: bool,
    detail: Option<String>,
) -> FleetLink {
    let protocol = protocol.into();
    let role = role.into();
    FleetLink {
        id: link_id(from.as_str(), to.as_str(), protocol.as_str(), role.as_str()),
        from,
        to,
        protocol,
        role,
        direction: direction.into(),
        same_host,
        status,
        secure,
        detail,
    }
}

fn link_id(from: &str, to: &str, protocol: &str, role: &str) -> String {
    format!(
        "link:{}:{}:{}:{}",
        sanitize_id(protocol),
        sanitize_id(role),
        sanitize_id(from),
        sanitize_id(to)
    )
}

fn driver_target_links(
    runtime_id: &str,
    io_drivers: &[IoDriverConfig],
    io_health: &[IoDriverStatus],
) -> Vec<FleetLink> {
    io_drivers
        .iter()
        .enumerate()
        .filter_map(|(index, driver)| {
            let protocol = protocol_from_driver_name(driver.name.as_str());
            let from = endpoint_instance_id(runtime_id, protocol.as_str(), index);
            let status = driver_link_status(io_health, index, driver);
            match protocol.as_str() {
                "modbus_tcp" => {
                    let address = driver_endpoint_address(&driver.params)?;
                    Some(fleet_link(
                        from,
                        modbus_external_id(address.as_str()),
                        protocol,
                        "client",
                        "outbound",
                        false,
                        status,
                        bool_param(&driver.params, "tls"),
                        Some(address),
                    ))
                }
                "ethercat" => {
                    let adapter = driver_adapter(&driver.params)?;
                    Some(fleet_link(
                        from,
                        ethercat_external_id(adapter.as_str()),
                        protocol,
                        "master",
                        "outbound",
                        false,
                        status,
                        false,
                        None,
                    ))
                }
                _ => None,
            }
        })
        .collect()
}

fn driver_target_externals(io_drivers: &[IoDriverConfig]) -> Vec<FleetExternal> {
    io_drivers
        .iter()
        .enumerate()
        .filter_map(|(index, driver)| {
            let protocol = protocol_from_driver_name(driver.name.as_str());
            match protocol.as_str() {
                "modbus_tcp" => {
                    let address = driver_endpoint_address(&driver.params)?;
                    Some(FleetExternal {
                        id: modbus_external_id(address.as_str()),
                        kind: "device".to_string(),
                        name: driver_display_name(protocol.as_str(), index, &driver.params),
                        via_protocol: vec![protocol],
                        direction: "outbound".to_string(),
                        source: Some("config".to_string()),
                    })
                }
                "ethercat" => {
                    let adapter = driver_adapter(&driver.params)?;
                    Some(FleetExternal {
                        id: ethercat_external_id(adapter.as_str()),
                        kind: "fieldbus".to_string(),
                        name: format!("EtherCAT segment ({adapter})"),
                        via_protocol: vec![protocol],
                        direction: "outbound".to_string(),
                        source: Some("config".to_string()),
                    })
                }
                _ => None,
            }
        })
        .collect()
}

fn driver_link_status(
    io_health: &[IoDriverStatus],
    index: usize,
    driver: &IoDriverConfig,
) -> String {
    driver_health_for_config(io_health, index, driver.name.as_str())
        .map(driver_health)
        .map(|(health, _)| health)
        .unwrap_or_else(|| "configured_policy".to_string())
}

fn modbus_external_id(address: &str) -> String {
    format!("external:modbus:{address}")
}

fn ethercat_external_id(adapter: &str) -> String {
    format!("external:ethercat:{adapter}")
}

fn driver_adapter(params: &toml::Value) -> Option<String> {
    params
        .get("adapter")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn ads_client_externals(config: Option<&AdsClientConfig>) -> Vec<FleetExternal> {
    let Some(config) = config else {
        return Vec::new();
    };
    config
        .connections
        .iter()
        .map(|connection| FleetExternal {
            id: ads_external_id(connection),
            kind: "plc".to_string(),
            name: format!("TwinCAT {}", connection.route.target_net_id.0),
            via_protocol: vec!["ads".to_string()],
            direction: "outbound".to_string(),
            source: Some("config".to_string()),
        })
        .collect()
}

fn ads_client_links(
    runtime_id: &str,
    config: Option<&AdsClientConfig>,
    status: Option<&AdsStatusReport>,
) -> Vec<FleetLink> {
    let Some(config) = config else {
        return Vec::new();
    };
    config
        .connections
        .iter()
        .map(|connection| {
            let connection_status = ads_status_for_connection(status, connection);
            fleet_link(
                endpoint_id(runtime_id, "ads"),
                ads_external_id(connection),
                "ads",
                "client",
                "outbound",
                false,
                connection_status
                    .map(ads_connection_status_health)
                    .unwrap_or_else(|| "configured_policy".to_string()),
                ads_route_secure(connection),
                Some(format!(
                    "{}:{} ({})",
                    connection.route.host,
                    connection.route.ams_port,
                    connection.route.target_net_id.0
                )),
            )
        })
        .collect()
}

fn ads_external_id(connection: &AdsConnectionConfig) -> String {
    format!("external:ads:{}", connection.route.target_net_id.0)
}

fn topology_shared(runtime_id: &str, io_drivers: &[IoDriverConfig]) -> Vec<FleetShared> {
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

fn topology_external(
    settings: &RuntimeSettings,
    io_drivers: &[IoDriverConfig],
    ads_client_config: Option<&AdsClientConfig>,
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
    external
}

fn topology_discovered(runtime_id: &str, entries: &[DiscoveryEntry]) -> Vec<FleetDiscovered> {
    entries
        .iter()
        .filter(|entry| !is_self_discovery_entry(runtime_id, entry))
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

fn discovered_hosts(
    runtime_id: &str,
    entries: &[DiscoveryEntry],
    mesh_evidence: Option<&crate::mesh::MeshTopologyEvidence>,
) -> Vec<FleetHost> {
    entries
        .iter()
        .filter(|entry| !is_self_discovery_entry(runtime_id, entry))
        .map(|entry| {
            let runtime = discovered_runtime(entry, mesh_evidence);
            FleetHost {
                host_id: discovered_host_id(entry),
                hostname: entry
                    .host_group
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| entry.name.to_string()),
                board: None,
                arch: "unknown".to_string(),
                os: "unknown".to_string(),
                ips: entry.addresses.iter().map(ToString::to_string).collect(),
                temp_c: None,
                uptime_s: None,
                load: None,
                containers: Vec::new(),
                runtimes: vec![runtime],
                source: Some("discovery".to_string()),
                last_seen_ms: Some(discovery_last_seen_ms(entry)),
            }
        })
        .collect()
}

fn discovered_runtime(
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

fn local_host(hostname: &str, settings: &RuntimeSettings) -> FleetHost {
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

fn current_container(runtime: FleetRuntime) -> FleetContainer {
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

fn current_process_is_containerized() -> bool {
    Path::new("/.dockerenv").exists()
        || fs::read_to_string("/proc/self/cgroup")
            .map(|cgroup| looks_containerized(cgroup.as_str()))
            .unwrap_or(false)
}

fn configured_io_drivers(state: &ControlState) -> Vec<IoDriverConfig> {
    let Some(root) = state.project_root.as_ref() else {
        return Vec::new();
    };
    IoConfig::load(root.join("io.toml"))
        .map(|config| config.drivers)
        .unwrap_or_default()
}

fn driver_health_for_config<'a>(
    io_health: &'a [IoDriverStatus],
    index: usize,
    driver: &str,
) -> Option<&'a IoDriverStatus> {
    if io_health
        .get(index)
        .is_some_and(|status| same_driver_name(status.name.as_str(), driver))
    {
        return io_health.get(index);
    }
    let wanted_protocol = protocol_from_driver_name(driver);
    io_health
        .iter()
        .filter(|status| protocol_from_driver_name(status.name.as_str()) == wanted_protocol)
        .nth(index)
}

fn same_driver_name(left: &str, right: &str) -> bool {
    protocol_from_driver_name(left) == protocol_from_driver_name(right)
}

fn io_snapshot_live(snapshot: Option<&IoSnapshot>, seen_ms: u64) -> Option<serde_json::Value> {
    let snapshot = snapshot?;
    if seen_ms == 0 {
        return None;
    }
    Some(json!({
        "value": {
            "inputs": snapshot.inputs.len(),
            "outputs": snapshot.outputs.len(),
            "memory": snapshot.memory.len(),
            "sample": io_snapshot_sample(snapshot),
        },
        "last_seen_ms": seen_ms,
    }))
}

fn io_snapshot_sample(snapshot: &IoSnapshot) -> Vec<serde_json::Value> {
    snapshot
        .inputs
        .iter()
        .map(|entry| ("input", entry))
        .chain(snapshot.outputs.iter().map(|entry| ("output", entry)))
        .chain(snapshot.memory.iter().map(|entry| ("memory", entry)))
        .take(8)
        .map(|(direction, entry)| {
            json!({
                "direction": direction,
                "name": entry.name.as_ref().map(|name| name.as_str()),
                "address": format_io_address(&entry.address),
                "value": format_io_value(entry),
            })
        })
        .collect()
}

fn format_io_value(entry: &IoSnapshotEntry) -> serde_json::Value {
    match &entry.value {
        crate::io::IoSnapshotValue::Value(value) => json!(format!("{value:?}")),
        crate::io::IoSnapshotValue::Error(error) => json!({ "error": error }),
        crate::io::IoSnapshotValue::Unresolved => json!("unresolved"),
    }
}

fn format_io_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        crate::io::IoSize::Bit => "X",
        crate::io::IoSize::Byte => "B",
        crate::io::IoSize::Word => "W",
        crate::io::IoSize::DWord => "D",
        crate::io::IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if address.size == crate::io::IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

fn driver_endpoint_address(params: &toml::Value) -> Option<String> {
    params
        .get("address")
        .or_else(|| params.get("broker"))
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn driver_display_name(protocol: &str, index: usize, params: &toml::Value) -> String {
    driver_endpoint_address(params)
        .map(|address| format!("{protocol} {address}"))
        .unwrap_or_else(|| format!("{protocol} #{index}"))
}

fn bool_param(params: &toml::Value, key: &str) -> bool {
    params
        .get(key)
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
}

fn redacted_toml_params(params: &toml::Value) -> serde_json::Value {
    toml_to_json_redacted(params, "")
}

fn ethercat_endpoint_children(protocol: &str, params: &toml::Value) -> Vec<FleetEndpointChild> {
    if protocol != "ethercat" {
        return Vec::new();
    }
    crate::io::configured_ethercat_modules(params)
        .map(|modules| {
            modules
                .into_iter()
                .map(|module| FleetEndpointChild {
                    id: format!("ethercat:slot:{}", module.slot),
                    kind: "field_slave".to_string(),
                    name: format!("{} (slot {})", module.model, module.slot),
                    slot: Some(module.slot),
                    model: Some(module.model),
                    channels: Some(module.channels),
                    health: "configured_policy".to_string(),
                    detail: "Configured in io.toml; live EtherCAT discovery may enrich this row."
                        .to_string(),
                    source: Some("config".to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn toml_to_json_redacted(value: &toml::Value, key: &str) -> serde_json::Value {
    if is_secret_param_key(key) {
        return serde_json::Value::String("<redacted>".to_string());
    }
    match value {
        toml::Value::String(value) => serde_json::Value::String(value.clone()),
        toml::Value::Integer(value) => json!(value),
        toml::Value::Float(value) => json!(value),
        toml::Value::Boolean(value) => json!(value),
        toml::Value::Datetime(value) => serde_json::Value::String(value.to_string()),
        toml::Value::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| toml_to_json_redacted(value, key))
                .collect(),
        ),
        toml::Value::Table(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), toml_to_json_redacted(value, key)))
                .collect(),
        ),
    }
}

fn is_secret_param_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "password"
            | "auth_token"
            | "token"
            | "secret"
            | "client_secret"
            | "source_ip"
            | "source_cidr"
            | "allowed_clients"
            | "clients"
    )
}

fn ads_client_params(config: &AdsClientConfig) -> serde_json::Value {
    json!({
        "connections": config.connections.iter().map(|connection| {
            json!({
                "name": connection.route.name,
                "target_net_id": connection.route.target_net_id.0,
                "host": connection.route.host,
                "ams_port": connection.route.ams_port,
                "local_net_id_set": connection.route.local_net_id.is_some(),
                "transport": match connection.route.security.transport {
                    trust_ads_core::TransportSecurity::Secure => "secure",
                    trust_ads_core::TransportSecurity::Plain => "plain",
                },
                "auto_add_route": connection.route.security.auto_add_route,
                "points": connection.points.len(),
            })
        }).collect::<Vec<_>>(),
    })
}

fn ads_server_params(config: &crate::ads::server::AdsServerRuntimeConfig) -> serde_json::Value {
    json!({
        "enabled": config.enabled,
        "listen": config.listen.as_ref().map(ToString::to_string),
        "ads_port": config.ads_port,
        "ams_net_id": config.ams_net_id.as_ref().map(|net_id| net_id.0.clone()),
        "insecure_transport": config.insecure_transport,
        "writes_enabled": config.writes_enabled,
        "symbol_namespace": config.symbol_namespace.to_string(),
        "allow_unpinned_clients": config.allow_unpinned_clients,
        "unsafe_allow_public_bind": config.unsafe_allow_public_bind,
        "expose": config.expose.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "writable": config.writable.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "clients_count": config.clients.len(),
        "max_symbols": config.max_symbols,
        "max_clients": config.max_clients,
        "max_subscriptions_per_client": config.max_subscriptions_per_client,
        "max_total_subscriptions": config.max_total_subscriptions,
        "max_frame_bytes": config.max_frame_bytes,
        "max_sumup_items": config.max_sumup_items,
        "max_write_bytes": config.max_write_bytes,
        "max_string_bytes": config.max_string_bytes,
        "read_timeout_ms": config.read_timeout_ms,
        "idle_timeout_ms": config.idle_timeout_ms,
        "min_notification_cycle_ms": config.min_notification_cycle_ms,
    })
}

fn mesh_detail(evidence: Option<&crate::mesh::MeshTopologyEvidence>, peer_count: usize) -> String {
    match evidence {
        Some(evidence) if evidence.is_ready() => {
            format!("Mesh session is ready; {peer_count} live peer(s) reported by liveliness.")
        }
        Some(evidence) => evidence
            .degraded_reason()
            .map(|reason| format!("Mesh session is not ready: {reason}"))
            .unwrap_or_else(|| "Mesh session is not ready.".to_string()),
        None => "Mesh is configured, but no live mesh service evidence is available.".to_string(),
    }
}

fn configured_mesh_link_status(
    target: &str,
    evidence: Option<&crate::mesh::MeshTopologyEvidence>,
) -> String {
    let Some(evidence) = evidence else {
        return "configured_policy".to_string();
    };
    let snapshot = evidence.liveliness_snapshot();
    if snapshot
        .peers
        .iter()
        .any(|peer| peer == target || target.contains(peer.as_str()))
    {
        "connected".to_string()
    } else if evidence.is_ready() {
        "degraded".to_string()
    } else {
        "configured_policy".to_string()
    }
}

fn realtime_health_and_detail(status: &LinuxRtRuntimeStatus) -> (String, String) {
    if !status.errors.is_empty() {
        return (
            "error".to_string(),
            format!(
                "Realtime posture has {} error(s): {}",
                status.errors.len(),
                status
                    .errors
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        );
    }
    if !status.active || !status.warnings.is_empty() {
        return (
            "degraded".to_string(),
            format!(
                "Realtime posture is not fully active; {} warning(s).",
                status.warnings.len()
            ),
        );
    }
    (
        "connected".to_string(),
        "Realtime posture evidence is active.".to_string(),
    )
}

fn ads_server_connected_clients(state: &ControlState) -> Option<usize> {
    #[cfg(feature = "ads-server")]
    {
        state
            .ads_server_runtime
            .lock()
            .ok()
            .and_then(|runtime| runtime.as_ref().map(|runtime| runtime.connected_clients()))
    }
    #[cfg(not(feature = "ads-server"))]
    {
        let _ = state;
        None
    }
}

fn ads_status_report(state: &ControlState) -> Option<AdsStatusReport> {
    let (tx, rx) = mpsc::channel();
    state
        .resource
        .send_command(ResourceCommand::AdsStatus { respond_to: tx })
        .ok()?;
    rx.recv_timeout(ADS_STATUS_TIMEOUT).ok()
}

fn ads_client_endpoint_health_and_detail(
    config: &AdsClientConfig,
    status: Option<&AdsStatusReport>,
) -> (String, String) {
    let Some(status) = status else {
        return (
            "configured_policy".to_string(),
            format!(
                "{} ADS connection(s) configured; no live ADS client status has been reported yet.",
                config.connections.len()
            ),
        );
    };
    let connection_statuses = config
        .connections
        .iter()
        .filter_map(|connection| ads_status_for_connection(Some(status), connection))
        .collect::<Vec<_>>();
    if connection_statuses.is_empty() {
        return (
            "configured_policy".to_string(),
            format!(
                "{} ADS connection(s) configured; live status has no matching connection yet.",
                config.connections.len()
            ),
        );
    }
    if connection_statuses
        .iter()
        .any(|connection| connection.state == AdsConnectionStatusState::Faulted)
    {
        return (
            "error".to_string(),
            "One or more ADS client connections are faulted.".to_string(),
        );
    }
    if connection_statuses.iter().any(|connection| {
        !matches!(connection.state, AdsConnectionStatusState::Connected)
            || connection.degraded_points > 0
    }) {
        return (
            "degraded".to_string(),
            "One or more ADS client connections are reconnecting, stale, or degraded.".to_string(),
        );
    }
    (
        "connected".to_string(),
        format!(
            "{} ADS client connection(s) are connected.",
            connection_statuses.len()
        ),
    )
}

fn ads_client_live(
    config: &AdsClientConfig,
    status: Option<&AdsStatusReport>,
) -> Option<serde_json::Value> {
    let status = status?;
    let connections = config
        .connections
        .iter()
        .map(|connection| {
            let live = ads_status_for_connection(Some(status), connection);
            json!({
                "name": connection.route.name.as_str(),
                "target": connection.route.target_net_id.0.as_str(),
                "state": live.map(|item| item.state),
                "point_count": live.map(|item| item.point_count),
                "degraded_points": live.map(|item| item.degraded_points),
                "last_good_value_ms": live.and_then(|item| item.last_good_value_ms),
            })
        })
        .collect::<Vec<_>>();
    let connected = status
        .connections
        .iter()
        .filter(|connection| connection.state == AdsConnectionStatusState::Connected)
        .count();
    Some(json!({
        "value": {
            "connected": connected,
            "total": config.connections.len(),
            "connections": connections,
        },
        "last_seen_ms": status
            .connections
            .iter()
            .filter_map(|connection| connection.last_good_value_ms)
            .max()
            .unwrap_or_else(now_ms),
    }))
}

fn ads_client_local_net_id(config: &AdsClientConfig) -> Option<String> {
    config
        .connections
        .iter()
        .find_map(|connection| connection.route.local_net_id.as_ref())
        .map(|net_id| net_id.0.clone())
}

fn ads_status_for_connection<'a>(
    status: Option<&'a AdsStatusReport>,
    connection: &AdsConnectionConfig,
) -> Option<&'a AdsConnectionStatus> {
    let status = status?;
    let route = &connection.route;
    status.connections.iter().find(|item| {
        item.name == route.name
            || item.target.as_ref().is_some_and(|target| {
                target.ams_net_id == route.target_net_id.0 && target.ams_port == route.ams_port
            })
    })
}

fn ads_connection_status_health(status: &AdsConnectionStatus) -> String {
    match status.state {
        AdsConnectionStatusState::Connected if status.degraded_points == 0 => "connected",
        AdsConnectionStatusState::Faulted => "error",
        AdsConnectionStatusState::Connected
        | AdsConnectionStatusState::Reconnecting
        | AdsConnectionStatusState::Stale => "degraded",
        AdsConnectionStatusState::Disabled | AdsConnectionStatusState::Unknown => {
            "configured_policy"
        }
    }
    .to_string()
}

fn ads_route_secure(connection: &AdsConnectionConfig) -> bool {
    !matches!(
        connection.route.security.transport,
        trust_ads_core::TransportSecurity::Plain
    )
}

fn discovery_health(entry: &DiscoveryEntry) -> String {
    if discovery_is_stale(entry) {
        "runtime_unreachable".to_string()
    } else {
        "connected".to_string()
    }
}

fn discovery_is_stale(entry: &DiscoveryEntry) -> bool {
    let seen = discovery_last_seen_ms(entry);
    now_ms().saturating_sub(seen) > DISCOVERY_STALE_AFTER_MS
}

fn newest_discovery_seen_ms(entries: &[DiscoveryEntry]) -> Option<u64> {
    entries.iter().map(discovery_last_seen_ms).max()
}

fn discovery_last_seen_ms(entry: &DiscoveryEntry) -> u64 {
    entry.last_seen_ns / 1_000_000
}

fn same_host_by_discovery(
    runtime_id: &str,
    entry: &DiscoveryEntry,
    entries: &[DiscoveryEntry],
) -> bool {
    if let Some(host_group) = entry.host_group.as_deref() {
        if entries.iter().any(|candidate| {
            candidate.name.as_str() == runtime_id
                && candidate.host_group.as_deref() == Some(host_group)
        }) {
            return true;
        }
    }
    false
}

fn mesh_peer_is_live(
    entry: &DiscoveryEntry,
    evidence: Option<&crate::mesh::MeshTopologyEvidence>,
) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };
    evidence.liveliness_snapshot().peers.iter().any(|peer| {
        peer == entry.name.as_str()
            || peer == entry.id.as_str()
            || entry.id.as_str().contains(peer.as_str())
    })
}

fn first_address_with_port(entry: &DiscoveryEntry, port: u16) -> Option<String> {
    entry
        .addresses
        .first()
        .map(|address| format!("{address}:{port}"))
}

fn discovery_protocols(entry: &DiscoveryEntry) -> Vec<String> {
    let mut protocols = vec!["discovery".to_string()];
    if entry.web_port.is_some() {
        protocols.push("web".to_string());
    }
    if entry.mesh_port.is_some() {
        protocols.push("mesh".to_string());
    }
    if entry.control.is_some() {
        protocols.push("control".to_string());
    }
    protocols
}

fn is_self_discovery_entry(runtime_id: &str, entry: &DiscoveryEntry) -> bool {
    entry.name.as_str() == runtime_id || entry.id.as_str() == runtime_id
}

fn discovered_runtime_id(entry: &DiscoveryEntry) -> String {
    format!("runtime:{}", sanitize_id(entry.name.as_str()))
}

fn discovered_host_id(entry: &DiscoveryEntry) -> String {
    entry.host_group.as_ref().map_or_else(
        || format!("host:discovery:{}", sanitize_id(entry.id.as_str())),
        |host_group| format!("host:{}", sanitize_id(host_group.as_str())),
    )
}

fn discovered_external_id(entry: &DiscoveryEntry) -> String {
    format!("external:discovery:{}", sanitize_id(entry.id.as_str()))
}

fn shared_mqtt_id(broker: &str) -> String {
    format!("shared:mqtt:{}", sanitize_id(broker))
}

fn configured_host_ips(settings: &RuntimeSettings) -> Vec<String> {
    let mut values = [
        settings.web.listen.as_str(),
        settings.opcua.listen.as_str(),
        settings.mesh.listen.as_str(),
    ]
    .iter()
    .filter_map(|value| host_from_listen(value))
    .filter(|host| host != "0.0.0.0" && host != "::" && host != "[::]")
    .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    if values.is_empty() {
        values.push("127.0.0.1".to_string());
    }
    values
}

fn host_from_listen(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(stripped) = trimmed.strip_prefix('[') {
        let (host, _) = stripped.split_once(']')?;
        return Some(host.to_string());
    }
    trimmed
        .rsplit_once(':')
        .map(|(host, _)| host.to_string())
        .or_else(|| Some(trimmed.to_string()))
}

fn host_name() -> String {
    let env_hostname = std::env::var("HOSTNAME").ok();
    let computer_name = std::env::var("COMPUTERNAME").ok();
    let os_hostname = os_host_name();
    host_name_from_sources(
        env_hostname.as_deref(),
        computer_name.as_deref(),
        os_hostname.as_deref(),
    )
}

fn host_name_from_sources(
    env_hostname: Option<&str>,
    computer_name: Option<&str>,
    os_hostname: Option<&str>,
) -> String {
    env_hostname
        .and_then(normalized_host_name)
        .or_else(|| computer_name.and_then(normalized_host_name))
        .or_else(|| os_hostname.and_then(normalized_host_name))
        .unwrap_or_else(|| "local-host".to_string())
}

fn os_host_name() -> Option<String> {
    os_host_name_from_files()
        .or_else(os_host_name_from_command)
        .and_then(|value| normalized_host_name(value.as_str()))
}

#[cfg(unix)]
fn os_host_name_from_files() -> Option<String> {
    ["/proc/sys/kernel/hostname", "/etc/hostname"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}

#[cfg(not(unix))]
fn os_host_name_from_files() -> Option<String> {
    None
}

fn os_host_name_from_command() -> Option<String> {
    let output = std::process::Command::new("hostname").output().ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn normalized_host_name(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn stable_host_id(hostname: &str) -> String {
    format!("host:{}", sanitize_id(hostname))
}

fn endpoint_id(runtime_id: &str, protocol: &str) -> String {
    format!("endpoint:{runtime_id}:{protocol}")
}

fn endpoint_instance_id(runtime_id: &str, protocol: &str, index: usize) -> String {
    if index == 0 {
        endpoint_id(runtime_id, protocol)
    } else {
        format!("endpoint:{runtime_id}:{protocol}:{index}")
    }
}

fn protocol_from_driver_name(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "modbus-tcp" | "modbus_tcp" => "modbus_tcp".to_string(),
        other => other.replace('-', "_"),
    }
}

fn driver_role(protocol: &str) -> &'static str {
    match protocol {
        "modbus_tcp" => "client",
        "ethercat" => "master",
        "mqtt" => "client",
        _ => "owned_driver",
    }
}

fn runtime_cloud_configured(settings: &RuntimeSettings) -> bool {
    !matches!(
        settings.runtime_cloud.profile,
        crate::config::RuntimeCloudProfile::Dev
    ) || !settings.runtime_cloud.link_preferences.is_empty()
        || !settings.runtime_cloud.wan_allow_write.is_empty()
}

fn host_board() -> Option<String> {
    [
        "/sys/firmware/devicetree/base/model",
        "/sys/class/dmi/id/product_name",
    ]
    .iter()
    .find_map(read_trimmed)
}

fn host_temp_c() -> Option<f64> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    for entry in entries.flatten() {
        let path = entry.path().join("temp");
        let Some(raw) = read_trimmed(path.as_path()) else {
            continue;
        };
        let Ok(value) = raw.parse::<f64>() else {
            continue;
        };
        if value > 1_000.0 {
            return Some((value / 1_000.0 * 10.0).round() / 10.0);
        }
        return Some(value);
    }
    None
}

fn host_uptime_s() -> Option<u64> {
    let text = read_trimmed("/proc/uptime")?;
    text.split_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value as u64)
}

fn host_load() -> Option<f64> {
    let text = read_trimmed("/proc/loadavg")?;
    text.split_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim_matches(char::from(0)).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn looks_containerized(cgroup: &str) -> bool {
    cgroup.contains("docker")
        || cgroup.contains("kubepods")
        || cgroup.contains("containerd")
        || cgroup.contains("libpod")
}

fn parse_container_id(cgroup: &str) -> Option<String> {
    cgroup.split(['/', ':', '\n']).rev().find_map(|part| {
        let trimmed = part.trim();
        if trimmed.len() >= 12 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
            Some(trimmed.chars().take(12).collect())
        } else {
            trimmed
                .strip_prefix("docker-")
                .and_then(|value| value.split('.').next())
                .filter(|value| value.len() >= 12)
                .map(|value| value.chars().take(12).collect())
        }
    })
}

fn sanitize_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized.to_string()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Serialize)]
struct FleetTopologyResponse {
    schema_version: u32,
    hosts: Vec<FleetHost>,
    links: Vec<FleetLink>,
    shared: Vec<FleetShared>,
    external: Vec<FleetExternal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    discovered: Vec<FleetDiscovered>,
}

#[derive(Debug, Serialize)]
struct FleetHost {
    host_id: String,
    hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    board: Option<String>,
    arch: String,
    os: String,
    ips: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temp_c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime_s: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    load: Option<f64>,
    containers: Vec<FleetContainer>,
    runtimes: Vec<FleetRuntime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_seen_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct FleetContainer {
    container_id: String,
    name: String,
    image: String,
    status: String,
    runtimes: Vec<FleetRuntime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    restart_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mem_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mounts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Debug, Serialize)]
struct FleetRuntime {
    runtime_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    web_listen: Option<String>,
    mode: String,
    cycle_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    load: Option<f64>,
    health: String,
    detail: String,
    endpoints: Vec<FleetEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_seen_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct FleetEndpoint {
    id: String,
    kind: String,
    protocol: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    health: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    live: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<FleetEndpointChild>,
    owned: bool,
    supports_test: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Debug, Serialize)]
struct FleetEndpointChild {
    id: String,
    kind: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channels: Option<u16>,
    health: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Debug, Serialize)]
struct FleetLink {
    id: String,
    from: String,
    to: String,
    protocol: String,
    role: String,
    direction: String,
    same_host: bool,
    status: String,
    secure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct FleetShared {
    id: String,
    kind: String,
    name: String,
    address: String,
    used_by: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FleetExternal {
    id: String,
    kind: String,
    name: String,
    via_protocol: Vec<String>,
    direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Debug, Serialize)]
struct FleetDiscovered {
    id: String,
    kind: String,
    name: String,
    addresses: Vec<String>,
    via_protocol: Vec<String>,
    direction: String,
    adopted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    web_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    web_tls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_seen_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{host_name_from_sources, normalized_host_name};

    #[test]
    fn host_name_uses_os_hostname_before_literal_fallback() {
        assert_eq!(
            host_name_from_sources(None, None, Some("raspberrypi")),
            "raspberrypi"
        );
        assert_eq!(
            host_name_from_sources(Some(""), Some("  "), Some("raspberrypi")),
            "raspberrypi"
        );
        assert_eq!(host_name_from_sources(None, None, None), "local-host");
    }

    #[test]
    fn host_name_normalization_trims_whitespace_and_trailing_dot() {
        assert_eq!(
            normalized_host_name(" raspberrypi.local. "),
            Some("raspberrypi.local".to_string())
        );
        assert_eq!(normalized_host_name("  "), None);
    }
}
