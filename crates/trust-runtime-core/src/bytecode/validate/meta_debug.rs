use super::*;

pub(super) fn validate_var_meta(
    tables: &ValidationContext<'_>,
    meta: &VarMeta,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let (strings, types, const_pool, ref_table) = (
        tables.strings,
        tables.types,
        tables.const_pool,
        tables.ref_table,
    );
    for (position, entry) in meta.entries.iter().enumerate() {
        budget.work(1)?;
        ensure_string_index(strings, entry.name_idx)?;
        ensure_type_index(types, entry.type_id)?;
        ensure_ref_index(ref_table, entry.ref_idx)?;
        if tables.first_var_ref(entry.ref_idx, budget)? != Some(position) {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate VAR_META ref_idx {}", entry.ref_idx).into(),
            ));
        }
        if entry.retain > 3 {
            return Err(BytecodeError::from(RejectionReason::InvalidRetainPolicy));
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
        if tables.named_var(name.as_str(), budget)? != Some(position) {
            return Err(BytecodeError::from(RejectionReason::DuplicateVarMetaName));
        }
        let reference = &ref_table.entries[entry.ref_idx as usize];
        if reference.location == RefLocation::Local {
            validate_local_var_meta(name, entry, reference, tables, budget)?;
        } else if name.starts_with("@local/") {
            return Err(BytecodeError::from(
                RejectionReason::ReservedLocalVarMetaNameRequiresALocalRef,
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_local_var_meta(
    name: &str,
    entry: &crate::bytecode::VarMetaEntry,
    reference: &crate::bytecode::RefEntry,
    tables: &ValidationContext<'_>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    budget.work(name.len())?;
    let Some(encoded) = name.strip_prefix("@local/") else {
        return Err(BytecodeError::from(
            RejectionReason::LocalVarMetaNameMustUseTheReservedLocalScope,
        ));
    };
    let mut parts = encoded.splitn(3, '/');
    let pou_id = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| BytecodeError::from(RejectionReason::InvalidLocalVarMetaPouId))?;
    let slot = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| BytecodeError::from(RejectionReason::InvalidLocalVarMetaSlot))?;
    if parts.next().is_none_or(str::is_empty) {
        return Err(BytecodeError::from(
            RejectionReason::LocalVarMetaNameIsMissingItsDisplayLabel,
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
        return Err(BytecodeError::from(
            RejectionReason::LocalVarMetaMustDescribeABaseLocalRef,
        ));
    }
    let Some(pou) = tables.local_owner(entry.ref_idx, budget)? else {
        return Err(BytecodeError::InvalidSection(
            format!(
                "local VAR_META ref {} is outside every POU local range",
                entry.ref_idx
            )
            .into(),
        ));
    };
    let expected_slot = entry.ref_idx - pou.local_ref_start;
    if pou.id != pou_id || slot != expected_slot || reference.offset != expected_slot {
        return Err(BytecodeError::from(
            RejectionReason::LocalVarMetaScopeDoesNotMatchItsPouLocalRef,
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
        return Err(BytecodeError::from(
            RejectionReason::LocalVarMetaTypeDisagreesWithThePouSignature,
        ));
    }
    Ok(())
}

pub(super) fn validate_retain_init(
    const_pool: &ConstPool,
    ref_table: &RefTable,
    retain: &RetainInit,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for entry in &retain.entries {
        budget.work(1)?;
        ensure_ref_index(ref_table, entry.ref_idx)?;
        ensure_const_index(const_pool, entry.const_idx)?;
    }
    Ok(())
}

pub(super) fn validate_debug_map(
    strings: &StringTable,
    tables: &ValidationContext<'_>,
    map: &DebugMap,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for entry in &map.entries {
        budget.work(1)?;
        let pou = tables
            .pou(entry.pou_id, budget)?
            .ok_or(BytecodeError::InvalidPouId(entry.pou_id))?;
        let end = pou
            .code_offset
            .checked_add(pou.code_length)
            .ok_or_else(|| BytecodeError::from(RejectionReason::PouCodeRangeOverflow))?;
        if entry.code_offset < pou.code_offset || entry.code_offset > end {
            return Err(BytecodeError::from(
                RejectionReason::DebugMapCodeOffsetOutOfBounds,
            ));
        }
        ensure_string_index(strings, entry.file_idx)?;
    }
    Ok(())
}

pub(super) fn ensure_string_index(strings: &StringTable, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= strings.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "string".into(),
            index: idx,
        });
    }
    Ok(())
}

pub(super) fn ensure_type_index(types: &TypeTable, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= types.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: idx,
        });
    }
    Ok(())
}

pub(super) fn ensure_const_index(pool: &ConstPool, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= pool.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "const".into(),
            index: idx,
        });
    }
    Ok(())
}

pub(super) fn ensure_ref_index(table: &RefTable, idx: u32) -> Result<(), BytecodeError> {
    if idx as usize >= table.entries.len() {
        return Err(BytecodeError::InvalidIndex {
            kind: "ref".into(),
            index: idx,
        });
    }
    Ok(())
}
