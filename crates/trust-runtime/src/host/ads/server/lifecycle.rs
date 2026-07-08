//! ADS server runtime lifecycle wiring.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use trust_ads_server::{
    ams_net_id_text_to_bytes, AdsIdentifyConfig, AdsIdentifyServer, AdsTcpServer,
    AdsTcpServerConfig, AdsTcpServerServices, DeviceInfo, SymbolSource,
};

use crate::control::ControlAuditEvent;
use crate::debug::{DebugControl, DebugSnapshot};
use crate::error::RuntimeError;
use crate::scheduler::{ResourceControl, StdClock};

use super::{
    AdsServerClientPolicy, AdsServerRuntimeAuditSink, AdsServerRuntimeConfig,
    AdsServerRuntimeWritePort, AdsServerSymbolSource, AdsServerValuePublisher,
};

const ADS_ROUTER_TCP_PORT: u16 = 48_898;
const ADS_ROUTER_UDP_PORT: u16 = 48_899;
const ADS_IDENTIFY_MAX_PACKET_BYTES: usize = 512;

/// Running ADS server runtime subsystem.
pub struct AdsServerRuntime {
    tcp: AdsTcpServer,
    identify: AdsIdentifyServer,
    identify_probe_addr: SocketAddr,
    policy: AdsServerClientPolicy,
    symbols: Arc<AdsServerSymbolSource>,
}

impl std::fmt::Debug for AdsServerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdsServerRuntime")
            .field("local_addr", &self.local_addr())
            .field("identify_addr", &self.identify_addr())
            .field("identify_bind_addr", &self.identify_bind_addr())
            .finish_non_exhaustive()
    }
}

impl AdsServerRuntime {
    /// Returns the actual TCP AMS router bind address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.tcp.local_addr()
    }

    /// Returns the concrete UDP identify address clients should probe.
    #[must_use]
    pub fn identify_addr(&self) -> SocketAddr {
        self.identify_probe_addr
    }

    /// Returns the actual UDP identify bind address.
    ///
    /// Broadcast discovery requires a wildcard UDP bind on many platforms even
    /// though the advertised ADS identity remains the configured concrete
    /// runtime address.
    #[must_use]
    pub fn identify_bind_addr(&self) -> SocketAddr {
        self.identify.local_addr()
    }

    /// Returns the number of currently connected ADS TCP clients.
    #[must_use]
    pub fn connected_clients(&self) -> usize {
        self.tcp.active_client_count()
    }

    /// Returns the live inbound client policy.
    #[must_use]
    pub fn policy(&self) -> AdsServerClientPolicy {
        self.policy.clone()
    }

    /// Returns the current ADS symbol version.
    #[must_use]
    pub fn symbol_version(&self) -> u32 {
        self.symbols.version()
    }

    /// Returns the number of currently exposed ADS symbols.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.symbols.snapshot().symbols.len()
    }

    /// Refreshes the ADS symbol table from the latest runtime snapshot.
    ///
    /// Existing ADS clients can detect the layout change through `SYM_VERSION`
    /// and re-resolve handles without the runtime rebinding the router socket.
    pub fn refresh_symbols(
        &self,
        config: &AdsServerRuntimeConfig,
        snapshot: &DebugSnapshot,
    ) -> Result<u32, RuntimeError> {
        self.symbols.refresh_from_runtime_snapshot(config, snapshot)
    }

    /// Requests shutdown of both ADS server sockets.
    pub fn shutdown(&mut self) {
        self.identify.shutdown();
        self.tcp.shutdown();
    }
}

impl Drop for AdsServerRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Starts the runtime ADS server if enabled.
///
/// # Errors
///
/// Fails closed when the runtime config is enabled but the AMS identity, write
/// port, TCP listener, or UDP identify responder cannot be created. Missing
/// initial runtime snapshots start as an empty/not-ready ADS server and can be
/// refreshed when a snapshot becomes available.
pub fn start_ads_server_runtime(
    resource_name: &str,
    config: &AdsServerRuntimeConfig,
    debug: DebugControl,
    resource: ResourceControl<StdClock>,
    snapshot_provider: Arc<dyn Fn() -> Option<DebugSnapshot> + Send + Sync>,
    audit_tx: Option<Sender<ControlAuditEvent>>,
) -> Result<Option<AdsServerRuntime>, RuntimeError> {
    start_ads_server_runtime_on_ports(
        resource_name,
        config,
        debug,
        resource,
        snapshot_provider,
        audit_tx,
        ADS_ROUTER_TCP_PORT,
        ADS_ROUTER_UDP_PORT,
    )
}

/// Starts the ADS server on an explicit TCP port.
///
/// Production uses 48898; tests use port 0 to avoid conflicts.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn start_ads_server_runtime_on_port(
    resource_name: &str,
    config: &AdsServerRuntimeConfig,
    debug: DebugControl,
    resource: ResourceControl<StdClock>,
    snapshot_provider: Arc<dyn Fn() -> Option<DebugSnapshot> + Send + Sync>,
    audit_tx: Option<Sender<ControlAuditEvent>>,
    tcp_port: u16,
) -> Result<Option<AdsServerRuntime>, RuntimeError> {
    let udp_port = if tcp_port == 0 {
        0
    } else {
        ADS_ROUTER_UDP_PORT
    };
    start_ads_server_runtime_on_ports(
        resource_name,
        config,
        debug,
        resource,
        snapshot_provider,
        audit_tx,
        tcp_port,
        udp_port,
    )
}

#[allow(clippy::too_many_arguments)]
fn start_ads_server_runtime_on_ports(
    resource_name: &str,
    config: &AdsServerRuntimeConfig,
    debug: DebugControl,
    resource: ResourceControl<StdClock>,
    snapshot_provider: Arc<dyn Fn() -> Option<DebugSnapshot> + Send + Sync>,
    audit_tx: Option<Sender<ControlAuditEvent>>,
    tcp_port: u16,
    udp_port: u16,
) -> Result<Option<AdsServerRuntime>, RuntimeError> {
    if !config.enabled {
        return Ok(None);
    }

    let symbols = Arc::new(match snapshot_provider() {
        Some(initial_snapshot) => {
            AdsServerSymbolSource::from_runtime_snapshot(config, &initial_snapshot)?
        }
        None => AdsServerSymbolSource::empty(config)?,
    });
    let policy = AdsServerClientPolicy::new(config);
    let publisher = Arc::new(AdsServerValuePublisher::new(
        config.clone(),
        snapshot_provider.clone(),
    ));
    let write_port = Arc::new(
        AdsServerRuntimeWritePort::new(config.clone(), policy.clone(), debug, resource).map_err(
            |err| RuntimeError::InvalidConfig(format!("runtime.ads_server: {err}").into()),
        )?,
    );
    let audit = Arc::new(AdsServerRuntimeAuditSink::new(audit_tx));
    let services = AdsTcpServerServices {
        symbols: symbols.clone(),
        values: publisher.clone(),
        writes: write_port,
        policy: Arc::new(policy.clone()),
        audit,
        clock: publisher,
    };
    let listen = config
        .listen
        .as_deref()
        .ok_or_else(|| RuntimeError::InvalidConfig("runtime.ads_server.listen is required".into()))?
        .parse::<IpAddr>()
        .map_err(|err| {
            RuntimeError::InvalidConfig(
                format!("runtime.ads_server.listen must be an IP address: {err}").into(),
            )
        })?;
    let ams_net_id = config.ams_net_id.as_ref().ok_or_else(|| {
        RuntimeError::InvalidConfig("runtime.ads_server.ams_net_id is required".into())
    })?;
    let local_net_id = ams_net_id_text_to_bytes(ams_net_id.0.as_str()).map_err(|err| {
        RuntimeError::InvalidConfig(
            format!("runtime.ads_server.ams_net_id is invalid: {err}").into(),
        )
    })?;
    let device_info = DeviceInfo {
        name: resource_name.chars().take(16).collect(),
        ..DeviceInfo::default()
    };
    let tcp = AdsTcpServer::start(
        AdsTcpServerConfig {
            bind_addr: SocketAddr::new(listen, tcp_port),
            local_net_id,
            ads_port: config.ads_port,
            max_frame_bytes: config.max_frame_bytes,
            max_clients: config.max_clients,
            max_subscriptions_per_client: config.max_subscriptions_per_client,
            max_total_subscriptions: config.max_total_subscriptions,
            max_sumup_items: config.max_sumup_items,
            max_write_bytes: config.max_write_bytes,
            max_handles_per_client: config.max_symbols.saturating_mul(4).max(64),
            min_notification_cycle: Duration::from_millis(config.min_notification_cycle_ms),
            read_timeout: Duration::from_millis(config.read_timeout_ms),
            idle_timeout: Duration::from_millis(config.idle_timeout_ms),
            device_info: device_info.clone(),
        },
        services,
    )
    .map_err(|err| RuntimeError::IoTransport(format!("ADS server startup failed: {err}").into()))?;
    let identify_bind_ip = if listen.is_ipv4() {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V6(Ipv6Addr::UNSPECIFIED)
    };
    let identify = AdsIdentifyServer::start(AdsIdentifyConfig {
        bind_addr: SocketAddr::new(identify_bind_ip, udp_port),
        local_net_id,
        source_port: 0,
        hostname: resource_name.to_string(),
        twincat_version: (
            device_info.major_version,
            device_info.minor_version,
            device_info.build,
        ),
        max_packet_bytes: ADS_IDENTIFY_MAX_PACKET_BYTES,
    })
    .map_err(|err| {
        RuntimeError::IoTransport(format!("ADS identify startup failed: {err}").into())
    })?;
    let identify_probe_addr = SocketAddr::new(listen, identify.local_addr().port());
    Ok(Some(AdsServerRuntime {
        tcp,
        identify,
        identify_probe_addr,
        policy,
        symbols,
    }))
}
