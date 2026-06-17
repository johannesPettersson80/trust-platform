//! Command dispatcher wire tests.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use trust_ads_core::{
    AdsDataTypeDescriptor, AmsNetId, IecDataType, PointQuality, SymbolDescriptor, SymbolFlag,
    SymbolSnapshot,
};

use super::{
    build_device_notification_payload, AdsDeviceState, CommandContext, CommandDispatcher,
    DeviceInfo, NotificationReceiver, NotificationSample, NotificationStamp, NotificationTarget,
    ADSIGRP_SUMUP_READ, ADSIGRP_SUMUP_READWRITE, ADSIGRP_SUMUP_READ_EX, ADSIGRP_SUMUP_WRITE,
    ADSIGRP_SYM_HNDBYNAME, ADSIGRP_SYM_INFOBYNAME, ADSIGRP_SYM_INFOBYNAMEEX,
    ADSIGRP_SYM_RELEASEHND, ADSIGRP_SYM_UPLOADINFO2, ADSIGRP_SYM_VALBYHND, ADSIGRP_SYM_VALBYNAME,
    ADSIGRP_SYM_VERSION, ADSTRANS_SERVERCYCLE, ADSTRANS_SERVERONCHA,
};
use crate::system_symbols::{
    ADSIGRP_DEVICE_DATA, ADSIOFFS_DEVICE_DATA_TIME_BASE, ONLINE_CHANGE_COUNT_NAME, TASK_COUNT_NAME,
    TWINCAT_DEVICE_TIME_BASE_100NS,
};
use crate::{
    AdsErrorCode, AdsServerAuditEvent, AdsServerError, AuditSink, ClientId, ClientPolicy, Clock,
    CommandId, RuntimeWritePort, SymbolSource, ValueIo,
};

struct FakeHost {
    snapshot: SymbolSnapshot,
    writes: RefCell<Vec<(String, Vec<u8>)>>,
    audits: RefCell<Vec<AdsServerAuditEvent>>,
    allow: bool,
}

impl FakeHost {
    fn new() -> Self {
        let flags = BTreeSet::from([SymbolFlag::Read, SymbolFlag::Write]);
        let readonly = BTreeSet::from([SymbolFlag::Read]);
        let mut setpoint = SymbolDescriptor::new(
            "GVL.Setpoint",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        );
        setpoint.flags = flags;
        let mut ready = SymbolDescriptor::new(
            "GVL.Ready",
            AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
            0x4020,
            4,
            1,
        );
        ready.flags = readonly;
        Self {
            snapshot: SymbolSnapshot::new("test", vec![setpoint, ready]),
            writes: RefCell::new(Vec::new()),
            audits: RefCell::new(Vec::new()),
            allow: true,
        }
    }

    fn context<'a>(&'a self, client: &'a ClientId) -> CommandContext<'a> {
        CommandContext {
            client,
            symbols: self,
            values: self,
            writes: self,
            policy: self,
            audit: self,
            clock: self,
            device_info: DeviceInfo {
                major_version: 1,
                minor_version: 2,
                build: 3,
                name: "trust-test".to_string(),
            },
            device_state: AdsDeviceState::default(),
            ads_port: 851,
            notification_receiver: Some(NotificationReceiver {
                net_id: [1, 2, 3, 4, 5, 6],
                port: 0x8001,
            }),
        }
    }
}

fn test_client() -> ClientId {
    ClientId::new(AmsNetId::new("1.2.3.4.5.6")).with_source_ip("127.0.0.1")
}

impl SymbolSource for FakeHost {
    fn snapshot(&self) -> Arc<SymbolSnapshot> {
        Arc::new(self.snapshot.clone())
    }

    fn version(&self) -> u32 {
        9
    }
}

impl ValueIo for FakeHost {
    fn read(&self, symbol: &SymbolDescriptor) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
        let bytes = match symbol.name.as_str() {
            "GVL.Setpoint" => 1.25_f32.to_le_bytes().to_vec(),
            "GVL.Ready" => vec![1],
            _ => {
                return Err(AdsServerError::device(
                    AdsErrorCode::SymbolNotFound,
                    "missing",
                ))
            }
        };
        Ok((bytes, PointQuality::good(10)))
    }
}

impl RuntimeWritePort for FakeHost {
    fn write(
        &self,
        symbol: &SymbolDescriptor,
        bytes: &[u8],
        _client: &ClientId,
    ) -> Result<(), AdsServerError> {
        self.writes
            .borrow_mut()
            .push((symbol.name.clone(), bytes.to_vec()));
        Ok(())
    }
}

impl ClientPolicy for FakeHost {
    fn permits(&self, _client: &ClientId) -> bool {
        self.allow
    }
}

impl AuditSink for FakeHost {
    fn record(&self, event: &AdsServerAuditEvent) {
        self.audits.borrow_mut().push(event.clone());
    }
}

impl Clock for FakeHost {
    fn now_ms(&self) -> u64 {
        123
    }
}

#[test]
fn read_device_info_response_is_wire_shaped() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();

    let response = dispatcher.dispatch(CommandId::ReadDeviceInfo, &[], &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(response[4], 1);
    assert_eq!(response[5], 2);
    assert_eq!(&response[6..8], &3_u16.to_le_bytes());
    assert_eq!(response.len(), 24);
}

#[test]
fn direct_read_returns_value_bytes() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(&response[8..12], &1.25_f32.to_le_bytes());
}

#[test]
fn direct_read_honors_smaller_requested_length() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &2_u32.to_le_bytes());
    assert_eq!(&response[8..10], &1.25_f32.to_le_bytes()[..2]);
    assert_eq!(response.len(), 10);
}

#[test]
fn direct_value_read_pads_to_requested_length() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(&response[8..12], &[1, 0, 0, 0]);
    assert_eq!(response.len(), 12);
}

#[test]
fn handle_by_name_then_read_by_handle() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&13_u32.to_le_bytes());
    request.extend_from_slice(b"GVL.Setpoint\0");

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);
    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let handle = u32::from_le_bytes(response[8..12].try_into().expect("handle bytes"));
    assert_eq!(dispatcher.handle_count(), 1);

    let mut read = Vec::new();
    read.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    read.extend_from_slice(&handle.to_le_bytes());
    read.extend_from_slice(&4_u32.to_le_bytes());
    let response = dispatcher.dispatch(CommandId::Read, &read, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[8..12], &1.25_f32.to_le_bytes());
}

#[test]
fn release_handle_accepts_payload_handle_form() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&13_u32.to_le_bytes());
    request.extend_from_slice(b"GVL.Setpoint\0");
    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);
    let handle = u32::from_le_bytes(response[8..12].try_into().expect("handle bytes"));

    let mut release = Vec::new();
    release.extend_from_slice(&ADSIGRP_SYM_RELEASEHND.to_le_bytes());
    release.extend_from_slice(&0_u32.to_le_bytes());
    release.extend_from_slice(&4_u32.to_le_bytes());
    release.extend_from_slice(&handle.to_le_bytes());
    let response = dispatcher.dispatch(CommandId::Write, &release, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.handle_count(), 0);
}

#[test]
fn release_handle_accepts_twincat_index_offset_form() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&13_u32.to_le_bytes());
    request.extend_from_slice(b"GVL.Setpoint\0");
    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);
    let handle = u32::from_le_bytes(response[8..12].try_into().expect("handle bytes"));

    let mut release = Vec::new();
    release.extend_from_slice(&ADSIGRP_SYM_RELEASEHND.to_le_bytes());
    release.extend_from_slice(&handle.to_le_bytes());
    release.extend_from_slice(&0_u32.to_le_bytes());
    let response = dispatcher.dispatch(CommandId::Write, &release, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.handle_count(), 0);
}

#[test]
fn handle_by_name_accepts_symbol_name_without_nul_terminator() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = b"GVL.Setpoint";
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(name);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(dispatcher.handle_count(), 1);
}

#[test]
fn symbol_info_by_name_returns_compact_index_and_size_tuple_for_ads_rs() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_INFOBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&12_u32.to_le_bytes());
    request.extend_from_slice(&13_u32.to_le_bytes());
    request.extend_from_slice(b"GVL.Setpoint\0");

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &12_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0x4020_u32.to_le_bytes());
    assert_eq!(&response[12..16], &0_u32.to_le_bytes());
    assert_eq!(&response[16..20], &4_u32.to_le_bytes());
}

#[test]
fn symbol_info_by_name_ex_returns_ads_symbol_entry_for_pyads_cache() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_INFOBYNAMEEX.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&256_u32.to_le_bytes());
    request.extend_from_slice(&13_u32.to_le_bytes());
    request.extend_from_slice(b"GVL.Setpoint\0");

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let length = u32::from_le_bytes(response[4..8].try_into().expect("length"));
    assert!(length > 30);
    assert_eq!(&response[12..16], &0x4020_u32.to_le_bytes());
    assert_eq!(&response[16..20], &0_u32.to_le_bytes());
    assert_eq!(&response[20..24], &4_u32.to_le_bytes());
    let name_len = u16::from_le_bytes(response[32..34].try_into().expect("name len"));
    let type_len = u16::from_le_bytes(response[34..36].try_into().expect("type len"));
    assert_eq!(name_len, 12);
    assert_eq!(type_len, 4);
    assert!(response[38..]
        .windows(b"GVL.Setpoint\0".len())
        .any(|window| window == b"GVL.Setpoint\0"));
}

#[test]
fn symbol_info_by_name_ex_accepts_symbol_name_without_nul_terminator() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = b"GVL.Setpoint";
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_INFOBYNAMEEX.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&256_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(name);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let length = u32::from_le_bytes(response[4..8].try_into().expect("length"));
    assert!(length > 30);
    assert_eq!(&response[12..16], &0x4020_u32.to_le_bytes());
}

#[test]
fn write_enqueues_and_audits_writable_symbol() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&2.5_f32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Write, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(host.writes.borrow().len(), 1);
    assert_eq!(host.audits.borrow().len(), 1);
}

#[test]
fn write_rejects_readonly_symbol_without_runtime_mutation() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let request = vec![
        0x20, 0x40, 0, 0, // group
        4, 0, 0, 0, // offset
        1, 0, 0, 0, // len
        0, // data
    ];

    let response = dispatcher.dispatch(CommandId::Write, &request, &ctx);

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidAccess.value()
    );
    assert!(host.writes.borrow().is_empty());
    assert_eq!(host.audits.borrow().len(), 1);
}

#[test]
fn symbol_version_read_returns_u32_version() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VERSION.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[8..12], &9_u32.to_le_bytes());
}

#[test]
fn device_data_time_base_read_matches_twincat_probe() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_DEVICE_DATA.to_le_bytes());
    request.extend_from_slice(&ADSIOFFS_DEVICE_DATA_TIME_BASE.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(
        &response[8..12],
        &TWINCAT_DEVICE_TIME_BASE_100NS.to_le_bytes()
    );
}

#[test]
fn symbol_value_by_name_returns_task_count_without_nul_terminator() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(TASK_COUNT_NAME.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(TASK_COUNT_NAME.as_bytes());

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(&response[8..12], &1_u32.to_le_bytes());
}

#[test]
fn symbol_value_by_name_pads_value_to_requested_length() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = b"GVL.Ready";
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(name);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(&response[8..12], &[1, 0, 0, 0]);
}

#[test]
fn task_info_ads_port_field_reports_runtime_port() {
    let host = FakeHost::new();
    let client = test_client();
    let mut ctx = host.context(&client);
    ctx.ads_port = 852;
    let mut dispatcher = CommandDispatcher::new();
    let name = b"TwinCAT_SystemInfoVarList._TaskInfo[1].AdsPort";
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(name);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &2_u32.to_le_bytes());
    assert_eq!(&response[8..10], &852_u16.to_le_bytes());
}

#[test]
fn task_info_block_by_name_returns_full_struct_bytes() {
    let host = FakeHost::new();
    let client = test_client();
    let mut ctx = host.context(&client);
    ctx.ads_port = 852;
    let mut dispatcher = CommandDispatcher::new();
    let name = b"TwinCAT_SystemInfoVarList._TaskInfo[1]";
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&128_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(name);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &128_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0x0850_2001_u32.to_le_bytes());
    assert_eq!(
        &response[12..16],
        &TWINCAT_DEVICE_TIME_BASE_100NS.to_le_bytes()
    );
    assert_eq!(&response[16..18], &20_u16.to_le_bytes());
    assert_eq!(&response[18..20], &852_u16.to_le_bytes());
}

#[test]
fn raw_task_info_base_address_reads_obj_id_prefix_for_short_read() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4040_u32.to_le_bytes());
    request.extend_from_slice(&0x0005_E9D0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0x0850_2001_u32.to_le_bytes());
    assert_eq!(response.len(), 12);
}

#[test]
fn raw_task_info_base_address_reads_full_task_info_block() {
    let host = FakeHost::new();
    let client = test_client();
    let mut ctx = host.context(&client);
    ctx.ads_port = 852;
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4040_u32.to_le_bytes());
    request.extend_from_slice(&0x0005_E9D0_u32.to_le_bytes());
    request.extend_from_slice(&128_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &128_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0x0850_2001_u32.to_le_bytes());
    assert_eq!(
        &response[12..16],
        &TWINCAT_DEVICE_TIME_BASE_100NS.to_le_bytes()
    );
    assert_eq!(&response[16..18], &20_u16.to_le_bytes());
    assert_eq!(&response[18..20], &852_u16.to_le_bytes());
}

#[test]
fn online_change_count_handle_by_name_reads_symbol_version() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = format!("{ONLINE_CHANGE_COUNT_NAME}\0");
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(name.as_bytes());

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let handle = u32::from_le_bytes(response[8..12].try_into().expect("handle"));

    let mut read = Vec::new();
    read.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    read.extend_from_slice(&handle.to_le_bytes());
    read.extend_from_slice(&4_u32.to_le_bytes());
    let response = dispatcher.dispatch(CommandId::Read, &read, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &4_u32.to_le_bytes());
    assert_eq!(&response[8..12], &9_u32.to_le_bytes());
}

#[test]
fn online_change_count_info_by_name_ex_returns_hidden_system_symbol() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = format!("{ONLINE_CHANGE_COUNT_NAME}\0");
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_INFOBYNAMEEX.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&256_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(name.as_bytes());

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert!(response[8..]
        .windows(ONLINE_CHANGE_COUNT_NAME.len())
        .any(|window| window == ONLINE_CHANGE_COUNT_NAME.as_bytes()));
    assert!(response[8..]
        .windows(b"UDINT\0".len())
        .any(|window| window == b"UDINT\0"));
}

#[test]
fn upload_info2_reports_symbol_and_datatype_table_sizes() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_UPLOADINFO2.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&24_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &24_u32.to_le_bytes());
    assert_eq!(&response[8..12], &2_u32.to_le_bytes());
    assert!(u32::from_le_bytes(response[12..16].try_into().expect("sym size")) > 0);
    assert_eq!(&response[16..20], &2_u32.to_le_bytes());
    assert!(u32::from_le_bytes(response[20..24].try_into().expect("dt size")) > 0);
    assert_eq!(&response[24..28], &0_u32.to_le_bytes());
    assert_eq!(&response[28..32], &0_u32.to_le_bytes());
}

#[test]
fn upload_info2_pads_to_requested_length_for_ads_rs() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_UPLOADINFO2.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&64_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Read, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &64_u32.to_le_bytes());
    assert_eq!(&response[8..12], &2_u32.to_le_bytes());
    assert!(u32::from_le_bytes(response[12..16].try_into().expect("sym size")) > 0);
    assert_eq!(&response[16..20], &2_u32.to_le_bytes());
    assert!(u32::from_le_bytes(response[20..24].try_into().expect("dt size")) > 0);
    assert!(response[32..72].iter().all(|byte| *byte == 0));
}

#[test]
fn handle_by_name_enforces_handle_limit() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_limits(usize::MAX, usize::MAX, usize::MAX, 1);
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&13_u32.to_le_bytes());
    request.extend_from_slice(b"GVL.Setpoint\0");

    let first = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);
    let second = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&first[0..4], &0_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(second[0..4].try_into().expect("second result")),
        AdsErrorCode::NoMemory.value()
    );
}

#[test]
fn handle_by_name_wraps_without_aliasing_live_handles() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&13_u32.to_le_bytes());
    request.extend_from_slice(b"GVL.Setpoint\0");

    let first = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);
    dispatcher.set_next_handle_for_test(u32::MAX);
    let max = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);
    let wrapped = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&first[8..12], &1_u32.to_le_bytes());
    assert_eq!(&max[8..12], &u32::MAX.to_le_bytes());
    assert_eq!(&wrapped[8..12], &2_u32.to_le_bytes());
    assert_eq!(dispatcher.handle_count(), 3);
}

#[test]
fn sumup_read_enforces_item_limit() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_limits(usize::MAX, 1, usize::MAX, usize::MAX);
    let mut sub = Vec::new();
    for _ in 0..2 {
        sub.extend_from_slice(&0x4020_u32.to_le_bytes());
        sub.extend_from_slice(&0_u32.to_le_bytes());
        sub.extend_from_slice(&4_u32.to_le_bytes());
    }
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READ.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&16_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidSize.value()
    );
}

#[test]
fn direct_write_enforces_write_byte_limit() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_limits(usize::MAX, usize::MAX, 2, usize::MAX);
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&2.5_f32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Write, &request, &ctx);

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidSize.value()
    );
    assert!(host.writes.borrow().is_empty());
}

#[test]
fn sumup_read_keeps_good_items_when_one_fails() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0x9999_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READ.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&16_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(response[12..16].try_into().expect("second result")),
        AdsErrorCode::InvalidGroup.value()
    );
    assert_eq!(&response[16..20], &1.25_f32.to_le_bytes());
}

#[test]
fn sumup_read_ex_returns_result_lengths_for_ads_rs_read_multi() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0x9999_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READ_EX.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&20_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &20_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(&response[12..16], &4_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(response[16..20].try_into().expect("second result")),
        AdsErrorCode::InvalidGroup.value()
    );
    assert_eq!(&response[20..24], &0_u32.to_le_bytes());
    assert_eq!(&response[24..28], &1.25_f32.to_le_bytes());
}

#[test]
fn sumup_write_returns_per_item_results() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&1_u32.to_le_bytes());
    sub.extend_from_slice(&1.0_f32.to_le_bytes());
    sub.push(0);
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_WRITE.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&8_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up write payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(response[12..16].try_into().expect("second result")),
        AdsErrorCode::InvalidAccess.value()
    );
    assert_eq!(host.writes.borrow().len(), 1);
}

#[test]
fn sumup_readwrite_returns_metadata_then_concatenated_data() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&1_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READWRITE.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&21_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read/write payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &21_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(&response[12..16], &4_u32.to_le_bytes());
    assert_eq!(&response[16..20], &0_u32.to_le_bytes());
    assert_eq!(&response[20..24], &1_u32.to_le_bytes());
    assert_eq!(&response[24..28], &1.25_f32.to_le_bytes());
    assert_eq!(response[28], 1);
}

#[test]
fn add_notification_accepts_supported_modes_and_delete_releases_handle() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();

    let response = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERONCHA, 4),
        &ctx,
    );

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let handle = u32::from_le_bytes(response[4..8].try_into().expect("handle"));
    assert_eq!(dispatcher.notification_count(), 1);
    assert_eq!(
        dispatcher.active_notifications()[0].transmission_mode,
        ADSTRANS_SERVERONCHA
    );
    assert_eq!(
        dispatcher.active_notifications()[0].receiver,
        NotificationReceiver {
            net_id: [1, 2, 3, 4, 5, 6],
            port: 0x8001,
        }
    );

    let response = dispatcher.dispatch(
        CommandId::DeleteDeviceNotification,
        &handle.to_le_bytes(),
        &ctx,
    );

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 0);
}

#[test]
fn add_notification_accepts_beckhoff_dotnet_v7_compact_request() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERONCHA.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_000_000_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let handle = u32::from_le_bytes(response[4..8].try_into().expect("handle"));
    assert_ne!(handle, 0);
    assert_eq!(dispatcher.notification_count(), 1);
    assert_eq!(dispatcher.active_notifications()[0].cycle_time_ms, 100);
}

#[test]
fn add_notification_accepts_symbol_version_watch() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VERSION.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERONCHA.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 1);
    let active = dispatcher.active_notifications();
    assert!(matches!(
        &active[0].target,
        NotificationTarget::SymbolVersion
    ));
}

#[test]
fn add_notification_accepts_symbol_value_handle_for_pyads() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut handle_request = Vec::new();
    handle_request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    handle_request.extend_from_slice(&0_u32.to_le_bytes());
    handle_request.extend_from_slice(&4_u32.to_le_bytes());
    handle_request.extend_from_slice(&13_u32.to_le_bytes());
    handle_request.extend_from_slice(b"GVL.Setpoint\0");
    let handle_response = dispatcher.dispatch(CommandId::ReadWrite, &handle_request, &ctx);
    let handle = u32::from_le_bytes(handle_response[8..12].try_into().expect("handle response"));
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    request.extend_from_slice(&handle.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERCYCLE.to_le_bytes());
    request.extend_from_slice(&10_u32.to_le_bytes());
    request.extend_from_slice(&10_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 1);
}

#[test]
fn add_notification_accepts_online_change_count_handle_as_symbol_version() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = format!("{ONLINE_CHANGE_COUNT_NAME}\0");
    let mut handle_request = Vec::new();
    handle_request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    handle_request.extend_from_slice(&0_u32.to_le_bytes());
    handle_request.extend_from_slice(&4_u32.to_le_bytes());
    handle_request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    handle_request.extend_from_slice(name.as_bytes());
    let handle_response = dispatcher.dispatch(CommandId::ReadWrite, &handle_request, &ctx);
    let handle = u32::from_le_bytes(handle_response[8..12].try_into().expect("handle response"));
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    request.extend_from_slice(&handle.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERONCHA.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let active = dispatcher.active_notifications();
    assert!(matches!(
        &active[0].target,
        NotificationTarget::SymbolVersion
    ));
}

#[test]
fn add_notification_accepts_task_count_handle_as_static_system_bytes() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = format!("{TASK_COUNT_NAME}\0");
    let mut handle_request = Vec::new();
    handle_request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    handle_request.extend_from_slice(&0_u32.to_le_bytes());
    handle_request.extend_from_slice(&4_u32.to_le_bytes());
    handle_request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    handle_request.extend_from_slice(name.as_bytes());
    let handle_response = dispatcher.dispatch(CommandId::ReadWrite, &handle_request, &ctx);
    let handle = u32::from_le_bytes(handle_response[8..12].try_into().expect("handle response"));
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    request.extend_from_slice(&handle.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERCYCLE.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let active = dispatcher.active_notifications();
    assert_eq!(
        active[0].target,
        NotificationTarget::SystemBytes(1_u32.to_le_bytes().to_vec())
    );
}

#[test]
fn add_notification_accepts_smaller_watch_and_rejects_too_large_watch() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();

    let unsupported = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(99, 4),
        &ctx,
    );
    assert_eq!(
        u32::from_le_bytes(unsupported[0..4].try_into().expect("result")),
        AdsErrorCode::TransmissionModeNotSupported.value()
    );

    let smaller = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 2),
        &ctx,
    );
    assert_eq!(&smaller[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 1);

    let too_large = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 5),
        &ctx,
    );
    assert_eq!(
        u32::from_le_bytes(too_large[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidWatchSize.value()
    );
}

#[test]
fn add_notification_enforces_per_client_limit() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_max_notifications(1);

    let first = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 4),
        &ctx,
    );
    let second = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 4),
        &ctx,
    );

    assert_eq!(&first[0..4], &0_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(second[0..4].try_into().expect("result")),
        AdsErrorCode::NoMemory.value()
    );
    assert_eq!(dispatcher.notification_count(), 1);
}

#[test]
fn device_notification_payload_matches_wire_matrix() {
    let payload = build_device_notification_payload(&[NotificationStamp::new(
        0x1122_3344_5566_7788,
        vec![NotificationSample::new(132, 1.25_f32.to_le_bytes())],
    )])
    .expect("notification payload");

    assert_eq!(&payload[0..4], &28_u32.to_le_bytes());
    assert_eq!(&payload[4..8], &1_u32.to_le_bytes());
    assert_eq!(&payload[8..16], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&payload[16..20], &1_u32.to_le_bytes());
    assert_eq!(&payload[20..24], &132_u32.to_le_bytes());
    assert_eq!(&payload[24..28], &4_u32.to_le_bytes());
    assert_eq!(&payload[28..32], &1.25_f32.to_le_bytes());
}

#[test]
fn invalidated_notification_sample_has_zero_size_data() {
    let payload = build_device_notification_payload(&[NotificationStamp::new(
        0x9988_7766_5544_3322,
        vec![NotificationSample::invalidated(133)],
    )])
    .expect("notification payload");

    assert_eq!(&payload[0..4], &24_u32.to_le_bytes());
    assert_eq!(&payload[4..8], &1_u32.to_le_bytes());
    assert_eq!(&payload[16..20], &1_u32.to_le_bytes());
    assert_eq!(&payload[20..24], &133_u32.to_le_bytes());
    assert_eq!(&payload[24..28], &0_u32.to_le_bytes());
    assert_eq!(payload.len(), 28);
}

fn add_notification_request(mode: u32, byte_len: u32) -> Vec<u8> {
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&byte_len.to_le_bytes());
    request.extend_from_slice(&mode.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_u32.to_le_bytes());
    request.extend_from_slice(&[0_u8; 16]);
    request
}
