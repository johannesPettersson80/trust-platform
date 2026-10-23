use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use trust_ads_core::{
    ads_bytes_from_value, value_from_ads_bytes, AdsDataTypeDescriptor, AdsRoute, ArrayDimension,
    IecDataType, PointQuality, SymbolDescriptor, SymbolFlag, TransportSecurity, UpdateMode,
};

use super::identity::parse_canonical_ams_net_id;
use super::transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationMode, AdsNotificationSample, AdsPointAddress,
    AdsReadResult, AdsResolvedHandle, AdsSubscribeRequest, AdsSubscription, AdsTransport,
    AdsTransportError, AdsWriteRequest,
};

mod connection;
mod source_policy;

use connection::connect_ads_client;
use source_policy::ads_source_for_route;
#[cfg(test)]
use source_policy::DEFAULT_SOURCE_PORT;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_NOTIFICATION_CYCLE: Duration = Duration::from_millis(100);

// Names mirror Beckhoff AdsDef.h; ads-rs exposes these as raw fields only.
const ADS_SYMBOL_FLAG_PERSISTENT: u32 = 1 << 0;
const ADS_SYMBOL_FLAG_READ_ONLY: u32 = 1 << 5;

const ADS_BASE_TYPE_INT: u32 = 2;
const ADS_BASE_TYPE_DINT: u32 = 3;
const ADS_BASE_TYPE_REAL: u32 = 4;
const ADS_BASE_TYPE_LREAL: u32 = 5;
const ADS_BASE_TYPE_SINT: u32 = 16;
const ADS_BASE_TYPE_USINT: u32 = 17;
const ADS_BASE_TYPE_UINT: u32 = 18;
const ADS_BASE_TYPE_UDINT: u32 = 19;
const ADS_BASE_TYPE_LINT: u32 = 20;
const ADS_BASE_TYPE_ULINT: u32 = 21;
const ADS_BASE_TYPE_STRING: u32 = 30;
const ADS_BASE_TYPE_BOOL: u32 = 33;
const ADS_BASE_TYPE_COMPOUND: u32 = 65;

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
        if self.route.ams_port == 0 {
            return Err(AdsTransportError::new(
                "ADS target AMS port must be non-zero",
            ));
        }
        let [a, b, c, d, e, f] = parse_canonical_ams_net_id(&self.route.target_net_id.0)
            .ok_or_else(|| {
                AdsTransportError::new(format!(
                    "invalid target AMS Net ID '{}'; expected six decimal octets in canonical form",
                    self.route.target_net_id.0
                ))
            })?;
        Ok(ads::AmsAddr::new(
            ads::AmsNetId::new(a, b, c, d, e, f),
            self.route.ams_port,
        ))
    }

    fn connect_with<F>(&mut self, connector: F) -> Result<(), AdsTransportError>
    where
        F: FnOnce(
            (&str, u16),
            AdsRsTimeouts,
            ads::Source,
        ) -> Result<ads::Client, AdsTransportError>,
    {
        if !matches!(self.route.security.transport, TransportSecurity::Plain) {
            return Err(AdsTransportError::new(
                "Secure ADS is reserved but not implemented by the ads-rs backend",
            ));
        }
        if self.route.security.auto_add_route {
            return Err(AdsTransportError::new(
                "auto_add_route=true is reserved for authoring tools; runtime will not write AMS routes",
            ));
        }

        let _target = self.target_addr()?;
        let source = ads_source_for_route(&self.route)?;

        self.release_notifications();
        self.release_symbol_handles();
        self.notification_receiver = None;
        self.client = None;

        let client = connector((self.route.host.as_str(), ads::PORT), self.timeouts, source)?;
        self.notification_receiver = Some(client.get_notification_channel());
        self.client = Some(client);
        Ok(())
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
        let route = self.route.clone();
        self.connect_with(move |address, timeouts, _source| {
            connect_ads_client(address, timeouts, &route)
        })
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

pub(super) fn validate_requested_size(request: &AdsHandleRequest) -> Result<(), AdsTransportError> {
    let expected = checked_byte_len(&request.data_type)?;
    if let AdsPointAddress::Index { size, .. } = request.address {
        let actual = usize::try_from(size).map_err(|_| {
            AdsTransportError::new(format!(
                "ADS point '{}' index size exceeds usize",
                request.point_name
            ))
        })?;
        if actual != expected {
            return Err(AdsTransportError::new(format!(
                "ADS point '{}' index size mismatch: expected {expected}, got {actual}",
                request.point_name
            )));
        }
    }
    Ok(())
}

pub(super) fn read_write_address(handle: &AdsResolvedHandle) -> (u32, u32) {
    match handle.address {
        AdsPointAddress::Symbol(_) => (ads::index::RW_SYMVAL_BYHANDLE, handle.handle),
        AdsPointAddress::Index {
            index_group,
            index_offset,
            ..
        } => (index_group, index_offset),
    }
}

pub(super) fn symbol_descriptor_from_ads(
    symbol: &ads::symbol::Symbol,
    types: &ads::symbol::TypeMap,
) -> Result<Option<SymbolDescriptor>, AdsTransportError> {
    let type_info = types.get(symbol.typ.as_str());
    let Some(data_type) = data_type_descriptor_from_ads(symbol, type_info)? else {
        return Ok(None);
    };
    let byte_size = u32::try_from(symbol.size).map_err(|_| {
        AdsTransportError::new(format!(
            "ADS symbol '{}' size {} exceeds u32",
            symbol.name, symbol.size
        ))
    })?;
    let mut descriptor = SymbolDescriptor::new(
        symbol.name.clone(),
        data_type,
        symbol.ix_group,
        symbol.ix_offset,
        byte_size,
    )
    .with_flag(SymbolFlag::Read);
    if symbol.flags & ADS_SYMBOL_FLAG_READ_ONLY == 0 {
        descriptor = descriptor.with_flag(SymbolFlag::Write);
    }
    if symbol.flags & ADS_SYMBOL_FLAG_PERSISTENT != 0 {
        // ADS exposes remanent symbol state as the Persistent bit; there is no
        // separate RETAIN bit in the symbol header. Mark both so runtime
        // guardrails can treat all remanent remote state conservatively.
        descriptor = descriptor
            .with_flag(SymbolFlag::Persistent)
            .with_flag(SymbolFlag::Retain);
    }
    descriptor
        .validate_byte_size()
        .map_err(|err| AdsTransportError::new(err.to_string()))?;
    Ok(Some(descriptor))
}

fn data_type_descriptor_from_ads(
    symbol: &ads::symbol::Symbol,
    type_info: Option<&ads::symbol::Type>,
) -> Result<Option<AdsDataTypeDescriptor>, AdsTransportError> {
    let base_type = type_info
        .filter(|_| symbol.base_type == ADS_BASE_TYPE_COMPOUND)
        .map_or(symbol.base_type, |data_type| data_type.base_type);
    let source_name = type_info.map_or(symbol.typ.as_str(), |data_type| data_type.name.as_str());
    let Some(iec_type) = iec_type_from_ads(base_type, source_name) else {
        return Ok(None);
    };
    let dimensions = type_info
        .map(|data_type| {
            data_type
                .array
                .iter()
                .map(|(lower, upper)| ArrayDimension {
                    lower: i64::from(*lower),
                    upper: i64::from(*upper),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let string_len = if matches!(iec_type, IecDataType::String) {
        Some(string_capacity(symbol, type_info, dimensions.as_slice())?)
    } else {
        None
    };
    Ok(Some(AdsDataTypeDescriptor {
        source_name: symbol.typ.clone(),
        iec_type,
        dimensions,
        string_len,
    }))
}

fn iec_type_from_ads(base_type: u32, type_name: &str) -> Option<IecDataType> {
    let leaf_name = scalar_type_name(type_name);
    match leaf_name.as_str() {
        "BOOL" => Some(IecDataType::Bool),
        "SINT" => Some(IecDataType::Sint),
        "INT" => Some(IecDataType::Int),
        "DINT" => Some(IecDataType::Dint),
        "LINT" => Some(IecDataType::Lint),
        "USINT" => Some(IecDataType::Usint),
        "UINT" => Some(IecDataType::Uint),
        "UDINT" => Some(IecDataType::Udint),
        "ULINT" => Some(IecDataType::Ulint),
        "REAL" => Some(IecDataType::Real),
        "LREAL" => Some(IecDataType::Lreal),
        "BYTE" => Some(IecDataType::Byte),
        "WORD" => Some(IecDataType::Word),
        "DWORD" => Some(IecDataType::Dword),
        "LWORD" => Some(IecDataType::Lword),
        value if value.starts_with("STRING") => Some(IecDataType::String),
        _ => match base_type {
            ADS_BASE_TYPE_INT => Some(IecDataType::Int),
            ADS_BASE_TYPE_DINT => Some(IecDataType::Dint),
            ADS_BASE_TYPE_REAL => Some(IecDataType::Real),
            ADS_BASE_TYPE_LREAL => Some(IecDataType::Lreal),
            ADS_BASE_TYPE_SINT => Some(IecDataType::Sint),
            ADS_BASE_TYPE_USINT => Some(IecDataType::Usint),
            ADS_BASE_TYPE_UINT => Some(IecDataType::Uint),
            ADS_BASE_TYPE_UDINT => Some(IecDataType::Udint),
            ADS_BASE_TYPE_LINT => Some(IecDataType::Lint),
            ADS_BASE_TYPE_ULINT => Some(IecDataType::Ulint),
            ADS_BASE_TYPE_STRING => Some(IecDataType::String),
            ADS_BASE_TYPE_BOOL => Some(IecDataType::Bool),
            _ => None,
        },
    }
}

fn scalar_type_name(type_name: &str) -> String {
    let upper = type_name.trim().to_ascii_uppercase();
    if let Some((_, leaf)) = upper.rsplit_once(" OF ") {
        leaf.trim().to_string()
    } else {
        upper
    }
}

fn string_capacity(
    symbol: &ads::symbol::Symbol,
    type_info: Option<&ads::symbol::Type>,
    dimensions: &[ArrayDimension],
) -> Result<u16, AdsTransportError> {
    let total_size = type_info.map_or(symbol.size, |data_type| data_type.size);
    let elements = dimensions.iter().try_fold(1usize, |acc, dimension| {
        acc.checked_mul(dimension.len().map_err(map_mapping_error)?)
            .ok_or_else(|| AdsTransportError::new("ADS STRING array element count overflowed"))
    })?;
    if elements == 0 || total_size < elements {
        return Err(AdsTransportError::new(format!(
            "ADS STRING symbol '{}' has invalid size metadata",
            symbol.name
        )));
    }
    let scalar_size = total_size / elements;
    if scalar_size == 0 || !total_size.is_multiple_of(elements) {
        return Err(AdsTransportError::new(format!(
            "ADS STRING symbol '{}' size is not divisible by array elements",
            symbol.name
        )));
    }
    u16::try_from(scalar_size - 1).map_err(|_| {
        AdsTransportError::new(format!(
            "ADS STRING symbol '{}' capacity exceeds u16",
            symbol.name
        ))
    })
}

pub(super) fn checked_byte_len(
    descriptor: &AdsDataTypeDescriptor,
) -> Result<usize, AdsTransportError> {
    let byte_len = descriptor.byte_len().map_err(map_mapping_error)?;
    u32::try_from(byte_len)
        .map_err(|_| AdsTransportError::new("ADS point byte length exceeds u32"))?;
    Ok(byte_len)
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn map_ads_error(error: ads::Error) -> AdsTransportError {
    AdsTransportError::new(error.to_string())
}

fn parse_local_ams_net_id(
    value: &trust_ads_core::AmsNetId,
) -> Result<ads::AmsNetId, AdsTransportError> {
    let [a, b, c, d, e, f] = parse_canonical_ams_net_id(&value.0).ok_or_else(|| {
        AdsTransportError::new(format!(
            "invalid local AMS Net ID '{}'; expected six decimal octets in canonical form",
            value.0
        ))
    })?;
    Ok(ads::AmsNetId::new(a, b, c, d, e, f))
}

pub(super) fn map_mapping_error(error: impl std::fmt::Display) -> AdsTransportError {
    AdsTransportError::new(error.to_string())
}

#[cfg(test)]
mod tests;
