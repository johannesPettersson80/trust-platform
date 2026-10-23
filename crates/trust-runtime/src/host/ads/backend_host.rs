#[cfg(any(target_os = "windows", test))]
use std::net::IpAddr;
#[cfg(target_os = "windows")]
use std::net::ToSocketAddrs;

use trust_ads_core::{AdsRoute, PointQuality, SymbolDescriptor};

use super::backend_ads_rs::AdsRsTransport;
use super::backend_windows::WindowsAdsTransport;
use super::transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationSample, AdsReadResult, AdsResolvedHandle,
    AdsSubscribeRequest, AdsSubscription, AdsTransport, AdsTransportError, AdsWriteRequest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostAdsBackendKind {
    DirectTcp,
    NativeWindowsRouter,
}

pub struct HostAdsClient {
    backend: HostAdsTransportInner,
}

enum HostAdsTransportInner {
    DirectTcp(AdsRsTransport),
    NativeWindowsRouter(WindowsAdsTransport),
}

impl HostAdsClient {
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
}

impl AdsTransport for HostAdsClient {
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

#[cfg(not(target_os = "windows"))]
fn detected_backend_kind(_target_host: &str) -> HostAdsBackendKind {
    HostAdsBackendKind::DirectTcp
}

#[cfg(target_os = "windows")]
fn detected_backend_kind(target_host: &str) -> HostAdsBackendKind {
    let resolved_targets = resolve_target_addresses(target_host);
    let mut local_addresses = vec![
        IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
    ];
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        local_addresses.extend(interfaces.into_iter().map(|interface| interface.ip()));
    }
    let computer_name = std::env::var("COMPUTERNAME").ok();
    select_backend_kind(
        true,
        target_host,
        resolved_targets.as_slice(),
        local_addresses.as_slice(),
        computer_name.as_deref(),
    )
}

#[cfg(target_os = "windows")]
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

#[cfg(any(target_os = "windows", test))]
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

#[cfg(any(target_os = "windows", test))]
fn normalized_host(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

#[cfg(any(target_os = "windows", test))]
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
    fn windows_same_computer_targets_use_native_router() {
        for (host, resolved, local, computer_name) in [
            ("localhost", vec![ip("127.0.0.1")], vec![], None),
            ("127.0.0.1", vec![ip("127.0.0.1")], vec![], None),
            (
                "192.168.77.11",
                vec![ip("192.168.77.11")],
                vec![ip("192.168.77.11")],
                Some("PLC-LAPTOP"),
            ),
            ("plc-laptop", vec![], vec![], Some("PLC-LAPTOP")),
        ] {
            assert_eq!(
                select_backend_kind(true, host, &resolved, &local, computer_name),
                HostAdsBackendKind::NativeWindowsRouter
            );
        }
    }

    #[test]
    fn linux_and_windows_remote_targets_preserve_direct_tcp() {
        assert_eq!(
            select_backend_kind(
                false,
                "127.0.0.1",
                &[ip("127.0.0.1")],
                &[ip("127.0.0.1")],
                None
            ),
            HostAdsBackendKind::DirectTcp
        );
        assert_eq!(
            select_backend_kind(
                true,
                "192.168.77.40",
                &[ip("192.168.77.40")],
                &[ip("192.168.77.11")],
                Some("PLC-LAPTOP")
            ),
            HostAdsBackendKind::DirectTcp
        );
    }
}
