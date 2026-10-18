//! EtherCAT I/O driver (EtherCAT backend v1).

#![allow(missing_docs)]

use std::collections::VecDeque;
use std::time::{Duration as StdDuration, Instant};

#[cfg(all(feature = "ethercat-wire", unix))]
use ethercrab::std::{ethercat_now, tx_rx_task};
#[cfg(all(feature = "ethercat-wire", unix))]
use ethercrab::{
    subdevice_group::{HasPdi, Op},
    DefaultLock, MainDevice, MainDeviceConfig, PduStorage, SubDeviceGroup, Timeouts,
};
use serde::Deserialize;
use smol_str::SmolStr;
#[cfg(all(feature = "ethercat-wire", unix))]
use std::collections::HashMap;
#[cfg(all(feature = "ethercat-wire", unix))]
use std::sync::{Arc, Mutex, OnceLock};
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
#[cfg(all(test, feature = "ethercat-wire", unix))]
#[path = "ethercat/identity_tests.rs"]
mod identity_tests;
#[cfg(all(test, feature = "ethercat-wire", unix))]
include!("ethercat/trace_cases.rs");

/// Public, sanitized EtherCAT module metadata for authoring and topology views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthercatModuleInfo {
    pub model: String,
    pub slot: u16,
    pub channels: u16,
}

/// One SubDevice that an identification read (specification 11, "Identity"): its position and
/// addresses on the bus and its identity. Crate-internal: a search turns it into a find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EthercatSubDeviceInfo {
    pub(crate) model: String,
    pub(crate) slot: u16,
    pub(crate) channels: u16,
    pub(crate) configured_address: u16,
    pub(crate) alias_address: u16,
    pub(crate) vendor_id: u32,
    pub(crate) product_id: u32,
    pub(crate) revision: u32,
    pub(crate) serial: u32,
    pub(crate) input_bytes: usize,
    pub(crate) output_bytes: usize,
}

/// Public, sanitized EtherCAT discovery result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthercatDiscoveryInfo {
    pub modules: Vec<EthercatModuleInfo>,
    pub(crate) subdevices: Vec<EthercatSubDeviceInfo>,
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
/// This opens the configured adapter and performs read-only EtherCAT identification. It maps the
/// process data in PRE-OP and never brings the bus to SAFE-OP or OP, so no output changes, and it
/// never writes PDO outputs or changes configured project files.
pub fn discover_ethercat_modules(
    params: &toml::Value,
) -> Result<EthercatDiscoveryInfo, RuntimeError> {
    let config = EthercatConfig::from_params(params)?;
    let discovery = identify_ethercat_subdevices(&config, open_bus)?;
    Ok(EthercatDiscoveryInfo {
        modules: discovery
            .subdevices
            .iter()
            .map(subdevice_module_info)
            .collect(),
        subdevices: discovery.subdevices.iter().map(subdevice_info).collect(),
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

fn subdevice_module_info(subdevice: &EthercatSubDevice) -> EthercatModuleInfo {
    EthercatModuleInfo {
        model: subdevice.name.to_string(),
        slot: subdevice.slot,
        channels: subdevice.channels,
    }
}

fn subdevice_info(subdevice: &EthercatSubDevice) -> EthercatSubDeviceInfo {
    EthercatSubDeviceInfo {
        model: subdevice.name.to_string(),
        slot: subdevice.slot,
        channels: subdevice.channels,
        configured_address: subdevice.configured_address,
        alias_address: subdevice.alias_address,
        vendor_id: subdevice.vendor_id,
        product_id: subdevice.product_id,
        revision: subdevice.revision,
        serial: subdevice.serial,
        input_bytes: subdevice.input_bytes,
        output_bytes: subdevice.output_bytes,
    }
}
