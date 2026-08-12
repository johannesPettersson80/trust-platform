use super::*;

pub(super) struct FleetRuntimeInputs<'a> {
    pub(super) state: &'a ControlState,
    pub(super) settings: &'a RuntimeSettings,
    pub(super) io_health: &'a [IoDriverStatus],
    pub(super) io_drivers: &'a [IoDriverConfig],
    pub(super) io_snapshot: Option<&'a IoSnapshot>,
    pub(super) io_snapshot_seen_ms: u64,
    pub(super) realtime: Option<&'a LinuxRtRuntimeStatus>,
    pub(super) mesh_evidence: Option<&'a crate::mesh::MeshTopologyEvidence>,
    pub(super) discovery_entries: &'a [DiscoveryEntry],
    pub(super) ads_client_config: Option<&'a AdsClientConfig>,
    pub(super) ads_status: Option<&'a AdsStatusReport>,
    pub(super) opcua_client_config: Option<&'a OpcUaClientConfig>,
    pub(super) opcua_client_status: Option<&'a OpcUaClientStatusReport>,
}

pub(super) fn runtime_node(runtime_id: &str, inputs: &FleetRuntimeInputs<'_>) -> FleetRuntime {
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

pub(super) fn runtime_health(
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

pub(super) fn runtime_detail(state: &ControlState) -> String {
    if let Some(error) = state.resource.last_error() {
        return format!("Runtime reported a fault: {error}");
    }
    "Runtime answered fleet.topology from its control channel.".to_string()
}

pub(super) fn io_endpoints(
    runtime_id: &str,
    io_health: &[IoDriverStatus],
    io_drivers: &[IoDriverConfig],
    io_snapshot: Option<&IoSnapshot>,
    io_snapshot_seen_ms: u64,
) -> Vec<FleetEndpoint> {
    if !io_drivers.is_empty() {
        let mut endpoints = Vec::with_capacity(io_drivers.len());
        let mut enabled_protocol_counts = std::collections::HashMap::<String, usize>::new();
        for (index, driver) in io_drivers.iter().enumerate() {
            let health = if driver.enabled {
                let protocol = protocol_from_driver_name(driver.name.as_str());
                let protocol_index = enabled_protocol_counts.entry(protocol).or_default();
                let health =
                    driver_health_for_config(io_health, *protocol_index, driver.name.as_str());
                *protocol_index += 1;
                health
            } else {
                None
            };
            endpoints.push(endpoint_from_driver_config(
                runtime_id,
                index,
                driver,
                health,
                io_snapshot,
                io_snapshot_seen_ms,
                LIVE_CONFIGURED_DRIVER_DETAILS,
            ));
        }
        return endpoints;
    }

    io_health
        .iter()
        .enumerate()
        .map(|(index, driver)| {
            endpoint_from_driver_health(runtime_id, index, driver, io_snapshot, io_snapshot_seen_ms)
        })
        .collect()
}

pub(super) fn endpoint_from_driver_config(
    runtime_id: &str,
    index: usize,
    driver: &IoDriverConfig,
    health: Option<&IoDriverStatus>,
    io_snapshot: Option<&IoSnapshot>,
    io_snapshot_seen_ms: u64,
    configured_details: ConfiguredDriverDetails<'_>,
) -> FleetEndpoint {
    let protocol = protocol_from_driver_name(driver.name.as_str());
    let (health_value, detail) = if driver.enabled {
        health.map(driver_health).unwrap_or_else(|| {
            (
                "configured_policy".to_string(),
                configured_details.missing_health.to_string(),
            )
        })
    } else {
        (
            "disabled".to_string(),
            "Disabled in io.toml; it will not run until enabled again.".to_string(),
        )
    };
    FleetEndpoint {
        id: endpoint_instance_id(runtime_id, protocol.as_str(), index),
        kind: "field".to_string(),
        protocol: protocol.clone(),
        name: driver_display_name(protocol.as_str(), index, &driver.params),
        address: driver_endpoint_address(&driver.params),
        role: Some(driver_role(protocol.as_str()).to_string()),
        health: health_value,
        detail,
        live: driver
            .enabled
            .then(|| io_snapshot_live(io_snapshot, io_snapshot_seen_ms))
            .flatten(),
        params: Some(redacted_toml_params(&driver.params)),
        children: ethercat_endpoint_children(
            protocol.as_str(),
            &driver.params,
            configured_details.ethercat_child,
        ),
        owned: true,
        supports_test: driver.enabled && matches!(protocol.as_str(), "modbus_tcp" | "mqtt"),
        source: Some("self".to_string()),
    }
}

pub(super) fn endpoint_from_driver_health(
    runtime_id: &str,
    index: usize,
    driver: &IoDriverStatus,
    io_snapshot: Option<&IoSnapshot>,
    io_snapshot_seen_ms: u64,
) -> FleetEndpoint {
    let protocol = protocol_from_driver_name(driver.name.as_str());
    let role = driver_role(protocol.as_str()).to_string();
    let supports_test = matches!(protocol.as_str(), "modbus_tcp" | "mqtt");
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
        supports_test,
        source: Some("self".to_string()),
    }
}

pub(super) fn driver_health(driver: &IoDriverStatus) -> (String, String) {
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
