use std::sync::{Arc, Mutex};

use super::*;
use trust_ads_core::{AdsDataTypeDescriptor, IecDataType};
use trust_runtime_core::value::Value;

#[test]
fn native_0x507_code_and_name_survive_transport_mapping() {
    let error = map_native_call(
        "read ADS state",
        NativeAdsError::Call {
            operation: "AdsSyncReadStateReqEx",
            code: 0x507,
            description: "Router: port not registered",
        },
    );

    let ads_error = error.ads_error().expect("structured ADS error");
    assert_eq!(ads_error.code, 0x507);
    assert_eq!(ads_error.name, "Router: port not registered");
    assert_eq!(error.failure_kind(), None);
}

#[test]
fn native_timeout_code_has_structured_timeout_semantics() {
    let error = map_native_call(
        "read ADS state",
        NativeAdsError::Call {
            operation: "AdsSyncReadStateReqEx",
            code: 0x745,
            description: "ADS client synchronous timeout",
        },
    );

    assert_eq!(error.ads_error().map(|error| error.code), Some(0x745));
    assert_eq!(
        error.failure_kind(),
        Some(AdsTransportFailureKind::TimedOut)
    );
}

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
            .expect("fake native state lock")
            .targets
            .push(*target);
    }
}

impl NativeAdsPort for FakeNativePort {
    fn read_state(
        &mut self,
        target: &AmsAddress,
    ) -> Result<trust_ads_windows::AdsDeviceState, NativeAdsError> {
        self.record_target(target);
        Ok(trust_ads_windows::AdsDeviceState {
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
    ) -> Result<usize, NativeAdsError> {
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
            other => panic!("unexpected native test read index group {other:#x}"),
        }
        Ok(buffer.len())
    }

    fn write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        index_offset: u32,
        bytes: &[u8],
    ) -> Result<(), NativeAdsError> {
        self.record_target(target);
        self.state
            .lock()
            .expect("fake native state lock")
            .writes
            .push((index_group, index_offset, bytes.to_vec()));
        Ok(())
    }

    fn read_write(
        &mut self,
        target: &AmsAddress,
        index_group: u32,
        _index_offset: u32,
        read_buffer: &mut [u8],
        write_bytes: &[u8],
    ) -> Result<usize, NativeAdsError> {
        self.record_target(target);
        self.state
            .lock()
            .expect("fake native state lock")
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
            other => panic!("unexpected native test read/write index group {other:#x}"),
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
    assert_eq!(bytes.len(), entry_size);
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

    let state = state.lock().expect("fake native state lock");
    assert_eq!(state.targets.len(), 1);
    assert_eq!(state.targets[0].net_id.octets, [10, 20, 30, 40, 1, 1]);
    assert_eq!(state.targets[0].port, 301);
}

#[test]
fn adapter_preserves_native_router_source_net_id_and_client_port() {
    let state = Arc::new(Mutex::new(FakeNativeState::default()));
    let mut transport =
        WindowsAdsTransport::with_test_port(native_route(851), FakeNativePort::new(state));
    transport.local_address = Some(AmsAddress::new(
        AmsNetId::new([10, 20, 30, 40, 1, 1]),
        32905,
    ));

    assert_eq!(
        transport.local_address(),
        Some(AmsAddress::new(
            AmsNetId::new([10, 20, 30, 40, 1, 1]),
            32905,
        ))
    );
}

#[test]
fn adapter_uploads_and_maps_native_symbol_metadata() {
    let state = Arc::new(Mutex::new(FakeNativeState::default()));
    let mut transport =
        WindowsAdsTransport::with_test_port(native_route(851), FakeNativePort::new(state));

    let symbols = transport
        .upload_symbol_table()
        .expect("native symbol upload");

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "MAIN.value");
    assert_eq!(symbols[0].byte_size, 4);
    assert_eq!(symbols[0].data_type.iec_type, IecDataType::Dint);
}

#[test]
fn adapter_resolves_handles_and_reads_and_writes_values() {
    let state = Arc::new(Mutex::new(FakeNativeState::default()));
    let mut transport = WindowsAdsTransport::with_test_port(
        native_route(501),
        FakeNativePort::new(Arc::clone(&state)),
    );
    let request = AdsHandleRequest {
        point_name: "value".to_string(),
        address: AdsPointAddress::Symbol("MAIN.value".to_string()),
        data_type: AdsDataTypeDescriptor::scalar("DINT", IecDataType::Dint),
    };

    let handle = transport
        .resolve_handles(&[request])
        .expect("resolve native handle")
        .pop()
        .expect("one handle");
    assert_eq!(handle.handle, 42);
    let reads = transport
        .sumup_read(std::slice::from_ref(&handle))
        .expect("read native value");
    assert_eq!(reads[0].value, Some(Value::DInt(123)));
    let qualities = transport
        .sumup_write(&[AdsWriteRequest {
            handle,
            value: Value::DInt(456),
        }])
        .expect("write native value");
    assert_eq!(qualities[0].state, QualityState::Good);

    let state = state.lock().expect("fake native state lock");
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

#[test]
fn on_change_polling_emits_initial_value_and_real_changes_only() {
    let first = Value::DInt(1);
    let changed = Value::DInt(2);
    assert!(should_emit_polled_notification(
        AdsNotificationMode::OnChange,
        None,
        Some(&first),
        None,
        QualityState::Good
    ));
    assert!(!should_emit_polled_notification(
        AdsNotificationMode::OnChange,
        Some(&first),
        Some(&first),
        Some(QualityState::Good),
        QualityState::Good
    ));
    assert!(should_emit_polled_notification(
        AdsNotificationMode::OnChange,
        Some(&first),
        Some(&changed),
        Some(QualityState::Good),
        QualityState::Good
    ));
}

#[test]
fn cyclic_polling_and_errors_are_never_suppressed() {
    let value = Value::DInt(1);
    assert!(should_emit_polled_notification(
        AdsNotificationMode::Cyclic,
        Some(&value),
        Some(&value),
        Some(QualityState::Good),
        QualityState::Good
    ));
    assert!(should_emit_polled_notification(
        AdsNotificationMode::OnChange,
        Some(&value),
        None,
        Some(QualityState::Good),
        QualityState::Error
    ));
    assert!(should_emit_polled_notification(
        AdsNotificationMode::OnChange,
        Some(&value),
        Some(&value),
        Some(QualityState::Error),
        QualityState::Good
    ));
}

#[test]
fn native_symbol_queries_are_nul_terminated() {
    assert_eq!(
        native_symbol_name("MAIN.value"),
        Ok(b"MAIN.value\0".to_vec())
    );
    assert!(native_symbol_name("MAIN.\0value").is_err());
}

#[cfg(not(windows))]
#[test]
fn native_connect_error_is_actionable_and_never_mentions_a_self_route_fix() {
    let mut route = AdsRoute::new(
        "local",
        trust_ads_core::AmsNetId::new("10.20.30.40.1.1"),
        "127.0.0.1",
        851,
    );
    route.security.transport = trust_ads_core::TransportSecurity::Plain;
    let error = WindowsAdsTransport::new(route)
        .connect()
        .expect_err("native DLL is Windows-only in this test");
    assert!(error.message().contains("installed TwinCAT native router"));
    assert!(error.message().contains("will not create a self-route"));
}
