use alloc::{boxed::Box, format, string::String, vec::Vec};

use smol_str::SmolStr;

use crate::bytecode::{ConstEntry, ConstPool, StringTable, TypeData, TypeEntry, TypeTable};
use crate::error::RuntimeError;
use crate::value::{
    DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, Value,
};

/// Decode every bytecode constant-pool entry into runtime core values.
pub fn decode_const_pool_entries(
    const_pool: &ConstPool,
    types: &TypeTable,
    strings: &StringTable,
) -> Result<Vec<Value>, RuntimeError> {
    let mut out = Vec::with_capacity(const_pool.entries.len());
    for entry in &const_pool.entries {
        out.push(decode_const_value(entry, types, strings)?);
    }
    Ok(out)
}

fn resolve_const_entry(
    types: &TypeTable,
    type_id: u32,
    depth: u8,
) -> Result<&TypeEntry, RuntimeError> {
    if depth > 32 {
        return Err(invalid_bytecode("const type recursion overflow"));
    }
    let entry = types
        .entries
        .get(type_id as usize)
        .ok_or_else(|| invalid_bytecode(format!("invalid const type index {type_id}")))?;
    match &entry.data {
        TypeData::Primitive { .. } | TypeData::Enum { .. } => Ok(entry),
        TypeData::Alias { target_type_id } => {
            resolve_const_entry(types, *target_type_id, depth + 1)
        }
        TypeData::Subrange { base_type_id, .. } => {
            resolve_const_entry(types, *base_type_id, depth + 1)
        }
        _ => Err(invalid_bytecode(format!(
            "unsupported const type kind at index {type_id}"
        ))),
    }
}

fn decode_const_value(
    entry: &ConstEntry,
    types: &TypeTable,
    strings: &StringTable,
) -> Result<Value, RuntimeError> {
    match &resolve_const_entry(types, entry.type_id, 0)?.data {
        TypeData::Enum { variants, .. } => {
            let bytes = read_exact::<8>(&entry.payload, "enum const payload")?;
            let numeric_value = i64::from_le_bytes(bytes);
            let type_entry = resolve_const_entry(types, entry.type_id, 0)?;
            let enum_name_idx = type_entry
                .name_idx
                .ok_or_else(|| invalid_bytecode("enum const missing type name"))?;
            let enum_name = strings
                .entries
                .get(enum_name_idx as usize)
                .cloned()
                .ok_or_else(|| invalid_bytecode("enum const type name index out of bounds"))?;
            let variant = variants
                .iter()
                .find(|variant| variant.value == numeric_value)
                .ok_or_else(|| invalid_bytecode("enum const variant value missing"))?;
            let variant_name = strings
                .entries
                .get(variant.name_idx as usize)
                .cloned()
                .ok_or_else(|| invalid_bytecode("enum const variant name index out of bounds"))?;
            Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
                enum_name,
                variant_name,
                numeric_value,
            ))))
        }
        TypeData::Primitive { prim_id, .. } => decode_primitive_constant(*prim_id, &entry.payload),
        _ => Err(invalid_bytecode("unsupported const type kind")),
    }
}

fn decode_primitive_constant(prim_id: u16, payload: &[u8]) -> Result<Value, RuntimeError> {
    match prim_id {
        1 => {
            let value = read_exact::<1>(payload, "BOOL const payload")?[0];
            Ok(Value::Bool(value != 0))
        }
        2 => Ok(Value::Byte(
            read_exact::<1>(payload, "BYTE const payload")?[0],
        )),
        3 => Ok(Value::Word(u16::from_le_bytes(read_exact::<2>(
            payload,
            "WORD const payload",
        )?))),
        4 => Ok(Value::DWord(u32::from_le_bytes(read_exact::<4>(
            payload,
            "DWORD const payload",
        )?))),
        5 => Ok(Value::LWord(u64::from_le_bytes(read_exact::<8>(
            payload,
            "LWORD const payload",
        )?))),
        6 => Ok(Value::SInt(i8::from_le_bytes(read_exact::<1>(
            payload,
            "SINT const payload",
        )?))),
        7 => Ok(Value::Int(i16::from_le_bytes(read_exact::<2>(
            payload,
            "INT const payload",
        )?))),
        8 => Ok(Value::DInt(i32::from_le_bytes(read_exact::<4>(
            payload,
            "DINT const payload",
        )?))),
        9 => Ok(Value::LInt(i64::from_le_bytes(read_exact::<8>(
            payload,
            "LINT const payload",
        )?))),
        10 => Ok(Value::USInt(
            read_exact::<1>(payload, "USINT const payload")?[0],
        )),
        11 => Ok(Value::UInt(u16::from_le_bytes(read_exact::<2>(
            payload,
            "UINT const payload",
        )?))),
        12 => Ok(Value::UDInt(u32::from_le_bytes(read_exact::<4>(
            payload,
            "UDINT const payload",
        )?))),
        13 => Ok(Value::ULInt(u64::from_le_bytes(read_exact::<8>(
            payload,
            "ULINT const payload",
        )?))),
        14 => Ok(Value::Real(f32::from_le_bytes(read_exact::<4>(
            payload,
            "REAL const payload",
        )?))),
        15 => Ok(Value::LReal(f64::from_le_bytes(read_exact::<8>(
            payload,
            "LREAL const payload",
        )?))),
        16 => Ok(Value::Time(Duration::from_nanos(i64::from_le_bytes(
            read_exact::<8>(payload, "TIME const payload")?,
        )))),
        17 => Ok(Value::LTime(Duration::from_nanos(i64::from_le_bytes(
            read_exact::<8>(payload, "LTIME const payload")?,
        )))),
        18 => Ok(Value::Date(DateValue::new(i64::from_le_bytes(
            read_exact::<8>(payload, "DATE const payload")?,
        )))),
        19 => Ok(Value::LDate(LDateValue::new(i64::from_le_bytes(
            read_exact::<8>(payload, "LDATE const payload")?,
        )))),
        20 => Ok(Value::Tod(TimeOfDayValue::new(i64::from_le_bytes(
            read_exact::<8>(payload, "TOD const payload")?,
        )))),
        21 => Ok(Value::LTod(LTimeOfDayValue::new(i64::from_le_bytes(
            read_exact::<8>(payload, "LTOD const payload")?,
        )))),
        22 => Ok(Value::Dt(DateTimeValue::new(i64::from_le_bytes(
            read_exact::<8>(payload, "DT const payload")?,
        )))),
        23 => Ok(Value::Ldt(LDateTimeValue::new(i64::from_le_bytes(
            read_exact::<8>(payload, "LDT const payload")?,
        )))),
        24 => {
            let text = core::str::from_utf8(payload)
                .map_err(|err| invalid_bytecode(format!("invalid STRING const UTF-8: {err}")))?;
            Ok(Value::String(SmolStr::new(text)))
        }
        25 => {
            if !payload.len().is_multiple_of(2) {
                return Err(invalid_bytecode("invalid WSTRING const payload length"));
            }
            let units = payload
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            let text = String::from_utf16(&units)
                .map_err(|err| invalid_bytecode(format!("invalid WSTRING const UTF-16: {err}")))?;
            Ok(Value::WString(text))
        }
        26 => Ok(Value::Char(
            read_exact::<1>(payload, "CHAR const payload")?[0],
        )),
        27 => Ok(Value::WChar(u16::from_le_bytes(read_exact::<2>(
            payload,
            "WCHAR const payload",
        )?))),
        other => Err(invalid_bytecode(format!(
            "unsupported const primitive id {other}"
        ))),
    }
}

fn read_exact<const N: usize>(payload: &[u8], kind: &str) -> Result<[u8; N], RuntimeError> {
    if payload.len() != N {
        return Err(invalid_bytecode(format!(
            "invalid {kind} length {}, expected {N}",
            payload.len()
        )));
    }
    let mut out = [0_u8; N];
    out.copy_from_slice(payload);
    Ok(out)
}

fn invalid_bytecode(message: impl Into<SmolStr>) -> RuntimeError {
    RuntimeError::InvalidBytecode(message.into())
}
