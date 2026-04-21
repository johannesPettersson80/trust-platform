use super::*;

mod validation;

pub(super) fn parse_runtime_toml_from_text(
    text: &str,
    file_name: &str,
) -> Result<RuntimeConfig, RuntimeError> {
    let raw: RuntimeToml = toml::from_str(text)
        .map_err(|err| RuntimeError::InvalidConfig(format!("{file_name}: {err}").into()))?;
    raw.into_config()
        .map_err(|err| prefix_invalid_config(file_name, err))
}

pub(super) fn parse_io_toml_from_text(
    text: &str,
    file_name: &str,
) -> Result<IoConfig, RuntimeError> {
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
    execution_backend: Option<String>,
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
    cloud: Option<RuntimeCloudSection>,
    realtime: Option<RealtimeSection>,
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
    host_group: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshSection {
    enabled: Option<bool>,
    role: Option<String>,
    listen: Option<String>,
    connect: Option<Vec<String>>,
    tls: Option<bool>,
    auth_token: Option<String>,
    publish: Option<Vec<String>>,
    subscribe: Option<IndexMap<String, String>>,
    zenohd_version: Option<String>,
    plugin_versions: Option<IndexMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeCloudSection {
    profile: Option<String>,
    wan: Option<RuntimeCloudWanSection>,
    links: Option<RuntimeCloudLinksSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeCloudWanSection {
    allow_write: Option<Vec<RuntimeCloudWanAllowRuleSection>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeCloudWanAllowRuleSection {
    action: String,
    target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeCloudLinksSection {
    transports: Option<Vec<RuntimeCloudLinkPreferenceRuleSection>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeCloudLinkPreferenceRuleSection {
    source: String,
    target: String,
    transport: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RealtimeSection {
    enabled: Option<bool>,
    require_preempt_rt_kernel: Option<bool>,
    lock_memory: Option<bool>,
    scheduler: Option<String>,
    priority: Option<u8>,
    cpu_affinity: Option<Vec<usize>>,
    strict: Option<bool>,
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
