//! Hosted transport and observation adapters around admitted PLC state.
use super::Runtime;
use crate::{error, value::Value};

impl Runtime {
    /// Register and start one ADS client connection.
    pub fn start_ads_connection<T>(
        &mut self,
        connection: &crate::ads::AdsConnectionConfig,
        transport: T,
        worker_tick_interval: std::time::Duration,
    ) -> Result<(), error::RuntimeError>
    where
        T: crate::ads::AdsTransport + Send + 'static,
    {
        let bindings = crate::ads::resolve_declared_bindings(self, connection).map_err(|err| {
            error::RuntimeError::InvalidConfig(
                format!("ADS connection '{}': {err}", connection.route.name).into(),
            )
        })?;
        let (bridge, worker) = crate::ads::AdsConnectionBridge::with_transport(transport, bindings)
            .map_err(|err| {
                error::RuntimeError::IoTransport(
                    format!("ADS connection '{}': {err}", connection.route.name).into(),
                )
            })?;
        let worker = worker.spawn(worker_tick_interval).map_err(|err| {
            error::RuntimeError::IoTransport(
                format!("ADS connection '{}': {err}", connection.route.name).into(),
            )
        })?;
        self.ads
            .add_connection(connection.route.clone(), bridge, worker);
        Ok(())
    }

    /// Stop all active ADS client workers.
    pub fn shutdown_ads(&mut self) -> Result<(), error::RuntimeError> {
        self.ads.shutdown()
    }

    /// Number of configured ADS client connections.
    #[must_use]
    pub fn ads_connection_count(&self) -> usize {
        self.ads.connection_count()
    }

    /// Record the ADS config hash loaded by the runtime bundle.
    pub fn set_ads_deployed_config_hash(&mut self, hash: Option<String>) {
        self.ads.set_deployed_ads_config_hash(hash);
    }

    /// Current ADS client status projection.
    #[must_use]
    pub fn ads_status_report(&self) -> crate::ads::diagnostics::AdsStatusReport {
        self.ads.status_report()
    }

    /// Imported ADS globals projected into the existing Live Values surface.
    #[must_use]
    pub fn ads_live_values(&self) -> Vec<crate::ads::AdsLiveValue> {
        let mut values = self.ads.live_values(self.storage());
        let forced_globals = self
            .debug_control()
            .as_ref()
            .map(|debug| {
                debug
                    .forced_snapshot()
                    .vars
                    .into_iter()
                    .filter_map(|forced| match forced.target {
                        crate::debug::ForcedVarTarget::Global(name) => Some(name),
                        _ => None,
                    })
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        for value in &mut values {
            value.forced = forced_globals.contains(value.point_name.as_str());
        }
        values
    }

    /// Commit a checked management value and its qualified ADS queue entry.
    /// Read-only/type/deadline rejection happens before either publication. A
    /// normal non-ADS global follows the same checked value operation alone.
    pub fn write_global_and_queue_ads(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<(), error::RuntimeError> {
        match self.ads.writable_live_bridge(name)? {
            Some(bridge) => {
                let canonical = self.state.write_global_canonical(name, value)?;
                bridge.commit_qualified_live_write(name, canonical);
                Ok(())
            }
            None => self.state.write_global(name, value),
        }
    }

    /// Queue a write for a writable imported ADS global, if one owns this name.
    pub fn queue_ads_live_write(&mut self, point_name: &str, value: Value) -> bool {
        self.ads.queue_live_write(point_name, value)
    }

    /// Whether an imported ADS global exists and permits writes to the PLC.
    #[must_use]
    pub fn ads_live_value_writable(&self, point_name: &str) -> Option<bool> {
        self.ads.live_value_writable(point_name)
    }

    /// Configure OPC UA client connections for this runtime.
    pub fn configure_opcua_client(
        &mut self,
        config: &crate::opcua::OpcUaClientConfig,
    ) -> Result<(), error::RuntimeError> {
        let mut subsystem = super::opcua_client_subsystem::OpcUaClientSubsystem::new();
        subsystem.configure(self, config)?;
        self.opcua_client = subsystem;
        Ok(())
    }

    /// Register and start one OPC UA client connection.
    pub fn start_opcua_client_connection<T>(
        &mut self,
        connection: &crate::opcua::OpcUaClientConnectionConfig,
        transport: T,
        worker_tick_interval: std::time::Duration,
    ) -> Result<(), error::RuntimeError>
    where
        T: crate::opcua::OpcUaClientTransport + Send + 'static,
    {
        let bindings = crate::opcua::resolve_opcua_client_bindings(self, connection)?;
        let (bridge, worker) = crate::opcua::OpcUaClientBridge::with_transport(
            connection.clone(),
            transport,
            bindings,
        )
        .map_err(|err| {
            error::RuntimeError::IoTransport(
                format!("OPC UA client connection '{}': {err}", connection.name).into(),
            )
        })?;
        let worker = worker.spawn(worker_tick_interval).map_err(|err| {
            error::RuntimeError::IoTransport(
                format!("OPC UA client connection '{}': {err}", connection.name).into(),
            )
        })?;
        self.opcua_client
            .add_connection(connection.clone(), bridge, Some(worker));
        Ok(())
    }

    /// Clear and stop active OPC UA client workers.
    pub fn reset_opcua_client_connections(&mut self) -> Result<(), error::RuntimeError> {
        self.opcua_client.shutdown()?;
        self.opcua_client = super::opcua_client_subsystem::OpcUaClientSubsystem::new();
        Ok(())
    }

    /// Record the OPC UA client config hash loaded by the runtime bundle.
    pub fn set_opcua_client_deployed_config_hash(&mut self, hash: Option<String>) {
        self.opcua_client.set_deployed_config_hash(hash);
    }

    /// Number of configured OPC UA client connections.
    #[must_use]
    pub fn opcua_client_connection_count(&self) -> usize {
        self.opcua_client.connection_count()
    }

    /// Current OPC UA client status projection.
    #[must_use]
    pub fn opcua_client_status_report(&self) -> crate::opcua::OpcUaClientStatusReport {
        self.opcua_client.status_report()
    }

    /// Snapshot for a live ADS device that overlaps a doctor target.
    #[must_use]
    pub fn active_ads_device_snapshot(
        &self,
        target: &crate::ads::diagnostics::TargetIdentity,
        local: Option<&crate::ads::diagnostics::LocalIdentity>,
    ) -> Option<crate::ads::onboarding::ActiveAdsDeviceSnapshot> {
        self.ads.active_device_snapshot(target, local)
    }

    /// Read the completed cycle count without exposing scheduler state.
    pub fn cycle_counter(&self) -> u64 {
        self.cycle_counter
    }

    /// Attach the existing hosted metrics sink to this composition.
    pub fn set_metrics_sink(
        &mut self,
        sink: std::sync::Arc<std::sync::Mutex<crate::metrics::RuntimeMetrics>>,
    ) {
        self.metrics.set_sink(sink);
        self.metrics.set_execution_backend(self.execution_backend());
    }

    /// Configure publication of the admitted controller's hosted telemetry.
    pub fn configure_openot_telemetry(
        &mut self,
        config: &crate::config::OpenOtTelemetryConfig,
        bundle_root: Option<&std::path::Path>,
    ) -> Result<(), error::RuntimeError> {
        self.openot_telemetry.configure(config, bundle_root)
    }
}
