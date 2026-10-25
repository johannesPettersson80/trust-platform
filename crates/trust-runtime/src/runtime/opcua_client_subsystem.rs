//! OPC UA client subsystem owned by the runtime scan cycle.

use crate::error::RuntimeError;
use crate::opcua::{
    OpcUaClientBridge, OpcUaClientConnectionConfig, OpcUaClientConnectionState,
    OpcUaClientConnectionStatus, OpcUaClientStatusReport, OpcUaClientWorkerThread,
};

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

    /// [`Self::apply_inputs_with_cycle`] for a compiled program, after its drivers read the
    /// inputs (specification 44 §13.3).
    pub(super) fn apply_inputs_compiled(
        &mut self,
        variables: &mut dyn trust_runtime_core::native::variables::CompiledVariables,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .apply_inputs_compiled(variables, now_ms)
                .map_err(|err| opcua_runtime_error(&connection.config.name, err))?;
        }
        Ok(())
    }

    /// [`Self::capture_outputs_with_cycle`] for a compiled program, after its scan.
    pub(super) fn capture_outputs_compiled(
        &mut self,
        variables: &mut dyn trust_runtime_core::native::variables::CompiledVariables,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .capture_outputs_compiled(variables, now_ms)
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
