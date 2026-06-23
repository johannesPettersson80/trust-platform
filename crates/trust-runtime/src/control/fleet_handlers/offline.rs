use std::fs;
use std::path::Path;

use serde_json::json;

use super::{
    ads_client_links, ads_client_params, ads_server_params, endpoint_from_driver_config,
    endpoint_id, fleet_link, host_name, local_host, opcua_client_links, opcua_client_params,
    runtime_cloud_configured, topology_external, topology_shared, FleetEndpoint, FleetRuntime,
    FleetTopologyResponse, FLEET_TOPOLOGY_SCHEMA_VERSION,
};
use crate::config::{IoConfig, IoDriverConfig, RuntimeConfig};
use crate::settings::RuntimeSettings;

pub(super) fn build_project_fleet_topology(
    project_root: &Path,
) -> Result<FleetTopologyResponse, String> {
    let runtime = RuntimeConfig::load(project_root.join("runtime.toml"))
        .map_err(|error| format!("failed to load runtime.toml: {error}"))?;
    let io_drivers = load_project_io_drivers(project_root)?;
    let ads_client_config = load_project_ads_config(project_root, &runtime)?;
    let opcua_client_config = load_project_opcua_client_config(project_root, &runtime)?;
    let settings = RuntimeSettings::from_runtime_config(&runtime, false, 1);
    let runtime_id = runtime.resource_name.to_string();

    let mut endpoints = io_drivers
        .iter()
        .enumerate()
        .map(|(index, driver)| {
            endpoint_from_driver_config(&runtime_id, index, driver, None, None, 0)
        })
        .collect::<Vec<_>>();
    endpoints.extend(offline_service_endpoints(
        &runtime_id,
        &settings,
        &runtime,
        ads_client_config.as_ref(),
        opcua_client_config.as_ref(),
    ));

    let mut host = local_host(host_name().as_str(), &settings);
    host.source = Some("config".to_string());
    host.last_seen_ms = None;
    host.runtimes.push(FleetRuntime {
        runtime_id: runtime_id.clone(),
        name: runtime.resource_name.to_string(),
        control_endpoint: Some(runtime.control_endpoint.to_string()),
        web_listen: settings
            .web
            .enabled
            .then(|| settings.web.listen.to_string()),
        mode: "stopped".to_string(),
        cycle_ms: settings.cycle_interval.as_millis() as u64,
        load: None,
        health: "configured_policy".to_string(),
        detail: "Loaded from project files; runtime is not running.".to_string(),
        endpoints,
        source: Some("config".to_string()),
        last_seen_ms: None,
    });

    let mut links = offline_topology_links(
        &runtime_id,
        &settings,
        io_drivers.as_slice(),
        ads_client_config.as_ref(),
        opcua_client_config.as_ref(),
    );
    links.sort_by(|left, right| left.id.cmp(&right.id));
    links.dedup_by(|left, right| left.id == right.id);

    Ok(FleetTopologyResponse {
        schema_version: FLEET_TOPOLOGY_SCHEMA_VERSION,
        hosts: vec![host],
        links,
        shared: topology_shared(&runtime_id, io_drivers.as_slice()),
        external: topology_external(
            &settings,
            io_drivers.as_slice(),
            ads_client_config.as_ref(),
            opcua_client_config.as_ref(),
            &[],
        ),
        discovered: Vec::new(),
    })
}

fn load_project_io_drivers(project_root: &Path) -> Result<Vec<IoDriverConfig>, String> {
    let path = project_root.join("io.toml");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    IoConfig::load(path)
        .map(|config| config.drivers)
        .map_err(|error| format!("failed to load io.toml: {error}"))
}

fn load_project_ads_config(
    project_root: &Path,
    runtime: &RuntimeConfig,
) -> Result<Option<crate::ads::AdsClientConfig>, String> {
    if !runtime.ads.enabled {
        return Ok(None);
    }
    let path = if runtime.ads.config_path.is_relative() {
        project_root.join(&runtime.ads.config_path)
    } else {
        runtime.ads.config_path.clone()
    };
    if !path.is_file() {
        return Err(format!(
            "runtime.ads.enabled=true but ADS config is missing at {}",
            path.display()
        ));
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    crate::ads::parse_ads_toml(text.as_str())
        .map(Some)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn load_project_opcua_client_config(
    project_root: &Path,
    runtime: &RuntimeConfig,
) -> Result<Option<crate::opcua::OpcUaClientConfig>, String> {
    if !runtime.opcua_client.enabled {
        return Ok(None);
    }
    let path = if runtime.opcua_client.config_path.is_relative() {
        project_root.join(&runtime.opcua_client.config_path)
    } else {
        runtime.opcua_client.config_path.clone()
    };
    if !path.is_file() {
        return Err(format!(
            "runtime.opcua_client.enabled=true but OPC UA client config is missing at {}",
            path.display()
        ));
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    crate::opcua::parse_opcua_client_toml(text.as_str())
        .map(Some)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn offline_service_endpoints(
    runtime_id: &str,
    settings: &RuntimeSettings,
    runtime: &RuntimeConfig,
    ads_client_config: Option<&crate::ads::AdsClientConfig>,
    opcua_client_config: Option<&crate::opcua::OpcUaClientConfig>,
) -> Vec<FleetEndpoint> {
    let mut endpoints = Vec::new();
    if settings.web.enabled {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "web"),
            kind: "service".to_string(),
            protocol: "web".to_string(),
            name: "Web / IDE".to_string(),
            address: Some(settings.web.listen.to_string()),
            role: Some("server".to_string()),
            health: "configured_policy".to_string(),
            detail: "Configured in runtime.toml; runtime is not running.".to_string(),
            live: None,
            params: Some(json!({
                "enabled": settings.web.enabled,
                "listen": settings.web.listen.to_string(),
                "auth": settings.web.auth.to_string(),
                "tls": settings.web.tls,
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("config".to_string()),
        });
    }
    if settings.opcua.enabled || !settings.opcua.expose.is_empty() {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "opcua"),
            kind: "service".to_string(),
            protocol: "opcua".to_string(),
            name: "OPC UA server".to_string(),
            address: Some(settings.opcua.listen.to_string()),
            role: Some("server".to_string()),
            health: if settings.opcua.enabled {
                "configured_policy"
            } else {
                "not_configured"
            }
            .to_string(),
            detail: "Configured in runtime.toml; runtime is not running.".to_string(),
            live: None,
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
            source: Some("config".to_string()),
        });
    }
    if settings.discovery.enabled {
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
            health: "configured_policy".to_string(),
            detail: "Configured in runtime.toml; runtime is not running.".to_string(),
            live: None,
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
            source: Some("config".to_string()),
        });
    }
    if settings.mesh.enabled || !settings.mesh.connect.is_empty() {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "mesh"),
            kind: "peer".to_string(),
            protocol: "mesh".to_string(),
            name: "Mesh / Zenoh".to_string(),
            address: Some(settings.mesh.listen.to_string()),
            role: Some(settings.mesh.role.as_str().to_string()),
            health: "configured_policy".to_string(),
            detail: "Configured in runtime.toml; runtime is not running.".to_string(),
            live: None,
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
            source: Some("config".to_string()),
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
            source: Some("config".to_string()),
        });
    }
    if settings.realtime.enabled {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "realtime_t0"),
            kind: "peer".to_string(),
            protocol: "realtime_t0".to_string(),
            name: "Realtime T0".to_string(),
            address: None,
            role: Some("same_host".to_string()),
            health: "configured_policy".to_string(),
            detail: "Configured in runtime.toml; runtime is not running.".to_string(),
            live: None,
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
            source: Some("config".to_string()),
        });
    }
    if let Some(config) = ads_client_config.filter(|config| !config.connections.is_empty()) {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "ads"),
            kind: "peer".to_string(),
            protocol: "ads".to_string(),
            name: "ADS client".to_string(),
            address: super::ads_client_local_net_id(config),
            role: Some("client".to_string()),
            health: "configured_policy".to_string(),
            detail: "Configured in ADS project config; runtime is not running.".to_string(),
            live: None,
            params: Some(ads_client_params(config)),
            children: Vec::new(),
            owned: true,
            supports_test: true,
            source: Some("config".to_string()),
        });
    }
    if let Some(config) = opcua_client_config.filter(|config| !config.connections.is_empty()) {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "opcua_client"),
            kind: "peer".to_string(),
            protocol: "opcua_client".to_string(),
            name: "OPC UA client".to_string(),
            address: config
                .connections
                .first()
                .map(|connection| connection.endpoint_url.clone()),
            role: Some("client".to_string()),
            health: "configured_policy".to_string(),
            detail: "Configured in OPC UA client project config; runtime is not running."
                .to_string(),
            live: None,
            params: Some(opcua_client_params(config)),
            children: Vec::new(),
            owned: true,
            supports_test: true,
            source: Some("config".to_string()),
        });
    }
    if runtime.ads_server.enabled || !runtime.ads_server.expose.is_empty() {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "ads_server"),
            kind: "service".to_string(),
            protocol: "ads_server".to_string(),
            name: "ADS server".to_string(),
            address: runtime.ads_server.listen.as_ref().map(ToString::to_string),
            role: Some("server".to_string()),
            health: if runtime.ads_server.enabled {
                "configured_policy"
            } else {
                "not_configured"
            }
            .to_string(),
            detail: "Configured in runtime.toml; runtime is not running.".to_string(),
            live: None,
            params: Some(ads_server_params(&runtime.ads_server)),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("config".to_string()),
        });
    }
    endpoints
}

fn offline_topology_links(
    runtime_id: &str,
    settings: &RuntimeSettings,
    io_drivers: &[IoDriverConfig],
    ads_client_config: Option<&crate::ads::AdsClientConfig>,
    opcua_client_config: Option<&crate::opcua::OpcUaClientConfig>,
) -> Vec<super::FleetLink> {
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
                "configured_policy".to_string(),
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
    let shared_links = io_drivers.iter().enumerate().filter_map(|(index, driver)| {
        let protocol = super::protocol_from_driver_name(driver.name.as_str());
        if protocol != "mqtt" {
            return None;
        }
        let broker = super::driver_endpoint_address(&driver.params)?;
        Some(fleet_link(
            super::endpoint_instance_id(runtime_id, protocol.as_str(), index),
            super::shared_mqtt_id(broker.as_str()),
            protocol,
            "publish_subscribe",
            "publish_subscribe",
            false,
            "configured_policy".to_string(),
            super::bool_param(&driver.params, "tls"),
            Some("MQTT broker referenced by io.toml".to_string()),
        ))
    });

    mesh_links
        .chain(cloud_links)
        .chain(shared_links)
        .chain(super::driver_target_links(runtime_id, io_drivers, &[]))
        .chain(ads_client_links(runtime_id, ads_client_config, None))
        .chain(opcua_client_links(runtime_id, opcua_client_config, None))
        .collect()
}
