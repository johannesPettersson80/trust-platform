use std::collections::BTreeMap;
use std::time::Duration;

use trust_ads_core::{
    ads_bytes_from_value, value_from_ads_bytes, AdsRoute, PointQuality, SymbolDescriptor,
    UpdateMode,
};

use super::backend_common::{
    checked_byte_len, map_mapping_error, now_ms, read_write_address, symbol_descriptor_from_ads,
    validate_requested_size, validate_route_policy,
};
use super::transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationMode, AdsNotificationSample, AdsPointAddress,
    AdsReadResult, AdsResolvedHandle, AdsSubscribeRequest, AdsSubscription, AdsTransport,
    AdsTransportError, AdsTransportFailureKind, AdsWriteRequest,
};

mod connection;
mod source_policy;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_NOTIFICATION_CYCLE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdsRsTimeouts {
    pub connect: Option<Duration>,
    pub read: Option<Duration>,
    pub write: Option<Duration>,
}

impl Default for AdsRsTimeouts {
    fn default() -> Self {
        Self {
            connect: Some(DEFAULT_TIMEOUT),
            read: Some(DEFAULT_TIMEOUT),
            write: Some(DEFAULT_TIMEOUT),
        }
    }
}

impl AdsRsTimeouts {
    fn into_ads(self) -> ads::Timeouts {
        ads::Timeouts {
            connect: self.connect,
            read: self.read,
            write: self.write,
        }
    }
}

pub struct AdsRsTransport {
    route: AdsRoute,
    timeouts: AdsRsTimeouts,
    client: Option<ads::Client>,
    notification_receiver: Option<crossbeam_channel::Receiver<ads::notif::Notification>>,
    symbol_handles_by_point: BTreeMap<String, u32>,
    subscriptions_by_handle: BTreeMap<u32, AdsResolvedHandle>,
}

impl AdsRsTransport {
    #[must_use]
    pub fn new(route: AdsRoute) -> Self {
        Self::with_timeouts(route, AdsRsTimeouts::default())
    }

    #[must_use]
    pub fn with_timeouts(route: AdsRoute, timeouts: AdsRsTimeouts) -> Self {
        Self {
            route,
            timeouts,
            client: None,
            notification_receiver: None,
            symbol_handles_by_point: BTreeMap::new(),
            subscriptions_by_handle: BTreeMap::new(),
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

    fn device(&self) -> Result<ads::Device<'_>, AdsTransportError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| AdsTransportError::new("ADS transport is not connected"))?;
        Ok(client.device(self.target_addr()?))
    }

    fn target_addr(&self) -> Result<ads::AmsAddr, AdsTransportError> {
        Ok(ads::AmsAddr::new(
            parse_ams_net_id(&self.route.target_net_id)?,
            self.route.ams_port,
        ))
    }

    fn release_symbol_handles(&mut self) {
        let handles = std::mem::take(&mut self.symbol_handles_by_point);
        if handles.is_empty() {
            return;
        }
        for handle in handles.into_values() {
            self.release_symbol_handle(handle);
        }
    }

    fn release_symbol_handle(&self, handle: u32) {
        let Some(client) = self.client.as_ref() else {
            return;
        };
        let Ok(target) = self.target_addr() else {
            return;
        };
        let device = client.device(target);
        let _ = device.write(ads::index::RELEASE_SYMHANDLE, 0, &handle.to_le_bytes());
    }

    fn release_notifications(&mut self) {
        let handles = std::mem::take(&mut self.subscriptions_by_handle);
        if handles.is_empty() {
            return;
        }
        let Ok(device) = self.device() else {
            return;
        };
        for notification_handle in handles.into_keys() {
            let _ = device.delete_notification(notification_handle);
        }
    }
}

impl Drop for AdsRsTransport {
    fn drop(&mut self) {
        self.release_notifications();
        self.release_symbol_handles();
    }
}

impl AdsTransport for AdsRsTransport {
    fn connect(&mut self) -> Result<(), AdsTransportError> {
        validate_route_policy(&self.route, "ads-rs")?;

        self.release_notifications();
        self.release_symbol_handles();
        self.notification_receiver = None;
        self.client = None;

        let client = connection::connect_ads_client(
            (self.route.host.as_str(), ads::PORT),
            self.timeouts,
            &self.route,
        )?;
        self.notification_receiver = Some(client.get_notification_channel());
        self.client = Some(client);
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), AdsTransportError> {
        self.release_notifications();
        self.release_symbol_handles();
        self.notification_receiver = None;
        self.client = None;
        Ok(())
    }

    fn read_state(&mut self) -> Result<AdsDeviceState, AdsTransportError> {
        let (state, _) = self.device()?.get_state().map_err(map_ads_error)?;
        Ok(match state {
            ads::AdsState::Run => AdsDeviceState::Run,
            ads::AdsState::Stop | ads::AdsState::Idle | ads::AdsState::Config => {
                AdsDeviceState::Stop
            }
            ads::AdsState::Error | ads::AdsState::Exception => AdsDeviceState::Fault,
            _ => AdsDeviceState::Unknown,
        })
    }

    fn upload_symbol_table(&mut self) -> Result<Vec<SymbolDescriptor>, AdsTransportError> {
        let (symbols, types) =
            ads::symbol::get_symbol_info(self.device()?).map_err(map_ads_error)?;
        let mut descriptors = Vec::new();
        for symbol in &symbols {
            if let Some(descriptor) = symbol_descriptor_from_ads(symbol, &types)? {
                descriptors.push(descriptor);
            }
        }
        Ok(descriptors)
    }

    fn resolve_handles(
        &mut self,
        requests: &[AdsHandleRequest],
    ) -> Result<Vec<AdsResolvedHandle>, AdsTransportError> {
        let mut resolved = Vec::with_capacity(requests.len());
        for request in requests {
            validate_requested_size(request)?;
            let handle = match &request.address {
                AdsPointAddress::Symbol(symbol_name) => {
                    let remote_size = ads::symbol::get_size(self.device()?, symbol_name)
                        .map_err(map_ads_error)?;
                    let expected = checked_byte_len(&request.data_type)?;
                    if remote_size != expected {
                        return Err(AdsTransportError::new(format!(
                            "ADS symbol '{symbol_name}' byte size mismatch: expected {expected}, got {remote_size}"
                        )));
                    }
                    let handle = request_symbol_handle(self.device()?, symbol_name)?;
                    if let Some(previous) = self
                        .symbol_handles_by_point
                        .insert(request.point_name.clone(), handle)
                    {
                        self.release_symbol_handle(previous);
                    }
                    handle
                }
                AdsPointAddress::Index { .. } => {
                    if let Some(previous) = self.symbol_handles_by_point.remove(&request.point_name)
                    {
                        self.release_symbol_handle(previous);
                    }
                    0
                }
            };
            resolved.push(AdsResolvedHandle {
                point_name: request.point_name.clone(),
                address: request.address.clone(),
                data_type: request.data_type.clone(),
                handle,
            });
        }
        Ok(resolved)
    }

    fn sumup_read(
        &mut self,
        handles: &[AdsResolvedHandle],
    ) -> Result<Vec<AdsReadResult>, AdsTransportError> {
        let device = self.device()?;
        let now = now_ms();
        let mut results = handles
            .iter()
            .map(|handle| AdsReadResult {
                point_name: handle.point_name.clone(),
                value: None,
                quality: PointQuality::error(now, "ADS read was not issued"),
            })
            .collect::<Vec<_>>();
        let mut request_handles = Vec::new();
        let mut buffers = Vec::new();
        for (index, handle) in handles.iter().enumerate() {
            match checked_byte_len(&handle.data_type) {
                Ok(len) => {
                    request_handles.push((index, handle));
                    buffers.push(vec![0; len]);
                }
                Err(error) => {
                    results[index].quality = PointQuality::error(now, error.to_string());
                }
            }
        }
        if request_handles.is_empty() {
            return Ok(results);
        }
        let mut requests = request_handles
            .iter()
            .zip(buffers.iter_mut())
            .map(|((_, handle), buffer)| {
                let (index_group, index_offset) = read_write_address(handle);
                ads::client::ReadRequest::new(index_group, index_offset, buffer)
            })
            .collect::<Vec<_>>();

        device.read_multi(&mut requests).map_err(map_ads_error)?;

        for ((result_index, handle), request) in request_handles.iter().zip(requests.iter()) {
            results[*result_index] = match request.data() {
                Ok(data) => match value_from_ads_bytes(&handle.data_type, data) {
                    Ok(value) => AdsReadResult {
                        point_name: handle.point_name.clone(),
                        value: Some(value),
                        quality: PointQuality::good(now),
                    },
                    Err(error) => AdsReadResult {
                        point_name: handle.point_name.clone(),
                        value: None,
                        quality: PointQuality::error(now, error.to_string()),
                    },
                },
                Err(error) => AdsReadResult {
                    point_name: handle.point_name.clone(),
                    value: None,
                    quality: PointQuality::error(now, error.to_string()),
                },
            };
        }
        Ok(results)
    }

    fn sumup_write(
        &mut self,
        writes: &[AdsWriteRequest],
    ) -> Result<Vec<PointQuality>, AdsTransportError> {
        let device = self.device()?;
        let now = now_ms();
        let mut qualities = Vec::with_capacity(writes.len());
        let mut request_writes = Vec::new();
        let mut buffers = Vec::new();
        for (index, write) in writes.iter().enumerate() {
            match checked_byte_len(&write.handle.data_type).and_then(|_| {
                ads_bytes_from_value(&write.handle.data_type, &write.value)
                    .map_err(map_mapping_error)
            }) {
                Ok(buffer) => {
                    qualities.push(PointQuality::stale("ADS write pending"));
                    request_writes.push((index, write));
                    buffers.push(buffer);
                }
                Err(error) => {
                    qualities.push(PointQuality::error(now, error.to_string()));
                }
            }
        }
        if request_writes.is_empty() {
            return Ok(qualities);
        }
        let mut requests = request_writes
            .iter()
            .zip(buffers.iter())
            .map(|((_, write), buffer)| {
                let (index_group, index_offset) = read_write_address(&write.handle);
                ads::client::WriteRequest::new(index_group, index_offset, buffer)
            })
            .collect::<Vec<_>>();

        device.write_multi(&mut requests).map_err(map_ads_error)?;

        for ((quality_index, _), request) in request_writes.iter().zip(requests.iter()) {
            qualities[*quality_index] = match request.ensure() {
                Ok(()) => PointQuality::good(now),
                Err(error) => PointQuality::error(now, error.to_string()),
            };
        }
        Ok(qualities)
    }

    fn subscribe(
        &mut self,
        request: AdsSubscribeRequest,
    ) -> Result<AdsSubscription, AdsTransportError> {
        if request.mode != UpdateMode::Notify {
            return Err(AdsTransportError::new(
                "ADS subscribe only supports notify mode; poll points use sumup_read",
            ));
        }
        let length = checked_byte_len(&request.handle.data_type)?;
        let transmission_mode = match request.notification_mode {
            AdsNotificationMode::OnChange => ads::notif::TransmissionMode::ServerOnChange,
            AdsNotificationMode::Cyclic => ads::notif::TransmissionMode::ServerCycle,
        };
        let attributes = ads::notif::Attributes::new(
            length,
            transmission_mode,
            Duration::ZERO,
            DEFAULT_NOTIFICATION_CYCLE,
        );
        let (index_group, index_offset) = read_write_address(&request.handle);
        let subscription_id = self
            .device()?
            .add_notification(index_group, index_offset, &attributes)
            .map_err(map_ads_error)?;
        self.subscriptions_by_handle
            .insert(subscription_id, request.handle.clone());
        Ok(AdsSubscription {
            point_name: request.handle.point_name,
            subscription_id,
        })
    }

    fn drain_notifications(&mut self) -> Result<Vec<AdsNotificationSample>, AdsTransportError> {
        let receiver = self
            .notification_receiver
            .as_ref()
            .ok_or_else(|| AdsTransportError::new("ADS notification receiver is not connected"))?;
        let now = now_ms();
        let mut samples = Vec::new();
        for notification in receiver.try_iter() {
            for sample in notification.samples() {
                let Some(handle) = self.subscriptions_by_handle.get(&sample.handle) else {
                    samples.push(AdsNotificationSample {
                        point_name: format!("<unknown notification {}>", sample.handle),
                        subscription_id: sample.handle,
                        value: None,
                        quality: PointQuality::error(
                            now,
                            format!("ADS notification handle {} is unknown", sample.handle),
                        ),
                    });
                    continue;
                };
                samples.push(match value_from_ads_bytes(&handle.data_type, sample.data) {
                    Ok(value) => AdsNotificationSample {
                        point_name: handle.point_name.clone(),
                        subscription_id: sample.handle,
                        value: Some(value),
                        quality: PointQuality::good(now),
                    },
                    Err(error) => AdsNotificationSample {
                        point_name: handle.point_name.clone(),
                        subscription_id: sample.handle,
                        value: None,
                        quality: PointQuality::error(now, error.to_string()),
                    },
                });
            }
        }
        Ok(samples)
    }

    fn symbol_version(&mut self) -> Result<u32, AdsTransportError> {
        let mut bytes = [0; 1];
        self.device()?
            .read_exact(ads::index::GET_SYMVERSION, 0, &mut bytes)
            .map_err(map_ads_error)?;
        Ok(u32::from(bytes[0]))
    }
}

fn parse_ams_net_id(value: &trust_ads_core::AmsNetId) -> Result<ads::AmsNetId, AdsTransportError> {
    value
        .0
        .parse::<ads::AmsNetId>()
        .map_err(|err| AdsTransportError::new(format!("invalid AMS Net ID '{}': {err}", value.0)))
}

fn request_symbol_handle(
    device: ads::Device<'_>,
    symbol_name: &str,
) -> Result<u32, AdsTransportError> {
    let mut handle_bytes = [0; 4];
    device
        .write_read_exact(
            ads::index::GET_SYMHANDLE_BYNAME,
            0,
            symbol_name.as_bytes(),
            &mut handle_bytes,
        )
        .map_err(map_ads_error)?;
    Ok(u32::from_le_bytes(handle_bytes))
}

fn map_ads_error(error: ads::Error) -> AdsTransportError {
    match error {
        ads::Error::Ads(context, name, code) => {
            AdsTransportError::new(format!("{context}: {name} ({code:#x})"))
                .with_ads_error(code, name)
        }
        ads::Error::Io(context, error) => map_io_error(context, error),
        error => AdsTransportError::new(error.to_string()),
    }
}

pub(super) fn map_io_error(context: &str, error: std::io::Error) -> AdsTransportError {
    let failure_kind = io_failure_kind(error.kind());
    let mut mapped = AdsTransportError::new(format!("{context}: {error}"));
    if let Some(kind) = failure_kind {
        mapped = mapped.with_failure_kind(kind);
    }
    mapped
}

fn io_failure_kind(kind: std::io::ErrorKind) -> Option<AdsTransportFailureKind> {
    match kind {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
            Some(AdsTransportFailureKind::TimedOut)
        }
        std::io::ErrorKind::ConnectionRefused => Some(AdsTransportFailureKind::ConnectionRefused),
        std::io::ErrorKind::HostUnreachable => Some(AdsTransportFailureKind::HostUnreachable),
        std::io::ErrorKind::NetworkUnreachable => Some(AdsTransportFailureKind::NetworkUnreachable),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_ads_core::{
        AdsDataTypeDescriptor, AdsSecurityPolicy, AmsNetId, IecDataType, TransportSecurity,
    };

    #[test]
    fn ads_error_code_and_name_survive_transport_mapping() {
        let error = map_ads_error(ads::Error::Ads(
            "read reply",
            "Router: port not registered",
            0x507,
        ));

        assert_eq!(
            error.ads_error(),
            Some(&super::super::transport::AdsTransportErrorInfo::new(
                0x507,
                "Router: port not registered"
            ))
        );
        assert_eq!(error.failure_kind(), None);
    }

    #[test]
    fn io_timeout_survives_transport_mapping_without_message_parsing() {
        let error = map_ads_error(ads::Error::Io(
            "receiving reply",
            std::io::Error::new(std::io::ErrorKind::TimedOut, "localized message"),
        ));

        assert_eq!(
            error.failure_kind(),
            Some(AdsTransportFailureKind::TimedOut)
        );
        assert!(error.ads_error().is_none());
    }

    #[test]
    fn io_connection_refusal_survives_transport_mapping_without_message_parsing() {
        let error = map_ads_error(ads::Error::Io(
            "connecting to target",
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "localized message"),
        ));

        assert_eq!(
            error.failure_kind(),
            Some(AdsTransportFailureKind::ConnectionRefused)
        );
        assert!(error.ads_error().is_none());
    }

    #[test]
    fn subscribe_rejects_poll_mode_before_connection() {
        let mut transport = AdsRsTransport::new(AdsRoute {
            name: "line1".to_string(),
            target_net_id: AmsNetId::new("5.23.91.12.1.1"),
            host: "192.168.10.5".to_string(),
            ams_port: 851,
            local_net_id: None,
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Plain,
                auto_add_route: false,
            },
        });
        let request = AdsSubscribeRequest {
            handle: AdsResolvedHandle {
                point_name: "line1_temp".to_string(),
                address: AdsPointAddress::Index {
                    index_group: 0x4020,
                    index_offset: 0,
                    size: 4,
                },
                data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                handle: 0,
            },
            mode: UpdateMode::Poll,
            notification_mode: AdsNotificationMode::OnChange,
        };

        let error = transport
            .subscribe(request)
            .expect_err("poll subscriptions must be rejected locally");

        assert!(error.message().contains("poll points use sumup_read"));
    }

    #[test]
    fn connect_rejects_secure_transport_before_network() {
        let mut transport = AdsRsTransport::new(AdsRoute {
            name: "line1".to_string(),
            target_net_id: AmsNetId::new("5.23.91.12.1.1"),
            host: "192.0.2.5".to_string(),
            ams_port: 851,
            local_net_id: None,
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Secure,
                auto_add_route: false,
            },
        });

        let error = transport
            .connect()
            .expect_err("secure transport is reserved");

        assert!(error.message().contains("Secure ADS is reserved"));
    }

    #[test]
    fn connect_rejects_auto_add_route_before_network() {
        let mut transport = AdsRsTransport::new(AdsRoute {
            name: "line1".to_string(),
            target_net_id: AmsNetId::new("5.23.91.12.1.1"),
            host: "192.0.2.5".to_string(),
            ams_port: 851,
            local_net_id: None,
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Plain,
                auto_add_route: true,
            },
        });

        let error = transport
            .connect()
            .expect_err("runtime must not write AMS routes");

        assert!(error.message().contains("auto_add_route=true"));
    }
}
