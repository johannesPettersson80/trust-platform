use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};

use crate::collections::OrderedMap as IndexMap;
use smol_str::SmolStr;

use crate::bytecode::{ConstPool, StringTable, TypeData, TypeEntry, TypeTable};
use crate::error::{PreparationDiagnostic, RuntimeError};
use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};

/// Decode every bytecode constant-pool entry into runtime core values.
#[cfg(any(feature = "hir", test))]
pub fn decode_const_pool_entries(
    const_pool: &ConstPool,
    types: &TypeTable,
    strings: &StringTable,
) -> Result<Vec<Value>, RuntimeError> {
    decode_const_pool_entries_charged(const_pool, types, strings, &mut |_, _| Ok(()))
}

pub(crate) fn decode_const_pool_entries_charged(
    const_pool: &ConstPool,
    types: &TypeTable,
    strings: &StringTable,
    charge: &mut impl FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<Value>, RuntimeError> {
    charge(
        const_pool
            .entries
            .len()
            .checked_mul(core::mem::size_of::<Value>())
            .ok_or(RuntimeError::Overflow)?,
        const_pool.entries.len(),
    )?;
    let mut out = Vec::with_capacity(const_pool.entries.len());
    for entry in &const_pool.entries {
        out.push(decode_const_payload(
            types,
            strings,
            entry.type_id,
            &entry.payload,
            0,
            charge,
        )?);
    }
    Ok(out)
}

fn const_type_entry(types: &TypeTable, type_id: u32) -> Result<&TypeEntry, RuntimeError> {
    types.entries.get(type_id as usize).ok_or_else(|| {
        PreparationDiagnostic::InvalidIndex {
            kind: "const type",
            index: type_id,
        }
        .into_runtime_error()
    })
}

fn decode_const_payload(
    types: &TypeTable,
    strings: &StringTable,
    type_id: u32,
    payload: &[u8],
    depth: u8,
    charge: &mut impl FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Value, RuntimeError> {
    charge(core::mem::size_of::<Value>(), 1)?;
    if depth > crate::bytecode::BYTECODE_MAX_CONST_NESTING {
        return Err(invalid_bytecode("const type recursion overflow"));
    }
    let entry = const_type_entry(types, type_id)?;
    match &entry.data {
        TypeData::Primitive { prim_id, .. } => {
            charge(
                payload.len().checked_mul(4).ok_or(RuntimeError::Overflow)?,
                payload.len(),
            )?;
            decode_primitive_constant(*prim_id, payload)
        }
        TypeData::Enum { variants, .. } => {
            let bytes = read_exact::<8>(payload, "enum const payload")?;
            let numeric_value = i64::from_le_bytes(bytes);
            let enum_name = string_at(strings, entry.name_idx, "enum const type name")?;
            charge(0, variants.len())?;
            let variant = variants
                .iter()
                .find(|variant| variant.value == numeric_value)
                .ok_or_else(|| invalid_bytecode("enum const variant value missing"))?;
            let variant_name =
                string_at(strings, Some(variant.name_idx), "enum const variant name")?;
            charge(core::mem::size_of::<EnumValue>(), 1)?;
            Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
                enum_name,
                variant_name,
                numeric_value,
            ))))
        }
        TypeData::Alias { target_type_id } => {
            decode_const_payload(types, strings, *target_type_id, payload, depth + 1, charge)
        }
        TypeData::Subrange { base_type_id, .. } => {
            decode_const_payload(types, strings, *base_type_id, payload, depth + 1, charge)
        }
        TypeData::Array { elem_type_id, dims } => {
            let mut reader = ConstPayloadReader::new(payload);
            let count = reader.read_u32("ARRAY const element count")? as usize;
            let expected = array_element_count(dims)?;
            if count != expected {
                return Err(PreparationDiagnostic::ConstantCount {
                    kind: "ARRAY const element count",
                    expected,
                    got: count,
                }
                .into_runtime_error());
            }
            charge(
                count
                    .checked_mul(core::mem::size_of::<Value>())
                    .and_then(|n| {
                        n.checked_add(dims.len().checked_mul(core::mem::size_of::<(i64, i64)>())?)
                    })
                    .and_then(|n| n.checked_add(core::mem::size_of::<ArrayValue>()))
                    .ok_or(RuntimeError::Overflow)?,
                count,
            )?;
            let mut elements = Vec::with_capacity(count);
            for _ in 0..count {
                let child = reader.read_child("ARRAY const element")?;
                elements.push(decode_const_payload(
                    types,
                    strings,
                    *elem_type_id,
                    child,
                    depth + 1,
                    charge,
                )?);
            }
            reader.finish("ARRAY const payload")?;
            Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
                elements,
                dims.clone(),
            ))))
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => {
            let mut reader = ConstPayloadReader::new(payload);
            let count = reader.read_u32("struct/union const field count")? as usize;
            if count != fields.len() {
                return Err(PreparationDiagnostic::ConstantCount {
                    kind: "struct/union const field count",
                    expected: fields.len(),
                    got: count,
                }
                .into_runtime_error());
            }
            charge(
                fields
                    .len()
                    .checked_mul(
                        4 * (core::mem::size_of::<Value>()
                            + core::mem::size_of::<SmolStr>()
                            + core::mem::size_of::<usize>()),
                    )
                    .and_then(|n| n.checked_add(core::mem::size_of::<StructValue>()))
                    .ok_or(RuntimeError::Overflow)?,
                fields.len(),
            )?;
            let mut values = IndexMap::with_capacity_and_hasher(fields.len(), Default::default());
            for field in fields {
                let name = string_at(strings, Some(field.name_idx), "const field name")?;
                let child = reader.read_child("struct/union const field")?;
                let value =
                    decode_const_payload(types, strings, field.type_id, child, depth + 1, charge)?;
                values.insert(name, value);
            }
            reader.finish("struct/union const payload")?;
            let type_name = string_at(strings, entry.name_idx, "struct/union const type name")?;
            Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
                type_name, values,
            ))))
        }
        TypeData::Reference { .. } => {
            let reference =
                u32::from_le_bytes(read_exact::<4>(payload, "REFERENCE const payload")?);
            if reference == u32::MAX {
                Ok(Value::Reference(None))
            } else {
                Err(invalid_bytecode(
                    "non-NULL REFERENCE const cannot be materialized without REF_TABLE context",
                ))
            }
        }
        _ => Err(PreparationDiagnostic::UnsupportedConstantType(type_id).into_runtime_error()),
    }
}

fn string_at(
    strings: &StringTable,
    index: Option<u32>,
    kind: &'static str,
) -> Result<SmolStr, RuntimeError> {
    let index =
        index.ok_or_else(|| PreparationDiagnostic::MissingString { kind }.into_runtime_error())?;
    strings.entries.get(index as usize).cloned().ok_or_else(|| {
        PreparationDiagnostic::StringIndexOutOfBounds { kind, index }.into_runtime_error()
    })
}

fn array_element_count(dims: &[(i64, i64)]) -> Result<usize, RuntimeError> {
    if dims
        .iter()
        .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX)
    {
        return Ok(0);
    }
    let mut count = 1_i128;
    for (lower, upper) in dims {
        if lower > upper {
            return Err(invalid_bytecode("invalid ARRAY const bounds"));
        }
        count = count
            .checked_mul(i128::from(*upper) - i128::from(*lower) + 1)
            .ok_or_else(|| invalid_bytecode("ARRAY const element count overflow"))?;
    }
    usize::try_from(count).map_err(|_| invalid_bytecode("ARRAY const element count overflow"))
}

struct ConstPayloadReader<'a> {
    remaining: &'a [u8],
}

impl<'a> ConstPayloadReader<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { remaining: payload }
    }

    fn read_u32(&mut self, kind: &'static str) -> Result<u32, RuntimeError> {
        let bytes = self.take(4, kind, false)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_child(&mut self, kind: &'static str) -> Result<&'a [u8], RuntimeError> {
        let bytes = self.take(4, kind, true)?;
        let len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        self.take(len, kind, false)
    }

    fn take(
        &mut self,
        len: usize,
        kind: &'static str,
        length: bool,
    ) -> Result<&'a [u8], RuntimeError> {
        if self.remaining.len() < len {
            return Err(PreparationDiagnostic::ConstantTruncated {
                kind,
                length,
                need: len,
                have: self.remaining.len(),
            }
            .into_runtime_error());
        }
        let (value, remaining) = self.remaining.split_at(len);
        self.remaining = remaining;
        Ok(value)
    }

    fn finish(self, kind: &'static str) -> Result<(), RuntimeError> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(PreparationDiagnostic::ConstantTrailing {
                kind,
                remaining: self.remaining.len(),
            }
            .into_runtime_error())
        }
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
                .map_err(|err| PreparationDiagnostic::ConstantUtf8(err).into_runtime_error())?;
            Ok(Value::String(SmolStr::new(text)))
        }
        25 => {
            if !payload.len().is_multiple_of(2) {
                return Err(invalid_bytecode("invalid WSTRING const payload length"));
            }
            let (units, remainder) = payload.as_chunks::<2>();
            debug_assert!(remainder.is_empty());
            let units = units
                .iter()
                .map(|unit| u16::from_le_bytes(*unit))
                .collect::<Vec<_>>();
            let text = String::from_utf16(&units)
                .map_err(|_| PreparationDiagnostic::ConstantUtf16.into_runtime_error())?;
            Ok(Value::WString(text))
        }
        26 => Ok(Value::Char(
            read_exact::<1>(payload, "CHAR const payload")?[0],
        )),
        27 => Ok(Value::WChar(u16::from_le_bytes(read_exact::<2>(
            payload,
            "WCHAR const payload",
        )?))),
        other => Err(PreparationDiagnostic::UnsupportedPrimitive(other).into_runtime_error()),
    }
}

fn read_exact<const N: usize>(payload: &[u8], kind: &'static str) -> Result<[u8; N], RuntimeError> {
    if payload.len() != N {
        return Err(PreparationDiagnostic::ConstantLength {
            kind,
            actual: payload.len(),
            expected: N,
        }
        .into_runtime_error());
    }
    let mut out = [0_u8; N];
    out.copy_from_slice(payload);
    Ok(out)
}

fn invalid_bytecode(message: &'static str) -> RuntimeError {
    RuntimeError::bytecode(
        crate::error::StableErrorCode::VmBytecodeDecode,
        SmolStr::new_static(message),
    )
}

#[cfg(test)]
mod preparation_budget_tests {
    use super::*;
    use crate::bytecode::{ConstEntry, TypeKind};
    use alloc::vec;
    #[test]
    fn child_payload_length_and_body_failures_keep_distinct_context() {
        let bytes = [2, 0, 0, 0, 7, 8];
        let mut reader = ConstPayloadReader::new(&bytes);
        assert_eq!(reader.read_child("ARRAY const element").unwrap(), &[7, 8]);
        reader.finish("ARRAY const payload").unwrap();

        let error = ConstPayloadReader::new(&[2, 0])
            .read_child("ARRAY const element")
            .unwrap_err();
        assert_eq!(
            error,
            PreparationDiagnostic::ConstantTruncated {
                kind: "ARRAY const element",
                length: true,
                need: 4,
                have: 2,
            }
            .into_runtime_error()
        );
        let error = ConstPayloadReader::new(&[2, 0, 0, 0, 7])
            .read_child("ARRAY const element")
            .unwrap_err();
        assert_eq!(
            error,
            PreparationDiagnostic::ConstantTruncated {
                kind: "ARRAY const element",
                length: false,
                need: 2,
                have: 1,
            }
            .into_runtime_error()
        );
    }

    #[test]
    fn invalid_text_constants_preserve_encoding_error_details() {
        let bytes = alloc::vec![0xff];
        let utf8 = core::str::from_utf8(&bytes).unwrap_err();
        assert_eq!(
            decode_primitive_constant(24, &bytes).unwrap_err(),
            PreparationDiagnostic::ConstantUtf8(utf8).into_runtime_error()
        );
        assert_eq!(
            decode_primitive_constant(25, &0xd800_u16.to_le_bytes()).unwrap_err(),
            PreparationDiagnostic::ConstantUtf16.into_runtime_error()
        );
    }

    #[test]
    fn array_reservation_is_charged_before_child_payload_expansion() {
        let types = TypeTable {
            offsets: vec![],
            entries: vec![
                TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: 7,
                        max_length: 0,
                    },
                },
                TypeEntry {
                    kind: TypeKind::Array,
                    name_idx: None,
                    data: TypeData::Array {
                        elem_type_id: 0,
                        dims: vec![(1, 1024)],
                    },
                },
            ],
        };
        // The declared count is valid, but children are deliberately absent.
        // A rejected reservation must win before the decoder visits a child.
        let constants = ConstPool {
            entries: vec![ConstEntry {
                type_id: 1,
                payload: 1024u32.to_le_bytes().to_vec(),
            }],
        };
        let mut charges = Vec::new();
        let error = decode_const_pool_entries_charged(
            &constants,
            &types,
            &StringTable::default(),
            &mut |bytes, work| {
                charges.push((bytes, work));
                if bytes > 1024 {
                    Err(crate::vm::VmTrap::BudgetExceeded.into_runtime_error())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            crate::vm::VmTrap::BudgetExceeded.into_runtime_error()
        );
        assert_eq!(
            charges.len(),
            3,
            "pool, root, then complete array reservation"
        );
    }
}
