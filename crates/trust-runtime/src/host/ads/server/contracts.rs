use smol_str::SmolStr;
use trust_ads_core::AmsNetId;

/// Default logical AMS target port for the ADS server.
pub const DEFAULT_ADS_SERVER_ADS_PORT: u16 = 851;
/// Default maximum number of exposed symbols.
pub const DEFAULT_ADS_SERVER_MAX_SYMBOLS: usize = 256;
/// Default maximum concurrent ADS clients.
pub const DEFAULT_ADS_SERVER_MAX_CLIENTS: usize = 8;
/// Default maximum subscriptions per client.
pub const DEFAULT_ADS_SERVER_MAX_SUBSCRIPTIONS_PER_CLIENT: usize = 64;
/// Default maximum total subscriptions.
pub const DEFAULT_ADS_SERVER_MAX_TOTAL_SUBSCRIPTIONS: usize = 256;
/// Default maximum AMS frame bytes.
pub const DEFAULT_ADS_SERVER_MAX_FRAME_BYTES: usize = 65_536;
/// Default maximum sum-up items per request.
pub const DEFAULT_ADS_SERVER_MAX_SUMUP_ITEMS: usize = 512;
/// Default maximum write payload bytes.
pub const DEFAULT_ADS_SERVER_MAX_WRITE_BYTES: usize = 8_192;
/// Default maximum string payload bytes.
pub const DEFAULT_ADS_SERVER_MAX_STRING_BYTES: usize = 4_096;
/// Default read timeout in milliseconds.
pub const DEFAULT_ADS_SERVER_READ_TIMEOUT_MS: u64 = 5_000;
/// Default idle timeout in milliseconds.
pub const DEFAULT_ADS_SERVER_IDLE_TIMEOUT_MS: u64 = 60_000;
/// Default minimum notification cycle in milliseconds.
pub const DEFAULT_ADS_SERVER_MIN_NOTIFICATION_CYCLE_MS: u64 = 50;

/// Source binding required for production plain ADS clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdsServerSourcePin {
    /// One concrete source IP address.
    Ip(SmolStr),
    /// One source CIDR range.
    Cidr(SmolStr),
    /// AMS Net ID only. Accepted only when explicitly enabled for lab/loopback.
    Unpinned,
}

/// One allowed external ADS client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsServerClientConfig {
    /// Client AMS Net ID asserted in AMS headers.
    pub ams_net_id: AmsNetId,
    /// Required source binding.
    pub source: AdsServerSourcePin,
}

/// Parsed `[runtime.ads_server]` config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsServerRuntimeConfig {
    /// Whether the ADS server is enabled.
    pub enabled: bool,
    /// Explicit local bind IP. `None` when disabled and not configured.
    pub listen: Option<SmolStr>,
    /// Logical AMS target port served by truST.
    pub ads_port: u16,
    /// Local AMS Net ID used by the server.
    pub ams_net_id: Option<AmsNetId>,
    /// Required acknowledgement for plain ADS.
    pub insecure_transport: bool,
    /// Global write gate.
    pub writes_enabled: bool,
    /// Optional namespace prefix for exposed symbols.
    pub symbol_namespace: SmolStr,
    /// Lab/loopback escape hatch for AMS-Net-ID-only clients.
    pub allow_unpinned_clients: bool,
    /// Explicit override allowing public/NAT-suspect bind addresses.
    pub unsafe_allow_public_bind: bool,
    /// Exposed variable glob allowlist.
    pub expose: Vec<SmolStr>,
    /// Writable variable glob allowlist.
    pub writable: Vec<SmolStr>,
    /// Allowed external ADS clients.
    pub clients: Vec<AdsServerClientConfig>,
    /// Maximum exposed symbols.
    pub max_symbols: usize,
    /// Maximum concurrent clients.
    pub max_clients: usize,
    /// Maximum subscriptions per client.
    pub max_subscriptions_per_client: usize,
    /// Maximum subscriptions across all clients.
    pub max_total_subscriptions: usize,
    /// Maximum AMS frame bytes.
    pub max_frame_bytes: usize,
    /// Maximum sum-up items per request.
    pub max_sumup_items: usize,
    /// Maximum write payload bytes.
    pub max_write_bytes: usize,
    /// Maximum string payload bytes.
    pub max_string_bytes: usize,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: u64,
    /// Idle timeout in milliseconds.
    pub idle_timeout_ms: u64,
    /// Minimum accepted notification cycle in milliseconds.
    pub min_notification_cycle_ms: u64,
}

impl Default for AdsServerRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: None,
            ads_port: DEFAULT_ADS_SERVER_ADS_PORT,
            ams_net_id: None,
            insecure_transport: false,
            writes_enabled: false,
            symbol_namespace: SmolStr::new(""),
            allow_unpinned_clients: false,
            unsafe_allow_public_bind: false,
            expose: Vec::new(),
            writable: Vec::new(),
            clients: Vec::new(),
            max_symbols: DEFAULT_ADS_SERVER_MAX_SYMBOLS,
            max_clients: DEFAULT_ADS_SERVER_MAX_CLIENTS,
            max_subscriptions_per_client: DEFAULT_ADS_SERVER_MAX_SUBSCRIPTIONS_PER_CLIENT,
            max_total_subscriptions: DEFAULT_ADS_SERVER_MAX_TOTAL_SUBSCRIPTIONS,
            max_frame_bytes: DEFAULT_ADS_SERVER_MAX_FRAME_BYTES,
            max_sumup_items: DEFAULT_ADS_SERVER_MAX_SUMUP_ITEMS,
            max_write_bytes: DEFAULT_ADS_SERVER_MAX_WRITE_BYTES,
            max_string_bytes: DEFAULT_ADS_SERVER_MAX_STRING_BYTES,
            read_timeout_ms: DEFAULT_ADS_SERVER_READ_TIMEOUT_MS,
            idle_timeout_ms: DEFAULT_ADS_SERVER_IDLE_TIMEOUT_MS,
            min_notification_cycle_ms: DEFAULT_ADS_SERVER_MIN_NOTIFICATION_CYCLE_MS,
        }
    }
}
