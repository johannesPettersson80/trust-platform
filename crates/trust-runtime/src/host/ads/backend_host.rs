#[cfg(any(windows, test))]
use std::net::IpAddr;
#[cfg(windows)]
use std::net::ToSocketAddrs;

use trust_ads_core::{AdsRoute, PointQuality, SymbolDescriptor};

use super::backend_ads_rs::AdsRsTransport;
use super::backend_windows::WindowsAdsTransport;
use super::transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationSample, AdsReadResult, AdsResolvedHandle,
    AdsSubscribeRequest, AdsSubscription, AdsTransport, AdsTransportError, AdsWriteRequest,
};

/// Concrete backend selected for an ADS target on the current host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostAdsBackendKind {
    /// Existing cross-platform ADS/TCP client used for remote targets.
    DirectTcp,
    /// Installed TwinCAT router API used for targets on the same Windows host.
    NativeWindowsRouter,
}

/// Host-aware ADS transport. It preserves direct ADS/TCP for remote devices,
/// but same-computer Windows traffic is never sent through the raw TCP client.
pub struct HostAdsTransport {
    backend: HostAdsTransportInner,
}

enum HostAdsTransportInner {
    DirectTcp(AdsRsTransport),
    NativeWindowsRouter(WindowsAdsTransport),
}

impl HostAdsTransport {
    #[must_use]
    pub fn new(route: AdsRoute) -> Self {
        let backend = match detected_backend_kind(route.host.as_str()) {
            HostAdsBackendKind::DirectTcp => {
                HostAdsTransportInner::DirectTcp(AdsRsTransport::new(route))
            }
            HostAdsBackendKind::NativeWindowsRouter => {
                HostAdsTransportInner::NativeWindowsRouter(WindowsAdsTransport::new(route))
            }
        };
        Self { backend }
    }

    #[must_use]
    pub fn backend_kind(&self) -> HostAdsBackendKind {
        match &self.backend {
            HostAdsTransportInner::DirectTcp(_) => HostAdsBackendKind::DirectTcp,
            HostAdsTransportInner::NativeWindowsRouter(_) => {
                HostAdsBackendKind::NativeWindowsRouter
            }
        }
    }

    pub fn connect(&mut self) -> Result<(), AdsTransportError> {
        <Self as AdsTransport>::connect(self)
    }

    pub fn disconnect(&mut self) -> Result<(), AdsTransportError> {
        <Self as AdsTransport>::disconnect(self)
    }

    pub fn read_state(&mut self) -> Result<AdsDeviceState, AdsTransportError> {
        <Self as AdsTransport>::read_state(self)
    }

    pub fn upload_symbol_table(&mut self) -> Result<Vec<SymbolDescriptor>, AdsTransportError> {
        <Self as AdsTransport>::upload_symbol_table(self)
    }

    pub fn resolve_handles(
        &mut self,
        requests: &[AdsHandleRequest],
    ) -> Result<Vec<AdsResolvedHandle>, AdsTransportError> {
        <Self as AdsTransport>::resolve_handles(self, requests)
    }

    pub fn sumup_read(
        &mut self,
        handles: &[AdsResolvedHandle],
    ) -> Result<Vec<AdsReadResult>, AdsTransportError> {
        <Self as AdsTransport>::sumup_read(self, handles)
    }

    pub fn sumup_write(
        &mut self,
        writes: &[AdsWriteRequest],
    ) -> Result<Vec<PointQuality>, AdsTransportError> {
        <Self as AdsTransport>::sumup_write(self, writes)
    }

    pub fn subscribe(
        &mut self,
        request: AdsSubscribeRequest,
    ) -> Result<AdsSubscription, AdsTransportError> {
        <Self as AdsTransport>::subscribe(self, request)
    }

    pub fn drain_notifications(&mut self) -> Result<Vec<AdsNotificationSample>, AdsTransportError> {
        <Self as AdsTransport>::drain_notifications(self)
    }

    pub fn symbol_version(&mut self) -> Result<u32, AdsTransportError> {
        <Self as AdsTransport>::symbol_version(self)
    }

    /// Native source AMS address assigned by the local TwinCAT router.
    ///
    /// This is available only while the native Windows backend is connected.
    #[must_use]
    pub fn native_local_address(&self) -> Option<(String, u16)> {
        match &self.backend {
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport
                .local_address()
                .map(|address| (address.net_id.to_string(), address.port)),
            HostAdsTransportInner::DirectTcp(_) => None,
        }
    }
}

impl AdsTransport for HostAdsTransport {
    fn connect(&mut self) -> Result<(), AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.connect(),
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport.connect(),
        }
    }

    fn disconnect(&mut self) -> Result<(), AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.disconnect(),
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport.disconnect(),
        }
    }

    fn read_state(&mut self) -> Result<AdsDeviceState, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.read_state(),
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport.read_state(),
        }
    }

    fn upload_symbol_table(&mut self) -> Result<Vec<SymbolDescriptor>, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.upload_symbol_table(),
            HostAdsTransportInner::NativeWindowsRouter(transport) => {
                transport.upload_symbol_table()
            }
        }
    }

    fn resolve_handles(
        &mut self,
        requests: &[AdsHandleRequest],
    ) -> Result<Vec<AdsResolvedHandle>, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.resolve_handles(requests),
            HostAdsTransportInner::NativeWindowsRouter(transport) => {
                transport.resolve_handles(requests)
            }
        }
    }

    fn sumup_read(
        &mut self,
        handles: &[AdsResolvedHandle],
    ) -> Result<Vec<AdsReadResult>, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.sumup_read(handles),
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport.sumup_read(handles),
        }
    }

    fn sumup_write(
        &mut self,
        writes: &[AdsWriteRequest],
    ) -> Result<Vec<PointQuality>, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.sumup_write(writes),
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport.sumup_write(writes),
        }
    }

    fn subscribe(
        &mut self,
        request: AdsSubscribeRequest,
    ) -> Result<AdsSubscription, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.subscribe(request),
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport.subscribe(request),
        }
    }

    fn drain_notifications(&mut self) -> Result<Vec<AdsNotificationSample>, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.drain_notifications(),
            HostAdsTransportInner::NativeWindowsRouter(transport) => {
                transport.drain_notifications()
            }
        }
    }

    fn symbol_version(&mut self) -> Result<u32, AdsTransportError> {
        match &mut self.backend {
            HostAdsTransportInner::DirectTcp(transport) => transport.symbol_version(),
            HostAdsTransportInner::NativeWindowsRouter(transport) => transport.symbol_version(),
        }
    }
}

#[cfg(not(windows))]
fn detected_backend_kind(_target_host: &str) -> HostAdsBackendKind {
    HostAdsBackendKind::DirectTcp
}

#[cfg(windows)]
fn detected_backend_kind(target_host: &str) -> HostAdsBackendKind {
    let resolved_targets = resolve_target_addresses(target_host);
    let mut local_addresses = vec![
        IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
    ];
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        local_addresses.extend(interfaces.into_iter().map(|interface| interface.ip()));
    }
    local_addresses.extend(
        resolved_targets
            .iter()
            .filter_map(|target| routed_source_address(*target)),
    );
    let computer_name = std::env::var("COMPUTERNAME").ok();
    select_backend_kind(
        true,
        target_host,
        resolved_targets.as_slice(),
        local_addresses.as_slice(),
        computer_name.as_deref(),
    )
}

pub(super) fn target_uses_native_windows_router(target_host: &str) -> bool {
    #[cfg(windows)]
    {
        detected_backend_kind(target_host) == HostAdsBackendKind::NativeWindowsRouter
    }
    #[cfg(not(windows))]
    {
        let _ = target_host;
        false
    }
}

#[cfg(windows)]
fn resolve_target_addresses(target_host: &str) -> Vec<IpAddr> {
    let normalized = target_host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    if let Ok(address) = normalized.parse::<IpAddr>() {
        return vec![address];
    }
    (normalized, ads::PORT)
        .to_socket_addrs()
        .map(|addresses| addresses.map(|address| address.ip()).collect())
        .unwrap_or_default()
}

#[cfg(windows)]
fn routed_source_address(target: IpAddr) -> Option<IpAddr> {
    let bind = match target {
        IpAddr::V4(_) => "0.0.0.0:0",
        IpAddr::V6(_) => "[::]:0",
    };
    let socket = std::net::UdpSocket::bind(bind).ok()?;
    socket.connect((target, 9)).ok()?;
    socket.local_addr().ok().map(|address| address.ip())
}

#[cfg(any(windows, test))]
fn select_backend_kind(
    platform_is_windows: bool,
    target_host: &str,
    resolved_targets: &[IpAddr],
    local_addresses: &[IpAddr],
    computer_name: Option<&str>,
) -> HostAdsBackendKind {
    if !platform_is_windows {
        return HostAdsBackendKind::DirectTcp;
    }
    let target_name = normalized_host(target_host);
    if target_name == "localhost"
        || computer_name.is_some_and(|name| target_name == normalized_host(name))
        || resolved_targets.iter().any(IpAddr::is_loopback)
        || resolved_targets.iter().any(|target| {
            local_addresses
                .iter()
                .any(|local| canonical_ip(*target) == canonical_ip(*local))
        })
    {
        HostAdsBackendKind::NativeWindowsRouter
    } else {
        HostAdsBackendKind::DirectTcp
    }
}

#[cfg(any(windows, test))]
fn normalized_host(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

#[cfg(any(windows, test))]
fn canonical_ip(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map_or(IpAddr::V6(address), IpAddr::V4),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(value: &str) -> IpAddr {
        value.parse().expect("test IP")
    }

    #[test]
    fn non_windows_always_preserves_direct_ads_tcp() {
        assert_eq!(
            select_backend_kind(
                false,
                "localhost",
                &[ip("127.0.0.1")],
                &[ip("127.0.0.1")],
                Some("PLC-DEV")
            ),
            HostAdsBackendKind::DirectTcp
        );
    }

    #[test]
    fn windows_loopback_and_localhost_use_native_router() {
        for (host, resolved) in [
            ("localhost", ip("127.0.0.1")),
            ("127.0.0.1", ip("127.0.0.1")),
            ("[::1]", ip("::1")),
        ] {
            assert_eq!(
                select_backend_kind(true, host, &[resolved], &[], None),
                HostAdsBackendKind::NativeWindowsRouter
            );
        }
    }

    #[test]
    fn windows_local_nic_ip_and_computer_name_use_native_router() {
        assert_eq!(
            select_backend_kind(
                true,
                "192.168.50.42",
                &[ip("192.168.50.42")],
                &[ip("192.168.50.42")],
                Some("PLC-DEV")
            ),
            HostAdsBackendKind::NativeWindowsRouter
        );
        assert_eq!(
            select_backend_kind(true, "plc-dev.", &[], &[], Some("PLC-DEV")),
            HostAdsBackendKind::NativeWindowsRouter
        );
    }

    #[test]
    fn windows_remote_target_preserves_direct_ads_tcp() {
        assert_eq!(
            select_backend_kind(
                true,
                "192.168.77.40",
                &[ip("192.168.77.40")],
                &[ip("192.168.50.42")],
                Some("PLC-DEV")
            ),
            HostAdsBackendKind::DirectTcp
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn linux_factory_uses_direct_transport_even_for_loopback() {
        let route = AdsRoute::new(
            "fixture",
            trust_ads_core::AmsNetId::new("127.0.0.1.1.1"),
            "127.0.0.1",
            851,
        );
        assert_eq!(
            HostAdsTransport::new(route).backend_kind(),
            HostAdsBackendKind::DirectTcp
        );
    }
}
