//! Runtime bundle configuration loading.

#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Deserialize;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSafeState, IoSize};
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
    pub discovery: DiscoveryConfig,
    pub mesh: MeshConfig,
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
    pub auth_token: Option<SmolStr>,
    pub publish: Vec<SmolStr>,
    pub subscribe: IndexMap<SmolStr, SmolStr>,
}

#[derive(Debug, Clone)]
pub struct IoConfig {
    pub driver: SmolStr,
    pub params: toml::Value,
    pub safe_state: IoSafeState,
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
        let raw: RuntimeToml = toml::from_str(&text)
            .map_err(|err| RuntimeError::InvalidConfig(format!("runtime.toml: {err}").into()))?;
        raw.into_config()
    }
}

impl IoConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|err| RuntimeError::InvalidConfig(format!("io.toml: {err}").into()))?;
        let raw: IoToml = toml::from_str(&text)
            .map_err(|err| RuntimeError::InvalidConfig(format!("io.toml: {err}").into()))?;
        raw.into_config()
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

#[derive(Debug, Deserialize)]
struct RuntimeToml {
    bundle: BundleSection,
    resource: ResourceSection,
    runtime: RuntimeSection,
}

#[derive(Debug, Deserialize)]
struct BundleSection {
    version: u32,
}

#[derive(Debug, Deserialize)]
struct ResourceSection {
    name: String,
    cycle_interval_ms: u64,
    tasks: Option<Vec<TaskSection>>,
}

#[derive(Debug, Deserialize)]
struct TaskSection {
    name: String,
    interval_ms: u64,
    priority: u8,
    programs: Vec<String>,
    single: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuntimeSection {
    control: ControlSection,
    log: LogSection,
    retain: RetainSection,
    watchdog: WatchdogSection,
    fault: FaultSection,
    web: Option<WebSection>,
    discovery: Option<DiscoverySection>,
    mesh: Option<MeshSection>,
}

#[derive(Debug, Deserialize)]
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
struct LogSection {
    level: String,
}

#[derive(Debug, Deserialize)]
struct RetainSection {
    mode: String,
    path: Option<String>,
    save_interval_ms: u64,
}

#[derive(Debug, Deserialize)]
struct WatchdogSection {
    enabled: bool,
    timeout_ms: u64,
    action: String,
}

#[derive(Debug, Deserialize)]
struct FaultSection {
    policy: String,
}

#[derive(Debug, Deserialize)]
struct WebSection {
    enabled: Option<bool>,
    listen: Option<String>,
    auth: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscoverySection {
    enabled: Option<bool>,
    service_name: Option<String>,
    advertise: Option<bool>,
    interfaces: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct MeshSection {
    enabled: Option<bool>,
    listen: Option<String>,
    auth_token: Option<String>,
    publish: Option<Vec<String>>,
    subscribe: Option<IndexMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct IoToml {
    io: IoSection,
}

#[derive(Debug, Deserialize)]
struct IoSection {
    driver: String,
    params: toml::Value,
    safe_state: Option<Vec<IoSafeEntry>>,
}

#[derive(Debug, Deserialize)]
struct IoSafeEntry {
    address: String,
    value: String,
}

impl RuntimeToml {
    fn into_config(self) -> Result<RuntimeConfig, RuntimeError> {
        let retain_mode = RetainMode::parse(&self.runtime.retain.mode)?;
        if matches!(retain_mode, RetainMode::File) && self.runtime.retain.path.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.retain.path required when mode=file".into(),
            ));
        }
        let watchdog_action = WatchdogAction::parse(&self.runtime.watchdog.action)?;
        let fault_policy = FaultPolicy::parse(&self.runtime.fault.policy)?;
        let tasks = self.resource.tasks.map(|tasks| {
            tasks
                .into_iter()
                .map(|task| TaskOverride {
                    name: SmolStr::new(task.name),
                    interval: Duration::from_millis(task.interval_ms as i64),
                    priority: task.priority,
                    programs: task.programs.into_iter().map(SmolStr::new).collect(),
                    single: task.single.map(SmolStr::new),
                })
                .collect()
        });
        let control_mode =
            ControlMode::parse(self.runtime.control.mode.as_deref().unwrap_or("production"))?;
        let debug_enabled = match self.runtime.control.debug_enabled {
            Some(value) => value,
            None => matches!(control_mode, ControlMode::Debug),
        };
        if self.runtime.control.endpoint.starts_with("tcp://")
            && self.runtime.control.auth_token.is_none()
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.control.auth_token required for tcp endpoint".into(),
            ));
        }
        let web_section = self.runtime.web.unwrap_or(WebSection {
            enabled: Some(true),
            listen: Some("0.0.0.0:8080".into()),
            auth: Some("local".into()),
        });
        let web_auth = WebAuthMode::parse(web_section.auth.as_deref().unwrap_or("local"))?;
        if matches!(web_auth, WebAuthMode::Token) && self.runtime.control.auth_token.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.web.auth=token requires runtime.control.auth_token".into(),
            ));
        }

        let discovery_section = self.runtime.discovery.unwrap_or(DiscoverySection {
            enabled: Some(true),
            service_name: Some("truST".into()),
            advertise: Some(true),
            interfaces: None,
        });

        let mesh_section = self.runtime.mesh.unwrap_or(MeshSection {
            enabled: Some(false),
            listen: Some("0.0.0.0:5200".into()),
            auth_token: None,
            publish: None,
            subscribe: None,
        });

        Ok(RuntimeConfig {
            bundle_version: self.bundle.version,
            resource_name: SmolStr::new(self.resource.name),
            cycle_interval: Duration::from_millis(self.resource.cycle_interval_ms as i64),
            control_endpoint: SmolStr::new(self.runtime.control.endpoint),
            control_auth_token: self.runtime.control.auth_token.map(SmolStr::new),
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
                enabled: web_section.enabled.unwrap_or(true),
                listen: SmolStr::new(web_section.listen.unwrap_or_else(|| "0.0.0.0:8080".into())),
                auth: web_auth,
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
                enabled: mesh_section.enabled.unwrap_or(false),
                listen: SmolStr::new(mesh_section.listen.unwrap_or_else(|| "0.0.0.0:5200".into())),
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
            tasks,
        })
    }
}

impl IoToml {
    fn into_config(self) -> Result<IoConfig, RuntimeError> {
        let mut safe_state = IoSafeState::default();
        if let Some(entries) = self.io.safe_state {
            for entry in entries {
                let address = IoAddress::parse(&entry.address)?;
                let value = parse_io_value(&entry.value, address.size)?;
                safe_state.outputs.push((address, value));
            }
        }
        Ok(IoConfig {
            driver: SmolStr::new(self.io.driver),
            params: self.io.params,
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
