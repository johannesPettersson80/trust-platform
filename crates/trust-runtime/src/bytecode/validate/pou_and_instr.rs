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
        validate_instruction_stream(
            strings,
            index,
            types,
            const_pool,
            ref_table,
            start,
            &bodies[start..end],
        )?;
    }
    Ok(())
}

fn validate_instruction_stream(
    strings: &StringTable,
    index: &PouIndex,
    types: &TypeTable,
    const_pool: &ConstPool,
    ref_table: &RefTable,
    _base: usize,
    code: &[u8],
) -> Result<(), BytecodeError> {
    let mut reader = BytecodeReader::new(code);
    let mut starts = Vec::new();
    let mut jumps = Vec::new();
    while reader.remaining() > 0 {
        let pc = reader.pos();
        starts.push(pc as i32);
        let opcode = reader.read_u8()?;
        match opcode {
            0x00 | 0x01 | 0x06 | 0x11 | 0x12 | 0x13 | 0x14 | 0x15 | 0x25 | 0x31 | 0x32 | 0x33 | 0x40
            | 0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x48 | 0x49 | 0x4A | 0x4B | 0x4C
            | 0x4D | 0x4E | 0x50 | 0x51 | 0x52 | 0x53 | 0x54 | 0x55 => {}
            0x02..=0x04 => {
                let offset = reader.read_i32()?;
                jumps.push((pc as i32, offset));
            }
            0x05 => {
                let pou_id = reader.read_u32()?;
                if !index.entries.iter().any(|pou| pou.id == pou_id) {
                    return Err(BytecodeError::InvalidPouId(pou_id));
                }
            }
            0x07 => {
                reader.read_u32()?; // vtable slot
            }
            0x08 => {
                let interface_type_id = reader.read_u32()?;
                let slot = reader.read_u32()?;
                let entry = types
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
                if symbol_idx as usize >= strings.entries.len() {
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
                ensure_const_index(const_pool, const_idx)?;
            }
            0x16 => {
                reader.read_u8()?;
            }
            0x20..=0x22 => {
                let ref_idx = reader.read_u32()?;
                ensure_ref_index(ref_table, ref_idx)?;
            }
            0x23 | 0x24 => {}
            0x30 => {
                let name_idx = reader.read_u32()?;
                ensure_string_index(strings, name_idx)?;
            }
            0x60 => {
                let type_id = reader.read_u32()?;
                ensure_type_index(types, type_id)?;
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
