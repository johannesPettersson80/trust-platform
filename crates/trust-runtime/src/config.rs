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
    discovery: Option<DiscoverySection>,
    mesh: Option<MeshSection>,
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
    auth_token: Option<String>,
    publish: Option<Vec<String>>,
    subscribe: Option<IndexMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoToml {
    io: IoSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoSection {
    driver: String,
    params: toml::Value,
    safe_state: Option<Vec<IoSafeEntry>>,
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
        if self.io.driver.trim().is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "io.driver must not be empty".into(),
            ));
        }
        if !self.io.params.is_table() {
            return Err(RuntimeError::InvalidConfig(
                "io.params must be a table".into(),
            ));
        }
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

[runtime.discovery]
enabled = true
service_name = "truST"
advertise = true
interfaces = ["eth0"]

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
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
}
