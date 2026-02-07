//! Runtime settings snapshot and updates.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::value::Duration;
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub log_level: SmolStr,
    pub watchdog: WatchdogPolicy,
    pub fault_policy: FaultPolicy,
    pub retain_mode: RetainMode,
    pub retain_save_interval: Option<Duration>,
    pub web: WebSettings,
    pub discovery: DiscoverySettings,
    pub mesh: MeshSettings,
    pub simulation: SimulationSettings,
}

impl RuntimeSettings {
    pub fn new(
        base: BaseSettings,
        web: WebSettings,
        discovery: DiscoverySettings,
        mesh: MeshSettings,
        simulation: SimulationSettings,
    ) -> Self {
        Self {
            log_level: base.log_level,
            watchdog: base.watchdog,
            fault_policy: base.fault_policy,
            retain_mode: base.retain_mode,
            retain_save_interval: base.retain_save_interval,
            web,
            discovery,
            mesh,
            simulation,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BaseSettings {
    pub log_level: SmolStr,
    pub watchdog: WatchdogPolicy,
    pub fault_policy: FaultPolicy,
    pub retain_mode: RetainMode,
    pub retain_save_interval: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct WebSettings {
    pub enabled: bool,
    pub listen: SmolStr,
    pub auth: SmolStr,
}

#[derive(Debug, Clone)]
pub struct DiscoverySettings {
    pub enabled: bool,
    pub service_name: SmolStr,
    pub advertise: bool,
    pub interfaces: Vec<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct MeshSettings {
    pub enabled: bool,
    pub listen: SmolStr,
    pub auth_token: Option<SmolStr>,
    pub publish: Vec<SmolStr>,
    pub subscribe: IndexMap<SmolStr, SmolStr>,
}

#[derive(Debug, Clone)]
pub struct SimulationSettings {
    pub enabled: bool,
    pub time_scale: u32,
    pub mode_label: SmolStr,
    pub warning: SmolStr,
}
