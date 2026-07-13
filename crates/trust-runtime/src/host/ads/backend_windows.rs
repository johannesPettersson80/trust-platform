use std::collections::BTreeMap;
use std::time::Duration;

use trust_ads_core::{
    ads_bytes_from_value, value_from_ads_bytes, AdsRoute, PointQuality, QualityState,
    SymbolDescriptor, UpdateMode,
};
use trust_ads_windows::{AdsError as NativeAdsError, AdsPort, AmsAddress, AmsNetId, TcAdsDll};

use super::backend_common::{
    checked_byte_len, map_mapping_error, now_ms, read_write_address, symbol_descriptor_from_ads,
    validate_requested_size, validate_route_policy,
};
use super::transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationMode, AdsNotificationSample, AdsPointAddress,
    AdsReadResult, AdsResolvedHandle, AdsSubscribeRequest, AdsSubscription, AdsTransport,
    AdsTransportError, AdsTransportFailureKind, AdsWriteRequest,
};

// The synchronous TcAdsDll API cannot be safely pre-empted. Keep its native
// deadline below the five-second doctor budget so a missing reply is bounded
// by the DLL before the engine evaluates the step deadline.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(4);
const SYMBOL_UPLOAD_INFO_BYTES: usize = 64;
const MAX_SYMBOL_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

trait NativeAdsPort: Send {
    fn read_state(
        &mut self,
        target: &AmsAddress,
    ) -> Result<trust_ads_windows::AdsDeviceState, NativeAdsError>;
    fn read(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, NativeAdsError>;
    fn write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), NativeAdsError>;
    fn read_write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, NativeAdsError>;
}

impl NativeAdsPort for AdsPort {
    fn read_state(
        &mut self,
        target: &AmsAddress,
    ) -> Result<trust_ads_windows::AdsDeviceState, NativeAdsError> {
        self.read_state(target)
    }

    fn read(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, NativeAdsError> {
        self.read(target, index_group, index_offset, buffer)
    }

    fn write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), NativeAdsError> {
        self.write(target, index_group, index_offset, bytes)
    }

    fn read_write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, NativeAdsError> {
        self.read_write(target, index_group, index_offset, read_buffer, write_bytes)
    }
}

/// Safe adapter from the runtime transport contract to the installed Windows
/// TwinCAT router. All foreign calls stay inside `trust-ads-windows`.
pub(super) struct WindowsAdsTransport {
    route: AdsRoute,
    port: Option<Box<dyn NativeAdsPort>>,
    local_address: Option<AmsAddress>,
    symbol_handles_by_point: BTreeMap<String, u32>,
    subscriptions: BTreeMap<u32, PollingSubscription>,
    next_subscription_id: u32,
}

#[derive(Clone)]
struct PollingSubscription {
    handle: AdsResolvedHandle,
    notification_mode: AdsNotificationMode,
    last_value: Option<trust_runtime_core::value::Value>,
    last_quality: Option<QualityState>,
}

impl WindowsAdsTransport {
    #[must_use]
    pub(super) fn new(route: AdsRoute) -> Self {
        Self {
            route,
            port: None,
            local_address: None,
            symbol_handles_by_point: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            next_subscription_id: 1,
        }
    }

    #[cfg(test)]
    fn with_test_port(route: AdsRoute, port: impl NativeAdsPort + 'static) -> Self {
        let mut transport = Self::new(route);
        transport.port = Some(Box::new(port));
        transport
    }

    fn target(&self) -> Result<AmsAddress, AdsTransportError> {
        let net_id = self
            .route
            .target_net_id
            .0
            .parse::<AmsNetId>()
            .map_err(|error| {
                AdsTransportError::new(format!(
                    "invalid target AMS Net ID '{}': {error}",
                    self.route.target_net_id.0
                ))
            })?;
        Ok(AmsAddress::new(net_id, self.route.ams_port))
    }

    fn port(&mut self) -> Result<&mut (dyn NativeAdsPort + '_), AdsTransportError> {
        match self.port.as_mut() {
            Some(port) => Ok(port.as_mut()),
            None => Err(AdsTransportError::new(
                "native Windows ADS transport is not connected",
            )),
        }
    }

    pub(super) fn local_address(&self) -> Option<AmsAddress> {
        self.local_address
    }

    fn read_exact(
        &mut self,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<(), AdsTransportError> {
        let target = self.target()?;
        let expected = buffer.len();
        let returned = self
            .port()?
            .read(&target, index_group, index_offset, buffer)
            .map_err(|error| {
                map_native_call("read ADS data through the local TwinCAT router", error)
            })?;
        if returned != expected {
            return Err(AdsTransportError::new(format!(
                "local TwinCAT router returned {returned} ADS bytes; expected {expected}"
            )));
        }
        Ok(())
    }

    fn read_write_exact(
        &mut self,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<(), AdsTransportError> {
        let target = self.target()?;
        let expected = read_buffer.len();
        let returned = self
            .port()?
            .read_write(&target, index_group, index_offset, read_buffer, write_bytes)
            .map_err(|error| {
                map_native_call(
                    "read/write ADS data through the local TwinCAT router",
                    error,
                )
            })?;
        if returned != expected {
            return Err(AdsTransportError::new(format!(
                "local TwinCAT router returned {returned} ADS bytes; expected {expected}"
            )));
        }
        Ok(())
    }

    fn write(
        &mut self,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), AdsTransportError> {
        let target = self.target()?;
        self.port()?
            .write(&target, index_group, index_offset, bytes)
            .map_err(|error| {
                map_native_call("write ADS data through the local TwinCAT router", error)
            })
    }

    fn release_symbol_handle(&mut self, handle: u32) {
        let _ = self.write(ads::index::RELEASE_SYMHANDLE, 0, &handle.to_le_bytes());
    }

    fn release_symbol_handles(&mut self) {
        let handles = std::mem::take(&mut self.symbol_handles_by_point);
        for handle in handles.into_values() {
            self.release_symbol_handle(handle);
        }
    }

    fn request_symbol_handle(&mut self, symbol_name: &str) -> Result<u32, AdsTransportError> {
        let mut handle_bytes = [0_u8; 4];
        let symbol_name = native_symbol_name(symbol_name)?;
        self.read_write_exact(
            ads::index::GET_SYMHANDLE_BYNAME,
            0,
            &mut handle_bytes,
            &symbol_name,
        )?;
        Ok(u32::from_le_bytes(handle_bytes))
    }

    fn symbol_size(&mut self, symbol_name: &str) -> Result<usize, AdsTransportError> {
        let mut info = [0_u8; 12];
        let symbol_name = native_symbol_name(symbol_name)?;
        self.read_write_exact(ads::index::GET_SYMINFO_BYNAME, 0, &mut info, &symbol_name)?;
        Ok(u32::from_le_bytes(info[8..12].try_into().expect("fixed slice")) as usize)
    }

    fn next_subscription_id(&mut self) -> Result<u32, AdsTransportError> {
        let id = self.next_subscription_id;
        self.next_subscription_id = self.next_subscription_id.checked_add(1).ok_or_else(|| {
            AdsTransportError::new("native ADS polling subscription IDs exhausted")
        })?;
        Ok(id)
    }
}

impl Drop for WindowsAdsTransport {
    fn drop(&mut self) {
        self.release_symbol_handles();
    }
}

impl AdsTransport for WindowsAdsTransport {
    fn connect(&mut self) -> Result<(), AdsTransportError> {
        validate_route_policy(&self.route, "native Windows ADS")?;
        let _ = self.target()?;
        self.disconnect()?;

        let library = TcAdsDll::load_installed().map_err(map_native_connect)?;
        let mut port = library.open_port().map_err(map_native_connect)?;
        port.set_timeout(DEFAULT_TIMEOUT)
            .map_err(map_native_connect)?;
        let local = port.local_address().map_err(map_native_connect)?;
        if local.net_id.octets.iter().all(|octet| *octet == 0) {
            return Err(AdsTransportError::new(
                "the local TwinCAT router returned an empty AMS Net ID; start the TwinCAT router/runtime and retry",
            ));
        }
        self.local_address = Some(local);
        self.port = Some(Box::new(port));
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), AdsTransportError> {
        self.subscriptions.clear();
        self.release_symbol_handles();
        self.port = None;
        self.local_address = None;
        Ok(())
    }

    fn read_state(&mut self) -> Result<AdsDeviceState, AdsTransportError> {
        let target = self.target()?;
        let state = self
            .port()?
            .read_state(&target)
            .map_err(|error| map_native_call("read ADS runtime state", error))?;
        Ok(match state.ads_state {
            5 => AdsDeviceState::Run,
            1 | 6 | 15 => AdsDeviceState::Stop,
            11 | 19 => AdsDeviceState::Fault,
            _ => AdsDeviceState::Unknown,
        })
    }

    fn upload_symbol_table(&mut self) -> Result<Vec<SymbolDescriptor>, AdsTransportError> {
        let mut upload_info = [0_u8; SYMBOL_UPLOAD_INFO_BYTES];
        self.read_exact(ads::index::SYM_UPLOAD_INFO2, 0, &mut upload_info)?;
        let symbol_len = u32::from_le_bytes(
            upload_info[4..8]
                .try_into()
                .expect("fixed symbol length slice"),
        ) as usize;
        let type_len = u32::from_le_bytes(
            upload_info[12..16]
                .try_into()
                .expect("fixed type length slice"),
        ) as usize;
        if symbol_len > MAX_SYMBOL_UPLOAD_BYTES || type_len > MAX_SYMBOL_UPLOAD_BYTES {
            return Err(AdsTransportError::new(format!(
                "ADS symbol metadata is too large (symbols={symbol_len} bytes, types={type_len} bytes; limit={MAX_SYMBOL_UPLOAD_BYTES})"
            )));
        }

        let mut type_data = vec![0_u8; type_len];
        self.read_exact(ads::index::SYM_DT_UPLOAD, 0, &mut type_data)?;
        let mut symbol_data = vec![0_u8; symbol_len];
        self.read_exact(ads::index::SYM_UPLOAD, 0, &mut symbol_data)?;
        let (symbols, types) = ads::symbol::decode_symbol_info(symbol_data, type_data)
            .map_err(|error| AdsTransportError::new(error.to_string()))?;
        symbols
            .iter()
            .filter_map(|symbol| match symbol_descriptor_from_ads(symbol, &types) {
                Ok(Some(descriptor)) => Some(Ok(descriptor)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
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
                    let remote_size = self.symbol_size(symbol_name)?;
                    let expected = checked_byte_len(&request.data_type)?;
                    if remote_size != expected {
                        return Err(AdsTransportError::new(format!(
                            "ADS symbol '{symbol_name}' byte size mismatch: expected {expected}, got {remote_size}"
                        )));
                    }
                    let handle = self.request_symbol_handle(symbol_name)?;
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
        let _ = self.port()?;
        let now = now_ms();
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            let result = checked_byte_len(&handle.data_type).and_then(|length| {
                let mut buffer = vec![0_u8; length];
                let (index_group, index_offset) = read_write_address(handle);
                self.read_exact(index_group, index_offset, &mut buffer)?;
                value_from_ads_bytes(&handle.data_type, &buffer).map_err(map_mapping_error)
            });
            results.push(match result {
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
            });
        }
        Ok(results)
    }

    fn sumup_write(
        &mut self,
        writes: &[AdsWriteRequest],
    ) -> Result<Vec<PointQuality>, AdsTransportError> {
        let _ = self.port()?;
        let now = now_ms();
        let mut qualities = Vec::with_capacity(writes.len());
        for write in writes {
            let result = checked_byte_len(&write.handle.data_type).and_then(|_| {
                let bytes = ads_bytes_from_value(&write.handle.data_type, &write.value)
                    .map_err(map_mapping_error)?;
                let (index_group, index_offset) = read_write_address(&write.handle);
                self.write(index_group, index_offset, &bytes)
            });
            qualities.push(match result {
                Ok(()) => PointQuality::good(now),
                Err(error) => PointQuality::error(now, error.to_string()),
            });
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
        let _ = self.port()?;
        let _ = checked_byte_len(&request.handle.data_type)?;
        // Deliberately avoid native callbacks crossing the FFI boundary. The
        // ADS worker calls `drain_notifications` on each tick; this adapter
        // polls subscribed values and preserves cyclic/on-change semantics.
        let subscription_id = self.next_subscription_id()?;
        let point_name = request.handle.point_name.clone();
        self.subscriptions.insert(
            subscription_id,
            PollingSubscription {
                handle: request.handle,
                notification_mode: request.notification_mode,
                last_value: None,
                last_quality: None,
            },
        );
        Ok(AdsSubscription {
            point_name,
            subscription_id,
        })
    }

    fn drain_notifications(&mut self) -> Result<Vec<AdsNotificationSample>, AdsTransportError> {
        let subscriptions = self
            .subscriptions
            .iter()
            .map(|(id, subscription)| (*id, subscription.clone()))
            .collect::<Vec<_>>();
        let mut samples = Vec::new();
        for (subscription_id, subscription) in subscriptions {
            let mut reads = self.sumup_read(std::slice::from_ref(&subscription.handle))?;
            let read = reads
                .pop()
                .expect("one native polling read produces one result");
            let emit = should_emit_polled_notification(
                subscription.notification_mode,
                subscription.last_value.as_ref(),
                read.value.as_ref(),
                subscription.last_quality,
                read.quality.state,
            );
            if let Some(current) = self.subscriptions.get_mut(&subscription_id) {
                current.last_quality = Some(read.quality.state);
                if read.quality.state == QualityState::Good {
                    current.last_value.clone_from(&read.value);
                }
            }
            if emit {
                samples.push(AdsNotificationSample {
                    point_name: read.point_name,
                    subscription_id,
                    value: read.value,
                    quality: read.quality,
                });
            }
        }
        Ok(samples)
    }

    fn symbol_version(&mut self) -> Result<u32, AdsTransportError> {
        let mut version = [0_u8; 1];
        self.read_exact(ads::index::GET_SYMVERSION, 0, &mut version)?;
        Ok(u32::from(version[0]))
    }
}

fn should_emit_polled_notification(
    mode: AdsNotificationMode,
    previous: Option<&trust_runtime_core::value::Value>,
    current: Option<&trust_runtime_core::value::Value>,
    previous_quality: Option<QualityState>,
    quality: QualityState,
) -> bool {
    quality != QualityState::Good
        || mode == AdsNotificationMode::Cyclic
        || previous_quality != Some(quality)
        || previous != current
}

fn native_symbol_name(symbol_name: &str) -> Result<Vec<u8>, AdsTransportError> {
    if symbol_name.as_bytes().contains(&0) {
        return Err(AdsTransportError::new(
            "ADS symbol names cannot contain a NUL byte",
        ));
    }
    let mut bytes = Vec::with_capacity(symbol_name.len().saturating_add(1));
    bytes.extend_from_slice(symbol_name.as_bytes());
    bytes.push(0);
    Ok(bytes)
}

fn map_native_connect(error: NativeAdsError) -> AdsTransportError {
    with_native_error_details(AdsTransportError::new(format!(
        "same-computer ADS requires the installed TwinCAT native router, but it could not be opened: {error}. Start the TwinCAT router/runtime and verify TcAdsDll.dll is installed. truST will not create a self-route or fall back to raw ADS/TCP for this local target"
    )), &error)
}

fn map_native_call(operation: &str, error: NativeAdsError) -> AdsTransportError {
    with_native_error_details(
        AdsTransportError::new(format!("{operation}: {error}")),
        &error,
    )
}

fn with_native_error_details(
    mut mapped: AdsTransportError,
    error: &NativeAdsError,
) -> AdsTransportError {
    if let NativeAdsError::Call {
        code, description, ..
    } = error
    {
        if let Ok(code) = u32::try_from(*code) {
            mapped = mapped.with_ads_error(code, *description);
            if matches!(code, 0x015 | 0x50C | 0x745) {
                mapped = mapped.with_failure_kind(AdsTransportFailureKind::TimedOut);
            } else if code == 0x01B {
                mapped = mapped.with_failure_kind(AdsTransportFailureKind::HostUnreachable);
            }
        }
    }
    mapped
}

#[cfg(test)]
#[path = "backend_windows/tests.rs"]
mod tests;
