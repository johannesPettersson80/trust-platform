//! ADS server write-back into runtime variables.

use glob::Pattern;
use trust_ads_core::{value_from_ads_bytes, SymbolDescriptor};
use trust_ads_server::{AdsErrorCode, AdsServerError, ClientId, RuntimeWritePort};

use crate::debug::DebugControl;
use crate::scheduler::{ResourceControl, ResourceState, StdClock};

use super::contracts::AdsServerRuntimeConfig;
use super::policy::AdsServerClientPolicy;
use super::symbols::global_name_for_server_symbol;

/// ADS server runtime write port.
#[derive(Clone)]
pub struct AdsServerRuntimeWritePort {
    config: AdsServerRuntimeConfig,
    policy: AdsServerClientPolicy,
    debug: DebugControl,
    resource: ResourceControl<StdClock>,
    writable_patterns: Vec<Pattern>,
}

impl AdsServerRuntimeWritePort {
    /// Creates a runtime write port.
    ///
    /// # Errors
    ///
    /// Returns an ADS access error when configured writable globs are invalid.
    pub fn new(
        config: AdsServerRuntimeConfig,
        policy: AdsServerClientPolicy,
        debug: DebugControl,
        resource: ResourceControl<StdClock>,
    ) -> Result<Self, AdsServerError> {
        let writable_patterns = config
            .writable
            .iter()
            .map(|pattern| {
                Pattern::new(pattern.as_str()).map_err(|err| {
                    AdsServerError::device(
                        AdsErrorCode::InvalidParameter,
                        format!(
                            "runtime.ads_server.writable invalid pattern '{}': {err}",
                            pattern
                        ),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            config,
            policy,
            debug,
            resource,
            writable_patterns,
        })
    }
}

impl RuntimeWritePort for AdsServerRuntimeWritePort {
    fn write(
        &self,
        symbol: &SymbolDescriptor,
        bytes: &[u8],
        client: &ClientId,
    ) -> Result<(), AdsServerError> {
        if !self.policy.permits_client(client) {
            return Err(access_denied("ADS client is not allowed"));
        }
        if !self.config.writes_enabled {
            return Err(access_denied("runtime.ads_server.writes_enabled is false"));
        }
        if !self.symbol_is_writable(symbol) {
            return Err(access_denied(format!(
                "ADS symbol '{}' is not in runtime.ads_server.writable",
                symbol.name
            )));
        }
        if !self.runtime_can_accept_writes() {
            return Err(AdsServerError::device(
                AdsErrorCode::NotReady,
                "runtime is not accepting ADS server writes",
            ));
        }
        let global_name = global_name_for_server_symbol(&self.config, symbol.name.as_str())
            .ok_or_else(|| {
                AdsServerError::device(
                    AdsErrorCode::NotFound,
                    format!(
                        "ADS symbol '{}' is not a writable runtime global",
                        symbol.name
                    ),
                )
            })?;
        let value = value_from_ads_bytes(&symbol.data_type, bytes).map_err(|err| {
            AdsServerError::device(
                AdsErrorCode::InvalidData,
                format!("failed to decode ADS write for '{}': {err}", symbol.name),
            )
        })?;
        self.debug
            .try_enqueue_global_write(global_name, value)
            .map_err(|refused| {
                // The bounded request queue is full, so the device cannot take the write now.
                // NoMemory is ADS's code for an exhausted resource cap, which the queue bound is.
                AdsServerError::device(AdsErrorCode::NoMemory, refused.to_string())
            })?;
        Ok(())
    }
}

impl AdsServerRuntimeWritePort {
    fn runtime_can_accept_writes(&self) -> bool {
        if self.resource.last_error().is_some() {
            return false;
        }
        !matches!(
            self.resource.state(),
            ResourceState::Boot | ResourceState::Faulted | ResourceState::Stopped
        )
    }

    fn symbol_is_writable(&self, symbol: &SymbolDescriptor) -> bool {
        let Some(global_name) = global_name_for_server_symbol(&self.config, symbol.name.as_str())
        else {
            return false;
        };
        self.writable_patterns
            .iter()
            .any(|pattern| pattern.matches(symbol.name.as_str()) || pattern.matches(global_name))
    }
}

fn access_denied(message: impl Into<String>) -> AdsServerError {
    AdsServerError::device(AdsErrorCode::AccessDenied, message)
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use trust_ads_core::{ads_bytes_from_value, AmsNetId};

    use super::super::{
        build_runtime_symbol_snapshot, AdsServerClientConfig, AdsServerClientPolicy,
        AdsServerRuntimeConfig, AdsServerSourcePin,
    };
    use super::*;
    use crate::debug::PendingVarTarget;
    use crate::memory::VariableStorage;
    use crate::value::{Duration, Value};

    fn config() -> AdsServerRuntimeConfig {
        AdsServerRuntimeConfig {
            enabled: true,
            listen: Some(SmolStr::new("192.168.10.20")),
            ads_port: 851,
            ams_net_id: Some(AmsNetId::new("192.168.10.20.1.1")),
            insecure_transport: true,
            writes_enabled: true,
            expose: vec![SmolStr::new("global.*")],
            writable: vec![SmolStr::new("global.setpoint")],
            clients: vec![AdsServerClientConfig {
                ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
                source: AdsServerSourcePin::Cidr(SmolStr::new("192.168.10.0/24")),
            }],
            ..AdsServerRuntimeConfig::default()
        }
    }

    fn snapshot_with_setpoint() -> crate::debug::DebugSnapshot {
        let mut storage = VariableStorage::new();
        storage.set_global("setpoint", Value::Real(0.0));
        crate::debug::DebugSnapshot {
            storage_abi: Default::default(),
            storage,
            now: Duration::from_millis(10),
        }
    }

    #[test]
    fn write_port_refuses_a_full_request_queue_without_mutation() {
        let config = config();
        let symbols = build_runtime_symbol_snapshot(&config, &snapshot_with_setpoint())
            .expect("symbol snapshot");
        let symbol = symbols.symbols.first().expect("symbol").clone();
        let policy = AdsServerClientPolicy::new(&config);
        let debug = DebugControl::new();
        let (resource, _rx) = ResourceControl::stub(StdClock::new());
        let write_port = AdsServerRuntimeWritePort::new(config, policy, debug.clone(), resource)
            .expect("write port");
        for index in 0..1_024 {
            debug
                .try_enqueue_global_write(format!("filler{index}"), Value::Int(0))
                .expect("a filler fits the bound");
        }

        let client = ClientId::new(AmsNetId::new("5.23.91.12.1.1")).with_source_ip("192.168.10.50");
        let error = write_port
            .write(
                &symbol,
                &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
                &client,
            )
            .expect_err("a full request queue refuses the ADS write");

        assert_eq!(error.code(), AdsErrorCode::NoMemory);
        assert_eq!(error.message(), "request queue full");
        let queued = debug.drain_var_writes();
        assert_eq!(queued.len(), 1_024, "the refused write must not be queued");
        assert!(queued.iter().all(|write| !matches!(
            &write.target,
            PendingVarTarget::Global(name) if name == "setpoint"
        )));
    }
}
