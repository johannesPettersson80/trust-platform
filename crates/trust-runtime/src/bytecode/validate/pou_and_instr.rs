fn validate_pou_index(
    strings: &StringTable,
    types: &TypeTable,
    const_pool: &ConstPool,
    ref_table: &RefTable,
    index: &PouIndex,
    bodies: &[u8],
) -> Result<(), BytecodeError> {
    let mut seen_pou_ids = HashSet::new();
    for entry in &index.entries {
        if !seen_pou_ids.insert(entry.id) {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate POU id {}", entry.id).into(),
            ));
        }
        validate_pou_local_ref_range(ref_table, entry)?;
        ensure_string_index(strings, entry.name_idx)?;
        if let Some(return_type_id) = entry.return_type_id {
            ensure_type_index(types, return_type_id)?;
        }
        if let Some(owner) = entry.owner_pou_id {
            if !index.entries.iter().any(|pou| pou.id == owner) {
                return Err(BytecodeError::InvalidPouId(owner));
            }
        }
        for param in &entry.params {
            ensure_string_index(strings, param.name_idx)?;
            ensure_type_index(types, param.type_id)?;
            if let Some(default_idx) = param.default_const_idx {
                ensure_const_index(const_pool, default_idx)?;
            }
        }
        if let Some(meta) = &entry.class_meta {
            if let Some(parent) = meta.parent_pou_id {
                if !index.entries.iter().any(|pou| pou.id == parent) {
                    return Err(BytecodeError::InvalidPouId(parent));
                }
            }
            for interface in &meta.interfaces {
                ensure_type_index(types, interface.interface_type_id)?;
                let interface_entry = types
                    .entries
                    .get(interface.interface_type_id as usize)
                    .ok_or_else(|| BytecodeError::InvalidIndex {
                        kind: "type".into(),
                        index: interface.interface_type_id,
                    })?;
                if !matches!(interface_entry.kind, TypeKind::Interface) {
                    return Err(BytecodeError::InvalidSection(
                        "interface mapping expects interface type".into(),
                    ));
                }
                if let TypeData::Interface { methods } = &interface_entry.data {
                    if interface.vtable_slots.len() != methods.len() {
                        return Err(BytecodeError::InvalidSection(
                            "interface mapping slot mismatch".into(),
                        ));
                    }
                }
            }
            for method in &meta.methods {
                ensure_string_index(strings, method.name_idx)?;
                if !index.entries.iter().any(|pou| pou.id == method.pou_id) {
                    return Err(BytecodeError::InvalidPouId(method.pou_id));
                }
            }
        }
        let start = entry.code_offset as usize;
        let end = start + entry.code_length as usize;
        if end > bodies.len() {
            return Err(BytecodeError::InvalidSection(
                "POU code out of bounds".into(),
            ));
        }
        let tables = InstructionValidationTables {
            strings,
            index,
            types,
            const_pool,
            ref_table,
        };
        validate_instruction_stream(&tables, entry, &bodies[start..end])?;
    }
    Ok(())
}

struct InstructionValidationTables<'a> {
    strings: &'a StringTable,
    index: &'a PouIndex,
    types: &'a TypeTable,
    const_pool: &'a ConstPool,
    ref_table: &'a RefTable,
}

fn validate_instruction_stream(
    tables: &InstructionValidationTables<'_>,
    pou: &PouEntry,
    code: &[u8],
) -> Result<(), BytecodeError> {
    let mut reader = BytecodeReader::new(code);
    let mut starts = Vec::new();
    let mut jumps = Vec::new();
    while reader.remaining() > 0 {
        let pc = reader.pos();
        starts.push(pc as i32);
        let opcode = reader.read_u8()?;
        if let Some(name) = unsupported_runtime_opcode_name(opcode) {
            return Err(BytecodeError::InvalidSection(
                format!("unsupported runtime opcode {name} (0x{opcode:02X})").into(),
            ));
        }
        match opcode {
            0x00 | 0x01 | 0x06 | 0x11 | 0x12 | 0x13 | 0x25 | 0x31 | 0x32 | 0x33 | 0x40
            | 0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x48 | 0x49 | 0x4C | 0x50
            | 0x51 | 0x52 | 0x53 | 0x54 | 0x55 => {}
            0x02..=0x04 => {
                let offset = reader.read_i32()?;
                jumps.push((pc as i32, offset));
            }
            0x05 => {
                let pou_id = reader.read_u32()?;
                if !tables.index.entries.iter().any(|pou| pou.id == pou_id) {
                    return Err(BytecodeError::InvalidPouId(pou_id));
                }
            }
            0x07 => {
                reader.read_u32()?; // vtable slot
            }
            0x08 => {
                let interface_type_id = reader.read_u32()?;
                let slot = reader.read_u32()?;
                let entry = tables
                    .types
                    .entries
                    .get(interface_type_id as usize)
                    .ok_or_else(|| BytecodeError::InvalidIndex {
                        kind: "type".into(),
                        index: interface_type_id,
                    })?;
                if !matches!(entry.kind, TypeKind::Interface) {
                    return Err(BytecodeError::InvalidSection(
                        "CALL_VIRTUAL expects interface type".into(),
                    ));
                }
                if let TypeData::Interface { methods } = &entry.data {
                    if slot as usize >= methods.len() {
                        return Err(BytecodeError::InvalidSection(
                            "CALL_VIRTUAL slot out of range".into(),
                        ));
                    }
                }
            }
            0x09 => {
                let kind = reader.read_u32()?;
                let symbol_idx = reader.read_u32()?;
                let arg_count = reader.read_u32()?;
                if kind > 3 {
                    return Err(BytecodeError::InvalidSection(
                        "CALL_NATIVE kind out of range".into(),
                    ));
                }
                if symbol_idx as usize >= tables.strings.entries.len() {
                    return Err(BytecodeError::InvalidIndex {
                        kind: "native symbol".into(),
                        index: symbol_idx,
                    });
                }
                if arg_count > 1024 {
                    return Err(BytecodeError::InvalidSection(
                        "CALL_NATIVE arg_count out of range".into(),
                    ));
                }
            }
            0x10 => {
                let const_idx = reader.read_u32()?;
                ensure_const_index(tables.const_pool, const_idx)?;
            }
            0x20..=0x22 => {
                let ref_idx = reader.read_u32()?;
                ensure_pou_ref_operand(tables.ref_table, pou, ref_idx)?;
            }
            0x23 | 0x24 => {}
            0x30 => {
                let name_idx = reader.read_u32()?;
                ensure_string_index(tables.strings, name_idx)?;
            }
            0x60 => {
                let type_id = reader.read_u32()?;
                ensure_type_index(tables.types, type_id)?;
            }
            0x61 => {}
            0x62 | 0x63 => {
                let operand = reader.read_u32()?;
                validate_partial_access_operand(operand)?;
            }
            0x70 => {
                reader.read_u32()?;
            }
            _ => return Err(BytecodeError::InvalidOpcode(opcode)),
        }
    }
    let code_len = code.len() as i32;
    let start_set: HashSet<i32> = starts.into_iter().collect();
    for (pc, offset) in jumps {
        let target = pc + 1 + 4 + offset;
        if target < 0 || target > code_len {
            return Err(BytecodeError::InvalidJumpTarget(target));
        }
        if target != code_len && !start_set.contains(&target) {
            return Err(BytecodeError::InvalidJumpTarget(target));
        }
    }
    Ok(())
}

fn validate_pou_local_ref_range(ref_table: &RefTable, pou: &PouEntry) -> Result<(), BytecodeError> {
    let start = pou.local_ref_start;
    let end = start.checked_add(pou.local_ref_count).ok_or_else(|| {
        BytecodeError::InvalidSection("POU local ref range overflow".into())
    })?;
    if end as usize > ref_table.entries.len() {
        return Err(BytecodeError::InvalidSection(
            "POU local ref range out of bounds".into(),
        ));
    }
    for ref_idx in start..end {
        let ref_entry = &ref_table.entries[ref_idx as usize];
        if ref_entry.location != RefLocation::Local {
            return Err(BytecodeError::InvalidSection(
                "POU local ref range contains non-local ref".into(),
            ));
        }
        if !ref_entry.segments.is_empty() {
            return Err(BytecodeError::InvalidSection(
                "POU local ref range contains path ref".into(),
            ));
        }
        if ref_entry.offset != ref_idx.saturating_sub(start) {
            return Err(BytecodeError::InvalidSection(
                "POU local ref range contains non-contiguous local offset".into(),
            ));
        }
    }
    Ok(())
}

fn pou_local_owner(ref_table: &RefTable, pou: &PouEntry) -> Option<u32> {
    if pou.local_ref_count == 0 {
        return None;
    }
    ref_table
        .entries
        .get(pou.local_ref_start as usize)
        .map(|entry| entry.owner_id)
}

fn ensure_pou_ref_operand(
    ref_table: &RefTable,
    pou: &PouEntry,
    ref_idx: u32,
) -> Result<(), BytecodeError> {
    ensure_ref_index(ref_table, ref_idx)?;
    let ref_entry = &ref_table.entries[ref_idx as usize];
    if ref_entry.location == RefLocation::Local {
        let end = pou.local_ref_start.checked_add(pou.local_ref_count).ok_or_else(|| {
            BytecodeError::InvalidSection("POU local ref range overflow".into())
        })?;
        if ref_idx < pou.local_ref_start || ref_idx >= end {
            if !ref_entry.segments.is_empty()
                && pou_local_owner(ref_table, pou) == Some(ref_entry.owner_id)
                && ref_entry.offset < pou.local_ref_count
            {
                return Ok(());
            }
            return Err(BytecodeError::InvalidSection(
                "local ref outside POU local range".into(),
            ));
        }
    }
    Ok(())
}

fn unsupported_runtime_opcode_name(opcode: u8) -> Option<&'static str> {
    match opcode {
        0x07 => Some("CALL_METHOD"),
        0x08 => Some("CALL_VIRTUAL"),
        0x14 => Some("ROT3"),
        0x15 => Some("ROT4"),
        0x16 => Some("CAST_IMPLICIT"),
        0x4A => Some("SHL"),
        0x4B => Some("SHR"),
        0x4D => Some("ROL"),
        0x4E => Some("ROR"),
        _ => None,
    }
}

fn validate_partial_access_operand(operand: u32) -> Result<(), BytecodeError> {
    if (operand & !0x3FF) != 0 {
        return Err(BytecodeError::InvalidSection(
            "partial-access operand out of range".into(),
        ));
    }
    let kind = (operand >> 8) & 0x03;
    let index = (operand & 0xFF) as u8;
    let max = match kind {
        0 => 63, // bit
        1 => 7,  // byte
        2 => 3,  // word
        3 => 1,  // dword
        _ => unreachable!(),
    };
    if index > max {
        return Err(BytecodeError::InvalidSection(
            "partial-access index out of range".into(),
        ));
    }
    Ok(())
}
