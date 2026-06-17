//! Hidden `TwinCAT`-compatible system symbols used by engineering tools.
//!
//! These symbols are not part of the exposed runtime symbol table. They are
//! compatibility surfaces that `TwinCAT` Target Browser probes while loading
//! task and symbol metadata from a PLC runtime port.

use std::collections::BTreeSet;

use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag};

/// ADS device-data index group.
pub const ADSIGRP_DEVICE_DATA: u32 = 0x2000;
/// Device-data offset for the task time base, expressed in 100 ns units.
pub const ADSIOFFS_DEVICE_DATA_TIME_BASE: u32 = 0x101;
/// `TwinCAT` reports a 10 ms task time base here as 100 ns ticks.
pub const TWINCAT_DEVICE_TIME_BASE_100NS: u32 = 100_000;

/// `TwinCAT` symbol-version system variable.
pub const ONLINE_CHANGE_COUNT_NAME: &str = "TwinCAT_SystemInfoVarList._AppInfo.OnlineChangeCnt";
/// `TwinCAT` task-count system variable.
pub const TASK_COUNT_NAME: &str = "TwinCAT_SystemInfoVarList._AppInfo.TaskCnt";

const ONLINE_CHANGE_COUNT_INDEX_GROUP: u32 = 0x4040;
const ONLINE_CHANGE_COUNT_INDEX_OFFSET: u32 = 0x0005_E8D8;
const TASK_COUNT_INDEX_GROUP: u32 = 0x4040;
const TASK_COUNT_INDEX_OFFSET: u32 = 0x0005_E8D4;
const TASK_INFO_INDEX_GROUP: u32 = 0x4040;
const TASK_INFO_INDEX_OFFSET: u32 = 0x0005_E9D0;
const TASK_INFO_BYTE_SIZE: u32 = 128;
const TASK_NAME_OFFSET: usize = 64;
const TASK_NAME_CAPACITY: usize = 64;
const TASK_NAME: &str = "truST_MainTask";

const TASK_INFO_NAME: &str = "TwinCAT_SystemInfoVarList._TaskInfo";
const TASK_INFO_1_NAME: &str = "TwinCAT_SystemInfoVarList._TaskInfo[1]";
const TASK_INFO_OBJ_ID_NAME: &str = "TwinCAT_SystemInfoVarList._TaskInfo[1].ObjId";
const TASK_INFO_CYCLE_TIME_NAME: &str = "TwinCAT_SystemInfoVarList._TaskInfo[1].CycleTime";
const TASK_INFO_PRIORITY_NAME: &str = "TwinCAT_SystemInfoVarList._TaskInfo[1].Priority";
const TASK_INFO_ADS_PORT_NAME: &str = "TwinCAT_SystemInfoVarList._TaskInfo[1].AdsPort";
const TASK_INFO_TASK_NAME_NAME: &str = "TwinCAT_SystemInfoVarList._TaskInfo[1].TaskName";

/// Returns fixed ADS device-data values probed by `TwinCAT` tools.
#[must_use]
pub fn device_data(index_group: u32, index_offset: u32) -> Option<Vec<u8>> {
    (index_group == ADSIGRP_DEVICE_DATA && index_offset == ADSIOFFS_DEVICE_DATA_TIME_BASE)
        .then(|| TWINCAT_DEVICE_TIME_BASE_100NS.to_le_bytes().to_vec())
}

/// Returns a hidden system symbol by ADS name.
#[must_use]
pub fn symbol_by_name(name: &str) -> Option<SymbolDescriptor> {
    match name {
        ONLINE_CHANGE_COUNT_NAME => Some(online_change_count_symbol()),
        TASK_COUNT_NAME => Some(task_count_symbol()),
        TASK_INFO_NAME => Some(task_info_symbol(TASK_INFO_NAME, task_array_type())),
        TASK_INFO_1_NAME => Some(task_info_symbol(TASK_INFO_1_NAME, task_struct_type())),
        TASK_INFO_OBJ_ID_NAME => Some(task_field_symbol(
            TASK_INFO_OBJ_ID_NAME,
            "OTCID",
            IecDataType::Udint,
            0,
            4,
        )),
        TASK_INFO_CYCLE_TIME_NAME => Some(task_field_symbol(
            TASK_INFO_CYCLE_TIME_NAME,
            "UDINT",
            IecDataType::Udint,
            4,
            4,
        )),
        TASK_INFO_PRIORITY_NAME => Some(task_field_symbol(
            TASK_INFO_PRIORITY_NAME,
            "UINT",
            IecDataType::Uint,
            8,
            2,
        )),
        TASK_INFO_ADS_PORT_NAME => Some(task_field_symbol(
            TASK_INFO_ADS_PORT_NAME,
            "UINT",
            IecDataType::Uint,
            10,
            2,
        )),
        TASK_INFO_TASK_NAME_NAME => Some(task_name_symbol()),
        _ => None,
    }
}

/// Returns a hidden system symbol by ADS address.
#[must_use]
pub fn symbol_by_address(index_group: u32, index_offset: u32) -> Option<SymbolDescriptor> {
    if index_group == ONLINE_CHANGE_COUNT_INDEX_GROUP
        && index_offset == ONLINE_CHANGE_COUNT_INDEX_OFFSET
    {
        return Some(online_change_count_symbol());
    }
    if index_group == TASK_COUNT_INDEX_GROUP && index_offset == TASK_COUNT_INDEX_OFFSET {
        return Some(task_count_symbol());
    }
    if index_group != TASK_INFO_INDEX_GROUP {
        return None;
    }
    match index_offset.saturating_sub(TASK_INFO_INDEX_OFFSET) {
        0 => Some(task_info_symbol(TASK_INFO_1_NAME, task_struct_type())),
        4 => Some(task_field_symbol(
            TASK_INFO_CYCLE_TIME_NAME,
            "UDINT",
            IecDataType::Udint,
            4,
            4,
        )),
        8 => Some(task_field_symbol(
            TASK_INFO_PRIORITY_NAME,
            "UINT",
            IecDataType::Uint,
            8,
            2,
        )),
        10 => Some(task_field_symbol(
            TASK_INFO_ADS_PORT_NAME,
            "UINT",
            IecDataType::Uint,
            10,
            2,
        )),
        64 => Some(task_name_symbol()),
        _ => None,
    }
}

/// Returns whether this symbol aliases the symbol-version counter.
#[must_use]
pub fn is_online_change_count_symbol(symbol: &SymbolDescriptor) -> bool {
    symbol.name == ONLINE_CHANGE_COUNT_NAME
        || (symbol.index_group == ONLINE_CHANGE_COUNT_INDEX_GROUP
            && symbol.index_offset == ONLINE_CHANGE_COUNT_INDEX_OFFSET)
}

/// Returns hidden system-symbol value bytes.
#[must_use]
pub fn read_symbol(
    symbol: &SymbolDescriptor,
    symbol_version: u32,
    ads_port: u16,
) -> Option<Vec<u8>> {
    match symbol.name.as_str() {
        ONLINE_CHANGE_COUNT_NAME => Some(symbol_version.to_le_bytes().to_vec()),
        TASK_COUNT_NAME => Some(1_u32.to_le_bytes().to_vec()),
        TASK_INFO_NAME | TASK_INFO_1_NAME => Some(task_info_bytes(ads_port)),
        TASK_INFO_OBJ_ID_NAME => Some(task_object_id().to_le_bytes().to_vec()),
        TASK_INFO_CYCLE_TIME_NAME => Some(TWINCAT_DEVICE_TIME_BASE_100NS.to_le_bytes().to_vec()),
        TASK_INFO_PRIORITY_NAME => Some(20_u16.to_le_bytes().to_vec()),
        TASK_INFO_ADS_PORT_NAME => Some(ads_port.to_le_bytes().to_vec()),
        TASK_INFO_TASK_NAME_NAME => Some(task_name_bytes().to_vec()),
        _ if symbol.index_group == TASK_COUNT_INDEX_GROUP
            && symbol.index_offset == TASK_COUNT_INDEX_OFFSET =>
        {
            Some(1_u32.to_le_bytes().to_vec())
        }
        _ if symbol.index_group == TASK_INFO_INDEX_GROUP
            && symbol.index_offset == TASK_INFO_INDEX_OFFSET =>
        {
            Some(task_info_bytes(ads_port))
        }
        _ => None,
    }
}

fn online_change_count_symbol() -> SymbolDescriptor {
    readonly_symbol(
        ONLINE_CHANGE_COUNT_NAME,
        AdsDataTypeDescriptor::scalar("UDINT", IecDataType::Udint),
        ONLINE_CHANGE_COUNT_INDEX_GROUP,
        ONLINE_CHANGE_COUNT_INDEX_OFFSET,
        4,
    )
}

fn task_count_symbol() -> SymbolDescriptor {
    readonly_symbol(
        TASK_COUNT_NAME,
        AdsDataTypeDescriptor::scalar("UDINT", IecDataType::Udint),
        TASK_COUNT_INDEX_GROUP,
        TASK_COUNT_INDEX_OFFSET,
        4,
    )
}

fn task_info_symbol(name: &str, data_type: AdsDataTypeDescriptor) -> SymbolDescriptor {
    readonly_symbol(
        name,
        data_type,
        TASK_INFO_INDEX_GROUP,
        TASK_INFO_INDEX_OFFSET,
        TASK_INFO_BYTE_SIZE,
    )
}

fn task_field_symbol(
    name: &str,
    source_name: &str,
    iec_type: IecDataType,
    relative_offset: u32,
    byte_size: u32,
) -> SymbolDescriptor {
    readonly_symbol(
        name,
        AdsDataTypeDescriptor::scalar(source_name, iec_type),
        TASK_INFO_INDEX_GROUP,
        TASK_INFO_INDEX_OFFSET + relative_offset,
        byte_size,
    )
}

fn task_name_symbol() -> SymbolDescriptor {
    readonly_symbol(
        TASK_INFO_TASK_NAME_NAME,
        AdsDataTypeDescriptor::string("STRING(63)", 63),
        TASK_INFO_INDEX_GROUP,
        TASK_INFO_INDEX_OFFSET
            + u32::try_from(TASK_NAME_OFFSET).expect("task-name offset fits u32"),
        u32::try_from(TASK_NAME_CAPACITY).expect("task-name capacity fits u32"),
    )
}

fn readonly_symbol(
    name: &str,
    data_type: AdsDataTypeDescriptor,
    index_group: u32,
    index_offset: u32,
    byte_size: u32,
) -> SymbolDescriptor {
    let mut symbol = SymbolDescriptor::new(name, data_type, index_group, index_offset, byte_size);
    symbol.flags = BTreeSet::from([SymbolFlag::Read]);
    symbol
}

fn task_array_type() -> AdsDataTypeDescriptor {
    AdsDataTypeDescriptor::scalar("ARRAY [1..1] OF PlcTaskSystemInfo", IecDataType::Byte)
}

fn task_struct_type() -> AdsDataTypeDescriptor {
    AdsDataTypeDescriptor::scalar("PlcTaskSystemInfo", IecDataType::Byte)
}

fn task_info_bytes(ads_port: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; usize::try_from(TASK_INFO_BYTE_SIZE).expect("task size fits usize")];
    bytes[0..4].copy_from_slice(&task_object_id().to_le_bytes());
    bytes[4..8].copy_from_slice(&TWINCAT_DEVICE_TIME_BASE_100NS.to_le_bytes());
    bytes[8..10].copy_from_slice(&20_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&ads_port.to_le_bytes());
    bytes[TASK_NAME_OFFSET..TASK_NAME_OFFSET + TASK_NAME_CAPACITY]
        .copy_from_slice(&task_name_bytes());
    bytes
}

fn task_name_bytes() -> [u8; TASK_NAME_CAPACITY] {
    let mut bytes = [0_u8; TASK_NAME_CAPACITY];
    let name = TASK_NAME.as_bytes();
    let len = name.len().min(TASK_NAME_CAPACITY.saturating_sub(1));
    bytes[..len].copy_from_slice(&name[..len]);
    bytes
}

fn task_object_id() -> u32 {
    0x0850_2001
}
