fn encode_snapshot(snapshot: &RetainSnapshot) -> Result<Vec<u8>, RuntimeError> {
    let payload = encode_snapshot_payload(snapshot)?;
    let checksum = crc32fast::hash(&payload);
    let mut out = Vec::with_capacity(4 + 2 + 8 + payload.len() + 4 + 4);
    out.extend_from_slice(RETAIN_MAGIC);
    out.extend_from_slice(&RETAIN_VERSION.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&checksum.to_le_bytes());
    out.extend_from_slice(RETAIN_TRAILER);
    Ok(out)
}

fn encode_snapshot_payload(snapshot: &RetainSnapshot) -> Result<Vec<u8>, RuntimeError> {
    let mut out = Vec::new();
    out.extend_from_slice(&(snapshot.values().len() as u32).to_le_bytes());
    for (name, value) in snapshot.values() {
        encode_string(&mut out, name.as_str());
        encode_value(&mut out, value)?;
    }
    Ok(out)
}

fn decode_snapshot(bytes: &[u8]) -> Result<RetainSnapshot, RuntimeError> {
    let mut reader = RetainReader::new(bytes);
    let magic = reader.read_bytes(4)?;
    if magic != RETAIN_MAGIC {
        return Err(RuntimeError::RetainCorruption(
            "invalid retain magic".into(),
        ));
    }
    let version = reader.read_u16()?;
    match version {
        RETAIN_V1_VERSION => {
            let snapshot = decode_snapshot_payload(&mut reader)?;
            reader.expect_finished("retain v1 payload")?;
            Ok(snapshot)
        }
        RETAIN_VERSION => decode_snapshot_v2(&mut reader),
        _ => Err(RuntimeError::RetainCorruption(
            format!("unsupported retain version {version}").into(),
        )),
    }
}

fn decode_snapshot_v2(reader: &mut RetainReader<'_>) -> Result<RetainSnapshot, RuntimeError> {
    let payload_len = reader.read_u64()?;
    let payload_len: usize = payload_len.try_into().map_err(|_| {
        RuntimeError::RetainCorruption("retain payload length exceeds platform size".into())
    })?;
    let payload = reader.read_bytes(payload_len)?;
    let expected_checksum = reader.read_u32()?;
    let trailer = reader.read_bytes(RETAIN_TRAILER.len())?;
    if trailer != RETAIN_TRAILER {
        return Err(RuntimeError::RetainCorruption(
            "invalid retain trailer".into(),
        ));
    }
    reader.expect_finished("retain v2 container")?;
    let actual_checksum = crc32fast::hash(payload);
    if actual_checksum != expected_checksum {
        return Err(RuntimeError::RetainCorruption(
            format!(
                "retain checksum mismatch expected {expected_checksum:#010x} got {actual_checksum:#010x}"
            )
            .into(),
        ));
    }
    let mut payload_reader = RetainReader::new(payload);
    let snapshot = decode_snapshot_payload(&mut payload_reader)?;
    payload_reader.expect_finished("retain v2 payload")?;
    Ok(snapshot)
}

fn decode_snapshot_payload(reader: &mut RetainReader<'_>) -> Result<RetainSnapshot, RuntimeError> {
    let count = reader.read_u32()? as usize;
    let mut values = IndexMap::new();
    for _ in 0..count {
        let name = SmolStr::new(reader.read_string()?);
        let value = decode_value(reader)?;
        values.insert(name, value);
    }
    Ok(RetainSnapshot::from_values(values))
}

#[derive(Debug, Clone, Copy)]
enum ValueTag {
    Bool = 1,
    SInt = 2,
    Int = 3,
    DInt = 4,
    LInt = 5,
    USInt = 6,
    UInt = 7,
    UDInt = 8,
    ULInt = 9,
    Real = 10,
    LReal = 11,
    Byte = 12,
    Word = 13,
    DWord = 14,
    LWord = 15,
    Time = 16,
    LTime = 17,
    Date = 18,
    LDate = 19,
    Tod = 20,
    LTod = 21,
    Dt = 22,
    Ldt = 23,
    String = 24,
    WString = 25,
    Char = 26,
    WChar = 27,
    Array = 28,
    Struct = 29,
    Enum = 30,
    Null = 31,
}

fn encode_value(out: &mut Vec<u8>, value: &Value) -> Result<(), RuntimeError> {
    match value {
        Value::Bool(v) => {
            out.push(ValueTag::Bool as u8);
            out.push(u8::from(*v));
        }
        Value::SInt(v) => {
            out.push(ValueTag::SInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Int(v) => {
            out.push(ValueTag::Int as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::DInt(v) => {
            out.push(ValueTag::DInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::LInt(v) => {
            out.push(ValueTag::LInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::USInt(v) => {
            out.push(ValueTag::USInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::UInt(v) => {
            out.push(ValueTag::UInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::UDInt(v) => {
            out.push(ValueTag::UDInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::ULInt(v) => {
            out.push(ValueTag::ULInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Real(v) => {
            out.push(ValueTag::Real as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::LReal(v) => {
            out.push(ValueTag::LReal as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Byte(v) => {
            out.push(ValueTag::Byte as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Word(v) => {
            out.push(ValueTag::Word as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::DWord(v) => {
            out.push(ValueTag::DWord as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::LWord(v) => {
            out.push(ValueTag::LWord as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Time(v) => {
            out.push(ValueTag::Time as u8);
            out.extend_from_slice(&v.as_nanos().to_le_bytes());
        }
        Value::LTime(v) => {
            out.push(ValueTag::LTime as u8);
            out.extend_from_slice(&v.as_nanos().to_le_bytes());
        }
        Value::Date(v) => {
            out.push(ValueTag::Date as u8);
            out.extend_from_slice(&v.ticks().to_le_bytes());
        }
        Value::LDate(v) => {
            out.push(ValueTag::LDate as u8);
            out.extend_from_slice(&v.nanos().to_le_bytes());
        }
        Value::Tod(v) => {
            out.push(ValueTag::Tod as u8);
            out.extend_from_slice(&v.ticks().to_le_bytes());
        }
        Value::LTod(v) => {
            out.push(ValueTag::LTod as u8);
            out.extend_from_slice(&v.nanos().to_le_bytes());
        }
        Value::Dt(v) => {
            out.push(ValueTag::Dt as u8);
            out.extend_from_slice(&v.ticks().to_le_bytes());
        }
        Value::Ldt(v) => {
            out.push(ValueTag::Ldt as u8);
            out.extend_from_slice(&v.nanos().to_le_bytes());
        }
        Value::String(v) => {
            out.push(ValueTag::String as u8);
            encode_string(out, v.as_str());
        }
        Value::WString(v) => {
            out.push(ValueTag::WString as u8);
            encode_string(out, v);
        }
        Value::Char(v) => {
            out.push(ValueTag::Char as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::WChar(v) => {
            out.push(ValueTag::WChar as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Array(array) => {
            out.push(ValueTag::Array as u8);
            out.extend_from_slice(&(array.elements().len() as u32).to_le_bytes());
            out.extend_from_slice(&(array.dimensions().len() as u32).to_le_bytes());
            for (lower, upper) in array.dimensions() {
                out.extend_from_slice(&lower.to_le_bytes());
                out.extend_from_slice(&upper.to_le_bytes());
            }
            for element in array.elements() {
                encode_value(out, element)?;
            }
        }
        Value::Struct(struct_value) => {
            out.push(ValueTag::Struct as u8);
            encode_string(out, struct_value.type_name().as_str());
            out.extend_from_slice(&(struct_value.fields().len() as u32).to_le_bytes());
            for (name, field) in struct_value.fields() {
                encode_string(out, name.as_str());
                encode_value(out, field)?;
            }
        }
        Value::Enum(enum_value) => {
            out.push(ValueTag::Enum as u8);
            encode_string(out, enum_value.type_name().as_str());
            encode_string(out, enum_value.variant_name().as_str());
            out.extend_from_slice(&enum_value.numeric_value().to_le_bytes());
        }
        Value::Null => {
            out.push(ValueTag::Null as u8);
        }
        Value::Reference(_) | Value::Instance(_) => {
            return Err(RuntimeError::RetainStore(
                "cannot retain reference/instance values".into(),
            ));
        }
    }
    Ok(())
}

fn decode_value(reader: &mut RetainReader<'_>) -> Result<Value, RuntimeError> {
    let tag = reader.read_u8()?;
    let value = match tag {
        x if x == ValueTag::Bool as u8 => Value::Bool(reader.read_u8()? != 0),
        x if x == ValueTag::SInt as u8 => Value::SInt(reader.read_i8()?),
        x if x == ValueTag::Int as u8 => Value::Int(reader.read_i16()?),
        x if x == ValueTag::DInt as u8 => Value::DInt(reader.read_i32()?),
        x if x == ValueTag::LInt as u8 => Value::LInt(reader.read_i64()?),
        x if x == ValueTag::USInt as u8 => Value::USInt(reader.read_u8()?),
        x if x == ValueTag::UInt as u8 => Value::UInt(reader.read_u16()?),
        x if x == ValueTag::UDInt as u8 => Value::UDInt(reader.read_u32()?),
        x if x == ValueTag::ULInt as u8 => Value::ULInt(reader.read_u64()?),
        x if x == ValueTag::Real as u8 => Value::Real(reader.read_f32()?),
        x if x == ValueTag::LReal as u8 => Value::LReal(reader.read_f64()?),
        x if x == ValueTag::Byte as u8 => Value::Byte(reader.read_u8()?),
        x if x == ValueTag::Word as u8 => Value::Word(reader.read_u16()?),
        x if x == ValueTag::DWord as u8 => Value::DWord(reader.read_u32()?),
        x if x == ValueTag::LWord as u8 => Value::LWord(reader.read_u64()?),
        x if x == ValueTag::Time as u8 => Value::Time(Duration::from_nanos(reader.read_i64()?)),
        x if x == ValueTag::LTime as u8 => Value::LTime(Duration::from_nanos(reader.read_i64()?)),
        x if x == ValueTag::Date as u8 => Value::Date(DateValue::new(reader.read_i64()?)),
        x if x == ValueTag::LDate as u8 => Value::LDate(LDateValue::new(reader.read_i64()?)),
        x if x == ValueTag::Tod as u8 => Value::Tod(TimeOfDayValue::new(reader.read_i64()?)),
        x if x == ValueTag::LTod as u8 => Value::LTod(LTimeOfDayValue::new(reader.read_i64()?)),
        x if x == ValueTag::Dt as u8 => Value::Dt(DateTimeValue::new(reader.read_i64()?)),
        x if x == ValueTag::Ldt as u8 => Value::Ldt(LDateTimeValue::new(reader.read_i64()?)),
        x if x == ValueTag::String as u8 => Value::String(SmolStr::new(reader.read_string()?)),
        x if x == ValueTag::WString as u8 => Value::WString(reader.read_string()?),
        x if x == ValueTag::Char as u8 => Value::Char(reader.read_u8()?),
        x if x == ValueTag::WChar as u8 => Value::WChar(reader.read_u16()?),
        x if x == ValueTag::Array as u8 => {
            let len = reader.read_u32()? as usize;
            let dims = reader.read_u32()? as usize;
            let mut dimensions = Vec::with_capacity(dims);
            for _ in 0..dims {
                dimensions.push((reader.read_i64()?, reader.read_i64()?));
            }
            let mut elements = Vec::with_capacity(len);
            for _ in 0..len {
                elements.push(decode_value(reader)?);
            }
            Value::Array(Box::new(
                ArrayValue::from_untyped_parts(elements, dimensions).map_err(|error| {
                    RuntimeError::RetainStore(format!("invalid retained array value: {error}").into())
                })?,
            ))
        }
        x if x == ValueTag::Struct as u8 => {
            let type_name = SmolStr::new(reader.read_string()?);
            let count = reader.read_u32()? as usize;
            let mut fields = IndexMap::new();
            for _ in 0..count {
                let name = SmolStr::new(reader.read_string()?);
                let value = decode_value(reader)?;
                fields.insert(name, value);
            }
            Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                type_name, fields,
            )))
        }
        x if x == ValueTag::Enum as u8 => {
            let type_name = SmolStr::new(reader.read_string()?);
            let variant_name = SmolStr::new(reader.read_string()?);
            let numeric_value = reader.read_i64()?;
            Value::Enum(Box::new(EnumValue::from_canonical_parts(
                type_name,
                variant_name,
                numeric_value,
            )))
        }
        x if x == ValueTag::Null as u8 => Value::Null,
        _ => return Err(RuntimeError::RetainCorruption("unknown retain value tag".into())),
    };
    Ok(value)
}
