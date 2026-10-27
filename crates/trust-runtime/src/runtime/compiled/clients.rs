//! The protocol clients of a compiled program on a computer (specification 11's ADS and OPC UA
//! clients, the OpenOT telemetry; specification 44 §13.3): their points bind the program's globals
//! by name, and their samples and outputs cross at the boundaries of its scan, as they do for the
//! bytecode engine; the telemetry reads its producers by their full names after the scan.

use trust_runtime_core::native::variables::CompiledVariables;

use super::super::opcua_client_subsystem::OpcUaClientSubsystem;
use super::named::{self, NamedVariable};
use super::CompiledRuntime;
use crate::config::OpenOtTelemetryConfig;
use crate::error::RuntimeError;
use crate::value::Value;

impl CompiledRuntime {
    /// The global `name` names, as a protocol client binds a point to it; none for a name that
    /// names no global.
    #[must_use]
    pub fn global(&self, name: &str) -> Option<NamedVariable> {
        named::global(&self.image.image(), name)
    }

    /// The program instance's variable `name`, `Instance.Variable`, names, as an MQTT tag binds
    /// to it; none for a name that names no program instance's own variable.
    #[must_use]
    pub fn program_variable(&self, name: &str) -> Option<NamedVariable> {
        named::program_variable(&self.image.image(), name)
    }

    /// The value of the global `name` names, as the last scan left it.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UndefinedVariable`] for a name that names no global, and the error of a
    /// global that holds no value.
    pub fn read_global(&self, name: &str) -> Result<Value, RuntimeError> {
        let global = self
            .global(name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.into()))?;
        self.cycle.program().read_named(global.name.as_str())
    }

    /// Records the hash of the ADS client configuration, which the status reports.
    pub fn set_ads_deployed_config_hash(&mut self, hash: Option<String>) {
        self.ads.set_deployed_ads_config_hash(hash);
    }

    /// Registers and starts the ADS client connection `connection`, whose points bind this
    /// program's globals.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidConfig`] for a point the program does not hold or whose type does
    /// not fit it, and [`RuntimeError::IoTransport`] for a connection whose worker does not start.
    pub fn start_ads_connection<T>(
        &mut self,
        connection: &crate::ads::AdsConnectionConfig,
        transport: T,
        worker_tick_interval: std::time::Duration,
    ) -> Result<(), RuntimeError>
    where
        T: crate::ads::AdsTransport + Send + 'static,
    {
        let failed = |err: &dyn std::fmt::Display| {
            format!("ADS connection '{}': {err}", connection.route.name)
        };
        let bindings = crate::ads::resolve_compiled_bindings(self, connection)
            .map_err(|err| RuntimeError::InvalidConfig(failed(&err).into()))?;
        let (bridge, worker) = crate::ads::AdsConnectionBridge::with_transport(transport, bindings)
            .map_err(|err| RuntimeError::IoTransport(failed(&err).into()))?;
        let worker = worker
            .spawn(worker_tick_interval)
            .map_err(|err| RuntimeError::IoTransport(failed(&err).into()))?;
        self.ads
            .add_connection(connection.route.clone(), bridge, worker);
        Ok(())
    }

    /// Configures the OpenOT telemetry, whose producers' outputs are read by their full names
    /// after each scan.
    ///
    /// # Errors
    ///
    /// The refusal of a configuration the telemetry does not take.
    pub fn configure_openot_telemetry(
        &mut self,
        config: &OpenOtTelemetryConfig,
        bundle_root: Option<&std::path::Path>,
    ) -> Result<(), RuntimeError> {
        self.openot_telemetry.configure(config, bundle_root)
    }

    /// Stops and forgets every OPC UA client connection.
    ///
    /// # Errors
    ///
    /// The error of a worker that does not stop.
    pub fn reset_opcua_client_connections(&mut self) -> Result<(), RuntimeError> {
        self.opcua_client.shutdown()?;
        self.opcua_client = OpcUaClientSubsystem::new();
        Ok(())
    }

    /// Records the hash of the OPC UA client configuration, which the status reports.
    pub fn set_opcua_client_deployed_config_hash(&mut self, hash: Option<String>) {
        self.opcua_client.set_deployed_config_hash(hash);
    }

    /// Registers and starts the OPC UA client connection `connection`, whose points bind this
    /// program's globals.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidConfig`] for a point the program does not hold or whose value does
    /// not fit it, and [`RuntimeError::IoTransport`] for a connection whose worker does not start.
    pub fn start_opcua_client_connection<T>(
        &mut self,
        connection: &crate::opcua::OpcUaClientConnectionConfig,
        transport: T,
        worker_tick_interval: std::time::Duration,
    ) -> Result<(), RuntimeError>
    where
        T: crate::opcua::OpcUaClientTransport + Send + 'static,
    {
        let bindings = crate::opcua::resolve_compiled_opcua_client_bindings(self, connection)?;
        let failed = |err: &dyn std::fmt::Display| {
            RuntimeError::IoTransport(
                format!("OPC UA client connection '{}': {err}", connection.name).into(),
            )
        };
        let (bridge, worker) = crate::opcua::OpcUaClientBridge::with_transport(
            connection.clone(),
            transport,
            bindings,
        )
        .map_err(|err| failed(&err))?;
        let worker = worker
            .spawn(worker_tick_interval)
            .map_err(|err| failed(&err))?;
        self.opcua_client
            .add_connection(connection.clone(), bridge, Some(worker));
        Ok(())
    }
}
