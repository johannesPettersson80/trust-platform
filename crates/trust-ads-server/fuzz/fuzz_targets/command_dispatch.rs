#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use trust_ads_core::{
    AdsDataTypeDescriptor, AmsNetId, IecDataType, PointQuality, SymbolDescriptor, SymbolFlag,
    SymbolSnapshot,
};
use trust_ads_server::{
    build_server_symbol_snapshot, AdsErrorCode, AdsServerAuditEvent, AdsServerError, AuditSink,
    ClientId, ClientPolicy, Clock, CommandContext, CommandDispatcher, CommandId, RuntimeWritePort,
    ServerSymbolSpec, SymbolSource, ValueIo, ADSIGRP_SUMUP_READ, ADSIGRP_SUMUP_READWRITE,
    ADSIGRP_SUMUP_WRITE, ADSIGRP_SYM_DT_UPLOAD, ADSIGRP_SYM_HNDBYNAME, ADSIGRP_SYM_RELEASEHND,
    ADSIGRP_SYM_UPLOAD, ADSIGRP_SYM_UPLOADINFO, ADSIGRP_SYM_UPLOADINFO2, ADSIGRP_SYM_VALBYHND,
    ADSIGRP_SYM_VERSION, ADSTRANS_SERVERCYCLE, ADSTRANS_SERVERONCHA,
    DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
};

const MAX_FUZZ_PAYLOAD: usize = 4096;

struct FuzzHost {
    snapshot: Arc<SymbolSnapshot>,
    version: u32,
    allow: bool,
}

impl FuzzHost {
    fn new(data: &[u8]) -> Self {
        let snapshot = build_server_symbol_snapshot(
            "fuzz",
            [
                ServerSymbolSpec::readable(
                    "GVL.Counter",
                    AdsDataTypeDescriptor::scalar("DINT", IecDataType::Dint),
                )
                .with_flag(SymbolFlag::Write),
                ServerSymbolSpec::readable(
                    "GVL.Ready",
                    AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
                ),
                ServerSymbolSpec::readable(
                    "GVL.Setpoint",
                    AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                )
                .with_flag(SymbolFlag::Write),
            ],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("static fuzz snapshot is valid");
        let version = u32_from(data, 1).unwrap_or(1).max(1);
        let allow = data.get(5).is_none_or(|byte| byte & 0x80 == 0);
        Self {
            snapshot: Arc::new(snapshot),
            version,
            allow,
        }
    }

    fn context<'a>(&'a self, client: &'a ClientId) -> CommandContext<'a> {
        CommandContext::new(client, self, self, self, self, self, self)
    }

    fn symbol_for(&self, data: &[u8], offset: usize) -> &SymbolDescriptor {
        let index = data.get(offset).copied().unwrap_or(0) as usize % self.snapshot.symbols.len();
        &self.snapshot.symbols[index]
    }
}

impl SymbolSource for FuzzHost {
    fn snapshot(&self) -> Arc<SymbolSnapshot> {
        Arc::clone(&self.snapshot)
    }

    fn version(&self) -> u32 {
        self.version
    }
}

impl ValueIo for FuzzHost {
    fn read(&self, symbol: &SymbolDescriptor) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
        let byte_len = usize::try_from(symbol.byte_size).map_err(|_| {
            AdsServerError::device(AdsErrorCode::InvalidSize, "symbol byte size exceeds usize")
        })?;
        let bytes = match symbol.data_type.iec_type {
            IecDataType::Bool => vec![1],
            _ => (0..byte_len)
                .map(|index| (index as u8) ^ (self.version as u8))
                .collect(),
        };
        Ok((bytes, PointQuality::good(1)))
    }
}

impl RuntimeWritePort for FuzzHost {
    fn write(
        &self,
        symbol: &SymbolDescriptor,
        bytes: &[u8],
        _client: &ClientId,
    ) -> Result<(), AdsServerError> {
        if bytes.len() == symbol.byte_size as usize {
            Ok(())
        } else {
            Err(AdsServerError::device(
                AdsErrorCode::InvalidSize,
                "write length does not match symbol size",
            ))
        }
    }
}

impl ClientPolicy for FuzzHost {
    fn permits(&self, _client: &ClientId) -> bool {
        self.allow
    }
}

impl AuditSink for FuzzHost {
    fn record(&self, _event: &AdsServerAuditEvent) {}
}

impl Clock for FuzzHost {
    fn now_ms(&self) -> u64 {
        1
    }
}

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_FUZZ_PAYLOAD)];
    let host = FuzzHost::new(data);
    let client = ClientId::new(AmsNetId::new("127.0.0.1.1.1")).with_source_ip("127.0.0.1");
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_max_notifications(8);

    if data.is_empty() {
        return;
    }

    match data[0] % 8 {
        0 => dispatch_raw(data, &mut dispatcher, &ctx),
        1 => dispatch_read_symbol(data, &host, &mut dispatcher, &ctx),
        2 => dispatch_write_symbol(data, &host, &mut dispatcher, &ctx),
        3 => dispatch_handle_sequence(data, &mut dispatcher, &ctx),
        4 => dispatch_sumup_read(data, &host, &mut dispatcher, &ctx),
        5 => dispatch_sumup_write(data, &host, &mut dispatcher, &ctx),
        6 => dispatch_notification(data, &host, &mut dispatcher, &ctx),
        _ => dispatch_builtin_read(data, &mut dispatcher, &ctx),
    }
});

fn dispatch_raw(data: &[u8], dispatcher: &mut CommandDispatcher, ctx: &CommandContext<'_>) {
    if data.len() < 2 {
        return;
    }
    let command = command_from_byte(data[1]);
    let _ = dispatcher.dispatch(command, &data[2..], ctx);
}

fn dispatch_read_symbol(
    data: &[u8],
    host: &FuzzHost,
    dispatcher: &mut CommandDispatcher,
    ctx: &CommandContext<'_>,
) {
    let symbol = host.symbol_for(data, 2);
    let mut payload = Vec::with_capacity(12);
    push_u32(&mut payload, symbol.index_group);
    push_u32(&mut payload, symbol.index_offset);
    push_u32(&mut payload, bounded_len(data, 3, symbol.byte_size));
    let _ = dispatcher.dispatch(CommandId::Read, &payload, ctx);
}

fn dispatch_write_symbol(
    data: &[u8],
    host: &FuzzHost,
    dispatcher: &mut CommandDispatcher,
    ctx: &CommandContext<'_>,
) {
    let symbol = host.symbol_for(data, 2);
    let requested_len = bounded_write_len(data, 3, symbol.byte_size);
    let mut payload = Vec::with_capacity(12 + requested_len);
    push_u32(&mut payload, symbol.index_group);
    push_u32(&mut payload, symbol.index_offset);
    push_u32(&mut payload, requested_len as u32);
    payload.extend((0..requested_len).map(|index| data.get(index + 4).copied().unwrap_or(0)));
    let _ = dispatcher.dispatch(CommandId::Write, &payload, ctx);
}

fn dispatch_handle_sequence(
    data: &[u8],
    dispatcher: &mut CommandDispatcher,
    ctx: &CommandContext<'_>,
) {
    let name = if data.get(2).copied().unwrap_or(0) & 1 == 0 {
        b"GVL.Setpoint\0".as_slice()
    } else {
        b"GVL.Missing\0".as_slice()
    };
    let handle_request = read_write_payload(ADSIGRP_SYM_HNDBYNAME, 0, 4, name);
    let response = dispatcher.dispatch(CommandId::ReadWrite, &handle_request, ctx);
    let Some(handle) = response_handle(&response) else {
        return;
    };

    let mut read_payload = Vec::with_capacity(12);
    push_u32(&mut read_payload, ADSIGRP_SYM_VALBYHND);
    push_u32(&mut read_payload, handle);
    push_u32(&mut read_payload, bounded_len(data, 3, 4));
    let _ = dispatcher.dispatch(CommandId::Read, &read_payload, ctx);

    let mut release_payload = Vec::with_capacity(12);
    push_u32(&mut release_payload, ADSIGRP_SYM_RELEASEHND);
    push_u32(&mut release_payload, 0);
    push_u32(&mut release_payload, 4);
    push_u32(&mut release_payload, handle);
    let _ = dispatcher.dispatch(CommandId::Write, &release_payload, ctx);
}

fn dispatch_sumup_read(
    data: &[u8],
    host: &FuzzHost,
    dispatcher: &mut CommandDispatcher,
    ctx: &CommandContext<'_>,
) {
    let mut requests = Vec::new();
    for offset in [2_usize, 3] {
        let symbol = host.symbol_for(data, offset);
        push_u32(&mut requests, symbol.index_group);
        push_u32(&mut requests, symbol.index_offset);
        push_u32(&mut requests, bounded_len(data, offset + 4, symbol.byte_size));
    }
    let payload = read_write_payload(ADSIGRP_SUMUP_READ, 2, 4096, &requests);
    let _ = dispatcher.dispatch(CommandId::ReadWrite, &payload, ctx);
}

fn dispatch_sumup_write(
    data: &[u8],
    host: &FuzzHost,
    dispatcher: &mut CommandDispatcher,
    ctx: &CommandContext<'_>,
) {
    let mut requests = Vec::new();
    let mut values = Vec::new();
    for offset in [2_usize, 3] {
        let symbol = host.symbol_for(data, offset);
        let write_len = bounded_write_len(data, offset + 4, symbol.byte_size);
        push_u32(&mut requests, symbol.index_group);
        push_u32(&mut requests, symbol.index_offset);
        push_u32(&mut requests, write_len as u32);
        values.extend((0..write_len).map(|index| data.get(index + offset + 8).copied().unwrap_or(0)));
    }
    requests.extend(values);
    let payload = read_write_payload(ADSIGRP_SUMUP_WRITE, 2, 8, &requests);
    let _ = dispatcher.dispatch(CommandId::ReadWrite, &payload, ctx);
}

fn dispatch_notification(
    data: &[u8],
    host: &FuzzHost,
    dispatcher: &mut CommandDispatcher,
    ctx: &CommandContext<'_>,
) {
    let symbol = host.symbol_for(data, 2);
    let mode = if data.get(3).copied().unwrap_or(0) & 1 == 0 {
        ADSTRANS_SERVERCYCLE
    } else {
        ADSTRANS_SERVERONCHA
    };
    let mut payload = Vec::with_capacity(40);
    push_u32(&mut payload, symbol.index_group);
    push_u32(&mut payload, symbol.index_offset);
    push_u32(&mut payload, bounded_len(data, 4, symbol.byte_size));
    push_u32(&mut payload, mode);
    push_u32(&mut payload, u32_from(data, 5).unwrap_or(0) % 1000);
    push_u32(&mut payload, u32_from(data, 9).unwrap_or(1) % 1000);
    payload.extend_from_slice(&[0; 16]);
    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &payload, ctx);
    if let Some(handle) = response_handle(&response) {
        let mut delete = Vec::with_capacity(4);
        push_u32(&mut delete, handle);
        let _ = dispatcher.dispatch(CommandId::DeleteDeviceNotification, &delete, ctx);
    }
}

fn dispatch_builtin_read(
    data: &[u8],
    dispatcher: &mut CommandDispatcher,
    ctx: &CommandContext<'_>,
) {
    let group = match data.get(2).copied().unwrap_or(0) % 6 {
        0 => ADSIGRP_SYM_VERSION,
        1 => ADSIGRP_SYM_UPLOADINFO,
        2 => ADSIGRP_SYM_UPLOADINFO2,
        3 => ADSIGRP_SYM_UPLOAD,
        4 => ADSIGRP_SYM_DT_UPLOAD,
        _ => ADSIGRP_SUMUP_READWRITE,
    };
    let mut payload = Vec::with_capacity(12);
    push_u32(&mut payload, group);
    push_u32(&mut payload, u32_from(data, 3).unwrap_or(0));
    push_u32(&mut payload, bounded_len(data, 7, 4096));
    let _ = dispatcher.dispatch(CommandId::Read, &payload, ctx);
}

fn command_from_byte(byte: u8) -> CommandId {
    match byte % 9 {
        0 => CommandId::ReadDeviceInfo,
        1 => CommandId::Read,
        2 => CommandId::Write,
        3 => CommandId::ReadState,
        4 => CommandId::WriteControl,
        5 => CommandId::AddDeviceNotification,
        6 => CommandId::DeleteDeviceNotification,
        7 => CommandId::DeviceNotification,
        _ => CommandId::ReadWrite,
    }
}

fn read_write_payload(
    index_group: u32,
    index_offset: u32,
    read_length: u32,
    write_data: &[u8],
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16 + write_data.len());
    push_u32(&mut payload, index_group);
    push_u32(&mut payload, index_offset);
    push_u32(&mut payload, read_length);
    push_u32(&mut payload, write_data.len() as u32);
    payload.extend_from_slice(write_data);
    payload
}

fn response_handle(response: &[u8]) -> Option<u32> {
    if response.get(0..4)? != AdsErrorCode::NoError.value().to_le_bytes() {
        return None;
    }
    let length = u32::from_le_bytes(response.get(4..8)?.try_into().ok()?);
    if length < 4 {
        return None;
    }
    Some(u32::from_le_bytes(response.get(8..12)?.try_into().ok()?))
}

fn bounded_len(data: &[u8], offset: usize, preferred: u32) -> u32 {
    match data.get(offset).copied().unwrap_or(0) % 4 {
        0 => preferred,
        1 => preferred.saturating_add(1),
        2 => 0,
        _ => u32_from(data, offset).unwrap_or(preferred).min(4096),
    }
}

fn bounded_write_len(data: &[u8], offset: usize, preferred: u32) -> usize {
    usize::try_from(bounded_len(data, offset, preferred).min(64)).unwrap_or(0)
}

fn u32_from(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
