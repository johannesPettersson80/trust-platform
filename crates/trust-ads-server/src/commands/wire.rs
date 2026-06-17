//! ADS command wire serialization helpers.

use std::collections::BTreeSet;

use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag};

use crate::{AdsErrorCode, CommandId, NotificationStamp};

const ADSDATATYPEFLAG_DATATYPE: u32 = 0x0000_0001;
const ADSSYMBOLFLAG_PERSISTENT: u32 = 0x0000_0001;
const ADSSYMBOLFLAG_BITVALUE: u32 = 0x0000_0002;
const ADSSYMBOLFLAG_READONLY: u32 = 0x0000_0020;

/// Builds a `DeviceNotification` command payload.
///
/// The first `length` field excludes itself and includes `stamp_count`,
/// matching Beckhoff's `AdsNotificationStream` layout.
///
/// # Errors
///
/// Returns `InvalidSize` if any count or byte length cannot fit on the ADS wire.
pub fn build_device_notification_payload(
    stamps: &[NotificationStamp],
) -> Result<Vec<u8>, AdsErrorCode> {
    let mut stream = Vec::new();
    stream.extend_from_slice(&u32_len(stamps.len())?.to_le_bytes());
    for stamp in stamps {
        stream.extend_from_slice(&stamp.timestamp_filetime.to_le_bytes());
        stream.extend_from_slice(&u32_len(stamp.samples.len())?.to_le_bytes());
        for sample in &stamp.samples {
            stream.extend_from_slice(&sample.handle.to_le_bytes());
            stream.extend_from_slice(&u32_len(sample.data.len())?.to_le_bytes());
            stream.extend_from_slice(&sample.data);
        }
    }

    let mut payload = Vec::with_capacity(4 + stream.len());
    payload.extend_from_slice(&u32_len(stream.len())?.to_le_bytes());
    payload.extend(stream);
    Ok(payload)
}

pub(super) fn read_response(code: AdsErrorCode, data: &[u8]) -> Vec<u8> {
    let (code, length) = if code == AdsErrorCode::NoError {
        match u32_len(data.len()) {
            Ok(length) => (code, length),
            Err(code) => (code, 0),
        }
    } else {
        (code, 0)
    };
    let mut response = Vec::with_capacity(8 + data.len());
    push_result(&mut response, code);
    response.extend_from_slice(&length.to_le_bytes());
    if code == AdsErrorCode::NoError {
        response.extend_from_slice(data);
    }
    response
}

pub(super) fn add_notification_response(code: AdsErrorCode, handle: u32) -> Vec<u8> {
    let mut response = Vec::with_capacity(8);
    push_result(&mut response, code);
    response.extend_from_slice(&handle.to_le_bytes());
    response
}

pub(super) fn error_response(command_id: CommandId, code: AdsErrorCode) -> Vec<u8> {
    match command_id {
        CommandId::Read | CommandId::ReadWrite => read_response(code, &[]),
        CommandId::AddDeviceNotification => add_notification_response(code, 0),
        _ => write_result(code),
    }
}

pub(super) fn write_result(code: AdsErrorCode) -> Vec<u8> {
    let mut response = Vec::with_capacity(4);
    push_result(&mut response, code);
    response
}

pub(super) fn push_result(out: &mut Vec<u8>, code: AdsErrorCode) {
    out.extend_from_slice(&code.value().to_le_bytes());
}

pub(super) fn sized_data(data: Vec<u8>, read_length: u32) -> (AdsErrorCode, Vec<u8>) {
    let Ok(read_length) = usize::try_from(read_length) else {
        return (AdsErrorCode::InvalidSize, Vec::new());
    };
    if data.len() <= read_length {
        return (AdsErrorCode::NoError, data);
    }
    (AdsErrorCode::NoError, data[..read_length].to_vec())
}

pub(super) fn sized_value_data(mut data: Vec<u8>, read_length: u32) -> (AdsErrorCode, Vec<u8>) {
    let Ok(read_length) = usize::try_from(read_length) else {
        return (AdsErrorCode::InvalidSize, Vec::new());
    };
    if data.len() > read_length {
        data.truncate(read_length);
    } else if data.len() < read_length {
        data.resize(read_length, 0);
    }
    (AdsErrorCode::NoError, data)
}

pub(super) fn u32_len(len: usize) -> Result<u32, AdsErrorCode> {
    u32::try_from(len).map_err(|_| AdsErrorCode::InvalidSize)
}

fn u16_len(len: usize) -> Result<u16, AdsErrorCode> {
    u16::try_from(len).map_err(|_| AdsErrorCode::InvalidSize)
}

fn i32_from_i64(value: i64) -> Result<i32, AdsErrorCode> {
    i32::try_from(value).map_err(|_| AdsErrorCode::InvalidSize)
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, AdsErrorCode> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(AdsErrorCode::InvalidSize)?;
    let bytes: [u8; 4] = slice.try_into().map_err(|_| AdsErrorCode::InvalidSize)?;
    Ok(u32::from_le_bytes(bytes))
}

pub(super) fn serialize_symbol_table(
    symbols: &[SymbolDescriptor],
) -> Result<Vec<u8>, AdsErrorCode> {
    let mut table = Vec::new();
    for symbol in symbols {
        table.extend(serialize_symbol_entry(symbol)?);
    }
    Ok(table)
}

pub(super) fn serialize_symbol_entry(symbol: &SymbolDescriptor) -> Result<Vec<u8>, AdsErrorCode> {
    let name = symbol.name.as_bytes();
    let ty = symbol.data_type.source_name.as_bytes();
    let comment: &[u8] = &[];
    let entry_len = 30 + name.len() + 1 + ty.len() + 1 + comment.len() + 1;
    let entry_len_u32 = u32_len(entry_len)?;
    let name_len = u16_len(name.len())?;
    let ty_len = u16_len(ty.len())?;
    let comment_len = u16_len(comment.len())?;

    let mut entry = Vec::with_capacity(entry_len);
    entry.extend_from_slice(&entry_len_u32.to_le_bytes());
    entry.extend_from_slice(&symbol.index_group.to_le_bytes());
    entry.extend_from_slice(&symbol.index_offset.to_le_bytes());
    entry.extend_from_slice(&symbol.byte_size.to_le_bytes());
    entry.extend_from_slice(&ads_data_type_id(&symbol.data_type).to_le_bytes());
    entry.extend_from_slice(&symbol_flags(symbol).to_le_bytes());
    entry.extend_from_slice(&name_len.to_le_bytes());
    entry.extend_from_slice(&ty_len.to_le_bytes());
    entry.extend_from_slice(&comment_len.to_le_bytes());
    entry.extend_from_slice(name);
    entry.push(0);
    entry.extend_from_slice(ty);
    entry.push(0);
    entry.extend_from_slice(comment);
    entry.push(0);
    Ok(entry)
}

pub(super) fn serialize_datatype_table(
    symbols: &[SymbolDescriptor],
) -> Result<Vec<u8>, AdsErrorCode> {
    let mut table = Vec::new();
    for descriptor in unique_datatype_descriptors(symbols) {
        table.extend(serialize_datatype_entry(&descriptor)?);
    }
    Ok(table)
}

pub(super) fn unique_datatype_descriptors(
    symbols: &[SymbolDescriptor],
) -> Vec<AdsDataTypeDescriptor> {
    let mut seen = BTreeSet::new();
    let mut descriptors = Vec::new();
    for symbol in symbols {
        push_unique_datatype(&mut seen, &mut descriptors, symbol.data_type.clone());
    }
    descriptors
}

fn push_unique_datatype(
    seen: &mut BTreeSet<String>,
    descriptors: &mut Vec<AdsDataTypeDescriptor>,
    descriptor: AdsDataTypeDescriptor,
) {
    if seen.insert(descriptor.source_name.clone()) {
        descriptors.push(descriptor);
    }
}

pub(super) fn serialize_datatype_entry(
    descriptor: &AdsDataTypeDescriptor,
) -> Result<Vec<u8>, AdsErrorCode> {
    let name = descriptor.source_name.as_bytes();
    let ty = descriptor.source_name.as_bytes();
    let comment: &[u8] = &[];
    let array_tail_len = descriptor.dimensions.len() * 8;
    let entry_len = 42 + name.len() + 1 + ty.len() + 1 + comment.len() + 1 + array_tail_len;
    let byte_len = u32_len(descriptor.byte_len().unwrap_or(0))?;
    let entry_len_u32 = u32_len(entry_len)?;
    let name_len = u16_len(name.len())?;
    let ty_len = u16_len(ty.len())?;
    let comment_len = u16_len(comment.len())?;
    let dimension_count = u16_len(descriptor.dimensions.len())?;

    let mut entry = Vec::with_capacity(entry_len);
    entry.extend_from_slice(&entry_len_u32.to_le_bytes());
    entry.extend_from_slice(&1_u32.to_le_bytes());
    entry.extend_from_slice(&0_u32.to_le_bytes());
    entry.extend_from_slice(&0_u32.to_le_bytes());
    entry.extend_from_slice(&byte_len.to_le_bytes());
    entry.extend_from_slice(&0_u32.to_le_bytes());
    entry.extend_from_slice(&ads_data_type_id(descriptor).to_le_bytes());
    entry.extend_from_slice(&ADSDATATYPEFLAG_DATATYPE.to_le_bytes());
    entry.extend_from_slice(&name_len.to_le_bytes());
    entry.extend_from_slice(&ty_len.to_le_bytes());
    entry.extend_from_slice(&comment_len.to_le_bytes());
    entry.extend_from_slice(&dimension_count.to_le_bytes());
    entry.extend_from_slice(&0_u16.to_le_bytes());
    entry.extend_from_slice(name);
    entry.push(0);
    entry.extend_from_slice(ty);
    entry.push(0);
    entry.extend_from_slice(comment);
    entry.push(0);
    for dimension in &descriptor.dimensions {
        entry.extend_from_slice(&i32_from_i64(dimension.lower)?.to_le_bytes());
        let elements = u32_len(dimension.len().unwrap_or(0))?;
        entry.extend_from_slice(&elements.to_le_bytes());
    }
    Ok(entry)
}

fn symbol_flags(symbol: &SymbolDescriptor) -> u32 {
    let mut flags = 0_u32;
    if symbol.flags.contains(&SymbolFlag::Persistent) {
        flags |= ADSSYMBOLFLAG_PERSISTENT;
    }
    if symbol.data_type.iec_type == IecDataType::Bool {
        flags |= ADSSYMBOLFLAG_BITVALUE;
    }
    if !symbol.flags.contains(&SymbolFlag::Write) {
        flags |= ADSSYMBOLFLAG_READONLY;
    }
    flags
}

fn ads_data_type_id(descriptor: &AdsDataTypeDescriptor) -> u32 {
    match descriptor.iec_type {
        IecDataType::Bool => 33,
        IecDataType::Sint => 16,
        IecDataType::Int => 2,
        IecDataType::Dint => 3,
        IecDataType::Lint => 20,
        IecDataType::Usint | IecDataType::Byte => 17,
        IecDataType::Uint | IecDataType::Word => 18,
        IecDataType::Udint | IecDataType::Dword => 19,
        IecDataType::Ulint | IecDataType::Lword => 21,
        IecDataType::Real => 4,
        IecDataType::Lreal => 5,
        IecDataType::String => 30,
    }
}
