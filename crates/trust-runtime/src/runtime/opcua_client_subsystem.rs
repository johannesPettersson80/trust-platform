//! OPC UA client polling subsystem owned by the runtime scan cycle.

use crate::error::RuntimeError;
use crate::memory::VariableStorage;
use crate::opcua::{
    OpcUaClientConfig, OpcUaClientConnectionConfig, OpcUaClientConnectionState,
    OpcUaClientConnectionStatus, OpcUaClientPointAccess, OpcUaClientPointConfig,
    OpcUaClientPointStatus, OpcUaClientStatusReport,
};
use crate::value::{Value, ValueRef};

use super::core::Runtime;

pub(super) struct OpcUaClientSubsystem {
    connections: Vec<OpcUaRuntimeClientConnection>,
    deployed_config_hash: Option<String>,
}

struct OpcUaRuntimeClientConnection {
    config: OpcUaClientConnectionConfig,
    bindings: Vec<OpcUaClientBinding>,
    state: OpcUaClientConnectionState,
    detail: String,
    next_poll_ms: u64,
    last_seen_ms: Option<u64>,
    point_statuses: Vec<OpcUaClientPointStatus>,
}

struct OpcUaClientBinding {
    point: OpcUaClientPointConfig,
    reference: ValueRef,
    last_written: Option<Value>,
}

impl OpcUaClientSubsystem {
    pub(super) fn new() -> Self {
        Self {
            connections: Vec::new(),
            deployed_config_hash: None,
        }
    }

    pub(super) fn configure(
        &mut self,
        runtime: &Runtime,
        config: &OpcUaClientConfig,
    ) -> Result<(), RuntimeError> {
        self.connections.clear();
        for connection in &config.connections {
            let bindings = resolve_bindings(runtime, connection)?;
            let point_statuses = connection
                .points
                .iter()
                .map(|point| OpcUaClientPointStatus {
                    var: point.var.clone(),
                    node_id: point.node_id.clone(),
                    state: OpcUaClientConnectionState::Configured,
                    last_seen_ms: None,
                    value: None,
                    detail: "Configured; no live read has completed yet.".to_string(),
                })
                .collect();
            self.connections.push(OpcUaRuntimeClientConnection {
                config: connection.clone(),
                bindings,
                state: OpcUaClientConnectionState::Configured,
                detail: "Configured; no live read has completed yet.".to_string(),
                next_poll_ms: 0,
                last_seen_ms: None,
                point_statuses,
            });
        }
        Ok(())
    }

    pub(super) fn set_deployed_config_hash(&mut self, hash: Option<String>) {
        self.deployed_config_hash = hash;
    }

    pub(super) fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub(super) fn apply_inputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            if now_ms < connection.next_poll_ms {
                continue;
            }
            connection.next_poll_ms =
                now_ms.saturating_add(connection.config.poll_interval_ms.max(10));
            let read_points = connection
                .bindings
                .iter()
                .filter(|binding| binding.point.access.can_read())
                .map(|binding| binding.point.clone())
                .collect::<Vec<_>>();
            if read_points.is_empty() {
                continue;
            }
            match crate::opcua::read_opcua_client_point_values(&connection.config, &read_points) {
                Ok(values) => {
                    connection.state = OpcUaClientConnectionState::Connected;
                    connection.detail = "OPC UA client read completed.".to_string();
                    connection.last_seen_ms = Some(now_ms);
                    for (var, value) in values {
                        if let Some(binding) = connection
                            .bindings
                            .iter_mut()
                            .find(|binding| binding.point.var == var)
                        {
                            if !storage.write_by_ref(binding.reference.clone(), value.clone()) {
                                set_point_status(
                                    &mut connection.point_statuses,
                                    &binding.point,
                                    OpcUaClientConnectionState::Faulted,
                                    Some(now_ms),
                                    None,
                                    "Failed to write OPC UA client value into runtime storage.",
                                );
                                connection.state = OpcUaClientConnectionState::Faulted;
                                connection.detail = format!(
                                    "Failed to write OPC UA client value for '{}'.",
                                    binding.point.var
                                );
                                continue;
                            }
                            if binding.point.access.can_write() {
                                binding.last_written = Some(value.clone());
                            }
                            set_point_status(
                                &mut connection.point_statuses,
                                &binding.point,
                                OpcUaClientConnectionState::Connected,
                                Some(now_ms),
                                Some(value),
                                "Live OPC UA client value read.",
                            );
                        }
                    }
                }
                Err(error) => {
                    connection.state = if connection.last_seen_ms.is_some() {
                        OpcUaClientConnectionState::Stale
                    } else {
                        OpcUaClientConnectionState::Reconnecting
                    };
                    connection.detail = format!("OPC UA client read failed: {error}");
                    for binding in &connection.bindings {
                        set_point_status(
                            &mut connection.point_statuses,
                            &binding.point,
                            connection.state,
                            connection.last_seen_ms,
                            None,
                            connection.detail.as_str(),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn capture_outputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            let mut writes = Vec::new();
            for binding in connection
                .bindings
                .iter()
                .filter(|binding| binding.point.access.can_write())
            {
                if binding.last_written.is_none() {
                    continue;
                }
                let Some(value) = storage.read_by_ref(binding.reference.clone()).cloned() else {
                    set_point_status(
                        &mut connection.point_statuses,
                        &binding.point,
                        OpcUaClientConnectionState::Faulted,
                        Some(now_ms),
                        None,
                        "Failed to read runtime storage for OPC UA client write.",
                    );
                    continue;
                };
                if binding.last_written.as_ref() == Some(&value) {
                    continue;
                }
                writes.push((binding.point.clone(), value));
            }
            if writes.is_empty() {
                continue;
            }
            match crate::opcua::write_opcua_client_point_values(&connection.config, &writes) {
                Ok(()) => {
                    connection.state = OpcUaClientConnectionState::Connected;
                    connection.detail = "OPC UA client write completed.".to_string();
                    connection.last_seen_ms = Some(now_ms);
                    for (point, value) in writes {
                        if let Some(binding) = connection
                            .bindings
                            .iter_mut()
                            .find(|binding| binding.point.var == point.var)
                        {
                            binding.last_written = Some(value.clone());
                        }
                        set_point_status(
                            &mut connection.point_statuses,
                            &point,
                            OpcUaClientConnectionState::Connected,
                            Some(now_ms),
                            Some(value),
                            "Live OPC UA client value written.",
                        );
                    }
                }
                Err(error) => {
                    connection.state = OpcUaClientConnectionState::Stale;
                    connection.detail = format!("OPC UA client write failed: {error}");
                    for (point, _) in writes {
                        set_point_status(
                            &mut connection.point_statuses,
                            &point,
                            OpcUaClientConnectionState::Stale,
                            connection.last_seen_ms,
                            None,
                            connection.detail.as_str(),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn status_report(&self) -> OpcUaClientStatusReport {
        OpcUaClientStatusReport {
            enabled: !self.connections.is_empty(),
            deployed_config_hash: self.deployed_config_hash.clone(),
            connections: self
                .connections
                .iter()
                .map(|connection| {
                    let degraded_points = connection
                        .point_statuses
                        .iter()
                        .filter(|status| status.state != OpcUaClientConnectionState::Connected)
                        .count();
                    OpcUaClientConnectionStatus {
                        name: connection.config.name.clone(),
                        endpoint_url: connection.config.endpoint_url.clone(),
                        state: connection.state,
                        point_count: connection.point_statuses.len(),
                        degraded_points,
                        last_seen_ms: connection.last_seen_ms,
                        detail: connection.detail.clone(),
                        points: connection.point_statuses.clone(),
                    }
                })
                .collect(),
        }
    }
}

fn resolve_bindings(
    runtime: &Runtime,
    connection: &OpcUaClientConnectionConfig,
) -> Result<Vec<OpcUaClientBinding>, RuntimeError> {
    let mut bindings = Vec::with_capacity(connection.points.len());
    for point in &connection.points {
        let reference = runtime
            .storage()
            .ref_for_global(point.var.as_str())
            .ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!(
                        "OPC UA client connection '{}': global '{}' does not exist",
                        connection.name, point.var
                    )
                    .into(),
                )
            })?;
        validate_initial_value(runtime, connection, point)?;
        bindings.push(OpcUaClientBinding {
            point: point.clone(),
            reference,
            last_written: None,
        });
    }
    Ok(bindings)
}

fn validate_initial_value(
    runtime: &Runtime,
    connection: &OpcUaClientConnectionConfig,
    point: &OpcUaClientPointConfig,
) -> Result<(), RuntimeError> {
    let value = runtime
        .storage()
        .get_global(point.var.as_str())
        .ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!(
                    "OPC UA client connection '{}': global '{}' does not exist",
                    connection.name, point.var
                )
                .into(),
            )
        })?;
    let Some(mapped) = crate::opcua::map_iec_value(value) else {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "OPC UA client connection '{}': global '{}' has unsupported value {value:?}",
                connection.name, point.var
            )
            .into(),
        ));
    };
    if mapped.data_type != point.data_type {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "OPC UA client connection '{}': global '{}' is {}, but point expects {}",
                connection.name,
                point.var,
                mapped.data_type.as_config_value(),
                point.data_type.as_config_value()
            )
            .into(),
        ));
    }
    if point.access == OpcUaClientPointAccess::Write {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "OPC UA client connection '{}': point '{}' must be readable before runtime can report live status",
                connection.name, point.var
            )
            .into(),
        ));
    }
    Ok(())
}

fn set_point_status(
    statuses: &mut [OpcUaClientPointStatus],
    point: &OpcUaClientPointConfig,
    state: OpcUaClientConnectionState,
    last_seen_ms: Option<u64>,
    value: Option<Value>,
    detail: &str,
) {
    if let Some(status) = statuses
        .iter_mut()
        .find(|status| status.var == point.var && status.node_id == point.node_id)
    {
        status.state = state;
        status.last_seen_ms = last_seen_ms;
        if value.is_some() {
            status.value = value;
        }
        status.detail = detail.to_string();
    }
}
