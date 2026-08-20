fn validate_string_table(_strings: &StringTable) -> Result<(), BytecodeError> {
    Ok(())
}

fn validate_type_table(strings: &StringTable, types: &TypeTable) -> Result<(), BytecodeError> {
    for entry in &types.entries {
        if let Some(name_idx) = entry.name_idx {
            ensure_string_index(strings, name_idx)?;
        }
        match &entry.data {
            TypeData::Array { elem_type_id, dims } => {
                ensure_type_index(types, *elem_type_id)?;
                for (lower, upper) in dims {
                    if lower > upper {
                        return Err(BytecodeError::InvalidSection("invalid array bounds".into()));
                    }
                }
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                for field in fields {
                    ensure_string_index(strings, field.name_idx)?;
                    ensure_type_index(types, field.type_id)?;
                }
            }
            TypeData::Enum {
                base_type_id,
                variants,
            } => {
                ensure_type_index(types, *base_type_id)?;
                for variant in variants {
                    ensure_string_index(strings, variant.name_idx)?;
                }
            }
            TypeData::Alias { target_type_id }
            | TypeData::Subrange {
                base_type_id: target_type_id,
                ..
            }
            | TypeData::Reference { target_type_id } => {
                ensure_type_index(types, *target_type_id)?;
            }
            TypeData::Pou { .. } => {}
            TypeData::Interface { methods } => {
                for method in methods {
                    ensure_string_index(strings, method.name_idx)?;
                }
            }
            TypeData::Primitive { .. } => {}
        }
    }
    Ok(())
}

fn validate_const_pool(
    _strings: &StringTable,
    types: &TypeTable,
    pool: &ConstPool,
) -> Result<(), BytecodeError> {
    for entry in &pool.entries {
        validate_const_payload(types, entry)?;
    }
    Ok(())
}

fn validate_const_payload(types: &TypeTable, entry: &ConstEntry) -> Result<(), BytecodeError> {
    let type_id = entry.type_id;
    let payload = &entry.payload;
    let entry = types
        .entries
        .get(type_id as usize)
        .ok_or_else(|| BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: type_id,
        })?;
    let mut reader = BytecodeReader::new(payload);
    validate_const_payload_entry(types, entry, &mut reader, 0)?;
    if reader.remaining() != 0 {
        return Err(BytecodeError::InvalidSection("const payload length".into()));
    }
    Ok(())
}

fn validate_const_payload_entry(
    types: &TypeTable,
    entry: &TypeEntry,
    reader: &mut BytecodeReader<'_>,
    depth: u8,
) -> Result<(), BytecodeError> {
    if depth > crate::bytecode::BYTECODE_MAX_CONST_NESTING {
        return Err(BytecodeError::InvalidSection(
            "const type recursion overflow".into(),
        ));
    }
    match &entry.data {
        TypeData::Primitive { prim_id, .. } => match prim_id {
            1 => {
                reader.read_u8()?;
            }
            2 | 6 | 10 | 26 => {
                reader.read_u8()?;
            }
            3 | 7 | 11 | 27 => {
                reader.read_u16()?;
            }
            4 | 8 | 12 => {
                reader.read_u32()?;
            }
            5 | 9 | 13 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 => {
                reader.read_u64()?;
            }
            14 => {
                reader.read_u32()?;
            }
            24 => {
                let payload = reader.read_bytes(reader.remaining())?;
                std::str::from_utf8(payload).map_err(|err| {
                    BytecodeError::InvalidSection(
                        format!("invalid STRING const UTF-8: {err}").into(),
                    )
                })?;
            }
            25 => {
                let payload = reader.read_bytes(reader.remaining())?;
                if payload.len() % 2 != 0 {
                    return Err(BytecodeError::InvalidSection(
                        "invalid WSTRING const payload length".into(),
                    ));
                }
                let units = payload
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect::<Vec<_>>();
                String::from_utf16(&units).map_err(|err| {
                    BytecodeError::InvalidSection(
                        format!("invalid WSTRING const UTF-16: {err}").into(),
                    )
                })?;
            }
            _ => {
                return Err(BytecodeError::InvalidSection("unknown primitive".into()));
            }
        },
        TypeData::Array { elem_type_id, dims } => {
            let count = reader.read_u32()? as usize;
            let expected = const_array_element_count(dims)?;
            if count != expected {
                return Err(BytecodeError::InvalidSection(
                    format!(
                        "array constant count mismatch: expected {expected}, got {count}"
                    )
                    .into(),
                ));
            }
            let elem = types.entries.get(*elem_type_id as usize).ok_or_else(|| {
                BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: *elem_type_id,
                }
            })?;
            for _ in 0..count {
                validate_const_child_payload(types, elem, reader, depth + 1)?;
            }
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => {
            let count = reader.read_u32()? as usize;
            if count != fields.len() {
                return Err(BytecodeError::InvalidSection(
                    "struct/union constant count mismatch".into(),
                ));
            }
            for field in fields {
                let field_type = types.entries.get(field.type_id as usize).ok_or_else(|| {
                    BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: field.type_id,
                }
            })?;
                validate_const_child_payload(types, field_type, reader, depth + 1)?;
            }
        }
        TypeData::Enum { .. } => {
            reader.read_i64()?;
        }
        TypeData::Alias { target_type_id } => {
            let target = types.entries.get(*target_type_id as usize).ok_or_else(|| {
                BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: *target_type_id,
                }
            })?;
            validate_const_payload_entry(types, target, reader, depth + 1)?;
        }
        TypeData::Subrange { base_type_id, .. } => {
            let base = types.entries.get(*base_type_id as usize).ok_or_else(|| {
                BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: *base_type_id,
                }
            })?;
            validate_const_payload_entry(types, base, reader, depth + 1)?;
        }
        TypeData::Reference { .. } => {
            if reader.read_u32()? != u32::MAX {
                return Err(BytecodeError::InvalidSection(
                    "REFERENCE const payload must encode NULL".into(),
                ));
            }
        }
        _ => {
            return Err(BytecodeError::InvalidSection(
                "unsupported const type".into(),
            ));
        }
    }
    Ok(())
}

fn validate_const_child_payload(
    types: &TypeTable,
    entry: &TypeEntry,
    reader: &mut BytecodeReader<'_>,
    depth: u8,
) -> Result<(), BytecodeError> {
    let len = reader.read_u32()? as usize;
    let payload = reader.read_bytes(len)?;
    let mut child = BytecodeReader::new(payload);
    validate_const_payload_entry(types, entry, &mut child, depth)?;
    if child.remaining() != 0 {
        return Err(BytecodeError::InvalidSection(
            "const child payload length".into(),
        ));
    }
    Ok(())
}

fn const_array_element_count(dims: &[(i64, i64)]) -> Result<usize, BytecodeError> {
    if dims
        .iter()
        .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX)
    {
        return Ok(0);
    }
    let mut count = 1_i128;
    for (lower, upper) in dims {
        if lower > upper {
            return Err(BytecodeError::InvalidSection("invalid array bounds".into()));
        }
        count = count
            .checked_mul(i128::from(*upper) - i128::from(*lower) + 1)
            .ok_or_else(|| BytecodeError::InvalidSection("array constant size overflow".into()))?;
    }
    usize::try_from(count)
        .map_err(|_| BytecodeError::InvalidSection("array constant size overflow".into()))
}

fn validate_ref_table(strings: &StringTable, table: &RefTable) -> Result<(), BytecodeError> {
    for entry in &table.entries {
        for segment in &entry.segments {
            if let RefSegment::Field { name_idx } = segment {
                ensure_string_index(strings, *name_idx)?;
            }
        }
    }
    Ok(())
}
