fn validate_var_meta(
    strings: &StringTable,
    types: &TypeTable,
    const_pool: &ConstPool,
    ref_table: &RefTable,
    pou_index: &PouIndex,
    meta: &VarMeta,
) -> Result<(), BytecodeError> {
    let mut names = HashSet::new();
    let mut refs = HashSet::new();
    for entry in &meta.entries {
        ensure_string_index(strings, entry.name_idx)?;
        ensure_type_index(types, entry.type_id)?;
        ensure_ref_index(ref_table, entry.ref_idx)?;
        if !refs.insert(entry.ref_idx) {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate VAR_META ref_idx {}", entry.ref_idx).into(),
            ));
        }
        if entry.retain > 3 {
            return Err(BytecodeError::InvalidSection(
                "invalid retain policy".into(),
            ));
        }
        if let Some(init_idx) = entry.init_const_idx {
            ensure_const_index(const_pool, init_idx)?;
        }
        let name = strings
            .entries
            .get(entry.name_idx as usize)
            .ok_or_else(|| BytecodeError::InvalidIndex {
                kind: "string".into(),
                index: entry.name_idx,
            })?;
        if !names.insert(name.to_ascii_uppercase()) {
            return Err(BytecodeError::InvalidSection(
                "duplicate VAR_META name".into(),
            ));
        }
        let reference = &ref_table.entries[entry.ref_idx as usize];
        if reference.location == RefLocation::Local {
            validate_local_var_meta(name, entry, reference, pou_index)?;
        } else if name.starts_with("@local/") {
            return Err(BytecodeError::InvalidSection(
                "reserved local VAR_META name requires a local ref".into(),
            ));
        }
    }
    Ok(())
}

fn validate_local_var_meta(
    name: &str,
    entry: &super::VarMetaEntry,
    reference: &super::RefEntry,
    pou_index: &PouIndex,
) -> Result<(), BytecodeError> {
    let Some(encoded) = name.strip_prefix("@local/") else {
        return Err(BytecodeError::InvalidSection(
            "local VAR_META name must use the reserved @local scope".into(),
        ));
    };
    let mut parts = encoded.splitn(3, '/');
    let pou_id = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| BytecodeError::InvalidSection("invalid local VAR_META POU id".into()))?;
    let slot = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| BytecodeError::InvalidSection("invalid local VAR_META slot".into()))?;
    if parts.next().is_none_or(str::is_empty) {
        return Err(BytecodeError::InvalidSection(
            "local VAR_META name is missing its display label".into(),
        ));
    }
    if entry.retain != 0 || entry.init_const_idx.is_some() {
        return Err(BytecodeError::InvalidSection(
            format!(
                "local VAR_META ref {} must use retain=0 and no initializer",
                entry.ref_idx
            )
            .into(),
        ));
    }
    if !reference.segments.is_empty() {
        return Err(BytecodeError::InvalidSection(
            "local VAR_META must describe a base local ref".into(),
        ));
    }
    let owners = pou_index
        .entries
        .iter()
        .filter(|pou| {
            pou.local_ref_start
                .checked_add(pou.local_ref_count)
                .is_some_and(|end| entry.ref_idx >= pou.local_ref_start && entry.ref_idx < end)
        })
        .collect::<Vec<_>>();
    if owners.len() != 1 {
        return Err(BytecodeError::InvalidSection(
            format!(
                "local VAR_META ref {} is outside every POU local range",
                entry.ref_idx
            )
            .into(),
        ));
    }
    let pou = owners[0];
    let expected_slot = entry.ref_idx - pou.local_ref_start;
    if pou.id != pou_id || slot != expected_slot || reference.offset != expected_slot {
        return Err(BytecodeError::InvalidSection(
            "local VAR_META scope does not match its POU local ref".into(),
        ));
    }
    let expected_declared_type = if matches!(pou.kind, PouKind::Function | PouKind::Method) {
        let return_slots = u32::from(pou.return_type_id.is_some());
        if return_slots == 1 && slot == 0 {
            pou.return_type_id
        } else {
            slot.checked_sub(return_slots)
                .and_then(|index| pou.params.get(index as usize))
                .map(|param| param.type_id)
        }
    } else {
        None
    };
    if expected_declared_type.is_some_and(|type_id| type_id != entry.type_id) {
        return Err(BytecodeError::InvalidSection(
            "local VAR_META type disagrees with the POU signature".into(),
        ));
    }
    Ok(())
}

fn validate_retain_init(
    const_pool: &ConstPool,
    ref_table: &RefTable,
    retain: &RetainInit,
) -> Result<(), BytecodeError> {
    for entry in &retain.entries {
        ensure_ref_index(ref_table, entry.ref_idx)?;
        ensure_const_index(const_pool, entry.const_idx)?;
    }
    Ok(())
}

fn validate_debug_map(
    strings: &StringTable,
    pou_index: &PouIndex,
    map: &DebugMap,
) -> Result<(), BytecodeError> {
    for entry in &map.entries {
        let pou = pou_index
            .entries
            .iter()
            .find(|pou| pou.id == entry.pou_id)
            .ok_or(BytecodeError::InvalidPouId(entry.pou_id))?;
        let end = pou
            .code_offset
            .checked_add(pou.code_length)
            .ok_or_else(|| BytecodeError::InvalidSection("POU code range overflow".into()))?;
        if entry.code_offset < pou.code_offset || entry.code_offset > end {
            return Err(BytecodeError::InvalidSection(
                "debug map code offset out of bounds".into(),
            ));
        }
        ensure_string_index(strings, entry.file_idx)?;
    }
    Ok(())
}

fn ensure_string_index(strings: &StringTable, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= strings.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "string".into(),
            index: idx,
        });
    }
    Ok(())
}

fn ensure_type_index(types: &TypeTable, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= types.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: idx,
        });
    }
    Ok(())
}

fn ensure_const_index(pool: &ConstPool, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= pool.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "const".into(),
            index: idx,
        });
    }
    Ok(())
}

fn ensure_ref_index(table: &RefTable, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= table.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "ref".into(),
            index: idx,
        });
    }
    Ok(())
}
