//! Runtime-neutral AMS/TCP listener.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use trust_ads_core::AmsNetId;

use crate::{
    ams_net_id_bytes_to_text, AdsErrorCode, AmsHeader, AmsNetIdBytes, AmsParseError, AmsState,
    AmsTcpFrame, AuditSink, ClientId, ClientPolicy, Clock, CommandContext, CommandDispatcher,
    CommandId, DeviceInfo, NotificationReceiver, NotificationSampler, RuntimeWritePort,
    SymbolSource, ValueIo, AMS_TCP_HEADER_LEN,
};

/// AMS system service port probed by `TwinCAT` tooling before it browses PLC runtime port 851.
const AMS_SYSTEM_SERVICE_PORT: u16 = 10_000;
/// AMS router service port queried by `TwinCAT` tooling for router metadata.
const AMS_ROUTER_PORT: u16 = 1;
/// TCOM server port probed by `TwinCAT` tooling while opening a target browser node.
const AMS_TCOM_SERVER_PORT: u16 = 10;
const ROUTER_METADATA_INDEX_GROUP: u32 = 1;
const ROUTER_METADATA_INDEX_OFFSET: u32 = 1;
const ROUTER_METADATA_BYTES: [u8; 40] = [
    0x90, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0xff, 0xe9, 0xff, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x2d, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const ROUTER_TCPIP_METADATA_INDEX_GROUP: u32 = 2;
const ROUTER_TCPIP_METADATA_INDEX_OFFSET: u32 = 1;
const ROUTER_TCPIP_METADATA_ENTRY_SIZE: usize = 48;
const ROUTER_TCPIP_METADATA_TABLE_SIZE: usize = 2_160;

/// Runtime-neutral ADS server services.
#[derive(Clone)]
pub struct AdsTcpServerServices {
    /// Symbol provider.
    pub symbols: Arc<dyn SymbolSource + Send + Sync>,
    /// Value reader.
    pub values: Arc<dyn ValueIo + Send + Sync>,
    /// Runtime write port.
    pub writes: Arc<dyn RuntimeWritePort + Send + Sync>,
    /// Client policy.
    pub policy: Arc<dyn ClientPolicy + Send + Sync>,
    /// Audit sink.
    pub audit: Arc<dyn AuditSink + Send + Sync>,
    /// Clock.
    pub clock: Arc<dyn Clock + Send + Sync>,
}

/// TCP listener configuration for an ADS target.
#[derive(Clone)]
pub struct AdsTcpServerConfig {
    /// Bind address, normally `<runtime-host-ip>:48898`.
    pub bind_addr: SocketAddr,
    /// Local AMS Net ID served by this target.
    pub local_net_id: AmsNetIdBytes,
    /// Logical AMS port served by this target, normally 851.
    pub ads_port: u16,
    /// Maximum AMS header + payload bytes accepted from one frame.
    pub max_frame_bytes: usize,
    /// Maximum concurrent TCP clients.
    pub max_clients: usize,
    /// Maximum notifications registered by one client connection.
    pub max_subscriptions_per_client: usize,
    /// Maximum notifications registered across all client connections.
    pub max_total_subscriptions: usize,
    /// Maximum sum-up request items per client command.
    pub max_sumup_items: usize,
    /// Maximum write payload bytes per client command.
    pub max_write_bytes: usize,
    /// Maximum symbol handles held by one client connection.
    pub max_handles_per_client: usize,
    /// Minimum server notification sampling interval.
    pub min_notification_cycle: Duration,
    /// Per-read timeout.
    pub read_timeout: Duration,
    /// Idle timeout before a client connection is closed.
    pub idle_timeout: Duration,
    /// Device info returned to ADS clients.
    pub device_info: DeviceInfo,
}

/// Running ADS TCP server.
pub struct AdsTcpServer {
    local_addr: SocketAddr,
    stop: Arc<AtomicBool>,
    active_clients: Arc<AtomicUsize>,
    join: Option<JoinHandle<()>>,
}

impl AdsTcpServer {
    /// Starts the listener thread.
    ///
    /// # Errors
    ///
    /// Returns a listener error when the TCP socket cannot bind or configure.
    pub fn start(
        config: AdsTcpServerConfig,
        services: AdsTcpServerServices,
    ) -> Result<Self, AdsTcpServerError> {
        let listener = TcpListener::bind(config.bind_addr).map_err(AdsTcpServerError::Bind)?;
        listener
            .set_nonblocking(true)
            .map_err(AdsTcpServerError::Configure)?;
        let local_addr = listener
            .local_addr()
            .map_err(AdsTcpServerError::Configure)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let active_clients = Arc::new(AtomicUsize::new(0));
        let active_clients_thread = active_clients.clone();
        let join = thread::Builder::new()
            .name("trust-runtime-ads-server".to_string())
            .spawn(move || {
                run_accept_loop(
                    listener,
                    config,
                    services,
                    stop_thread,
                    active_clients_thread,
                );
            })
            .map_err(AdsTcpServerError::Spawn)?;
        Ok(Self {
            local_addr,
            stop,
            active_clients,
            join: Some(join),
        })
    }

    /// Returns the actual bound socket address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns the number of currently connected TCP clients.
    #[must_use]
    pub fn active_client_count(&self) -> usize {
        self.active_clients.load(Ordering::SeqCst)
    }

    /// Requests shutdown and joins the listener thread.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for AdsTcpServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Error returned by the ADS TCP listener.
#[derive(Debug)]
pub enum AdsTcpServerError {
    /// Failed to bind the listener socket.
    Bind(io::Error),
    /// Failed to configure a socket.
    Configure(io::Error),
    /// Failed to spawn the listener thread.
    Spawn(io::Error),
}

impl core::fmt::Display for AdsTcpServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(err) => write!(f, "failed to bind ADS TCP listener: {err}"),
            Self::Configure(err) => write!(f, "failed to configure ADS TCP listener: {err}"),
            Self::Spawn(err) => write!(f, "failed to spawn ADS TCP listener: {err}"),
        }
    }
}

impl std::error::Error for AdsTcpServerError {}

#[allow(clippy::needless_pass_by_value)]
fn run_accept_loop(
    listener: TcpListener,
    config: AdsTcpServerConfig,
    services: AdsTcpServerServices,
    stop: Arc<AtomicBool>,
    active_clients: Arc<AtomicUsize>,
) {
    let total_subscriptions = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if active_clients.load(Ordering::SeqCst) >= config.max_clients {
                    drop(stream);
                    continue;
                }
                active_clients.fetch_add(1, Ordering::SeqCst);
                let services = services.clone();
                let config = config.clone();
                let active_clients_for_thread = active_clients.clone();
                let total_subscriptions_for_thread = total_subscriptions.clone();
                thread::spawn(move || {
                    handle_client(
                        stream,
                        peer,
                        config,
                        services,
                        total_subscriptions_for_thread,
                    );
                    active_clients_for_thread.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn handle_client(
    mut stream: TcpStream,
    peer: SocketAddr,
    config: AdsTcpServerConfig,
    services: AdsTcpServerServices,
    total_subscriptions: Arc<AtomicUsize>,
) {
    let _ = stream.set_read_timeout(Some(config.read_timeout));
    let _ = stream.set_write_timeout(Some(config.read_timeout));
    let Ok(write_stream) = stream.try_clone() else {
        return;
    };
    let writer = Arc::new(Mutex::new(write_stream));
    let notification_state = Arc::new(Mutex::new(Vec::new()));
    let notification_stop = Arc::new(AtomicBool::new(false));
    let notification_join = spawn_notification_loop(
        writer.clone(),
        notification_state.clone(),
        config.clone(),
        services.clone(),
        notification_stop.clone(),
    );
    let mut dispatcher = CommandDispatcher::with_limits(
        config.max_subscriptions_per_client,
        config.max_sumup_items,
        config.max_write_bytes,
        config.max_handles_per_client,
    );
    let mut last_activity = Instant::now();
    while let Ok(Some(frame)) = read_frame(
        &mut stream,
        config.max_frame_bytes,
        last_activity,
        config.idle_timeout,
    ) {
        last_activity = Instant::now();
        let before_notifications = dispatcher.notification_count();
        let reservation = if frame.header.command_id == CommandId::AddDeviceNotification {
            try_reserve_subscription(&total_subscriptions, config.max_total_subscriptions)
        } else {
            SubscriptionReservation::NotNeeded
        };
        let response = if matches!(reservation, SubscriptionReservation::Rejected) {
            response_for_payload(&frame, add_notification_limit_payload())
        } else {
            response_for_frame(&frame, peer, &config, &services, &mut dispatcher)
        };
        let after_notifications = dispatcher.notification_count();
        reconcile_subscription_reservation(
            &total_subscriptions,
            reservation,
            before_notifications,
            after_notifications,
        );
        if after_notifications < before_notifications {
            total_subscriptions
                .fetch_sub(before_notifications - after_notifications, Ordering::SeqCst);
        }
        if after_notifications != before_notifications {
            sync_notification_state(&notification_state, &dispatcher);
        }
        let Ok(bytes) = response.to_bytes() else {
            break;
        };
        let Ok(mut writer) = writer.lock() else {
            break;
        };
        if writer.write_all(&bytes).is_err() {
            break;
        }
    }
    notification_stop.store(true, Ordering::SeqCst);
    if let Some(join) = notification_join {
        let _ = join.join();
    }
    total_subscriptions.fetch_sub(dispatcher.notification_count(), Ordering::SeqCst);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SubscriptionReservation {
    NotNeeded,
    Reserved,
    Rejected,
}

fn try_reserve_subscription(
    total_subscriptions: &AtomicUsize,
    max_total_subscriptions: usize,
) -> SubscriptionReservation {
    if max_total_subscriptions == 0 {
        return SubscriptionReservation::Rejected;
    }
    let result = total_subscriptions.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
        (current < max_total_subscriptions).then_some(current + 1)
    });
    if result.is_ok() {
        SubscriptionReservation::Reserved
    } else {
        SubscriptionReservation::Rejected
    }
}

fn reconcile_subscription_reservation(
    total_subscriptions: &AtomicUsize,
    reservation: SubscriptionReservation,
    before_notifications: usize,
    after_notifications: usize,
) {
    if matches!(reservation, SubscriptionReservation::Reserved)
        && after_notifications <= before_notifications
    {
        total_subscriptions.fetch_sub(1, Ordering::SeqCst);
    }
}

fn add_notification_limit_payload() -> Vec<u8> {
    let mut payload = Vec::with_capacity(8);
    payload.extend_from_slice(&AdsErrorCode::NoMemory.value().to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload
}

fn sync_notification_state(
    notification_state: &Mutex<Vec<crate::ActiveNotification>>,
    dispatcher: &CommandDispatcher,
) {
    if let Ok(mut state) = notification_state.lock() {
        *state = dispatcher.active_notifications();
    }
}

fn spawn_notification_loop(
    writer: Arc<Mutex<TcpStream>>,
    notification_state: Arc<Mutex<Vec<crate::ActiveNotification>>>,
    config: AdsTcpServerConfig,
    services: AdsTcpServerServices,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    thread::Builder::new()
        .name("trust-runtime-ads-server-notify".to_string())
        .spawn(move || {
            run_notification_loop(writer, notification_state, config, services, stop);
        })
        .ok()
}

#[allow(clippy::needless_pass_by_value)]
fn run_notification_loop(
    writer: Arc<Mutex<TcpStream>>,
    notification_state: Arc<Mutex<Vec<crate::ActiveNotification>>>,
    config: AdsTcpServerConfig,
    services: AdsTcpServerServices,
    stop: Arc<AtomicBool>,
) {
    let mut sampler = NotificationSampler::default();
    let mut next_invoke_id = 1_u32;
    let sleep_interval = config.min_notification_cycle.max(Duration::from_millis(1));
    while !stop.load(Ordering::SeqCst) {
        thread::sleep(sleep_interval);
        let registrations = notification_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default();
        if registrations.is_empty() {
            continue;
        }
        let grouped = group_notifications_by_receiver(registrations);
        for (receiver, registrations) in grouped {
            let Ok(Some(stamp)) = sampler.sample(
                registrations.as_slice(),
                services.symbols.as_ref(),
                services.values.as_ref(),
                services.clock.now_ms(),
                config.min_notification_cycle,
            ) else {
                continue;
            };
            let Ok(payload) = crate::build_device_notification_payload(&[stamp]) else {
                continue;
            };
            let Ok(data_length) = u32::try_from(payload.len()) else {
                continue;
            };
            let frame = AmsTcpFrame {
                header: crate::AmsHeader {
                    target_net_id: receiver.net_id,
                    target_port: receiver.port,
                    source_net_id: config.local_net_id,
                    source_port: config.ads_port,
                    command_id: CommandId::DeviceNotification,
                    state: AmsState::Request,
                    data_length,
                    error_code: 0,
                    invoke_id: next_invoke_id,
                },
                payload,
            };
            next_invoke_id = next_invoke_id.wrapping_add(1).max(1);
            let Ok(bytes) = frame.to_bytes() else {
                continue;
            };
            let Ok(mut writer) = writer.lock() else {
                return;
            };
            if writer.write_all(&bytes).is_err() {
                return;
            }
        }
    }
}

fn group_notifications_by_receiver(
    registrations: Vec<crate::ActiveNotification>,
) -> BTreeMap<NotificationReceiver, Vec<crate::ActiveNotification>> {
    let mut grouped = BTreeMap::new();
    for registration in registrations {
        grouped
            .entry(registration.receiver)
            .or_insert_with(Vec::new)
            .push(registration);
    }
    grouped
}

fn read_frame(
    stream: &mut TcpStream,
    max_frame_bytes: usize,
    last_activity: Instant,
    idle_timeout: Duration,
) -> io::Result<Option<AmsTcpFrame>> {
    let mut prefix = [0_u8; AMS_TCP_HEADER_LEN];
    if !read_exact_or_timeout(stream, &mut prefix, last_activity, idle_timeout)? {
        return Ok(None);
    }
    let ams_len = u32::from_le_bytes([prefix[2], prefix[3], prefix[4], prefix[5]]) as usize;
    if ams_len > max_frame_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            AmsParseError::FrameTooLarge {
                length: ams_len,
                max: max_frame_bytes,
            },
        ));
    }
    let mut bytes = Vec::with_capacity(AMS_TCP_HEADER_LEN + ams_len);
    bytes.extend_from_slice(&prefix);
    bytes.resize(AMS_TCP_HEADER_LEN + ams_len, 0);
    if !read_exact_or_timeout(
        stream,
        &mut bytes[AMS_TCP_HEADER_LEN..],
        last_activity,
        idle_timeout,
    )? {
        return Ok(None);
    }
    AmsTcpFrame::parse(&bytes, max_frame_bytes)
        .map(Some)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn read_exact_or_timeout(
    stream: &mut TcpStream,
    bytes: &mut [u8],
    last_activity: Instant,
    idle_timeout: Duration,
) -> io::Result<bool> {
    let mut read = 0_usize;
    while read < bytes.len() {
        match stream.read(&mut bytes[read..]) {
            Ok(0) => return Ok(false),
            Ok(count) => read += count,
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                if last_activity.elapsed() >= idle_timeout {
                    return Ok(false);
                }
            }
            Err(err) => return Err(err),
        }
    }
    Ok(true)
}

fn response_for_frame(
    frame: &AmsTcpFrame,
    peer: SocketAddr,
    config: &AdsTcpServerConfig,
    services: &AdsTcpServerServices,
    dispatcher: &mut CommandDispatcher,
) -> AmsTcpFrame {
    if frame.header.state != AmsState::Request || frame.header.target_net_id != config.local_net_id
    {
        return empty_error_response(frame, AdsErrorCode::AccessDenied);
    }
    if !target_port_is_known(frame.header.target_port, config.ads_port) {
        return empty_error_response(frame, AdsErrorCode::AccessDenied);
    }
    if !target_port_accepts_command(
        frame.header.target_port,
        config.ads_port,
        frame.header.command_id,
    ) {
        return empty_error_response(frame, AdsErrorCode::ServiceNotSupported);
    }

    let client = ClientId::new(AmsNetId::new(ams_net_id_bytes_to_text(
        frame.header.source_net_id,
    )))
    .with_source_ip(peer.ip().to_string());
    let mut ctx = CommandContext::new(
        &client,
        services.symbols.as_ref(),
        services.values.as_ref(),
        services.writes.as_ref(),
        services.policy.as_ref(),
        services.audit.as_ref(),
        services.clock.as_ref(),
    )
    .with_notification_receiver(NotificationReceiver {
        net_id: frame.header.source_net_id,
        port: frame.header.source_port,
    });
    ctx.device_info = config.device_info.clone();
    ctx.ads_port = config.ads_port;
    if frame.header.target_port == AMS_ROUTER_PORT {
        return router_service_response(frame, &ctx, config.ads_port);
    }
    if frame.header.target_port == AMS_TCOM_SERVER_PORT {
        return tcom_service_response(frame, &ctx);
    }
    let payload = dispatcher.dispatch(frame.header.command_id, frame.payload.as_slice(), &ctx);
    let header = frame
        .header
        .response_for(payload.len(), 0)
        .unwrap_or_else(|_| no_memory_response_header(&frame.header));
    AmsTcpFrame { header, payload }
}

fn target_port_accepts_command(target_port: u16, runtime_port: u16, command_id: CommandId) -> bool {
    if target_port == runtime_port {
        return true;
    }
    match target_port {
        AMS_SYSTEM_SERVICE_PORT => {
            matches!(command_id, CommandId::ReadDeviceInfo | CommandId::ReadState)
        }
        AMS_ROUTER_PORT => matches!(
            command_id,
            CommandId::ReadDeviceInfo | CommandId::ReadState | CommandId::Read
        ),
        AMS_TCOM_SERVER_PORT => matches!(
            command_id,
            CommandId::ReadDeviceInfo | CommandId::ReadState | CommandId::ReadWrite
        ),
        _ => false,
    }
}

fn target_port_is_known(target_port: u16, runtime_port: u16) -> bool {
    target_port == runtime_port
        || target_port == AMS_SYSTEM_SERVICE_PORT
        || target_port == AMS_ROUTER_PORT
        || target_port == AMS_TCOM_SERVER_PORT
}

fn router_service_response(
    frame: &AmsTcpFrame,
    ctx: &CommandContext<'_>,
    runtime_port: u16,
) -> AmsTcpFrame {
    if !ctx.policy.permits(ctx.client) {
        return response_for_payload_with_error(
            frame,
            read_payload(AdsErrorCode::AccessDenied, &[]),
            0,
        );
    }
    let payload = match frame.header.command_id {
        CommandId::ReadDeviceInfo => device_info_payload(&router_device_info()),
        CommandId::ReadState => read_state_payload(),
        CommandId::Read => router_read_payload(frame.payload.as_slice(), runtime_port),
        _ => read_payload(AdsErrorCode::ServiceNotSupported, &[]),
    };
    response_for_payload(frame, payload)
}

fn router_device_info() -> DeviceInfo {
    DeviceInfo {
        major_version: 3,
        minor_version: 1,
        build: 0,
        name: "TCROUTER".to_string(),
    }
}

fn tcom_service_response(frame: &AmsTcpFrame, ctx: &CommandContext<'_>) -> AmsTcpFrame {
    if !ctx.policy.permits(ctx.client) {
        return response_for_payload_with_error(
            frame,
            read_payload(AdsErrorCode::AccessDenied, &[]),
            0,
        );
    }
    let payload = match frame.header.command_id {
        CommandId::ReadDeviceInfo => device_info_payload(&tcom_device_info()),
        CommandId::ReadState => read_state_payload(),
        CommandId::ReadWrite => tcom_read_write_payload(frame.payload.as_slice()),
        _ => read_payload(AdsErrorCode::ServiceNotSupported, &[]),
    };
    response_for_payload(frame, payload)
}

fn tcom_device_info() -> DeviceInfo {
    DeviceInfo {
        major_version: 1,
        minor_version: 0,
        build: 0,
        name: "TCOM Server".to_string(),
    }
}

fn tcom_read_write_payload(payload: &[u8]) -> Vec<u8> {
    let Some((index_group, _index_offset, read_len, write_data)) =
        parse_read_write_request(payload)
    else {
        return read_payload(AdsErrorCode::InvalidSize, &[]);
    };
    if index_group == 0x112 && read_len == 0x400 && write_data == [0x01, 0x00, 0x60, 0x03] {
        return read_payload(AdsErrorCode::NoError, &[]);
    }
    read_payload(AdsErrorCode::ServiceNotSupported, &[])
}

fn device_info_payload(info: &DeviceInfo) -> Vec<u8> {
    let mut response = Vec::with_capacity(24);
    response.extend_from_slice(&AdsErrorCode::NoError.value().to_le_bytes());
    response.push(info.major_version);
    response.push(info.minor_version);
    response.extend_from_slice(&info.build.to_le_bytes());
    let mut name = [0_u8; 16];
    let bytes = info.name.as_bytes();
    let len = bytes.len().min(name.len());
    name[..len].copy_from_slice(&bytes[..len]);
    response.extend_from_slice(&name);
    response
}

fn read_state_payload() -> Vec<u8> {
    let mut response = Vec::with_capacity(8);
    response.extend_from_slice(&AdsErrorCode::NoError.value().to_le_bytes());
    response.extend_from_slice(&5_u16.to_le_bytes());
    response.extend_from_slice(&0_u16.to_le_bytes());
    response
}

fn router_read_payload(payload: &[u8], runtime_port: u16) -> Vec<u8> {
    let Some((index_group, index_offset, read_len)) = parse_read_request(payload) else {
        return read_payload(AdsErrorCode::InvalidSize, &[]);
    };
    match (index_group, index_offset) {
        (ROUTER_METADATA_INDEX_GROUP, ROUTER_METADATA_INDEX_OFFSET) => {
            let read_len = read_len.min(ROUTER_METADATA_BYTES.len());
            read_payload(AdsErrorCode::NoError, &ROUTER_METADATA_BYTES[..read_len])
        }
        (ROUTER_TCPIP_METADATA_INDEX_GROUP, ROUTER_TCPIP_METADATA_INDEX_OFFSET) => {
            if read_len == 0 {
                return read_payload(AdsErrorCode::InvalidSize, &[]);
            }
            let table = router_tcpip_metadata_table(runtime_port);
            let rounded_read_len = read_len.saturating_add(ROUTER_TCPIP_METADATA_ENTRY_SIZE - 1)
                / ROUTER_TCPIP_METADATA_ENTRY_SIZE
                * ROUTER_TCPIP_METADATA_ENTRY_SIZE;
            let read_len = rounded_read_len.min(table.len());
            read_payload(AdsErrorCode::NoError, &table[..read_len])
        }
        _ => read_payload(AdsErrorCode::InvalidOffset, &[]),
    }
}

fn router_tcpip_metadata_table(runtime_port: u16) -> Vec<u8> {
    let mut table = Vec::with_capacity(ROUTER_TCPIP_METADATA_TABLE_SIZE);
    push_router_service_entry(
        &mut table,
        0,
        AMS_SYSTEM_SERVICE_PORT,
        1,
        4,
        "TcSysSrv.TcpIp",
    );
    push_router_service_entry(&mut table, 1, AMS_ROUTER_PORT, 2, 15, "TCROUTER.Router");
    push_router_service_entry(
        &mut table,
        1,
        runtime_port,
        8,
        2,
        format!("Port_{runtime_port}").as_str(),
    );
    table.resize(ROUTER_TCPIP_METADATA_TABLE_SIZE, 0);
    table
}

fn push_router_service_entry(
    table: &mut Vec<u8>,
    flags: u32,
    port: u16,
    service_class: u32,
    service_kind: u32,
    name: &str,
) {
    table.extend_from_slice(&flags.to_le_bytes());
    table.extend_from_slice(&(0x0050_0000_u32 | u32::from(port)).to_le_bytes());
    table.extend_from_slice(&service_class.to_le_bytes());
    table.extend_from_slice(&service_kind.to_le_bytes());
    let mut name_bytes = [0_u8; 32];
    let bytes = name.as_bytes();
    let len = bytes.len().min(name_bytes.len());
    name_bytes[..len].copy_from_slice(&bytes[..len]);
    table.extend_from_slice(&name_bytes);
}

fn parse_read_request(payload: &[u8]) -> Option<(u32, u32, usize)> {
    if payload.len() != 12 {
        return None;
    }
    let index_group = u32::from_le_bytes(payload[0..4].try_into().ok()?);
    let index_offset = u32::from_le_bytes(payload[4..8].try_into().ok()?);
    let read_len = u32::from_le_bytes(payload[8..12].try_into().ok()?);
    Some((index_group, index_offset, usize::try_from(read_len).ok()?))
}

fn parse_read_write_request(payload: &[u8]) -> Option<(u32, u32, u32, &[u8])> {
    if payload.len() < 16 {
        return None;
    }
    let index_group = u32::from_le_bytes(payload[0..4].try_into().ok()?);
    let index_offset = u32::from_le_bytes(payload[4..8].try_into().ok()?);
    let read_len = u32::from_le_bytes(payload[8..12].try_into().ok()?);
    let write_len = u32::from_le_bytes(payload[12..16].try_into().ok()?);
    let write_len = usize::try_from(write_len).ok()?;
    let write_data = payload.get(16..16 + write_len)?;
    if payload.len() != 16 + write_len {
        return None;
    }
    Some((index_group, index_offset, read_len, write_data))
}

fn read_payload(code: AdsErrorCode, data: &[u8]) -> Vec<u8> {
    let length = u32::try_from(data.len()).unwrap_or(0);
    let mut response = Vec::with_capacity(8 + data.len());
    response.extend_from_slice(&code.value().to_le_bytes());
    response.extend_from_slice(&length.to_le_bytes());
    if code == AdsErrorCode::NoError {
        response.extend_from_slice(data);
    }
    response
}

fn empty_error_response(frame: &AmsTcpFrame, code: AdsErrorCode) -> AmsTcpFrame {
    response_for_payload_with_error(frame, Vec::new(), code.value())
}

fn response_for_payload(frame: &AmsTcpFrame, payload: Vec<u8>) -> AmsTcpFrame {
    response_for_payload_with_error(frame, payload, 0)
}

fn response_for_payload_with_error(
    frame: &AmsTcpFrame,
    payload: Vec<u8>,
    ams_error_code: u32,
) -> AmsTcpFrame {
    let header = frame
        .header
        .response_for(payload.len(), ams_error_code)
        .unwrap_or_else(|_| no_memory_response_header(&frame.header));
    AmsTcpFrame { header, payload }
}

fn no_memory_response_header(header: &AmsHeader) -> AmsHeader {
    AmsHeader {
        target_net_id: header.source_net_id,
        target_port: header.source_port,
        source_net_id: header.target_net_id,
        source_port: header.target_port,
        command_id: header.command_id,
        state: AmsState::Response,
        data_length: 0,
        error_code: AdsErrorCode::NoMemory.value(),
        invoke_id: header.invoke_id,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use trust_ads_core::{
        AdsDataTypeDescriptor, IecDataType, PointQuality, SymbolDescriptor, SymbolSnapshot,
    };

    use crate::{
        ams_net_id_text_to_bytes, AdsErrorCode, AdsServerAuditEvent, AdsServerError, AmsHeader,
        AmsState, AmsTcpFrame, AuditSink, ClientId, ClientPolicy, Clock, CommandId,
        RuntimeWritePort, SymbolSource, ValueIo, ADSTRANS_SERVERCYCLE,
    };

    use super::{AdsTcpServer, AdsTcpServerConfig, AdsTcpServerServices};

    struct Harness {
        snapshot: SymbolSnapshot,
        writes: Mutex<Vec<Vec<u8>>>,
    }

    impl Harness {
        fn new() -> Self {
            let symbol = SymbolDescriptor::new(
                "global.setpoint",
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                0x4020,
                0,
                4,
            );
            Self {
                snapshot: SymbolSnapshot::new("test", vec![symbol]),
                writes: Mutex::new(Vec::new()),
            }
        }
    }

    impl SymbolSource for Harness {
        fn snapshot(&self) -> Arc<SymbolSnapshot> {
            Arc::new(self.snapshot.clone())
        }

        fn version(&self) -> u32 {
            1
        }
    }

    impl ValueIo for Harness {
        fn read(
            &self,
            _symbol: &SymbolDescriptor,
        ) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
            Ok((12.5_f32.to_le_bytes().to_vec(), PointQuality::good(1)))
        }
    }

    impl RuntimeWritePort for Harness {
        fn write(
            &self,
            _symbol: &SymbolDescriptor,
            bytes: &[u8],
            _client: &ClientId,
        ) -> Result<(), AdsServerError> {
            self.writes
                .lock()
                .expect("writes poisoned")
                .push(bytes.to_vec());
            Ok(())
        }
    }

    impl ClientPolicy for Harness {
        fn permits(&self, client: &ClientId) -> bool {
            client.ams_net_id.0 == "5.23.91.12.1.1"
        }
    }

    impl AuditSink for Harness {
        fn record(&self, _event: &AdsServerAuditEvent) {}
    }

    impl Clock for Harness {
        fn now_ms(&self) -> u64 {
            1
        }
    }

    #[test]
    fn ams_net_id_text_conversion_round_trips() {
        let bytes = ams_net_id_text_to_bytes("192.168.10.20.1.1").expect("parse net id");

        assert_eq!(bytes, [192, 168, 10, 20, 1, 1]);
        assert_eq!(crate::ams_net_id_bytes_to_text(bytes), "192.168.10.20.1.1");
        assert!(ams_net_id_text_to_bytes("192.168.10.20.1").is_err());
        assert!(ams_net_id_text_to_bytes("192.168.10.999.1.1").is_err());
    }

    #[test]
    fn tcp_listener_serves_read_device_info() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let request = request_frame(CommandId::ReadDeviceInfo, Vec::new());

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.command_id, CommandId::ReadDeviceInfo);
        assert_eq!(response.header.error_code, 0);
        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4], 1);
        assert_eq!(response.payload[8..13], *b"truST");
        server.shutdown();
    }

    #[test]
    fn tcp_listener_dispatches_direct_read() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x4020_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        let request = request_frame(CommandId::Read, payload);

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4..8], 4_u32.to_le_bytes());
        assert_eq!(response.payload[8..12], 12.5_f32.to_le_bytes());
        server.shutdown();
    }

    #[test]
    fn tcp_listener_rejects_wrong_target_port() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut request = request_frame(CommandId::ReadDeviceInfo, Vec::new());
        request.header.target_port = 852;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(
            response.header.error_code,
            AdsErrorCode::AccessDenied.value()
        );
        assert!(response.payload.is_empty());
        server.shutdown();
    }

    #[test]
    fn tcp_listener_serves_system_service_read_state() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut request = request_frame(CommandId::ReadState, Vec::new());
        request.header.target_port = super::AMS_SYSTEM_SERVICE_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.command_id, CommandId::ReadState);
        assert_eq!(response.header.error_code, 0);
        assert_eq!(response.header.source_port, super::AMS_SYSTEM_SERVICE_PORT);
        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4..6], 5_u16.to_le_bytes());
        assert_eq!(response.payload[6..8], 0_u16.to_le_bytes());
        server.shutdown();
    }

    #[test]
    fn tcp_listener_rejects_symbol_read_on_system_service_port() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x4020_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        let mut request = request_frame(CommandId::Read, payload);
        request.header.target_port = super::AMS_SYSTEM_SERVICE_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(
            response.header.error_code,
            AdsErrorCode::ServiceNotSupported.value()
        );
        assert!(response.payload.is_empty());
        server.shutdown();
    }

    #[test]
    fn tcp_listener_serves_router_metadata_read() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut payload = Vec::new();
        payload.extend_from_slice(&super::ROUTER_METADATA_INDEX_GROUP.to_le_bytes());
        payload.extend_from_slice(&super::ROUTER_METADATA_INDEX_OFFSET.to_le_bytes());
        payload.extend_from_slice(&40_u32.to_le_bytes());
        let mut request = request_frame(CommandId::Read, payload);
        request.header.target_port = super::AMS_ROUTER_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.command_id, CommandId::Read);
        assert_eq!(response.header.error_code, 0);
        assert_eq!(response.header.source_port, super::AMS_ROUTER_PORT);
        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4..8], 40_u32.to_le_bytes());
        assert_eq!(&response.payload[8..48], &super::ROUTER_METADATA_BYTES);
        server.shutdown();
    }

    #[test]
    fn tcp_listener_serves_router_tcpip_metadata_read() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut payload = Vec::new();
        payload.extend_from_slice(&super::ROUTER_TCPIP_METADATA_INDEX_GROUP.to_le_bytes());
        payload.extend_from_slice(&super::ROUTER_TCPIP_METADATA_INDEX_OFFSET.to_le_bytes());
        payload.extend_from_slice(&40_u32.to_le_bytes());
        let mut request = request_frame(CommandId::Read, payload);
        request.header.target_port = super::AMS_ROUTER_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.command_id, CommandId::Read);
        assert_eq!(response.header.error_code, 0);
        assert_eq!(response.header.source_port, super::AMS_ROUTER_PORT);
        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4..8], 48_u32.to_le_bytes());
        assert_eq!(response.payload[12..16], 0x0050_2710_u32.to_le_bytes());
        assert_eq!(response.payload[24..36], *b"TcSysSrv.Tcp");
        server.shutdown();
    }

    #[test]
    fn tcp_listener_serves_router_tcpip_metadata_table_with_runtime_port() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut payload = Vec::new();
        payload.extend_from_slice(&super::ROUTER_TCPIP_METADATA_INDEX_GROUP.to_le_bytes());
        payload.extend_from_slice(&super::ROUTER_TCPIP_METADATA_INDEX_OFFSET.to_le_bytes());
        payload.extend_from_slice(&2160_u32.to_le_bytes());
        let mut request = request_frame(CommandId::Read, payload);
        request.header.target_port = super::AMS_ROUTER_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.command_id, CommandId::Read);
        assert_eq!(response.header.error_code, 0);
        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4..8], 2160_u32.to_le_bytes());
        assert!(
            response.payload[8..]
                .windows(b"Port_851".len())
                .any(|window| window == b"Port_851"),
            "router service table must advertise the served runtime port"
        );
        server.shutdown();
    }

    #[test]
    fn tcp_listener_rejects_unknown_router_metadata_offset() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut payload = Vec::new();
        payload.extend_from_slice(&super::ROUTER_METADATA_INDEX_GROUP.to_le_bytes());
        payload.extend_from_slice(&851_u32.to_le_bytes());
        payload.extend_from_slice(&40_u32.to_le_bytes());
        let mut request = request_frame(CommandId::Read, payload);
        request.header.target_port = super::AMS_ROUTER_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.error_code, 0);
        assert_eq!(
            response.payload[0..4],
            AdsErrorCode::InvalidOffset.value().to_le_bytes()
        );
        assert_eq!(response.payload[4..8], 0_u32.to_le_bytes());
        server.shutdown();
    }

    #[test]
    fn tcp_listener_serves_router_device_info() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut request = request_frame(CommandId::ReadDeviceInfo, Vec::new());
        request.header.target_port = super::AMS_ROUTER_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.command_id, CommandId::ReadDeviceInfo);
        assert_eq!(response.header.error_code, 0);
        assert_eq!(response.header.source_port, super::AMS_ROUTER_PORT);
        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4], 3);
        assert_eq!(response.payload[5], 1);
        assert_eq!(response.payload[8..16], *b"TCROUTER");
        server.shutdown();
    }

    #[test]
    fn tcp_listener_serves_tcom_browser_probe() {
        let harness = Arc::new(Harness::new());
        let mut server = AdsTcpServer::start(config(), services(harness)).expect("start server");
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x112_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&0x400_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        payload.extend_from_slice(&[0x01, 0x00, 0x60, 0x03]);
        let mut request = request_frame(CommandId::ReadWrite, payload);
        request.header.target_port = super::AMS_TCOM_SERVER_PORT;

        let response = send_frame(server.local_addr(), &request);

        assert_eq!(response.header.command_id, CommandId::ReadWrite);
        assert_eq!(response.header.error_code, 0);
        assert_eq!(response.header.source_port, super::AMS_TCOM_SERVER_PORT);
        assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
        assert_eq!(response.payload[4..8], 0_u32.to_le_bytes());
        server.shutdown();
    }

    #[test]
    fn tcp_listener_reports_bind_conflict() {
        let harness = Arc::new(Harness::new());
        let mut server =
            AdsTcpServer::start(config(), services(harness.clone())).expect("start server");
        let mut conflict = config();
        conflict.bind_addr = server.local_addr();

        let Err(error) = AdsTcpServer::start(conflict, services(harness)) else {
            panic!("bind conflict should fail")
        };

        assert!(matches!(error, super::AdsTcpServerError::Bind(_)));
        server.shutdown();
    }

    #[test]
    fn tcp_listener_delivers_server_cycle_notification() {
        let harness = Arc::new(Harness::new());
        let mut config = config();
        config.min_notification_cycle = Duration::from_millis(5);
        let mut server = AdsTcpServer::start(config, services(harness)).expect("start server");
        let mut stream = std::net::TcpStream::connect(server.local_addr()).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("read timeout");
        let request = request_frame(
            CommandId::AddDeviceNotification,
            add_notification_payload(3),
        );

        stream
            .write_all(&request.to_bytes().expect("request bytes"))
            .expect("write request");
        let response = read_stream_frame(&mut stream);

        assert_eq!(response.header.command_id, CommandId::AddDeviceNotification);
        assert_eq!(&response.payload[0..4], &0_u32.to_le_bytes());
        let handle = u32::from_le_bytes(response.payload[4..8].try_into().expect("handle"));
        assert_eq!(handle, 1);

        let notification = read_stream_frame(&mut stream);
        assert_eq!(
            notification.header.command_id,
            CommandId::DeviceNotification
        );
        assert_eq!(notification.header.state, AmsState::Request);
        assert_eq!(notification.header.invoke_id, 1);
        assert_eq!(notification.header.target_port, 0x8001);
        assert_eq!(notification.header.source_port, 851);
        assert_eq!(&notification.payload[4..8], &1_u32.to_le_bytes());
        assert_eq!(&notification.payload[16..20], &1_u32.to_le_bytes());
        assert_eq!(&notification.payload[20..24], &handle.to_le_bytes());
        assert_eq!(&notification.payload[24..28], &4_u32.to_le_bytes());
        assert_eq!(&notification.payload[28..32], &12.5_f32.to_le_bytes());
        server.shutdown();
    }

    #[test]
    fn tcp_listener_notifications_keep_registering_ads_port_on_multiplexed_connection() {
        let harness = Arc::new(Harness::new());
        let mut config = config();
        config.min_notification_cycle = Duration::from_millis(50);
        let mut server = AdsTcpServer::start(config, services(harness)).expect("start server");
        let mut stream = std::net::TcpStream::connect(server.local_addr()).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("read timeout");
        let request = request_frame_from_source_port(
            CommandId::AddDeviceNotification,
            add_notification_payload(ADSTRANS_SERVERCYCLE),
            0x8001,
        );

        stream
            .write_all(&request.to_bytes().expect("request bytes"))
            .expect("write add notification");
        let response = read_stream_frame(&mut stream);
        assert_eq!(response.header.command_id, CommandId::AddDeviceNotification);
        assert_eq!(&response.payload[0..4], &0_u32.to_le_bytes());

        let router_frame =
            request_frame_from_source_port(CommandId::ReadDeviceInfo, Vec::new(), 0xFFFF);
        stream
            .write_all(&router_frame.to_bytes().expect("router frame bytes"))
            .expect("write router frame");
        let router_response = read_stream_frame(&mut stream);
        assert_eq!(router_response.header.target_port, 0xFFFF);

        let notification = read_stream_frame(&mut stream);
        assert_eq!(
            notification.header.command_id,
            CommandId::DeviceNotification
        );
        assert_eq!(notification.header.target_port, 0x8001);
        server.shutdown();
    }

    #[test]
    fn tcp_listener_enforces_notification_limits() {
        let harness = Arc::new(Harness::new());
        let mut config = config();
        config.max_total_subscriptions = 1;
        let mut server = AdsTcpServer::start(config, services(harness)).expect("start server");
        let mut stream = std::net::TcpStream::connect(server.local_addr()).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("read timeout");

        stream
            .write_all(
                &request_frame(
                    CommandId::AddDeviceNotification,
                    add_notification_payload(3),
                )
                .to_bytes()
                .expect("request bytes"),
            )
            .expect("first request");
        let first = read_stream_frame(&mut stream);
        assert_eq!(&first.payload[0..4], &0_u32.to_le_bytes());

        stream
            .write_all(
                &request_frame(
                    CommandId::AddDeviceNotification,
                    add_notification_payload(3),
                )
                .to_bytes()
                .expect("request bytes"),
            )
            .expect("second request");
        let second = read_stream_frame(&mut stream);
        assert_eq!(
            u32::from_le_bytes(second.payload[0..4].try_into().expect("result")),
            AdsErrorCode::NoMemory.value()
        );
        server.shutdown();
    }

    fn config() -> AdsTcpServerConfig {
        AdsTcpServerConfig {
            bind_addr: "127.0.0.1:0".parse().expect("bind addr"),
            local_net_id: ams_net_id_text_to_bytes("127.0.0.1.1.1").expect("local net id"),
            ads_port: 851,
            max_frame_bytes: 4096,
            max_clients: 4,
            max_subscriptions_per_client: 4,
            max_total_subscriptions: 8,
            max_sumup_items: 16,
            max_write_bytes: 1024,
            max_handles_per_client: 16,
            min_notification_cycle: Duration::from_millis(25),
            read_timeout: Duration::from_millis(250),
            idle_timeout: Duration::from_secs(1),
            device_info: crate::DeviceInfo::default(),
        }
    }

    fn services(harness: Arc<Harness>) -> AdsTcpServerServices {
        AdsTcpServerServices {
            symbols: harness.clone(),
            values: harness.clone(),
            writes: harness.clone(),
            policy: harness.clone(),
            audit: harness.clone(),
            clock: harness,
        }
    }

    fn request_frame(command_id: CommandId, payload: Vec<u8>) -> AmsTcpFrame {
        request_frame_from_source_port(command_id, payload, 0x8001)
    }

    fn request_frame_from_source_port(
        command_id: CommandId,
        payload: Vec<u8>,
        source_port: u16,
    ) -> AmsTcpFrame {
        AmsTcpFrame {
            header: AmsHeader {
                target_net_id: ams_net_id_text_to_bytes("127.0.0.1.1.1").expect("target"),
                target_port: 851,
                source_net_id: ams_net_id_text_to_bytes("5.23.91.12.1.1").expect("source"),
                source_port,
                command_id,
                state: AmsState::Request,
                data_length: u32::try_from(payload.len()).expect("test payload fits u32"),
                error_code: 0,
                invoke_id: 42,
            },
            payload,
        }
    }

    fn send_frame(addr: std::net::SocketAddr, frame: &AmsTcpFrame) -> AmsTcpFrame {
        let mut stream = std::net::TcpStream::connect(addr).expect("connect");
        stream
            .write_all(&frame.to_bytes().expect("request bytes"))
            .expect("write request");
        read_stream_frame(&mut stream)
    }

    fn read_stream_frame(stream: &mut std::net::TcpStream) -> AmsTcpFrame {
        let mut prefix = [0_u8; crate::AMS_TCP_HEADER_LEN];
        stream.read_exact(&mut prefix).expect("read prefix");
        let ams_len = u32::from_le_bytes([prefix[2], prefix[3], prefix[4], prefix[5]]) as usize;
        let mut bytes = Vec::from(prefix);
        bytes.resize(crate::AMS_TCP_HEADER_LEN + ams_len, 0);
        stream
            .read_exact(&mut bytes[crate::AMS_TCP_HEADER_LEN..])
            .expect("read rest");
        AmsTcpFrame::parse(&bytes, 4096).expect("parse response")
    }

    fn add_notification_payload(mode: u32) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x4020_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        payload.extend_from_slice(&mode.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&[0_u8; 16]);
        payload
    }
}
