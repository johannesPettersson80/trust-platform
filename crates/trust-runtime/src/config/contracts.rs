#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub bundle_version: u32,
    pub resource_name: SmolStr,
    pub cycle_interval: Duration,
    pub execution_backend: ExecutionBackend,
    pub execution_backend_source: ExecutionBackendSource,
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
    pub runtime_cloud_profile: RuntimeCloudProfile,
    pub runtime_cloud_wan_allow_write: Vec<RuntimeCloudWanAllowRule>,
    pub runtime_cloud_link_preferences: Vec<RuntimeCloudLinkPreferenceRule>,
    pub realtime: LinuxRtConfig,
    pub openot: OpenOtTelemetryConfig,
    pub observability: HistorianConfig,
    pub hmi_persistence: HmiPersistenceConfig,
    pub ads: AdsRuntimeConfig,
    pub ads_server: AdsServerRuntimeConfig,
    pub opcua_client: OpcUaClientRuntimeConfig,
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
    pub host_group: Option<SmolStr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshRole {
    Peer,
    Client,
    Router,
}

impl MeshRole {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "peer" => Ok(Self::Peer),
            "client" => Ok(Self::Client),
            "router" => Ok(Self::Router),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.mesh.role '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Peer => "peer",
            Self::Client => "client",
            Self::Router => "router",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshConfig {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCloudProfile {
    Dev,
    Plant,
    Wan,
}

impl RuntimeCloudProfile {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "dev" => Ok(Self::Dev),
            "plant" => Ok(Self::Plant),
            "wan" => Ok(Self::Wan),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.cloud.profile '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Plant => "plant",
            Self::Wan => "wan",
        }
    }

    #[must_use]
    pub const fn requires_secure_transport(self) -> bool {
        !matches!(self, Self::Dev)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCloudWanAllowRule {
    pub action: SmolStr,
    pub target: SmolStr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCloudPreferredTransport {
    Realtime,
    Zenoh,
    Mesh,
    Mqtt,
    ModbusTcp,
    OpcUa,
    Discovery,
    Web,
}

impl RuntimeCloudPreferredTransport {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "realtime" => Ok(Self::Realtime),
            "zenoh" => Ok(Self::Zenoh),
            "mesh" => Ok(Self::Mesh),
            "mqtt" => Ok(Self::Mqtt),
            "modbus-tcp" | "modbus_tcp" => Ok(Self::ModbusTcp),
            "opcua" => Ok(Self::OpcUa),
            "discovery" => Ok(Self::Discovery),
            "web" => Ok(Self::Web),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.cloud.links.transports[].transport '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::Zenoh => "zenoh",
            Self::Mesh => "mesh",
            Self::Mqtt => "mqtt",
            Self::ModbusTcp => "modbus-tcp",
            Self::OpcUa => "opcua",
            Self::Discovery => "discovery",
            Self::Web => "web",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCloudLinkPreferenceRule {
    pub source: SmolStr,
    pub target: SmolStr,
    pub transport: RuntimeCloudPreferredTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOtTelemetryFenceMode {
    Fenced,
    Unfenced,
}

impl OpenOtTelemetryFenceMode {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "fenced" => Ok(Self::Fenced),
            "unfenced" => Ok(Self::Unfenced),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.openot.fence_mode '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fenced => "fenced",
            Self::Unfenced => "unfenced",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOtTelemetrySource {
    Heartbeat,
    StFb,
}

impl OpenOtTelemetrySource {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "heartbeat" => Ok(Self::Heartbeat),
            "st-fb" => Ok(Self::StFb),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.openot.source '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Heartbeat => "heartbeat",
            Self::StFb => "st-fb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtTelemetryConfig {
    pub enabled: bool,
    pub path: PathBuf,
    pub capacity: usize,
    pub fence_mode: OpenOtTelemetryFenceMode,
    pub allow_unfenced_for_proof: bool,
    pub source: OpenOtTelemetrySource,
    /// Back-compat alias for a single ST-FB producer path.
    pub producer_instance: Option<SmolStr>,
    /// Normalized ST-FB producer paths drained in stable order.
    pub producer_instances: Vec<SmolStr>,
    pub persistence: OpenOtPersistenceConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtPersistenceConfig {
    pub enabled: bool,
    pub backend: Option<OpenOtPersistenceBackend>,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub queue_capacity: usize,
    pub shutdown_timeout_ms: u64,
    pub retry_initial_ms: u64,
    pub retry_max_ms: u64,
    pub retry_multiplier: u8,
    pub retry_max_attempts: u32,
    pub sqlite: Option<OpenOtSqlitePersistenceConfig>,
    pub postgresql: Option<OpenOtPostgreSqlPersistenceConfig>,
    pub timescaledb: Option<OpenOtTimescaleDbPersistenceConfig>,
    pub mysql: Option<OpenOtMySqlPersistenceConfig>,
    pub sqlserver: Option<OpenOtSqlServerPersistenceConfig>,
    pub influxdb3: Option<OpenOtInfluxDb3PersistenceConfig>,
}

impl Default for OpenOtPersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: None,
            batch_size: 256,
            flush_interval_ms: 250,
            queue_capacity: 4096,
            shutdown_timeout_ms: 5000,
            retry_initial_ms: 250,
            retry_max_ms: 30000,
            retry_multiplier: 2,
            retry_max_attempts: 20,
            sqlite: None,
            postgresql: None,
            timescaledb: None,
            mysql: None,
            sqlserver: None,
            influxdb3: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOtPersistenceBackend {
    Sqlite,
    PostgreSql,
    TimescaleDb,
    MySql,
    SqlServer,
    InfluxDb3,
}

impl OpenOtPersistenceBackend {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "sqlite" => Ok(Self::Sqlite),
            "postgresql" => Ok(Self::PostgreSql),
            "timescaledb" => Ok(Self::TimescaleDb),
            "mysql" => Ok(Self::MySql),
            "sqlserver" => Ok(Self::SqlServer),
            "influxdb3" => Ok(Self::InfluxDb3),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.openot.persistence.backend '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::PostgreSql => "postgresql",
            Self::TimescaleDb => "timescaledb",
            Self::MySql => "mysql",
            Self::SqlServer => "sqlserver",
            Self::InfluxDb3 => "influxdb3",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtSqlitePersistenceConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOtPersistenceTlsMode {
    Require,
}

impl OpenOtPersistenceTlsMode {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "require" => Ok(Self::Require),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid OpenOT persistence TLS mode '{text}'").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtPostgreSqlPersistenceConfig {
    pub connection_url_env: SmolStr,
    pub schema: SmolStr,
    pub tls: OpenOtPersistenceTlsMode,
    pub ca_cert_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtTimescaleDbPersistenceConfig {
    pub connection_url_env: SmolStr,
    pub schema: SmolStr,
    pub tls: OpenOtPersistenceTlsMode,
    pub ca_cert_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtMySqlPersistenceConfig {
    pub connection_url_env: SmolStr,
    pub database: SmolStr,
    pub tls: OpenOtPersistenceTlsMode,
    pub ca_cert_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtSqlServerPersistenceConfig {
    pub connection_url_env: SmolStr,
    pub schema: SmolStr,
    pub tls: OpenOtPersistenceTlsMode,
    pub ca_cert_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOtInfluxDb3PersistenceConfig {
    pub host_env: SmolStr,
    pub token_env: SmolStr,
    pub database: SmolStr,
    pub spool_path: PathBuf,
    pub max_bytes: u64,
    pub ca_cert_path: Option<PathBuf>,
}

impl Default for OpenOtTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: PathBuf::new(),
            capacity: 4096,
            fence_mode: OpenOtTelemetryFenceMode::Fenced,
            allow_unfenced_for_proof: false,
            source: OpenOtTelemetrySource::Heartbeat,
            producer_instance: None,
            producer_instances: Vec::new(),
            persistence: OpenOtPersistenceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsRuntimeConfig {
    pub enabled: bool,
    pub config_path: PathBuf,
    pub worker_tick_interval: Duration,
}

impl Default for AdsRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            config_path: PathBuf::from("ads.toml"),
            worker_tick_interval: Duration::from_millis(20),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientRuntimeConfig {
    pub enabled: bool,
    pub config_path: PathBuf,
    pub poll_interval: Duration,
}

impl Default for OpcUaClientRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            config_path: PathBuf::from("opcua_client.toml"),
            poll_interval: Duration::from_millis(250),
        }
    }
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
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeBundle {
    pub root: PathBuf,
    pub runtime: RuntimeConfig,
    pub io: IoConfig,
    pub ads: Option<crate::ads::AdsClientConfig>,
    pub ads_config_hash: Option<String>,
    pub opcua_client: Option<crate::opcua::OpcUaClientConfig>,
    pub opcua_client_config_hash: Option<String>,
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

#[cfg(test)]
#[path = "contracts/contract_tests.rs"]
mod contract_tests;
