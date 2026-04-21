//! Runtime bundle configuration loading.

#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use glob::Pattern;
use indexmap::IndexMap;
use serde::Deserialize;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::execution_backend::{ExecutionBackend, ExecutionBackendSource};
use crate::historian::{AlertRule, HistorianConfig, RecordingMode};
use crate::io::{IoAddress, IoSafeState, IoSize};
use crate::linux_rt::{LinuxRtConfig, LinuxRtSchedulerPolicy};
use crate::opcua::{
    OpcUaMessageSecurityMode, OpcUaRuntimeConfig, OpcUaSecurityPolicy, OpcUaSecurityProfile,
};
use crate::simulation::SimulationConfig;
use crate::value::Duration;
use crate::value::Value;
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};

mod parser;

pub use parser::ControlMode;

#[cfg(unix)]
pub const SYSTEM_IO_CONFIG_PATH: &str = "/etc/trust/io.toml";
#[cfg(windows)]
pub const SYSTEM_IO_CONFIG_PATH: &str = r"C:\ProgramData\truST\io.toml";

include!("config/contracts.rs");
include!("config/loaders.rs");

#[cfg(test)]
mod tests;
