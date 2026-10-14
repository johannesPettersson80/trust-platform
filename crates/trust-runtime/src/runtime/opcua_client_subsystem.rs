//! OPC UA client subsystem owned by the runtime scan cycle.

use crate::error::RuntimeError;
#[cfg(test)]
use crate::memory::VariableStorage;
use crate::opcua::{
    resolve_opcua_client_bindings, OpcUaClientBridge, OpcUaClientConfig,
    OpcUaClientConnectionConfig, OpcUaClientConnectionState, OpcUaClientConnectionStatus,
    OpcUaClientStatusReport, OpcUaClientWorkerThread,
};

use super::Runtime;

pub(super) struct OpcUaClientSubsystem {
    connections: Vec<OpcUaRuntimeClientConnection>,
    deployed_config_hash: Option<String>,
}

struct OpcUaRuntimeClientConnection {
    config: OpcUaClientConnectionConfig,
    bridge: OpcUaClientBridge,
    worker: Option<OpcUaClientWorkerThread>,
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
        self.shutdown()?;
        self.connections.clear();
        for connection in &config.connections {
            let bindings = resolve_opcua_client_bindings(runtime, connection)?;
            let bridge = OpcUaClientBridge::new(bindings).map_err(|err| {
                RuntimeError::IoTransport(
                    format!("OPC UA client connection '{}': {err}", connection.name).into(),
                )
            })?;
            self.add_connection(connection.clone(), bridge, None);
        }
        Ok(())
    }

    pub(super) fn add_connection(
        &mut self,
        config: OpcUaClientConnectionConfig,
        bridge: OpcUaClientBridge,
        worker: Option<OpcUaClientWorkerThread>,
    ) {
        self.connections.push(OpcUaRuntimeClientConnection {
            config,
            bridge,
            worker,
        });
    }

    pub(super) fn set_deployed_config_hash(&mut self, hash: Option<String>) {
        self.deployed_config_hash = hash;
    }

    pub(super) fn prepare_rebind(
        &self,
        candidate: &crate::Runtime,
    ) -> Result<Vec<Vec<crate::opcua::OpcUaClientBinding>>, RuntimeError> {
        self.connections
            .iter()
            .map(|connection| resolve_opcua_client_bindings(candidate, &connection.config))
            .collect()
    }

    pub(super) fn commit_rebind(&mut self, bindings: Vec<Vec<crate::opcua::OpcUaClientBinding>>) {
        for (connection, bindings) in self.connections.iter_mut().zip(bindings) {
            connection.bridge.replace_bindings(bindings);
        }
    }

    pub(super) fn connection_count(&self) -> usize {
        self.connections.len()
    }

    #[cfg(test)]
    pub(super) fn apply_inputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .apply_inputs(storage, now_ms)
                .map_err(|err| opcua_runtime_error(&connection.config.name, err))?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn capture_outputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .capture_outputs(storage, now_ms)
                .map_err(|err| opcua_runtime_error(&connection.config.name, err))?;
        }
        Ok(())
    }

    pub(super) fn apply_inputs_with_cycle(
        &mut self,
        access: &mut trust_runtime_core::vm::CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .apply_inputs_with_cycle(access, now_ms)
                .map_err(|err| opcua_runtime_error(&connection.config.name, err))?;
        }
        Ok(())
    }

    pub(super) fn capture_outputs_with_cycle(
        &mut self,
        access: &mut trust_runtime_core::vm::CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .capture_outputs_with_cycle(access, now_ms)
                .map_err(|err| opcua_runtime_error(&connection.config.name, err))?;
        }
        Ok(())
    }

    pub(super) fn status_report(&self) -> OpcUaClientStatusReport {
        OpcUaClientStatusReport {
            enabled: !self.connections.is_empty(),
            deployed_config_hash: self.deployed_config_hash.clone(),
            connections: self.connections.iter().map(connection_status).collect(),
        }
    }

    pub(super) fn shutdown(&mut self) -> Result<(), RuntimeError> {
        let mut first_error = None;
        for connection in &mut self.connections {
            if let Some(worker) = connection.worker.take()
                && let Err(err) = worker.shutdown()
            {
                first_error
                    .get_or_insert_with(|| opcua_runtime_error(&connection.config.name, err));
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for OpcUaClientSubsystem {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn connection_status(connection: &OpcUaRuntimeClientConnection) -> OpcUaClientConnectionStatus {
    let snapshot = connection.bridge.snapshot();
    let degraded_points = snapshot
        .point_statuses
        .iter()
        .filter(|status| status.state != OpcUaClientConnectionState::Connected)
        .count();
    OpcUaClientConnectionStatus {
        name: connection.config.name.clone(),
        endpoint_url: connection.config.endpoint_url.clone(),
        state: snapshot.state,
        point_count: snapshot.point_statuses.len(),
        degraded_points,
        last_seen_ms: snapshot.last_seen_ms,
        detail: snapshot.detail,
        points: snapshot.point_statuses,
    }
}

fn opcua_runtime_error(
    connection: &smol_str::SmolStr,
    err: crate::opcua::OpcUaClientBridgeError,
) -> RuntimeError {
    if let Some(error) = err.execution_cause() {
        return error.clone();
    }
    RuntimeError::IoTransport(format!("OPC UA client connection '{connection}': {err}").into())
}
