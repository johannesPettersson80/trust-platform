use std::collections::BTreeMap;
use std::time::Duration;

use trust_ads_core::{
    ads_bytes_from_value, value_from_ads_bytes, AdsRoute, PointQuality, QualityState,
    SymbolDescriptor, TransportSecurity, UpdateMode,
};
use trust_tcads_native::{
    AdsDeviceState as NativeDeviceState, AdsPort, AmsAddress, AmsNetId, Error as NativeError,
    TcAdsDll,
};

use super::backend_ads_rs::{
    checked_byte_len, map_mapping_error, now_ms, read_write_address, symbol_descriptor_from_ads,
    validate_requested_size,
};
use super::transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationMode, AdsNotificationSample, AdsPointAddress,
    AdsReadResult, AdsResolvedHandle, AdsSubscribeRequest, AdsSubscription, AdsTransport,
    AdsTransportError, AdsWriteRequest,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(4);
const SYMBOL_UPLOAD_INFO_BYTES: usize = 64;
const MAX_SYMBOL_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

trait NativeAdsPort: Send {
    fn read_state(&mut self, target: &AmsAddress) -> Result<NativeDeviceState, NativeError>;
    fn read(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, NativeError>;
    fn write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), NativeError>;
    fn read_write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, NativeError>;
}

impl NativeAdsPort for AdsPort {
    fn read_state(&mut self, target: &AmsAddress) -> Result<NativeDeviceState, NativeError> {
        self.read_state(target)
    }

    fn read(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, NativeError> {
        self.read(target, index_group, index_offset, buffer)
    }

    fn write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), NativeError> {
        self.write(target, index_group, index_offset, bytes)
    }

    fn read_write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, NativeError> {
        self.read_write(target, index_group, index_offset, read_buffer, write_bytes)
    }
}

pub(super) struct WindowsAdsTransport {
    route: AdsRoute,
    port: Option<Box<dyn NativeAdsPort>>,
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
            .map_err(|error| map_native_call("read ADS data through TwinCAT", error))?;
        if returned != expected {
            return Err(AdsTransportError::new(format!(
                "TwinCAT returned {returned} ADS bytes; expected {expected}"
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
            .map_err(|error| map_native_call("read/write ADS data through TwinCAT", error))?;
        if returned != expected {
            return Err(AdsTransportError::new(format!(
                "TwinCAT returned {returned} ADS bytes; expected {expected}"
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
            .map_err(|error| map_native_call("write ADS data through TwinCAT", error))
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
        if !matches!(self.route.security.transport, TransportSecurity::Plain) {
            return Err(AdsTransportError::new(
                "Secure ADS is reserved but not implemented by the native Windows backend",
            ));
        }
        if self.route.security.auto_add_route {
            return Err(AdsTransportError::new(
                "auto_add_route=true is reserved for authoring tools; runtime will not write AMS routes",
            ));
        }
        let _ = self.target()?;
        self.disconnect()?;
        let library = TcAdsDll::load_installed().map_err(map_native_connect)?;
        let mut port = library.open_port().map_err(map_native_connect)?;
        port.set_timeout(DEFAULT_TIMEOUT)
            .map_err(map_native_connect)?;
        let local = port.local_address().map_err(map_native_connect)?;
        if local.net_id.octets.iter().all(|octet| *octet == 0) {
            return Err(AdsTransportError::new(
                "the local TwinCAT router returned an empty AMS Net ID",
            ));
        }
        self.port = Some(Box::new(port));
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), AdsTransportError> {
        self.subscriptions.clear();
        self.release_symbol_handles();
        self.port = None;
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
                "ADS symbol metadata is too large (symbols={symbol_len}, types={type_len})"
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
                .expect("one native polling read returns one result");
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

fn map_native_connect(error: NativeError) -> AdsTransportError {
    AdsTransportError::new(format!(
        "same-computer ADS requires the installed TwinCAT native router: {error}"
    ))
}

fn map_native_call(operation: &str, error: NativeError) -> AdsTransportError {
    AdsTransportError::new(format!("{operation}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use trust_ads_core::{AdsDataTypeDescriptor, IecDataType};
    use trust_runtime_core::value::Value;

    use super::*;

    #[derive(Debug, Default)]
    struct FakeNativeState {
        targets: Vec<AmsAddress>,
        read_write_names: Vec<Vec<u8>>,
        writes: Vec<(u32, u32, Vec<u8>)>,
    }

    struct FakeNativePort {
        state: Arc<Mutex<FakeNativeState>>,
        symbol_upload: Vec<u8>,
    }

    impl FakeNativePort {
        fn new(state: Arc<Mutex<FakeNativeState>>) -> Self {
            Self {
                state,
                symbol_upload: dint_symbol_upload("MAIN.value"),
            }
        }

        fn record_target(&self, target: &AmsAddress) {
            self.state
                .lock()
                .expect("fake native state")
                .targets
                .push(*target);
        }
    }

    impl NativeAdsPort for FakeNativePort {
        fn read_state(&mut self, target: &AmsAddress) -> Result<NativeDeviceState, NativeError> {
            self.record_target(target);
            Ok(NativeDeviceState {
                ads_state: 5,
                device_state: 0,
            })
        }

        fn read(
            &mut self,
            target: &AmsAddress,
            index_group: u32,
            _index_offset: u32,
            buffer: &mut [u8],
        ) -> Result<usize, NativeError> {
            self.record_target(target);
            match index_group {
                ads::index::SYM_UPLOAD_INFO2 => {
                    buffer.fill(0);
                    buffer[4..8].copy_from_slice(&(self.symbol_upload.len() as u32).to_le_bytes());
                }
                ads::index::SYM_DT_UPLOAD => assert!(buffer.is_empty()),
                ads::index::SYM_UPLOAD => buffer.copy_from_slice(&self.symbol_upload),
                ads::index::RW_SYMVAL_BYHANDLE => {
                    buffer.copy_from_slice(&123_i32.to_le_bytes());
                }
                ads::index::GET_SYMVERSION => buffer.copy_from_slice(&[7]),
                other => panic!("unexpected native read group {other:#x}"),
            }
            Ok(buffer.len())
        }

        fn write(
            &mut self,
            target: &AmsAddress,
            index_group: u32,
            index_offset: u32,
            bytes: &[u8],
        ) -> Result<(), NativeError> {
            self.record_target(target);
            self.state.lock().expect("fake native state").writes.push((
                index_group,
                index_offset,
                bytes.to_vec(),
            ));
            Ok(())
        }

        fn read_write(
            &mut self,
            target: &AmsAddress,
            index_group: u32,
            _index_offset: u32,
            read_buffer: &mut [u8],
            write_bytes: &[u8],
        ) -> Result<usize, NativeError> {
            self.record_target(target);
            self.state
                .lock()
                .expect("fake native state")
                .read_write_names
                .push(write_bytes.to_vec());
            match index_group {
                ads::index::GET_SYMINFO_BYNAME => {
                    read_buffer.fill(0);
                    read_buffer[8..12].copy_from_slice(&4_u32.to_le_bytes());
                }
                ads::index::GET_SYMHANDLE_BYNAME => {
                    read_buffer.copy_from_slice(&42_u32.to_le_bytes());
                }
                other => panic!("unexpected native read/write group {other:#x}"),
            }
            Ok(read_buffer.len())
        }
    }

    fn dint_symbol_upload(name: &str) -> Vec<u8> {
        let typ = "DINT";
        let entry_size = 4 + 26 + name.len() + 1 + typ.len() + 1 + 1;
        let mut bytes = Vec::with_capacity(entry_size);
        bytes.extend_from_slice(&(entry_size as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4020_u32.to_le_bytes());
        bytes.extend_from_slice(&12_u32.to_le_bytes());
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(typ.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(typ.as_bytes());
        bytes.push(0);
        bytes.push(0);
        bytes
    }

    fn native_route(port: u16) -> AdsRoute {
        AdsRoute::new(
            "local",
            trust_ads_core::AmsNetId::new("10.20.30.40.1.1"),
            "127.0.0.1",
            port,
        )
    }

    #[test]
    fn adapter_sends_exact_target_net_id_and_logical_port() {
        let state = Arc::new(Mutex::new(FakeNativeState::default()));
        let mut transport = WindowsAdsTransport::with_test_port(
            native_route(301),
            FakeNativePort::new(Arc::clone(&state)),
        );
        assert_eq!(transport.read_state(), Ok(AdsDeviceState::Run));
        let state = state.lock().expect("fake native state");
        assert_eq!(state.targets[0].net_id.octets, [10, 20, 30, 40, 1, 1]);
        assert_eq!(state.targets[0].port, 301);
    }

    #[test]
    fn adapter_uploads_symbols_and_supports_live_reads_and_writes() {
        let state = Arc::new(Mutex::new(FakeNativeState::default()));
        let mut transport = WindowsAdsTransport::with_test_port(
            native_route(851),
            FakeNativePort::new(Arc::clone(&state)),
        );
        let symbols = transport.upload_symbol_table().expect("native symbols");
        assert_eq!(symbols[0].name, "MAIN.value");
        assert_eq!(symbols[0].data_type.iec_type, IecDataType::Dint);
        let handle = transport
            .resolve_handles(&[AdsHandleRequest {
                point_name: "value".to_string(),
                address: AdsPointAddress::Symbol("MAIN.value".to_string()),
                data_type: AdsDataTypeDescriptor::scalar("DINT", IecDataType::Dint),
            }])
            .expect("native handle")
            .pop()
            .expect("one handle");
        assert_eq!(
            transport
                .sumup_read(std::slice::from_ref(&handle))
                .expect("native read")[0]
                .value,
            Some(Value::DInt(123))
        );
        assert_eq!(
            transport
                .sumup_write(&[AdsWriteRequest {
                    handle,
                    value: Value::DInt(456),
                }])
                .expect("native write")[0]
                .state,
            QualityState::Good
        );
        let state = state.lock().expect("fake native state");
        assert!(state
            .read_write_names
            .iter()
            .all(|name| name.last() == Some(&0)));
        assert!(state.writes.iter().any(|(group, offset, bytes)| {
            *group == ads::index::RW_SYMVAL_BYHANDLE
                && *offset == 42
                && bytes.as_slice() == 456_i32.to_le_bytes()
        }));
    }
}
