//! Runtime settings snapshot and updates.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::config::{
    MeshRole, RuntimeCloudLinkPreferenceRule, RuntimeCloudProfile, RuntimeCloudWanAllowRule,
};
use crate::execution_backend::{ExecutionBackend, ExecutionBackendSource};
use crate::linux_rt::LinuxRtConfig;
use crate::value::Duration;
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub cycle_interval: Duration,
    pub execution_backend: ExecutionBackend,
    pub execution_backend_source: ExecutionBackendSource,
    pub log_level: SmolStr,
    pub watchdog: WatchdogPolicy,
    pub fault_policy: FaultPolicy,
    pub retain_mode: RetainMode,
    pub retain_save_interval: Option<Duration>,
    pub web: WebSettings,
    pub discovery: DiscoverySettings,
    pub mesh: MeshSettings,
    pub runtime_cloud: RuntimeCloudSettings,
    pub realtime: LinuxRtConfig,
    pub opcua: OpcUaSettings,
    pub simulation: SimulationSettings,
}

impl RuntimeSettings {
    pub fn new(
        cycle_interval: Duration,
        base: BaseSettings,
        web: WebSettings,
        discovery: DiscoverySettings,
        mesh: MeshSettings,
        simulation: SimulationSettings,
    ) -> Self {
        Self {
            cycle_interval,
            execution_backend: ExecutionBackend::BytecodeVm,
            execution_backend_source: ExecutionBackendSource::Default,
            log_level: base.log_level,
            watchdog: base.watchdog,
            fault_policy: base.fault_policy,
            retain_mode: base.retain_mode,
            retain_save_interval: base.retain_save_interval,
            web,
            discovery,
            mesh,
            runtime_cloud: RuntimeCloudSettings::default(),
            realtime: LinuxRtConfig::default(),
            opcua: OpcUaSettings::default(),
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
    pub tls: bool,
}

#[derive(Debug, Clone)]
pub struct DiscoverySettings {
    pub enabled: bool,
    pub service_name: SmolStr,
    pub advertise: bool,
    pub interfaces: Vec<SmolStr>,
    pub host_group: Option<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct MeshSettings {
    pub enabled: bool,
    pub role: MeshRole,
    pub listen: SmolStr,
    pub connect: Vec<SmolStr>,
    pub tls: bool,
    pub auth_token: Option<SmolStr>,
    pub publish: Vec<SmolStr>,
    pub subscribe: IndexMap<SmolStr, SmolStr>,
    pub zenohd_version: SmolStr,
    pub plugin_versions: IndexMap<SmolStr, SmolStr>,
}

#[derive(Debug, Clone)]
pub struct RuntimeCloudSettings {
    pub profile: RuntimeCloudProfile,
    pub wan_allow_write: Vec<RuntimeCloudWanAllowRule>,
    pub link_preferences: Vec<RuntimeCloudLinkPreferenceRule>,
}

impl Default for RuntimeCloudSettings {
    fn default() -> Self {
        Self {
            profile: RuntimeCloudProfile::Dev,
            wan_allow_write: Vec::new(),
            link_preferences: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpcUaSettings {
    pub enabled: bool,
    pub listen: SmolStr,
    pub endpoint_path: SmolStr,
    pub namespace_uri: SmolStr,
    pub publish_interval_ms: u64,
    pub max_nodes: usize,
    pub expose: Vec<SmolStr>,
    pub security_policy: SmolStr,
    pub security_mode: SmolStr,
    pub allow_anonymous: bool,
    pub username_set: bool,
}

impl Default for OpcUaSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: SmolStr::new("0.0.0.0:4840"),
            endpoint_path: SmolStr::new("/"),
            namespace_uri: SmolStr::new("urn:trust:runtime"),
            publish_interval_ms: 250,
            max_nodes: 128,
            expose: Vec::new(),
            security_policy: SmolStr::new("basic256sha256"),
            security_mode: SmolStr::new("sign_and_encrypt"),
            allow_anonymous: false,
            username_set: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimulationSettings {
    pub enabled: bool,
    pub time_scale: u32,
    pub mode_label: SmolStr,
    pub warning: SmolStr,
}
