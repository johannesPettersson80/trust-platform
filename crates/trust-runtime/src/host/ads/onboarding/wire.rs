use crate::ads::diagnostics::{LocalIdentity, TargetIdentity};
use crate::ads::onboarding::errors::{OnboardingWireError, OnboardingWireErrorKind};
use crate::ads::onboarding::route::{RouteAddRequest, RouteRemoveRequest};
#[cfg(feature = "ads-wire")]
use crate::ads::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationMode, AdsPointAddress, AdsResolvedHandle,
    AdsSubscribeRequest, AdsSubscription, AdsTransportError, AdsWriteRequest, HostAdsTransport,
};
#[cfg(feature = "ads-wire")]
use std::collections::BTreeMap;
#[cfg(feature = "ads-wire")]
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
#[cfg(feature = "ads-wire")]
use std::time::{Duration, Instant};
#[cfg(feature = "ads-wire")]
use trust_ads_core::{
    ads_bytes_from_value, AdsRoute, AdsSecurityPolicy, AmsNetId, TransportSecurity, UpdateMode,
};
use trust_ads_core::{AdsDataTypeDescriptor, QualityState, SymbolDescriptor, SymbolFlag};
use trust_runtime_core::value::Value;

mod contract;
#[cfg(feature = "ads-wire")]
mod host_policy;
#[cfg(feature = "ads-wire")]
mod identity;
mod mock;

pub use contract::{
    AdsOnboardingWire, AdsReadUpdateSample, AdsRouteRequirement, AdsRouterProbe,
    DirectedIdentityObservation, DirectedIdentityTransport, GuardedWriteProbe,
    NativeAmsSourceAddress, ObservedAdsIdentity,
};
pub use mock::{MockAdsOnboardingScenario, MockAdsOnboardingWire};

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
#[cfg(feature = "ads-wire")]
const READ_UPDATE_SAMPLE_TIMEOUT: Duration = Duration::from_secs(4);
#[cfg(feature = "ads-wire")]
const READ_UPDATE_SAMPLE_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Host-aware implementation of the setup/control-plane wire boundary.
///
/// Remote targets use the cross-platform ADS/TCP client. Targets on the same
/// Windows computer use the installed TwinCAT router API without a self-route.
#[cfg(feature = "ads-wire")]
pub struct AdsRsOnboardingWire {
    tcp_probe_timeout: Duration,
    udp_broadcast_window: Duration,
    transport_key: Option<String>,
    transport: Option<HostAdsTransport>,
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
        Self::with_discovery_timeouts(tcp_probe_timeout, DEFAULT_UDP_BROADCAST_WINDOW)
    }

    pub fn with_discovery_timeouts(
        tcp_probe_timeout: Duration,
        udp_broadcast_window: Duration,
    ) -> Self {
        Self {
            tcp_probe_timeout,
            udp_broadcast_window,
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
    ) -> Result<&mut HostAdsTransport, OnboardingWireError> {
        let key = self.transport_key_for(target);
        if self.transport_key.as_deref() != Some(key.as_str()) {
            self.transport = None;
            self.symbols_by_name.clear();
            self.handles_by_id.clear();
            self.transport_key = Some(key);
        }
        if self.transport.is_none() {
            let route = self.route_for_target(target);
            let mut transport = HostAdsTransport::new(route);
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

    fn subscribe_symbol(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
        notification_mode: AdsNotificationMode,
    ) -> Result<AdsSubscription, OnboardingWireError> {
        if !self.symbols_by_name.contains_key(symbol) {
            let _ = self.upload_symbols(target)?;
        }
        let handle = self.resolve_handle(target, symbol)?;
        let resolved = self.handles_by_id.get(&handle).cloned().ok_or_else(|| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::NotificationFailure,
                format!("ADS handle {handle} is not resolved for read updates"),
            )
        })?;
        self.ensure_transport(target, OnboardingWireErrorKind::NotificationFailure)?
            .subscribe(AdsSubscribeRequest {
                handle: resolved,
                mode: UpdateMode::Notify,
                notification_mode,
            })
            .map_err(|error| {
                map_transport_error(
                    OnboardingWireErrorKind::NotificationFailure,
                    "subscribe ADS read update",
                    error,
                )
            })
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
        identity::identify_target(target_ip, self.tcp_probe_timeout).map(|result| {
            let identity = result.identity;
            TargetIdentity {
                name: identity.name,
                ip: identity.ip,
                ams_net_id: identity.ams_net_id,
                ams_port: identity.preferred_ams_port.unwrap_or(DEFAULT_ADS_PLC_PORT),
                tc_version: identity.tc_version,
            }
        })
    }

    fn directed_identity(
        &mut self,
        target_ip: &str,
    ) -> Result<DirectedIdentityObservation, OnboardingWireError> {
        identity::identify_target(target_ip, self.tcp_probe_timeout)
    }

    fn directed_identities(
        &mut self,
        target_ip: &str,
    ) -> Result<Vec<DirectedIdentityObservation>, OnboardingWireError> {
        identity::identify_targets(target_ip, self.tcp_probe_timeout)
    }

    fn udp_identify_all(
        &mut self,
        target_ip: &str,
    ) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
        identity::identify_all(target_ip, self.udp_broadcast_window)
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

    fn probe_ads_router(
        &mut self,
        target: &TargetIdentity,
    ) -> Result<AdsRouterProbe, OnboardingWireError> {
        host_policy::probe_ads_router(self, target)
    }

    fn route_requirement(&self, target_ip: &str) -> AdsRouteRequirement {
        host_policy::route_requirement(target_ip)
    }

    fn native_source_address(&self) -> Option<NativeAmsSourceAddress> {
        self.transport
            .as_ref()
            .and_then(HostAdsTransport::native_local_address)
            .map(|(ams_net_id, ams_port)| NativeAmsSourceAddress::new(ams_net_id, ams_port))
    }

    fn check_route(
        &mut self,
        target: &TargetIdentity,
        local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError> {
        self.local_identity = Some(local.clone());
        self.transport = None;
        let failure_kind = host_policy::route_failure_kind(&target.ip);
        self.ensure_transport(target, failure_kind).map(drop)
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
        let mut payloads = Vec::with_capacity(results.len());
        for result in results {
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
            let handle = resolved
                .iter()
                .find(|handle| handle.point_name == result.point_name)
                .ok_or_else(|| {
                    OnboardingWireError::new(
                        OnboardingWireErrorKind::WrongPlcPort,
                        format!(
                            "ADS sum-up read returned unknown point '{}'",
                            result.point_name
                        ),
                    )
                })?;
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
        self.write_one_value(target, &resolved, probe.value.clone(), "probe value")?;
        let probe_read_back = self.read_until_value(target, &resolved, &probe.value)?;
        if probe_read_back != probe.value {
            let _ = self.write_one_value(target, &resolved, original.clone(), "restore value");
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!(
                    "ADS guarded write read-back mismatch for '{}': wrote {:?}, read {:?}",
                    probe.symbol, probe.value, probe_read_back
                ),
            ));
        }
        self.write_one_value(target, &resolved, original.clone(), "restore value")?;
        let restored = self.read_until_value(target, &resolved, &original)?;
        if restored != original {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                format!(
                    "ADS guarded write restore mismatch for '{}': original {:?}, restored {:?}",
                    probe.symbol, original, restored
                ),
            ));
        }
        Ok(())
    }

    fn subscribe_notification(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<(), OnboardingWireError> {
        self.subscribe_symbol(target, symbol, AdsNotificationMode::OnChange)
            .map(drop)
    }

    fn read_update_sample(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<AdsReadUpdateSample, OnboardingWireError> {
        let subscription = self.subscribe_symbol(target, symbol, AdsNotificationMode::Cyclic)?;
        let deadline = Instant::now()
            .checked_add(READ_UPDATE_SAMPLE_TIMEOUT)
            .unwrap_or_else(Instant::now);
        loop {
            let samples = self
                .ensure_transport(target, OnboardingWireErrorKind::NotificationFailure)?
                .drain_notifications()
                .map_err(|error| {
                    map_transport_error(
                        OnboardingWireErrorKind::NotificationFailure,
                        "read subscribed ADS update",
                        error,
                    )
                })?;
            if let Some(sample) = samples
                .into_iter()
                .find(|sample| sample.subscription_id == subscription.subscription_id)
            {
                if sample.quality.state != QualityState::Good || sample.value.is_none() {
                    return Err(OnboardingWireError::new(
                        OnboardingWireErrorKind::NotificationFailure,
                        sample.quality.detail.unwrap_or_else(|| {
                            format!(
                                "ADS read update for '{}' contained no readable value",
                                sample.point_name
                            )
                        }),
                    ));
                }
                return Ok(AdsReadUpdateSample {
                    point_name: sample.point_name,
                    subscription_id: sample.subscription_id,
                    quality: sample.quality.state,
                });
            }
            if Instant::now() >= deadline {
                return Err(OnboardingWireError::new(
                    OnboardingWireErrorKind::NotificationFailure,
                    format!(
                        "no ADS read update arrived for '{symbol}' within {} ms",
                        READ_UPDATE_SAMPLE_TIMEOUT.as_millis()
                    ),
                )
                .with_transport_failure(crate::ads::AdsTransportFailureKind::TimedOut));
            }
            std::thread::sleep(READ_UPDATE_SAMPLE_POLL_INTERVAL);
        }
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
    error: AdsTransportError,
) -> OnboardingWireError {
    let ads_error = error.ads_error().cloned();
    let transport_failure = error.failure_kind();
    let mut mapped = OnboardingWireError::new(kind, format!("{context}: {error}"));
    if let Some(error) = ads_error {
        mapped = mapped.with_ads_error(error.code, error.name);
    }
    if let Some(failure) = transport_failure {
        mapped = mapped.with_transport_failure(failure);
    }
    mapped
}

#[cfg(all(test, feature = "ads-wire"))]
mod transport_error_tests {
    use super::*;

    #[test]
    fn exact_ads_0x507_survives_transport_to_onboarding_boundary() {
        let error = AdsTransportError::new("localized native error")
            .with_ads_error(0x507, "Router: port not registered");

        let error = map_transport_error(
            OnboardingWireErrorKind::NoSymbols,
            "upload ADS symbol table",
            error,
        );

        let ads_error = error.ads_error.expect("structured ADS error");
        assert_eq!(ads_error.code, 0x507);
        assert_eq!(ads_error.name, "Router: port not registered");
    }

    #[test]
    fn timeout_semantics_survive_transport_to_onboarding_boundary() {
        let error = AdsTransportError::new("localized native error")
            .with_failure_kind(crate::ads::AdsTransportFailureKind::TimedOut);

        let error = map_transport_error(
            OnboardingWireErrorKind::NoSymbols,
            "upload ADS symbol table",
            error,
        );

        assert_eq!(
            error.transport_failure,
            Some(crate::ads::AdsTransportFailureKind::TimedOut)
        );
    }
}
