use crate::ads::diagnostics::{LocalIdentity, TargetIdentity};
use crate::ads::onboarding::errors::{OnboardingWireError, OnboardingWireErrorKind};
use crate::ads::onboarding::route::{RouteAddRequest, RouteRemoveRequest};
#[cfg(feature = "ads-wire")]
use crate::ads::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationMode, AdsPointAddress, AdsReadResult,
    AdsResolvedHandle, AdsRsTransport, AdsSubscribeRequest, AdsWriteRequest,
};
#[cfg(feature = "ads-wire")]
use std::collections::BTreeMap;
#[cfg(feature = "ads-wire")]
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
#[cfg(feature = "ads-wire")]
use std::time::Duration;
#[cfg(feature = "ads-wire")]
use trust_ads_core::{
    ads_bytes_from_value, AdsRoute, AdsSecurityPolicy, AmsNetId, QualityState, TransportSecurity,
    UpdateMode,
};
use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag};
use trust_runtime_core::value::Value;

#[cfg(feature = "ads-wire")]
const DEFAULT_ADS_PLC_PORT: u16 = 851;
#[cfg(feature = "ads-wire")]
const DEFAULT_TCP_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
#[cfg(feature = "ads-wire")]
const DEFAULT_UDP_BROADCAST_WINDOW: Duration = Duration::from_millis(900);
#[cfg(feature = "ads-wire")]
const GUARDED_WRITE_READBACK_ATTEMPTS: usize = 20;
#[cfg(feature = "ads-wire")]
const GUARDED_WRITE_READBACK_DELAY: Duration = Duration::from_millis(50);

/// Thin setup/control-plane wire boundary for ADS onboarding.
///
/// Real ADS/socket calls belong behind this trait. Business logic and public
/// schema types must not expose raw wire crate types.
pub trait AdsOnboardingWire {
    fn udp_identify(&mut self, target_ip: &str) -> Result<TargetIdentity, OnboardingWireError>;
    fn udp_identify_all(
        &mut self,
        target_ip: &str,
    ) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
        self.udp_identify(target_ip).map(|target| vec![target])
    }
    fn tcp_probe_48898(&mut self, target_ip: &str) -> Result<(), OnboardingWireError>;
    fn check_route(
        &mut self,
        target: &TargetIdentity,
        local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError>;
    fn verify_ams_target(&mut self, target: &TargetIdentity) -> Result<(), OnboardingWireError>;
    fn read_state(&mut self, target: &TargetIdentity) -> Result<String, OnboardingWireError>;
    fn upload_symbols(
        &mut self,
        target: &TargetIdentity,
    ) -> Result<Vec<SymbolDescriptor>, OnboardingWireError>;
    fn resolve_handle(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<u32, OnboardingWireError>;
    fn sumup_read(
        &mut self,
        target: &TargetIdentity,
        handles: &[u32],
    ) -> Result<Vec<Vec<u8>>, OnboardingWireError>;
    fn guarded_write_probe(
        &mut self,
        target: &TargetIdentity,
        probe: &GuardedWriteProbe,
    ) -> Result<(), OnboardingWireError>;
    fn subscribe_notification(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<(), OnboardingWireError>;
    fn symbol_version(&mut self, target: &TargetIdentity) -> Result<u32, OnboardingWireError>;
    fn add_route(&mut self, request: &RouteAddRequest) -> Result<(), OnboardingWireError>;
    fn remove_route(&mut self, request: &RouteRemoveRequest) -> Result<(), OnboardingWireError>;
}

/// Explicit, reversible write probe requested by a user.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardedWriteProbe {
    pub symbol: String,
    pub data_type: AdsDataTypeDescriptor,
    pub value: Value,
}

/// `ads-rs` implementation of the setup/control-plane wire boundary.
#[cfg(feature = "ads-wire")]
pub struct AdsRsOnboardingWire {
    tcp_probe_timeout: Duration,
    udp_broadcast_window: Duration,
    transport_key: Option<String>,
    transport: Option<AdsRsTransport>,
    local_identity: Option<LocalIdentity>,
    symbols_by_name: BTreeMap<String, SymbolDescriptor>,
    handles_by_id: BTreeMap<u32, AdsResolvedHandle>,
}

#[cfg(feature = "ads-wire")]
impl AdsRsOnboardingWire {
    pub fn new() -> Self {
        Self {
            tcp_probe_timeout: DEFAULT_TCP_PROBE_TIMEOUT,
            udp_broadcast_window: DEFAULT_UDP_BROADCAST_WINDOW,
            transport_key: None,
            transport: None,
            local_identity: None,
            symbols_by_name: BTreeMap::new(),
            handles_by_id: BTreeMap::new(),
        }
    }

    pub fn with_tcp_probe_timeout(tcp_probe_timeout: Duration) -> Self {
        Self {
            tcp_probe_timeout,
            udp_broadcast_window: DEFAULT_UDP_BROADCAST_WINDOW,
            transport_key: None,
            transport: None,
            local_identity: None,
            symbols_by_name: BTreeMap::new(),
            handles_by_id: BTreeMap::new(),
        }
    }

    fn ensure_transport(
        &mut self,
        target: &TargetIdentity,
        failure_kind: OnboardingWireErrorKind,
    ) -> Result<&mut AdsRsTransport, OnboardingWireError> {
        let key = self.transport_key_for(target);
        if self.transport_key.as_deref() != Some(key.as_str()) {
            self.transport = None;
            self.symbols_by_name.clear();
            self.handles_by_id.clear();
            self.transport_key = Some(key);
        }
        if self.transport.is_none() {
            let route = self.route_for_target(target);
            let mut transport = AdsRsTransport::new(route);
            transport
                .connect()
                .map_err(|error| map_transport_error(failure_kind, "connect ADS target", error))?;
            self.transport = Some(transport);
        }
        Ok(self
            .transport
            .as_mut()
            .expect("transport initialized above"))
    }

    fn transport_key_for(&self, target: &TargetIdentity) -> String {
        let local_net_id = self
            .local_identity
            .as_ref()
            .map(|identity| identity.ams_net_id.as_str())
            .unwrap_or("");
        format!(
            "{}|{}|{}|{}",
            target.ip, target.ams_net_id, target.ams_port, local_net_id
        )
    }

    fn route_for_target(&self, target: &TargetIdentity) -> AdsRoute {
        let mut route = AdsRoute::new(
            target
                .name
                .clone()
                .unwrap_or_else(|| "ads-onboarding".to_string()),
            AmsNetId::new(target.ams_net_id.clone()),
            target.ip.clone(),
            target.ams_port,
        );
        route.security = AdsSecurityPolicy {
            transport: TransportSecurity::Plain,
            auto_add_route: false,
        };
        route.local_net_id = self
            .local_identity
            .as_ref()
            .map(|identity| AmsNetId::new(identity.ams_net_id.clone()));
        route
    }

    fn read_one_value(
        &mut self,
        target: &TargetIdentity,
        resolved: &AdsResolvedHandle,
    ) -> Result<Value, OnboardingWireError> {
        let mut results = self
            .ensure_transport(target, OnboardingWireErrorKind::WrongPlcPort)?
            .sumup_read(std::slice::from_ref(resolved))
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::WrongPlcPort,
                    "ADS guarded write read-back",
                    error,
                )
            })?;
        let result = results.pop().ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                "ADS guarded write read-back returned no result",
            )
        })?;
        if result.quality.state != QualityState::Good {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                result
                    .quality
                    .detail
                    .unwrap_or_else(|| "ADS guarded write read-back quality was not good".into()),
            ));
        }
        result.value.ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                "ADS guarded write read-back returned no value",
            )
        })
    }

    fn read_until_value(
        &mut self,
        target: &TargetIdentity,
        resolved: &AdsResolvedHandle,
        expected: &Value,
    ) -> Result<Value, OnboardingWireError> {
        let mut last = self.read_one_value(target, resolved)?;
        for _ in 0..GUARDED_WRITE_READBACK_ATTEMPTS {
            if &last == expected {
                return Ok(last);
            }
            std::thread::sleep(GUARDED_WRITE_READBACK_DELAY);
            last = self.read_one_value(target, resolved)?;
        }
        Ok(last)
    }

    fn write_one_value(
        &mut self,
        target: &TargetIdentity,
        resolved: &AdsResolvedHandle,
        value: Value,
        context: &str,
    ) -> Result<(), OnboardingWireError> {
        let mut qualities = self
            .ensure_transport(target, OnboardingWireErrorKind::WrongPlcPort)?
            .sumup_write(&[AdsWriteRequest {
                handle: resolved.clone(),
                value,
            }])
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::WrongPlcPort,
                    "ADS guarded write",
                    error,
                )
            })?;
        let quality = qualities.pop().ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!("ADS guarded write {context} returned no quality"),
            )
        })?;
        if quality.state == QualityState::Good {
            Ok(())
        } else {
            Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                quality.detail.unwrap_or_else(|| {
                    format!("ADS guarded write {context} returned non-good quality")
                }),
            ))
        }
    }

    fn restore_original_value(
        &mut self,
        target: &TargetIdentity,
        resolved: &AdsResolvedHandle,
        original: &Value,
    ) -> Result<(), OnboardingWireError> {
        self.write_one_value(target, resolved, original.clone(), "restore value")?;
        let restored = self.read_until_value(target, resolved, original)?;
        if restored == *original {
            Ok(())
        } else {
            Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!(
                    "ADS guarded write restore mismatch for '{}': original {:?}, restored {:?}",
                    resolved.point_name, original, restored
                ),
            ))
        }
    }
}

#[cfg(feature = "ads-wire")]
impl Default for AdsRsOnboardingWire {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "ads-wire")]
impl AdsOnboardingWire for AdsRsOnboardingWire {
    fn udp_identify(&mut self, target_ip: &str) -> Result<TargetIdentity, OnboardingWireError> {
        let info = ads::udp::get_info((target_ip, ads::UDP_PORT)).map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::UdpIdentifyBlocked,
                format!("ADS UDP identify failed for {target_ip}: {error}"),
            )
        })?;
        Ok(TargetIdentity {
            name: Some(info.hostname),
            ip: target_ip.to_string(),
            ams_net_id: info.netid.to_string(),
            ams_port: DEFAULT_ADS_PLC_PORT,
            tc_version: tc_version_string(info.twincat_version),
        })
    }

    fn udp_identify_all(
        &mut self,
        target_ip: &str,
    ) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
        udp_identify_all(target_ip, self.udp_broadcast_window)
    }

    fn tcp_probe_48898(&mut self, target_ip: &str) -> Result<(), OnboardingWireError> {
        let address = first_socket_addr((target_ip, ads::PORT)).map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::Tcp48898Blocked,
                format!("resolve ADS router address for {target_ip}: {error}"),
            )
        })?;
        TcpStream::connect_timeout(&address, self.tcp_probe_timeout)
            .map(drop)
            .map_err(|error| {
                OnboardingWireError::new(
                    OnboardingWireErrorKind::Tcp48898Blocked,
                    format!("TCP 48898 probe failed for {target_ip}: {error}"),
                )
            })
    }

    fn check_route(
        &mut self,
        target: &TargetIdentity,
        local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError> {
        self.local_identity = Some(local.clone());
        self.transport = None;
        self.ensure_transport(target, OnboardingWireErrorKind::RouteMissing)
            .map(drop)
    }

    fn verify_ams_target(&mut self, target: &TargetIdentity) -> Result<(), OnboardingWireError> {
        self.ensure_transport(target, OnboardingWireErrorKind::WrongAmsNetId)
            .map(drop)?;
        Ok(())
    }

    fn read_state(&mut self, target: &TargetIdentity) -> Result<String, OnboardingWireError> {
        let state = self
            .ensure_transport(target, OnboardingWireErrorKind::WrongPlcPort)?
            .read_state()
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::WrongPlcPort,
                    "read ADS runtime state",
                    error,
                )
            })?;
        Ok(match state {
            AdsDeviceState::Run => "run",
            AdsDeviceState::Stop => "stop",
            AdsDeviceState::Fault => "fault",
            AdsDeviceState::Unknown => "unknown",
        }
        .to_string())
    }

    fn upload_symbols(
        &mut self,
        target: &TargetIdentity,
    ) -> Result<Vec<SymbolDescriptor>, OnboardingWireError> {
        let symbols = self
            .ensure_transport(target, OnboardingWireErrorKind::NoSymbols)?
            .upload_symbol_table()
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::NoSymbols,
                    "upload ADS symbol table",
                    error,
                )
            })?;
        self.symbols_by_name = symbols
            .iter()
            .map(|symbol| (symbol.name.clone(), symbol.clone()))
            .collect();
        Ok(symbols)
    }

    fn resolve_handle(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<u32, OnboardingWireError> {
        if !self.symbols_by_name.contains_key(symbol) {
            let _ = self.upload_symbols(target)?;
        }
        let descriptor = self.symbols_by_name.get(symbol).cloned().ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::NoSymbols,
                format!("ADS symbol '{symbol}' is missing from the uploaded table"),
            )
        })?;
        let request = AdsHandleRequest {
            point_name: symbol.to_string(),
            address: AdsPointAddress::Symbol(symbol.to_string()),
            data_type: descriptor.data_type,
        };
        let mut resolved = self
            .ensure_transport(target, OnboardingWireErrorKind::WrongPlcPort)?
            .resolve_handles(&[request])
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::WrongPlcPort,
                    "resolve ADS symbol handle",
                    error,
                )
            })?;
        let resolved = resolved.pop().ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                "ADS handle resolve returned no handle",
            )
        })?;
        let handle = resolved.handle;
        self.handles_by_id.insert(handle, resolved);
        Ok(handle)
    }

    fn sumup_read(
        &mut self,
        target: &TargetIdentity,
        handles: &[u32],
    ) -> Result<Vec<Vec<u8>>, OnboardingWireError> {
        let resolved = handles
            .iter()
            .map(|handle| {
                self.handles_by_id.get(handle).cloned().ok_or_else(|| {
                    OnboardingWireError::new(
                        OnboardingWireErrorKind::WrongPlcPort,
                        format!("ADS handle {handle} is not resolved in this doctor run"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let results = self
            .ensure_transport(target, OnboardingWireErrorKind::WrongPlcPort)?
            .sumup_read(&resolved)
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::WrongPlcPort,
                    "ADS sum-up read",
                    error,
                )
            })?;
        project_sumup_payloads(&resolved, results)
    }

    fn guarded_write_probe(
        &mut self,
        target: &TargetIdentity,
        probe: &GuardedWriteProbe,
    ) -> Result<(), OnboardingWireError> {
        if !self.symbols_by_name.contains_key(probe.symbol.as_str()) {
            let _ = self.upload_symbols(target)?;
        }
        let descriptor = self
            .symbols_by_name
            .get(probe.symbol.as_str())
            .cloned()
            .ok_or_else(|| {
                OnboardingWireError::new(
                    OnboardingWireErrorKind::NoSymbols,
                    format!(
                        "ADS write probe symbol '{}' is missing from the uploaded table",
                        probe.symbol
                    ),
                )
            })?;
        if !descriptor.flags.contains(&SymbolFlag::Write) {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::UnsupportedOperation,
                format!("ADS write probe symbol '{}' is not writable", probe.symbol),
            ));
        }
        if !write_probe_type_matches(&probe.data_type, &descriptor.data_type) {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::UnsupportedOperation,
                format!(
                    "ADS write probe symbol '{}' type mismatch: requested {:?}, remote {:?}",
                    probe.symbol, probe.data_type, descriptor.data_type
                ),
            ));
        }

        let handle = self.resolve_handle(target, probe.symbol.as_str())?;
        let resolved = self.handles_by_id.get(&handle).cloned().ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!("ADS handle {handle} is not resolved for guarded write"),
            )
        })?;
        let original = self.read_one_value(target, &resolved)?;
        let probe_write =
            self.write_one_value(target, &resolved, probe.value.clone(), "probe value");
        require_probe_write(probe_write, || {
            self.restore_original_value(target, &resolved, &original)
        })?;
        let probe_read_back = self.read_until_value(target, &resolved, &probe.value);
        require_expected_probe_readback(
            probe_read_back,
            &probe.value,
            probe.symbol.as_str(),
            || self.restore_original_value(target, &resolved, &original),
        )?;
        self.restore_original_value(target, &resolved, &original)
    }

    fn subscribe_notification(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<(), OnboardingWireError> {
        if !self.symbols_by_name.contains_key(symbol) {
            let _ = self.upload_symbols(target)?;
        }
        let handle = self.resolve_handle(target, symbol)?;
        let resolved = self.handles_by_id.get(&handle).cloned().ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::NotificationFailure,
                format!("ADS handle {handle} is not resolved for notification"),
            )
        })?;
        self.ensure_transport(target, OnboardingWireErrorKind::NotificationFailure)?
            .subscribe(AdsSubscribeRequest {
                handle: resolved,
                mode: UpdateMode::Notify,
                notification_mode: AdsNotificationMode::OnChange,
            })
            .map(drop)
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::NotificationFailure,
                    "subscribe ADS notification",
                    error,
                )
            })
    }

    fn symbol_version(&mut self, target: &TargetIdentity) -> Result<u32, OnboardingWireError> {
        self.ensure_transport(target, OnboardingWireErrorKind::WrongPlcPort)?
            .symbol_version()
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::WrongPlcPort,
                    "read ADS symbol version",
                    error,
                )
            })
    }

    fn add_route(&mut self, request: &RouteAddRequest) -> Result<(), OnboardingWireError> {
        let local_net_id = request
            .local
            .ams_net_id
            .parse::<ads::AmsNetId>()
            .map_err(|error| {
                OnboardingWireError::new(
                    OnboardingWireErrorKind::UnsupportedOperation,
                    format!(
                        "invalid runtime-host AMS Net ID '{}': {error}",
                        request.local.ams_net_id
                    ),
                )
            })?;
        ads::udp::add_route(
            (request.target.ip.as_str(), ads::UDP_PORT),
            local_net_id,
            request.local.chosen_ip.as_str(),
            Some(request.route_name.as_str()),
            Some(request.credentials.username.as_str()),
            Some(request.credentials.password.as_str()),
            false,
        )
        .map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::CredentialsRejected,
                format!("TwinCAT UDP AddRoute failed: {error}"),
            )
        })
    }

    fn remove_route(&mut self, _request: &RouteRemoveRequest) -> Result<(), OnboardingWireError> {
        Err(unsupported(
            "route removal is implemented in the route slice",
        ))
    }
}

mod mock;
pub use mock::*;

#[cfg(feature = "ads-wire")]
fn udp_identify_all(
    target_ip: &str,
    receive_window: Duration,
) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
    let request = ads::udp::Message::new(ads::udp::ServiceId::Identify, ads::AmsAddr::default());
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| {
        OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("bind ADS UDP identify socket: {error}"),
        )
    })?;
    socket.set_broadcast(true).map_err(|error| {
        OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("enable ADS UDP broadcast: {error}"),
        )
    })?;
    socket
        .set_read_timeout(Some(receive_window))
        .map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::UdpIdentifyBlocked,
                format!("set ADS UDP identify timeout: {error}"),
            )
        })?;
    socket
        .send_to(request.as_bytes(), (target_ip, ads::UDP_PORT))
        .map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::UdpIdentifyBlocked,
                format!("send ADS UDP identify to {target_ip}: {error}"),
            )
        })?;

    let mut results = Vec::new();
    let mut reply = [0_u8; 576];
    loop {
        match socket.recv_from(&mut reply) {
            Ok((len, peer)) => {
                let Ok(message) =
                    ads::udp::Message::parse(&reply[..len], ads::udp::ServiceId::Identify, true)
                else {
                    continue;
                };
                let identity = target_identity_from_udp_message(&message, peer);
                push_unique_target_identity(&mut results, identity);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(error) => {
                return Err(OnboardingWireError::new(
                    OnboardingWireErrorKind::UdpIdentifyBlocked,
                    format!("receive ADS UDP identify replies: {error}"),
                ));
            }
        }
    }

    if results.is_empty() {
        Err(OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("ADS UDP identify failed for {target_ip}: no target answered"),
        ))
    } else {
        Ok(results)
    }
}

#[cfg(feature = "ads-wire")]
fn push_unique_target_identity(results: &mut Vec<TargetIdentity>, identity: TargetIdentity) {
    let seen = results.iter().any(|existing| {
        existing.ams_net_id == identity.ams_net_id && existing.ams_port == identity.ams_port
    });
    if !seen {
        results.push(identity);
    }
}

#[cfg(feature = "ads-wire")]
fn project_sumup_payloads(
    resolved: &[AdsResolvedHandle],
    results: Vec<AdsReadResult>,
) -> Result<Vec<Vec<u8>>, OnboardingWireError> {
    if results.len() != resolved.len() {
        return Err(OnboardingWireError::new(
            OnboardingWireErrorKind::WrongPlcPort,
            format!(
                "ADS sum-up read returned {} results for {} requested handles",
                results.len(),
                resolved.len()
            ),
        ));
    }
    let mut results_by_point = BTreeMap::new();
    for result in results {
        if !resolved
            .iter()
            .any(|handle| handle.point_name == result.point_name)
        {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!(
                    "ADS sum-up read returned unknown point '{}'",
                    result.point_name
                ),
            ));
        }
        let point_name = result.point_name.clone();
        if results_by_point
            .insert(point_name.clone(), result)
            .is_some()
        {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!("ADS sum-up read returned duplicate point '{point_name}'"),
            ));
        }
    }

    let mut payloads = Vec::with_capacity(resolved.len());
    for handle in resolved {
        let result = results_by_point
            .remove(handle.point_name.as_str())
            .ok_or_else(|| {
                OnboardingWireError::new(
                    OnboardingWireErrorKind::WrongPlcPort,
                    format!(
                        "ADS sum-up read omitted requested point '{}'",
                        handle.point_name
                    ),
                )
            })?;
        if result.quality.state != QualityState::Good {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                result
                    .quality
                    .detail
                    .unwrap_or_else(|| "ADS sum-up read returned non-good quality".to_string()),
            ));
        }
        let Some(value) = result.value else {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                "ADS sum-up read returned no value",
            ));
        };
        let bytes = ads_bytes_from_value(&handle.data_type, &value).map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!("map ADS value for '{}': {error}", handle.point_name),
            )
        })?;
        payloads.push(bytes);
    }
    Ok(payloads)
}

#[cfg(feature = "ads-wire")]
fn require_expected_probe_readback(
    readback: Result<Value, OnboardingWireError>,
    expected: &Value,
    symbol: &str,
    restore: impl FnOnce() -> Result<(), OnboardingWireError>,
) -> Result<(), OnboardingWireError> {
    let failure = match readback {
        Ok(actual) if actual == *expected => return Ok(()),
        Ok(actual) => OnboardingWireError::new(
            OnboardingWireErrorKind::WrongPlcPort,
            format!(
                "ADS guarded write read-back mismatch for '{symbol}': wrote {expected:?}, read {actual:?}"
            ),
        ),
        Err(error) => error,
    };
    Err(restore_after_probe_failure(failure, restore))
}

#[cfg(feature = "ads-wire")]
fn require_probe_write(
    write: Result<(), OnboardingWireError>,
    restore: impl FnOnce() -> Result<(), OnboardingWireError>,
) -> Result<(), OnboardingWireError> {
    match write {
        Ok(()) => Ok(()),
        Err(error) => Err(restore_after_probe_failure(error, restore)),
    }
}

#[cfg(feature = "ads-wire")]
fn restore_after_probe_failure(
    mut failure: OnboardingWireError,
    restore: impl FnOnce() -> Result<(), OnboardingWireError>,
) -> OnboardingWireError {
    if let Err(restore_error) = restore() {
        failure.detail = format!(
            "{}; restoring the original value also failed: {}",
            failure.detail, restore_error
        );
    }
    failure
}

#[cfg(feature = "ads-wire")]
fn target_identity_from_udp_message(
    message: &ads::udp::Message,
    peer: SocketAddr,
) -> TargetIdentity {
    TargetIdentity {
        name: message
            .get_str(ads::udp::Tag::ComputerName)
            .filter(|name| !name.is_empty())
            .map(ToString::to_string),
        ip: peer.ip().to_string(),
        ams_net_id: message.get_source().netid().to_string(),
        ams_port: DEFAULT_ADS_PLC_PORT,
        tc_version: message
            .get_bytes(ads::udp::Tag::TCVersion)
            .and_then(parse_tc_version_tag)
            .and_then(tc_version_string),
    }
}

#[cfg(feature = "ads-wire")]
fn parse_tc_version_tag(bytes: &[u8]) -> Option<(u8, u8, u16)> {
    (bytes.len() >= 4).then(|| (bytes[0], bytes[1], u16::from_le_bytes([bytes[2], bytes[3]])))
}

#[cfg(feature = "ads-wire")]
fn tc_version_string(version: (u8, u8, u16)) -> Option<String> {
    if version == (0, 0, 0) {
        None
    } else {
        Some(format!("{}.{}.{}", version.0, version.1, version.2))
    }
}

#[cfg(feature = "ads-wire")]
fn first_socket_addr(target: (&str, u16)) -> Result<SocketAddr, std::io::Error> {
    target.to_socket_addrs()?.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no socket address resolved",
        )
    })
}

#[cfg(feature = "ads-wire")]
fn unsupported(message: &'static str) -> OnboardingWireError {
    OnboardingWireError::new(OnboardingWireErrorKind::UnsupportedOperation, message)
}

#[cfg(feature = "ads-wire")]
fn write_probe_type_matches(
    requested: &AdsDataTypeDescriptor,
    remote: &AdsDataTypeDescriptor,
) -> bool {
    requested.iec_type == remote.iec_type
        && requested.dimensions == remote.dimensions
        && requested.string_len == remote.string_len
        && requested.byte_len().ok() == remote.byte_len().ok()
}

#[cfg(feature = "ads-wire")]
fn map_transport_error(
    kind: OnboardingWireErrorKind,
    context: &'static str,
    error: impl std::fmt::Display,
) -> OnboardingWireError {
    OnboardingWireError::new(kind, format!("{context}: {error}"))
}

#[cfg(all(test, feature = "ads-wire"))]
mod tests;
