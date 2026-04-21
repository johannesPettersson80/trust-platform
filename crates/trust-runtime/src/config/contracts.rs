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
