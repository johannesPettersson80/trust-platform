//! EtherCAT I/O driver (EtherCAT backend v1).

#![allow(missing_docs)]

use std::collections::VecDeque;
use std::time::{Duration as StdDuration, Instant};

#[cfg(all(feature = "ethercat-wire", unix))]
use ethercrab::std::{ethercat_now, tx_rx_task};
#[cfg(all(feature = "ethercat-wire", unix))]
use ethercrab::{
    subdevice_group::Op, MainDevice, MainDeviceConfig, PduStorage, SubDeviceGroup, Timeouts,
};
use serde::Deserialize;
use smol_str::SmolStr;
#[cfg(all(feature = "ethercat-wire", unix))]
use std::sync::{Arc, Mutex};
#[cfg(all(feature = "ethercat-wire", unix))]
use tokio::runtime::Runtime as TokioRuntime;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverErrorPolicy, IoDriverHealth};

include!("ethercat/models.rs");
include!("ethercat/mock_bus.rs");
include!("ethercat/ethercrab_bus.rs");
include!("ethercat/driver.rs");
include!("ethercat/config.rs");
include!("ethercat/tests.rs");
#[cfg(test)]
include!("ethercat/trace_cases.rs");

/// Public, sanitized EtherCAT module metadata for authoring and topology views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthercatModuleInfo {
    pub model: String,
    pub slot: u16,
    pub channels: u16,
}

/// Public, sanitized EtherCAT discovery result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthercatDiscoveryInfo {
    pub modules: Vec<EthercatModuleInfo>,
    pub input_bytes: usize,
    pub output_bytes: usize,
}

/// Parse configured EtherCAT modules with the same defaults and validation used by the driver.
pub fn configured_ethercat_modules(
    params: &toml::Value,
) -> Result<Vec<EthercatModuleInfo>, RuntimeError> {
    let config = EthercatConfig::from_params(params)?;
    Ok(config.modules.iter().map(module_info).collect())
}

/// Discover EtherCAT modules from the runtime host.
///
/// This opens the configured adapter and performs read-only EtherCAT discovery. It never writes PDO
/// outputs or changes configured project files.
pub fn discover_ethercat_modules(
    params: &toml::Value,
) -> Result<EthercatDiscoveryInfo, RuntimeError> {
    let config = EthercatConfig::from_params(params)?;
    let mut bus = build_bus(&config)?;
    let discovery = bus.discover(&config)?;
    Ok(EthercatDiscoveryInfo {
        modules: discovery.modules.iter().map(module_info).collect(),
        input_bytes: discovery.input_bytes,
        output_bytes: discovery.output_bytes,
    })
}

fn module_info(module: &EthercatModuleConfig) -> EthercatModuleInfo {
    EthercatModuleInfo {
        model: module.model.to_string(),
        slot: module.slot,
        channels: module.channels,
    }
}
