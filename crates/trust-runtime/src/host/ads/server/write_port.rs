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
        self.debug.enqueue_global_write(global_name, value);
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
