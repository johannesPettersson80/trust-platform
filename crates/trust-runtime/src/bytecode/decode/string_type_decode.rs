fn decode_string_table(
    version: BytecodeVersion,
    reader: &mut BytecodeReader<'_>,
    context: &str,
) -> Result<StringTable, BytecodeError> {
    let count = read_bounded_count(reader, 4, context)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let len = reader.read_u32()? as usize;
        let bytes = reader.read_bytes(len)?;
        let string = std::str::from_utf8(bytes)
            .map_err(|_| BytecodeError::InvalidSection("invalid utf-8".into()))?;
        entries.push(SmolStr::new(string));
        if version.minor >= 1 {
            let entry_len = 4usize + len;
            let padded = align4(entry_len);
            let padding = padded.saturating_sub(entry_len);
            if padding > 0 {
                reader.read_bytes(padding)?;
            }
        }
    }
    Ok(StringTable { entries })
}

fn decode_type_table(version: BytecodeVersion, payload: &[u8]) -> Result<TypeTable, BytecodeError> {
    let mut reader = BytecodeReader::new(payload);
    let minimum_entry_bytes = if version.minor >= 1 { 4 } else { 12 };
    let count = read_bounded_count(&mut reader, minimum_entry_bytes, "TYPE_TABLE")?;
    if version.minor >= 1 {
        let mut offsets = Vec::with_capacity(count);
        for _ in 0..count {
            offsets.push(reader.read_u32()?);
        }
        let base = reader.pos();
        let mut entries = Vec::with_capacity(count);
        for (idx, offset) in offsets.iter().enumerate() {
            let offset = *offset as usize;
            let next = if idx + 1 < offsets.len() {
                offsets[idx + 1] as usize
            } else {
                payload.len()
            };
            validate_type_range(payload.len(), base, idx, offset, next, &offsets)?;

            let mut entry_reader = BytecodeReader::new(&payload[offset..next]);
            let entry = decode_type_entry(&mut entry_reader)?;
            if entry_reader.remaining() != 0 {
                return Err(BytecodeError::InvalidSection(
                    "type entry length mismatch".into(),
                ));
            }
            entries.push(entry);
        }
        Ok(TypeTable { offsets, entries })
    } else {
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(decode_type_entry(&mut reader)?);
        }
        Ok(TypeTable {
            offsets: Vec::new(),
            entries,
        })
    }
}

fn validate_type_range(
    payload_len: usize,
    base: usize,
    idx: usize,
    offset: usize,
    next: usize,
    offsets: &[u32],
) -> Result<(), BytecodeError> {
    if offset < base || offset > payload_len || next > payload_len || next < offset {
        return Err(BytecodeError::InvalidSection(
            "type table offset out of bounds".into(),
        ));
    }
    if idx > 0 && offset < offsets[idx - 1] as usize {
        return Err(BytecodeError::InvalidSection(
            "type table offsets not sorted".into(),
        ));
    }
    Ok(())
}

fn decode_type_entry(reader: &mut BytecodeReader<'_>) -> Result<TypeEntry, BytecodeError> {
    let kind = reader.read_u8()?;
    let _flags = reader.read_u8()?;
    let _reserved = reader.read_u16()?;
    let name_idx = optional_u32(reader.read_u32()?);
    let kind = TypeKind::from_raw(kind)
        .ok_or_else(|| BytecodeError::InvalidSection("invalid type kind".into()))?;
    let data = match kind {
        TypeKind::Primitive => {
            let prim_id = reader.read_u16()?;
            let max_length = reader.read_u16()?;
            TypeData::Primitive {
                prim_id,
                max_length,
            }
        }
        TypeKind::Array => {
            let elem_type_id = reader.read_u32()?;
            let dim_count = read_bounded_count(reader, 16, "TYPE_TABLE array dimension")?;
            let mut dims = Vec::with_capacity(dim_count);
            for _ in 0..dim_count {
                let lower = reader.read_i64()?;
                let upper = reader.read_i64()?;
                dims.push((lower, upper));
            }
            TypeData::Array { elem_type_id, dims }
        }
        TypeKind::Struct => {
            let field_count = read_bounded_count(reader, 8, "TYPE_TABLE struct field")?;
            let mut fields = Vec::with_capacity(field_count);
            for _ in 0..field_count {
                let name_idx = reader.read_u32()?;
                let type_id = reader.read_u32()?;
                fields.push(Field { name_idx, type_id });
            }
            TypeData::Struct { fields }
        }
        TypeKind::Enum => {
            let base_type_id = reader.read_u32()?;
            let variant_count = read_bounded_count(reader, 12, "TYPE_TABLE enum variant")?;
            let mut variants = Vec::with_capacity(variant_count);
            for _ in 0..variant_count {
                let name_idx = reader.read_u32()?;
                let value = reader.read_i64()?;
                variants.push(EnumVariant { name_idx, value });
            }
            TypeData::Enum {
                base_type_id,
                variants,
            }
        }
        TypeKind::Alias => {
            let target_type_id = reader.read_u32()?;
            TypeData::Alias { target_type_id }
        }
        TypeKind::Subrange => {
            let base_type_id = reader.read_u32()?;
            let lower = reader.read_i64()?;
            let upper = reader.read_i64()?;
            TypeData::Subrange {
                base_type_id,
                lower,
                upper,
            }
        }
        TypeKind::Reference => {
            let target_type_id = reader.read_u32()?;
            TypeData::Reference { target_type_id }
        }
        TypeKind::Union => {
            let field_count = read_bounded_count(reader, 8, "TYPE_TABLE union field")?;
            let mut fields = Vec::with_capacity(field_count);
            for _ in 0..field_count {
                let name_idx = reader.read_u32()?;
                let type_id = reader.read_u32()?;
                fields.push(Field { name_idx, type_id });
            }
            TypeData::Union { fields }
        }
        TypeKind::FunctionBlock | TypeKind::Class => {
            let pou_id = reader.read_u32()?;
            TypeData::Pou { pou_id }
        }
        TypeKind::Interface => {
            let method_count = read_bounded_count(reader, 8, "TYPE_TABLE interface method")?;
            let mut methods = Vec::with_capacity(method_count);
            for _ in 0..method_count {
                let name_idx = reader.read_u32()?;
                let slot = reader.read_u32()?;
                methods.push(InterfaceMethod { name_idx, slot });
            }
            TypeData::Interface { methods }
        }
    };
    Ok(TypeEntry {
        kind,
        name_idx,
        data,
    })
}
