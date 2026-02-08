//! Runtime bundle configuration loading.

#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use glob::Pattern;
use indexmap::IndexMap;
use serde::Deserialize;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::historian::{AlertRule, HistorianConfig, RecordingMode};
use crate::io::{IoAddress, IoSafeState, IoSize};
use crate::opcua::{
    OpcUaMessageSecurityMode, OpcUaRuntimeConfig, OpcUaSecurityPolicy, OpcUaSecurityProfile,
};
use crate::simulation::SimulationConfig;
use crate::value::Duration;
use crate::value::Value;
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};

#[cfg(unix)]
pub const SYSTEM_IO_CONFIG_PATH: &str = "/etc/trust/io.toml";
#[cfg(windows)]
pub const SYSTEM_IO_CONFIG_PATH: &str = r"C:\ProgramData\truST\io.toml";

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub bundle_version: u32,
    pub resource_name: SmolStr,
    pub cycle_interval: Duration,
    pub control_endpoint: SmolStr,
    pub control_auth_token: Option<SmolStr>,
    pub control_debug_enabled: bool,
    pub control_mode: ControlMode,
    pub log_level: SmolStr,
    pub retain_mode: RetainMode,
    pub retain_path: Option<PathBuf>,
    pub retain_save_interval: Duration,
    pub watchdog: WatchdogPolicy,
    pub fault_policy: FaultPolicy,
    pub web: WebConfig,
    pub tls: TlsConfig,
    pub deploy: DeployConfig,
    pub discovery: DiscoveryConfig,
    pub mesh: MeshConfig,
    pub observability: HistorianConfig,
    pub opcua: OpcUaRuntimeConfig,
    pub tasks: Option<Vec<TaskOverride>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAuthMode {
    Local,
    Token,
}

impl WebAuthMode {
    fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "token" => Ok(Self::Token),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.web.auth '{text}'").into(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebConfig {
    pub enabled: bool,
    pub listen: SmolStr,
    pub auth: WebAuthMode,
    pub tls: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    Disabled,
    SelfManaged,
    Provisioned,
}

impl TlsMode {
    fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "disabled" => Ok(Self::Disabled),
            "self-managed" | "self_managed" => Ok(Self::SelfManaged),
            "provisioned" => Ok(Self::Provisioned),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.tls.mode '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub fn enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub mode: TlsMode,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub ca_path: Option<PathBuf>,
    pub require_remote: bool,
}

#[derive(Debug, Clone)]
pub struct DeployConfig {
    pub require_signed: bool,
    pub keyring_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub enabled: bool,
    pub service_name: SmolStr,
    pub advertise: bool,
    pub interfaces: Vec<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct MeshConfig {
    pub enabled: bool,
    pub listen: SmolStr,
    pub tls: bool,
    pub auth_token: Option<SmolStr>,
    pub publish: Vec<SmolStr>,
    pub subscribe: IndexMap<SmolStr, SmolStr>,
}

#[derive(Debug, Clone)]
pub struct IoConfig {
    pub drivers: Vec<IoDriverConfig>,
    pub safe_state: IoSafeState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IoDriverConfig {
    pub name: SmolStr,
    pub params: toml::Value,
}

#[derive(Debug, Clone)]
pub struct RuntimeBundle {
    pub root: PathBuf,
    pub runtime: RuntimeConfig,
    pub io: IoConfig,
    pub simulation: Option<SimulationConfig>,
    pub bytecode: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TaskOverride {
    pub name: SmolStr,
    pub interval: Duration,
    pub priority: u8,
    pub programs: Vec<SmolStr>,
    pub single: Option<SmolStr>,
}

impl RuntimeBundle {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let root = root.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(RuntimeError::InvalidBundle(
                format!("project folder not found: {}", root.display()).into(),
            ));
        }
        let runtime_path = root.join("runtime.toml");
        let io_path = root.join("io.toml");
        let simulation_path = root.join("simulation.toml");
        let program_path = root.join("program.stbc");

        if !runtime_path.is_file() {
            return Err(RuntimeError::InvalidBundle(
                format!(
                    "missing runtime.toml at {} (run `trust-runtime` to auto-create a project folder)",
                    runtime_path.display()
                )
                .into(),
            ));
        }
        if !program_path.is_file() {
            return Err(RuntimeError::InvalidBundle(
                format!(
                    "missing program.stbc at {} (run `trust-runtime` to auto-create a project folder)",
                    program_path.display()
                )
                .into(),
            ));
        }

        let runtime = RuntimeConfig::load(&runtime_path)?;
        let io = if io_path.is_file() {
            IoConfig::load(&io_path)?
        } else if let Some(system_io) = load_system_io_config()? {
            system_io
        } else {
            return Err(RuntimeError::InvalidBundle(
                format!(
                    "missing io.toml at {} and no system io config at {} (run `trust-runtime setup` or `trust-runtime`)",
                    io_path.display(),
                    system_io_config_path().display()
                )
                .into(),
            ));
        };
        let bytecode = std::fs::read(&program_path).map_err(|err| {
            RuntimeError::InvalidBundle(format!("failed to read program.stbc: {err}").into())
        })?;
        let simulation = SimulationConfig::load_optional(&simulation_path)?;

        Ok(Self {
            root,
            runtime,
            io,
            simulation,
            bytecode,
        })
    }
}

impl RuntimeConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|err| RuntimeError::InvalidConfig(format!("runtime.toml: {err}").into()))?;
        parse_runtime_toml_from_text(&text, "runtime.toml")
    }
}

impl IoConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|err| RuntimeError::InvalidConfig(format!("io.toml: {err}").into()))?;
        parse_io_toml_from_text(&text, "io.toml")
    }
}

#[must_use]
pub fn system_io_config_path() -> PathBuf {
    PathBuf::from(SYSTEM_IO_CONFIG_PATH)
}

pub fn load_system_io_config() -> Result<Option<IoConfig>, RuntimeError> {
    let path = system_io_config_path();
    if !path.is_file() {
        return Ok(None);
    }
    IoConfig::load(path).map(Some)
}

pub fn validate_runtime_toml_text(text: &str) -> Result<(), RuntimeError> {
    parse_runtime_toml_from_text(text, "runtime.toml").map(|_| ())
}

pub fn validate_io_toml_text(text: &str) -> Result<(), RuntimeError> {
    parse_io_toml_from_text(text, "io.toml").map(|_| ())
}

fn parse_runtime_toml_from_text(
    text: &str,
    file_name: &str,
) -> Result<RuntimeConfig, RuntimeError> {
    let raw: RuntimeToml = toml::from_str(text)
        .map_err(|err| RuntimeError::InvalidConfig(format!("{file_name}: {err}").into()))?;
    raw.into_config()
        .map_err(|err| prefix_invalid_config(file_name, err))
}

fn parse_io_toml_from_text(text: &str, file_name: &str) -> Result<IoConfig, RuntimeError> {
    let raw: IoToml = toml::from_str(text)
        .map_err(|err| RuntimeError::InvalidConfig(format!("{file_name}: {err}").into()))?;
    raw.into_config()
        .map_err(|err| prefix_invalid_config(file_name, err))
}

fn prefix_invalid_config(file_name: &str, err: RuntimeError) -> RuntimeError {
    match err {
        RuntimeError::InvalidConfig(message) => {
            RuntimeError::InvalidConfig(format!("{file_name}: {message}").into())
        }
        other => other,
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeToml {
    bundle: BundleSection,
    resource: ResourceSection,
    runtime: RuntimeSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleSection {
    version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceSection {
    name: String,
    cycle_interval_ms: u64,
    tasks: Option<Vec<TaskSection>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskSection {
    name: String,
    interval_ms: u64,
    priority: u8,
    programs: Vec<String>,
    single: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSection {
    control: ControlSection,
    log: LogSection,
    retain: RetainSection,
    watchdog: WatchdogSection,
    fault: FaultSection,
    web: Option<WebSection>,
    tls: Option<TlsSection>,
    deploy: Option<DeploySection>,
    discovery: Option<DiscoverySection>,
    mesh: Option<MeshSection>,
    observability: Option<ObservabilitySection>,
    opcua: Option<OpcUaSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlSection {
    endpoint: String,
    auth_token: Option<String>,
    debug_enabled: Option<bool>,
    mode: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    Production,
    Debug,
}

impl ControlMode {
    fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "production" => Ok(Self::Production),
            "debug" => Ok(Self::Debug),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.control.mode '{text}'").into(),
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LogSection {
    level: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainSection {
    mode: String,
    path: Option<String>,
    save_interval_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WatchdogSection {
    enabled: bool,
    timeout_ms: u64,
    action: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FaultSection {
    policy: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WebSection {
    enabled: Option<bool>,
    listen: Option<String>,
    auth: Option<String>,
    tls: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TlsSection {
    mode: Option<String>,
    cert_path: Option<String>,
    key_path: Option<String>,
    ca_path: Option<String>,
    require_remote: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeploySection {
    require_signed: Option<bool>,
    keyring_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoverySection {
    enabled: Option<bool>,
    service_name: Option<String>,
    advertise: Option<bool>,
    interfaces: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshSection {
    enabled: Option<bool>,
    listen: Option<String>,
    tls: Option<bool>,
    auth_token: Option<String>,
    publish: Option<Vec<String>>,
    subscribe: Option<IndexMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservabilitySection {
    enabled: Option<bool>,
    sample_interval_ms: Option<u64>,
    mode: Option<String>,
    include: Option<Vec<String>>,
    history_path: Option<String>,
    max_entries: Option<usize>,
    prometheus_enabled: Option<bool>,
    prometheus_path: Option<String>,
    alerts: Option<Vec<AlertSection>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlertSection {
    name: String,
    variable: String,
    above: Option<f64>,
    below: Option<f64>,
    debounce_samples: Option<u32>,
    hook: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpcUaSection {
    enabled: Option<bool>,
    listen: Option<String>,
    endpoint_path: Option<String>,
    namespace_uri: Option<String>,
    publish_interval_ms: Option<u64>,
    max_nodes: Option<usize>,
    expose: Option<Vec<String>>,
    security_policy: Option<String>,
    security_mode: Option<String>,
    allow_anonymous: Option<bool>,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoToml {
    io: IoSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoSection {
    driver: Option<String>,
    params: Option<toml::Value>,
    drivers: Option<Vec<IoDriverSection>>,
    safe_state: Option<Vec<IoSafeEntry>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoDriverSection {
    name: String,
    params: Option<toml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoSafeEntry {
    address: String,
    value: String,
}

impl RuntimeToml {
    fn into_config(self) -> Result<RuntimeConfig, RuntimeError> {
        if self.bundle.version != 1 {
            return Err(RuntimeError::InvalidConfig(
                format!("unsupported bundle.version {}", self.bundle.version).into(),
            ));
        }
        if self.resource.name.trim().is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "resource.name must not be empty".into(),
            ));
        }
        if self.resource.cycle_interval_ms == 0 {
            return Err(RuntimeError::InvalidConfig(
                "resource.cycle_interval_ms must be >= 1".into(),
            ));
        }
        if self.runtime.control.endpoint.trim().is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.control.endpoint must not be empty".into(),
            ));
        }
        if self.runtime.log.level.trim().is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.log.level must not be empty".into(),
            ));
        }
        if self.runtime.retain.save_interval_ms == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.retain.save_interval_ms must be >= 1".into(),
            ));
        }
        if self.runtime.watchdog.timeout_ms == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.watchdog.timeout_ms must be >= 1".into(),
            ));
        }
        let retain_mode = RetainMode::parse(&self.runtime.retain.mode)?;
        if matches!(retain_mode, RetainMode::File)
            && self
                .runtime
                .retain
                .path
                .as_deref()
                .is_none_or(|path| path.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.retain.path required when mode=file".into(),
            ));
        }
        let watchdog_action = WatchdogAction::parse(&self.runtime.watchdog.action)?;
        let fault_policy = FaultPolicy::parse(&self.runtime.fault.policy)?;
        let tasks = self
            .resource
            .tasks
            .map(|tasks| {
                tasks
                    .into_iter()
                    .map(|task| {
                        if task.name.trim().is_empty() {
                            return Err(RuntimeError::InvalidConfig(
                                "resource.tasks[].name must not be empty".into(),
                            ));
                        }
                        if task.interval_ms == 0 {
                            return Err(RuntimeError::InvalidConfig(
                                "resource.tasks[].interval_ms must be >= 1".into(),
                            ));
                        }
                        if task.programs.is_empty() {
                            return Err(RuntimeError::InvalidConfig(
                                "resource.tasks[].programs must not be empty".into(),
                            ));
                        }
                        if task
                            .programs
                            .iter()
                            .any(|program| program.trim().is_empty())
                        {
                            return Err(RuntimeError::InvalidConfig(
                                "resource.tasks[].programs entries must not be empty".into(),
                            ));
                        }
                        Ok(TaskOverride {
                            name: SmolStr::new(task.name),
                            interval: Duration::from_millis(task.interval_ms as i64),
                            priority: task.priority,
                            programs: task.programs.into_iter().map(SmolStr::new).collect(),
                            single: task.single.map(SmolStr::new),
                        })
                    })
                    .collect::<Result<Vec<_>, RuntimeError>>()
            })
            .transpose()?;
        let control_auth_token = self.runtime.control.auth_token.and_then(|token| {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(SmolStr::new(trimmed))
            }
        });
        let control_mode =
            ControlMode::parse(self.runtime.control.mode.as_deref().unwrap_or("production"))?;
        let debug_enabled = match self.runtime.control.debug_enabled {
            Some(value) => value,
            None => matches!(control_mode, ControlMode::Debug),
        };
        if self.runtime.control.endpoint.starts_with("tcp://") && control_auth_token.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.control.auth_token required for tcp endpoint".into(),
            ));
        }
        let web_section = self.runtime.web.unwrap_or(WebSection {
            enabled: Some(true),
            listen: Some("0.0.0.0:8080".into()),
            auth: Some("local".into()),
            tls: Some(false),
        });
        if web_section
            .listen
            .as_deref()
            .is_some_and(|listen| listen.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.web.listen must not be empty".into(),
            ));
        }
        let web_auth = WebAuthMode::parse(web_section.auth.as_deref().unwrap_or("local"))?;
        if matches!(web_auth, WebAuthMode::Token) && control_auth_token.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.web.auth=token requires runtime.control.auth_token".into(),
            ));
        }
        let web_enabled = web_section.enabled.unwrap_or(true);
        let web_listen = web_section.listen.unwrap_or_else(|| "0.0.0.0:8080".into());
        let web_tls = web_section.tls.unwrap_or(false);

        let tls_section = self.runtime.tls.unwrap_or(TlsSection {
            mode: Some("disabled".into()),
            cert_path: None,
            key_path: None,
            ca_path: None,
            require_remote: Some(false),
        });
        let tls_mode = TlsMode::parse(tls_section.mode.as_deref().unwrap_or("disabled"))?;
        let tls_cert_path = parse_optional_path("runtime.tls.cert_path", tls_section.cert_path)?;
        let tls_key_path = parse_optional_path("runtime.tls.key_path", tls_section.key_path)?;
        let tls_ca_path = parse_optional_path("runtime.tls.ca_path", tls_section.ca_path)?;
        let tls_require_remote = tls_section.require_remote.unwrap_or(false);

        if web_tls && !tls_mode.enabled() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.web.tls=true requires runtime.tls.mode != 'disabled'".into(),
            ));
        }
        if tls_mode.enabled() {
            if tls_cert_path.is_none() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.tls.cert_path required when TLS is enabled".into(),
                ));
            }
            if tls_key_path.is_none() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.tls.key_path required when TLS is enabled".into(),
                ));
            }
            if matches!(tls_mode, TlsMode::Provisioned) && tls_ca_path.is_none() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.tls.ca_path required when runtime.tls.mode='provisioned'".into(),
                ));
            }
        }
        if tls_require_remote && web_enabled && listen_is_remote(&web_listen) && !web_tls {
            return Err(RuntimeError::InvalidConfig(
                "runtime.web.tls must be true when runtime.tls.require_remote=true and runtime.web.listen is remote".into(),
            ));
        }

        let deploy_section = self.runtime.deploy.unwrap_or(DeploySection {
            require_signed: Some(false),
            keyring_path: None,
        });
        if deploy_section
            .keyring_path
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.deploy.keyring_path must not be empty".into(),
            ));
        }
        if deploy_section.require_signed.unwrap_or(false)
            && deploy_section
                .keyring_path
                .as_deref()
                .is_none_or(|path| path.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.deploy.keyring_path required when runtime.deploy.require_signed=true"
                    .into(),
            ));
        }

        let discovery_section = self.runtime.discovery.unwrap_or(DiscoverySection {
            enabled: Some(true),
            service_name: Some("truST".into()),
            advertise: Some(true),
            interfaces: None,
        });
        if discovery_section
            .service_name
            .as_deref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.discovery.service_name must not be empty".into(),
            ));
        }

        let mesh_section = self.runtime.mesh.unwrap_or(MeshSection {
            enabled: Some(false),
            listen: Some("0.0.0.0:5200".into()),
            tls: Some(false),
            auth_token: None,
            publish: None,
            subscribe: None,
        });
        if mesh_section
            .listen
            .as_deref()
            .is_some_and(|listen| listen.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.mesh.listen must not be empty".into(),
            ));
        }
        let mesh_enabled = mesh_section.enabled.unwrap_or(false);
        let mesh_listen = mesh_section.listen.unwrap_or_else(|| "0.0.0.0:5200".into());
        let mesh_tls = mesh_section.tls.unwrap_or(false);
        if mesh_tls && !tls_mode.enabled() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.mesh.tls=true requires runtime.tls.mode != 'disabled'".into(),
            ));
        }
        if tls_require_remote && mesh_enabled && listen_is_remote(&mesh_listen) && !mesh_tls {
            return Err(RuntimeError::InvalidConfig(
                "runtime.mesh.tls must be true when runtime.tls.require_remote=true and runtime.mesh.listen is remote".into(),
            ));
        }

        let observability_section = self.runtime.observability.unwrap_or(ObservabilitySection {
            enabled: Some(false),
            sample_interval_ms: Some(1_000),
            mode: Some("all".into()),
            include: Some(Vec::new()),
            history_path: Some("history/historian.jsonl".into()),
            max_entries: Some(20_000),
            prometheus_enabled: Some(true),
            prometheus_path: Some("/metrics".into()),
            alerts: Some(Vec::new()),
        });
        let sample_interval_ms = observability_section.sample_interval_ms.unwrap_or(1_000);
        if sample_interval_ms == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.observability.sample_interval_ms must be >= 1".into(),
            ));
        }
        let max_entries = observability_section.max_entries.unwrap_or(20_000);
        if max_entries == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.observability.max_entries must be >= 1".into(),
            ));
        }
        let mode = match observability_section
            .mode
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "all" => RecordingMode::All,
            "allowlist" => RecordingMode::Allowlist,
            other => {
                return Err(RuntimeError::InvalidConfig(
                    format!("invalid runtime.observability.mode '{other}'").into(),
                ))
            }
        };
        let include = observability_section
            .include
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.trim().to_string())
            .filter(|entry| !entry.is_empty())
            .map(SmolStr::new)
            .collect::<Vec<_>>();
        for pattern in &include {
            Pattern::new(pattern.as_str()).map_err(|err| {
                RuntimeError::InvalidConfig(
                    format!(
                        "runtime.observability.include invalid pattern '{}': {err}",
                        pattern
                    )
                    .into(),
                )
            })?;
        }
        if matches!(mode, RecordingMode::Allowlist) && include.is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.observability.include must not be empty when mode='allowlist'".into(),
            ));
        }
        let history_path = observability_section
            .history_path
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| "history/historian.jsonl".to_string());
        let prometheus_path = observability_section
            .prometheus_path
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| "/metrics".to_string());
        if !prometheus_path.starts_with('/') {
            return Err(RuntimeError::InvalidConfig(
                "runtime.observability.prometheus_path must start with '/'".into(),
            ));
        }
        let alerts = observability_section
            .alerts
            .unwrap_or_default()
            .into_iter()
            .map(|alert| {
                if alert.name.trim().is_empty() {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.observability.alerts[].name must not be empty".into(),
                    ));
                }
                if alert.variable.trim().is_empty() {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.observability.alerts[].variable must not be empty".into(),
                    ));
                }
                if alert.above.is_none() && alert.below.is_none() {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.observability.alerts[] requires above and/or below".into(),
                    ));
                }
                let debounce_samples = alert.debounce_samples.unwrap_or(1);
                if debounce_samples == 0 {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.observability.alerts[].debounce_samples must be >= 1".into(),
                    ));
                }
                Ok(AlertRule {
                    name: SmolStr::new(alert.name.trim()),
                    variable: SmolStr::new(alert.variable.trim()),
                    above: alert.above,
                    below: alert.below,
                    debounce_samples,
                    hook: alert.hook.and_then(|hook| {
                        let trimmed = hook.trim().to_string();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(SmolStr::new(trimmed))
                        }
                    }),
                })
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;

        let opcua_section = self.runtime.opcua.unwrap_or(OpcUaSection {
            enabled: Some(false),
            listen: Some("0.0.0.0:4840".into()),
            endpoint_path: Some("/".into()),
            namespace_uri: Some("urn:trust:runtime".into()),
            publish_interval_ms: Some(250),
            max_nodes: Some(128),
            expose: Some(Vec::new()),
            security_policy: Some("basic256sha256".into()),
            security_mode: Some("sign_and_encrypt".into()),
            allow_anonymous: Some(false),
            username: None,
            password: None,
        });
        if opcua_section
            .listen
            .as_deref()
            .is_some_and(|listen| listen.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.listen must not be empty".into(),
            ));
        }
        if opcua_section
            .endpoint_path
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.endpoint_path must not be empty".into(),
            ));
        }
        if opcua_section
            .namespace_uri
            .as_deref()
            .is_some_and(|uri| uri.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.namespace_uri must not be empty".into(),
            ));
        }
        let publish_interval_ms = opcua_section.publish_interval_ms.unwrap_or(250);
        if publish_interval_ms == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.publish_interval_ms must be >= 1".into(),
            ));
        }
        let max_nodes = opcua_section.max_nodes.unwrap_or(128);
        if max_nodes == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.max_nodes must be >= 1".into(),
            ));
        }
        let expose = opcua_section
            .expose
            .unwrap_or_default()
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(SmolStr::new)
            .collect::<Vec<_>>();
        for pattern in &expose {
            Pattern::new(pattern.as_str()).map_err(|err| {
                RuntimeError::InvalidConfig(
                    format!("runtime.opcua.expose invalid pattern '{}': {err}", pattern).into(),
                )
            })?;
        }
        let security_policy_raw = opcua_section
            .security_policy
            .as_deref()
            .unwrap_or("basic256sha256");
        let security_mode_raw = opcua_section
            .security_mode
            .as_deref()
            .unwrap_or("sign_and_encrypt");
        let security_policy = OpcUaSecurityPolicy::parse(security_policy_raw).ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!("invalid runtime.opcua.security_policy '{security_policy_raw}'").into(),
            )
        })?;
        let security_mode =
            OpcUaMessageSecurityMode::parse(security_mode_raw).ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!("invalid runtime.opcua.security_mode '{security_mode_raw}'").into(),
                )
            })?;
        let allow_anonymous = opcua_section.allow_anonymous.unwrap_or(false);
        match (security_policy, security_mode) {
            (OpcUaSecurityPolicy::None, OpcUaMessageSecurityMode::None)
            | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::Sign)
            | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::SignAndEncrypt)
            | (OpcUaSecurityPolicy::Aes128Sha256RsaOaep, OpcUaMessageSecurityMode::Sign)
            | (
                OpcUaSecurityPolicy::Aes128Sha256RsaOaep,
                OpcUaMessageSecurityMode::SignAndEncrypt,
            ) => {}
            (policy, mode) => {
                return Err(RuntimeError::InvalidConfig(
                    format!("unsupported runtime.opcua security profile {policy:?}/{mode:?}")
                        .into(),
                ))
            }
        }
        let username = opcua_section
            .username
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(SmolStr::new);
        let password = opcua_section
            .password
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(SmolStr::new);
        if username.is_some() ^ password.is_some() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.username/password must both be set or both be omitted".into(),
            ));
        }
        let opcua_enabled = opcua_section.enabled.unwrap_or(false);
        if opcua_enabled && !allow_anonymous && username.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua requires anonymous access or username/password when enabled".into(),
            ));
        }
        let endpoint_path = opcua_section
            .endpoint_path
            .unwrap_or_else(|| "/".to_string())
            .trim()
            .to_string();
        if !endpoint_path.starts_with('/') {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.endpoint_path must start with '/'".into(),
            ));
        }
        let opcua = OpcUaRuntimeConfig {
            enabled: opcua_enabled,
            listen: SmolStr::new(
                opcua_section
                    .listen
                    .unwrap_or_else(|| "0.0.0.0:4840".to_string())
                    .trim(),
            ),
            endpoint_path: SmolStr::new(endpoint_path),
            namespace_uri: SmolStr::new(
                opcua_section
                    .namespace_uri
                    .unwrap_or_else(|| "urn:trust:runtime".to_string())
                    .trim(),
            ),
            publish_interval_ms,
            max_nodes,
            expose,
            security: OpcUaSecurityProfile {
                policy: security_policy,
                mode: security_mode,
                allow_anonymous,
            },
            username,
            password,
        };

        Ok(RuntimeConfig {
            bundle_version: self.bundle.version,
            resource_name: SmolStr::new(self.resource.name),
            cycle_interval: Duration::from_millis(self.resource.cycle_interval_ms as i64),
            control_endpoint: SmolStr::new(self.runtime.control.endpoint),
            control_auth_token,
            control_debug_enabled: debug_enabled,
            control_mode,
            log_level: SmolStr::new(self.runtime.log.level),
            retain_mode,
            retain_path: self.runtime.retain.path.map(PathBuf::from),
            retain_save_interval: Duration::from_millis(
                self.runtime.retain.save_interval_ms as i64,
            ),
            watchdog: WatchdogPolicy {
                enabled: self.runtime.watchdog.enabled,
                timeout: Duration::from_millis(self.runtime.watchdog.timeout_ms as i64),
                action: watchdog_action,
            },
            fault_policy,
            web: WebConfig {
                enabled: web_enabled,
                listen: SmolStr::new(web_listen),
                auth: web_auth,
                tls: web_tls,
            },
            tls: TlsConfig {
                mode: tls_mode,
                cert_path: tls_cert_path,
                key_path: tls_key_path,
                ca_path: tls_ca_path,
                require_remote: tls_require_remote,
            },
            deploy: DeployConfig {
                require_signed: deploy_section.require_signed.unwrap_or(false),
                keyring_path: deploy_section.keyring_path.and_then(|path| {
                    let path = path.trim();
                    if path.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(path))
                    }
                }),
            },
            discovery: DiscoveryConfig {
                enabled: discovery_section.enabled.unwrap_or(true),
                service_name: SmolStr::new(
                    discovery_section
                        .service_name
                        .unwrap_or_else(|| "truST".into()),
                ),
                advertise: discovery_section.advertise.unwrap_or(true),
                interfaces: discovery_section
                    .interfaces
                    .unwrap_or_default()
                    .into_iter()
                    .map(SmolStr::new)
                    .collect(),
            },
            mesh: MeshConfig {
                enabled: mesh_enabled,
                listen: SmolStr::new(mesh_listen),
                tls: mesh_tls,
                auth_token: mesh_section.auth_token.and_then(|token| {
                    let trimmed = token.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(SmolStr::new(trimmed))
                    }
                }),
                publish: mesh_section
                    .publish
                    .unwrap_or_default()
                    .into_iter()
                    .map(SmolStr::new)
                    .collect(),
                subscribe: mesh_section
                    .subscribe
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(k, v)| (SmolStr::new(k), SmolStr::new(v)))
                    .collect(),
            },
            observability: HistorianConfig {
                enabled: observability_section.enabled.unwrap_or(false),
                sample_interval_ms,
                mode,
                include,
                history_path: PathBuf::from(history_path),
                max_entries,
                prometheus_enabled: observability_section.prometheus_enabled.unwrap_or(true),
                prometheus_path: SmolStr::new(prometheus_path),
                alerts,
            },
            opcua,
            tasks,
        })
    }
}

impl IoToml {
    fn into_config(self) -> Result<IoConfig, RuntimeError> {
        let legacy_driver = self
            .io
            .driver
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned);
        let legacy_params = self
            .io
            .params
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
        let explicit_drivers = self.io.drivers.unwrap_or_default();

        if legacy_driver.is_some() && !explicit_drivers.is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "use either io.driver/io.params or io.drivers, not both".into(),
            ));
        }

        let drivers = if let Some(driver) = legacy_driver {
            if !legacy_params.is_table() {
                return Err(RuntimeError::InvalidConfig(
                    "io.params must be a table".into(),
                ));
            }
            vec![IoDriverConfig {
                name: SmolStr::new(driver),
                params: legacy_params,
            }]
        } else {
            if explicit_drivers.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "io.driver or io.drivers must be set".into(),
                ));
            }
            explicit_drivers
                .into_iter()
                .enumerate()
                .map(|(idx, driver)| {
                    if driver.name.trim().is_empty() {
                        return Err(RuntimeError::InvalidConfig(
                            format!("io.drivers[{idx}].name must not be empty").into(),
                        ));
                    }
                    let params = driver
                        .params
                        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
                    if !params.is_table() {
                        return Err(RuntimeError::InvalidConfig(
                            format!("io.drivers[{idx}].params must be a table").into(),
                        ));
                    }
                    Ok(IoDriverConfig {
                        name: SmolStr::new(driver.name),
                        params,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut safe_state = IoSafeState::default();
        if let Some(entries) = self.io.safe_state {
            for entry in entries {
                let address = IoAddress::parse(&entry.address)?;
                let value = parse_io_value(&entry.value, address.size)?;
                safe_state.outputs.push((address, value));
            }
        }
        Ok(IoConfig {
            drivers,
            safe_state,
        })
    }
}

fn parse_io_value(text: &str, size: IoSize) -> Result<Value, RuntimeError> {
    let trimmed = text.trim();
    let upper = trimmed.to_ascii_uppercase();
    match size {
        IoSize::Bit => match upper.as_str() {
            "TRUE" | "1" => Ok(Value::Bool(true)),
            "FALSE" | "0" => Ok(Value::Bool(false)),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid BOOL safe_state value '{trimmed}'").into(),
            )),
        },
        IoSize::Byte => Ok(Value::Byte(parse_u64(trimmed)? as u8)),
        IoSize::Word => Ok(Value::Word(parse_u64(trimmed)? as u16)),
        IoSize::DWord => Ok(Value::DWord(parse_u64(trimmed)? as u32)),
        IoSize::LWord => Ok(Value::LWord(parse_u64(trimmed)?)),
    }
}

fn parse_u64(text: &str) -> Result<u64, RuntimeError> {
    let trimmed = text.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).map_err(|err| {
            RuntimeError::InvalidConfig(format!("invalid hex value '{trimmed}': {err}").into())
        });
    }
    trimmed.parse::<u64>().map_err(|err| {
        RuntimeError::InvalidConfig(format!("invalid numeric value '{trimmed}': {err}").into())
    })
}

fn parse_optional_path(
    field: &str,
    value: Option<String>,
) -> Result<Option<PathBuf>, RuntimeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must not be empty").into(),
        ));
    }
    Ok(Some(PathBuf::from(trimmed)))
}

fn listen_is_remote(listen: &str) -> bool {
    if let Ok(addr) = listen.parse::<std::net::SocketAddr>() {
        return !addr.ip().is_loopback();
    }
    let host = listen
        .rsplit_once(':')
        .map_or(listen, |(host, _)| host)
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    !host.eq_ignore_ascii_case("localhost") && host != "127.0.0.1" && host != "::1"
}

#[cfg(test)]
mod tests {
    use super::{validate_io_toml_text, validate_runtime_toml_text};

    fn runtime_toml() -> String {
        r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"

[runtime.web]
enabled = true
listen = "0.0.0.0:8080"
auth = "local"
tls = false

[runtime.discovery]
enabled = true
service_name = "truST"
advertise = true
interfaces = ["eth0"]

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
publish = []
subscribe = {}
"#
        .to_string()
    }

    fn io_toml() -> String {
        r#"
[io]
driver = "loopback"
params = {}
"#
        .to_string()
    }

    #[test]
    fn runtime_schema_rejects_unknown_keys() {
        let text = format!("{}\n[runtime.extra]\nflag = true\n", runtime_toml());
        let err = validate_runtime_toml_text(&text).expect_err("runtime schema should fail");
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn runtime_schema_rejects_invalid_ranges() {
        let text = runtime_toml().replace("cycle_interval_ms = 100", "cycle_interval_ms = 0");
        let err = validate_runtime_toml_text(&text).expect_err("cycle interval range should fail");
        assert!(err
            .to_string()
            .contains("resource.cycle_interval_ms must be >= 1"));
    }

    #[test]
    fn runtime_schema_requires_control_auth_for_tcp_endpoints() {
        let text = runtime_toml().replace(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            "endpoint = \"tcp://127.0.0.1:5000\"",
        );
        let err = validate_runtime_toml_text(&text).expect_err("tcp auth should fail");
        assert!(err
            .to_string()
            .contains("runtime.control.auth_token required for tcp endpoint"));
    }

    #[test]
    fn runtime_schema_requires_deploy_keyring_when_signed_deploy_enabled() {
        let text = format!(
            "{}\n[runtime.deploy]\nrequire_signed = true\n",
            runtime_toml()
        );
        let err = validate_runtime_toml_text(&text).expect_err("signed deploy config should fail");
        assert!(err.to_string().contains(
            "runtime.deploy.keyring_path required when runtime.deploy.require_signed=true"
        ));
    }

    #[test]
    fn runtime_schema_requires_tls_credentials_when_tls_enabled() {
        let text = format!(
            "{}\n[runtime.tls]\nmode = \"self-managed\"\n",
            runtime_toml().replace("tls = false", "tls = true")
        );
        let err = validate_runtime_toml_text(&text).expect_err("tls credential config should fail");
        assert!(err
            .to_string()
            .contains("runtime.tls.cert_path required when TLS is enabled"));
    }

    #[test]
    fn runtime_schema_rejects_remote_web_without_tls_when_required() {
        let text = format!(
            "{}\n[runtime.tls]\nmode = \"disabled\"\nrequire_remote = true\n",
            runtime_toml()
        );
        let err = validate_runtime_toml_text(&text).expect_err("remote tls policy should fail");
        assert!(err.to_string().contains(
            "runtime.web.tls must be true when runtime.tls.require_remote=true and runtime.web.listen is remote"
        ));
    }

    #[test]
    fn runtime_schema_rejects_provisioned_tls_without_ca_path() {
        let text = format!(
            "{}\n[runtime.tls]\nmode = \"provisioned\"\ncert_path = \"certs/server.pem\"\nkey_path = \"certs/server.key\"\n",
            runtime_toml().replace("tls = false", "tls = true")
        );
        let err = validate_runtime_toml_text(&text)
            .expect_err("provisioned tls without ca path should fail");
        assert!(err
            .to_string()
            .contains("runtime.tls.ca_path required when runtime.tls.mode='provisioned'"));
    }

    #[test]
    fn runtime_schema_accepts_web_tls_with_self_managed_cert_paths() {
        let text = format!(
            "{}\n[runtime.tls]\nmode = \"self-managed\"\ncert_path = \"security/server-cert.pem\"\nkey_path = \"security/server-key.pem\"\n",
            runtime_toml().replace("tls = false", "tls = true")
        );
        validate_runtime_toml_text(&text).expect("web tls config should be valid");
    }

    #[test]
    fn runtime_schema_rejects_allowlist_without_patterns() {
        let text = format!(
            "{}\n[runtime.observability]\nmode = \"allowlist\"\ninclude = []\n",
            runtime_toml()
        );
        let err = validate_runtime_toml_text(&text).expect_err("allowlist requires include");
        assert!(err
            .to_string()
            .contains("runtime.observability.include must not be empty when mode='allowlist'"));
    }

    #[test]
    fn runtime_schema_rejects_prometheus_path_without_leading_slash() {
        let text = format!(
            "{}\n[runtime.observability]\nprometheus_path = \"metrics\"\n",
            runtime_toml()
        );
        let err = validate_runtime_toml_text(&text).expect_err("prometheus path should fail");
        assert!(err
            .to_string()
            .contains("runtime.observability.prometheus_path must start with '/'"));
    }

    #[test]
    fn runtime_schema_rejects_opcua_endpoint_path_without_leading_slash() {
        let text = format!(
            "{}\n[runtime.opcua]\nenabled = true\nallow_anonymous = true\nendpoint_path = \"interop\"\nsecurity_policy = \"none\"\nsecurity_mode = \"none\"\n",
            runtime_toml()
        );
        let err = validate_runtime_toml_text(&text).expect_err("opcua endpoint path should fail");
        assert!(err
            .to_string()
            .contains("runtime.opcua.endpoint_path must start with '/'"));
    }

    #[test]
    fn runtime_schema_requires_opcua_credentials_or_anonymous_when_enabled() {
        let text = format!("{}\n[runtime.opcua]\nenabled = true\n", runtime_toml());
        let err = validate_runtime_toml_text(&text).expect_err("opcua auth config should fail");
        assert!(err
            .to_string()
            .contains("runtime.opcua requires anonymous access or username/password when enabled"));
    }

    #[test]
    fn runtime_schema_accepts_opcua_secure_profile_with_user_credentials() {
        let text = format!(
            "{}\n[runtime.opcua]\nenabled = true\nallow_anonymous = false\nsecurity_policy = \"basic256sha256\"\nsecurity_mode = \"sign_and_encrypt\"\nusername = \"operator\"\npassword = \"secret\"\n",
            runtime_toml()
        );
        validate_runtime_toml_text(&text).expect("opcua secure profile should be valid");
    }

    #[test]
    fn io_schema_rejects_unknown_keys() {
        let text = io_toml().replace("params = {}", "params = {}\nunknown = true");
        let err = validate_io_toml_text(&text).expect_err("io schema should fail");
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn io_schema_requires_table_params() {
        let text = io_toml().replace("params = {}", "params = 42");
        let err = validate_io_toml_text(&text).expect_err("io.params type should fail");
        assert!(err.to_string().contains("io.params must be a table"));
    }

    #[test]
    fn io_schema_accepts_multiple_drivers() {
        let text = r#"
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]

[[io.drivers]]
name = "modbus-tcp"
params = { address = "127.0.0.1:502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" }

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883", topic_in = "trust/io/in", topic_out = "trust/io/out", reconnect_ms = 500, keep_alive_s = 5, allow_insecure_remote = false }
"#;
        validate_io_toml_text(text).expect("io.drivers profile should be valid");
    }

    #[test]
    fn io_schema_rejects_mixed_single_and_multi_driver_fields() {
        let text = r#"
[io]
driver = "loopback"
params = {}

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883" }
"#;
        let err = validate_io_toml_text(text)
            .expect_err("mixed io.driver and io.drivers should be rejected");
        assert!(err
            .to_string()
            .contains("use either io.driver/io.params or io.drivers"));
    }

    #[test]
    fn io_schema_rejects_empty_multi_driver_list() {
        let text = r#"
[io]
drivers = []
"#;
        let err =
            validate_io_toml_text(text).expect_err("empty io.drivers list should be rejected");
        assert!(err
            .to_string()
            .contains("io.driver or io.drivers must be set"));
    }
}
