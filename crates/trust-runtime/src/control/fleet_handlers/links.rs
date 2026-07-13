use super::*;

pub(super) fn topology_links(runtime_id: &str, inputs: &FleetRuntimeInputs<'_>) -> Vec<FleetLink> {
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
        .chain(opcua_client_links(
            runtime_id,
            inputs.opcua_client_config,
            inputs.opcua_client_status,
        ))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fleet_link(
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

pub(super) fn link_id(from: &str, to: &str, protocol: &str, role: &str) -> String {
    format!(
        "link:{}:{}:{}:{}",
        sanitize_id(protocol),
        sanitize_id(role),
        sanitize_id(from),
        sanitize_id(to)
    )
}

pub(super) fn driver_target_links(
    runtime_id: &str,
    io_drivers: &[IoDriverConfig],
    io_health: &[IoDriverStatus],
) -> Vec<FleetLink> {
    let mut links = Vec::new();
    let mut enabled_index = 0usize;
    for (index, driver) in io_drivers.iter().enumerate() {
        let protocol = protocol_from_driver_name(driver.name.as_str());
        let from = endpoint_instance_id(runtime_id, protocol.as_str(), index);
        let status = if driver.enabled {
            let status = driver_link_status(io_health, enabled_index, driver);
            enabled_index += 1;
            status
        } else {
            "disabled".to_string()
        };
        let link = match protocol.as_str() {
            "modbus_tcp" => driver_endpoint_address(&driver.params).map(|address| {
                fleet_link(
                    from,
                    modbus_external_id(address.as_str()),
                    protocol,
                    "client",
                    "outbound",
                    false,
                    status,
                    bool_param(&driver.params, "tls"),
                    Some(address),
                )
            }),
            "ethercat" => driver_adapter(&driver.params).map(|adapter| {
                fleet_link(
                    from,
                    ethercat_external_id(adapter.as_str()),
                    protocol,
                    "master",
                    "outbound",
                    false,
                    status,
                    false,
                    None,
                )
            }),
            _ => None,
        };
        let Some(link) = link else {
            continue;
        };
        links.push(link);
    }
    links
}

pub(super) fn driver_target_externals(io_drivers: &[IoDriverConfig]) -> Vec<FleetExternal> {
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

pub(super) fn driver_link_status(
    io_health: &[IoDriverStatus],
    index: usize,
    driver: &IoDriverConfig,
) -> String {
    driver_health_for_config(io_health, index, driver.name.as_str())
        .map(driver_health)
        .map(|(health, _)| health)
        .unwrap_or_else(|| "configured_policy".to_string())
}

pub(super) fn modbus_external_id(address: &str) -> String {
    format!("external:modbus:{address}")
}

pub(super) fn ethercat_external_id(adapter: &str) -> String {
    format!("external:ethercat:{adapter}")
}

pub(super) fn driver_adapter(params: &toml::Value) -> Option<String> {
    params
        .get("adapter")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn ads_client_externals(config: Option<&AdsClientConfig>) -> Vec<FleetExternal> {
    let Some(config) = config else {
        return Vec::new();
    };
    config
        .connections
        .iter()
        .map(|connection| FleetExternal {
            id: ads_external_id(connection),
            kind: "plc".to_string(),
            name: format!("ADS device {}", connection.route.target_net_id.0),
            via_protocol: vec!["ads".to_string()],
            direction: "outbound".to_string(),
            source: Some("config".to_string()),
        })
        .collect()
}

pub(super) fn ads_client_links(
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

pub(super) fn ads_external_id(connection: &AdsConnectionConfig) -> String {
    format!("external:ads:{}", connection.route.target_net_id.0)
}

pub(super) fn opcua_client_externals(config: Option<&OpcUaClientConfig>) -> Vec<FleetExternal> {
    let Some(config) = config else {
        return Vec::new();
    };
    config
        .connections
        .iter()
        .map(|connection| FleetExternal {
            id: opcua_client_external_id(connection),
            kind: "opcua_server".to_string(),
            name: format!("OPC UA server {}", connection.name),
            via_protocol: vec!["opcua_client".to_string()],
            direction: "outbound".to_string(),
            source: Some("config".to_string()),
        })
        .collect()
}

pub(super) fn opcua_client_links(
    runtime_id: &str,
    config: Option<&OpcUaClientConfig>,
    status: Option<&OpcUaClientStatusReport>,
) -> Vec<FleetLink> {
    let Some(config) = config else {
        return Vec::new();
    };
    config
        .connections
        .iter()
        .map(|connection| {
            let connection_status = opcua_client_status_for_connection(status, connection);
            fleet_link(
                endpoint_id(runtime_id, "opcua_client"),
                opcua_client_external_id(connection),
                "opcua_client",
                "client",
                "outbound",
                false,
                connection_status
                    .map(opcua_client_connection_health)
                    .unwrap_or_else(|| "configured_policy".to_string()),
                !matches!(
                    connection.security.policy,
                    crate::opcua::OpcUaSecurityPolicy::None
                ),
                Some(connection.endpoint_url.clone()),
            )
        })
        .collect()
}

pub(super) fn opcua_client_external_id(
    connection: &crate::opcua::OpcUaClientConnectionConfig,
) -> String {
    format!(
        "external:opcua:{}",
        sanitize_id(connection.endpoint_url.as_str())
    )
}
