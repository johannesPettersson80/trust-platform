//! Runtime settings snapshot and updates.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::config::{
    MeshRole, RuntimeCloudLinkPreferenceRule, RuntimeCloudProfile, RuntimeCloudWanAllowRule,
    RuntimeConfig, WebAuthMode,
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

    /// Build the runtime settings snapshot used by status/control surfaces from parsed config.
    #[must_use]
    pub fn from_runtime_config(
        config: &RuntimeConfig,
        simulation_enabled: bool,
        time_scale: u32,
    ) -> Self {
        let mut settings = Self::new(
            config.cycle_interval,
            BaseSettings {
                log_level: config.log_level.clone(),
                watchdog: config.watchdog,
                fault_policy: config.fault_policy,
                retain_mode: config.retain_mode,
                retain_save_interval: Some(config.retain_save_interval),
            },
            WebSettings {
                enabled: config.web.enabled,
                listen: config.web.listen.clone(),
                auth: SmolStr::new(match config.web.auth {
                    WebAuthMode::Local => "local",
                    WebAuthMode::Token => "token",
                }),
                tls: config.web.tls,
            },
            DiscoverySettings {
                enabled: config.discovery.enabled,
                service_name: config.discovery.service_name.clone(),
                advertise: config.discovery.advertise,
                interfaces: config.discovery.interfaces.clone(),
                host_group: config.discovery.host_group.clone(),
            },
            MeshSettings {
                enabled: config.mesh.enabled,
                role: config.mesh.role,
                listen: config.mesh.listen.clone(),
                connect: config.mesh.connect.clone(),
                tls: config.mesh.tls,
                auth_token: config.mesh.auth_token.clone(),
                publish: config.mesh.publish.clone(),
                subscribe: config.mesh.subscribe.clone(),
                zenohd_version: config.mesh.zenohd_version.clone(),
                plugin_versions: config.mesh.plugin_versions.clone(),
            },
            SimulationSettings {
                enabled: simulation_enabled,
                time_scale,
                mode_label: SmolStr::new(if simulation_enabled {
                    "simulation"
                } else {
                    "production"
                }),
                warning: SmolStr::new(""),
            },
        );
        settings.execution_backend = config.execution_backend;
        settings.execution_backend_source = config.execution_backend_source;
        settings.opcua = OpcUaSettings {
            enabled: config.opcua.enabled,
            listen: config.opcua.listen.clone(),
            endpoint_path: config.opcua.endpoint_path.clone(),
            namespace_uri: config.opcua.namespace_uri.clone(),
            publish_interval_ms: config.opcua.publish_interval_ms,
            max_nodes: config.opcua.max_nodes,
            expose: config.opcua.expose.clone(),
            security_policy: SmolStr::new(config.opcua.security.policy.as_config_value()),
            security_mode: SmolStr::new(config.opcua.security.mode.as_config_value()),
            allow_anonymous: config.opcua.security.allow_anonymous,
            username_set: config.opcua.username.is_some(),
        };
        settings.runtime_cloud.profile = config.runtime_cloud_profile;
        settings.runtime_cloud.wan_allow_write = config.runtime_cloud_wan_allow_write.clone();
        settings.runtime_cloud.link_preferences = config.runtime_cloud_link_preferences.clone();
        settings.realtime = config.realtime.clone();
        settings
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
