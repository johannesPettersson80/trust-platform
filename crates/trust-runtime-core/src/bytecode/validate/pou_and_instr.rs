use super::*;

pub(super) fn validate_pou_index(
    tables: &ValidationContext<'_>,
    bodies: &[u8],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let (strings, types, const_pool, ref_table, index) = (
        tables.strings,
        tables.types,
        tables.const_pool,
        tables.ref_table,
        tables.index,
    );
    let mut decoded_instruction_count = 0usize;
    budget.temporary(|budget| validate_pou_local_ref_partition(tables, budget))?;
    for (position, entry) in index.entries.iter().enumerate() {
        budget.work(1)?;
        if tables.pou_position(entry.id, budget)? != Some(position) {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate POU id {}", entry.id).into(),
            ));
        }
        validate_pou_local_ref_range(ref_table, entry, budget)?;
        ensure_string_index(strings, entry.name_idx)?;
        if let Some(return_type_id) = entry.return_type_id {
            ensure_type_index(types, return_type_id)?;
        }
        if let Some(owner) = entry.owner_pou_id {
            if tables.pou(owner, budget)?.is_none() {
                return Err(BytecodeError::InvalidPouId(owner));
            }
        }
        budget.work(entry.params.len())?;
        for param in &entry.params {
            ensure_string_index(strings, param.name_idx)?;
            ensure_type_index(types, param.type_id)?;
            if let Some(default_idx) = param.default_const_idx {
                ensure_const_index(const_pool, default_idx)?;
            }
        }
        validate_param_direction_metadata(entry, budget)?;
        if let Some(meta) = &entry.class_meta {
            if let Some(parent) = meta.parent_pou_id {
                if tables.pou(parent, budget)?.is_none() {
                    return Err(BytecodeError::InvalidPouId(parent));
                }
            }
            budget.work(meta.interfaces.len())?;
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
                    return Err(BytecodeError::from(
                        RejectionReason::InterfaceMappingExpectsInterfaceType,
                    ));
                }
                if let TypeData::Interface { methods } = &interface_entry.data {
                    if interface.vtable_slots.len() != methods.len() {
                        return Err(BytecodeError::from(
                            RejectionReason::InterfaceMappingSlotMismatch,
                        ));
                    }
                }
            }
            budget.work(meta.methods.len())?;
            for method in &meta.methods {
                ensure_string_index(strings, method.name_idx)?;
                if tables.pou(method.pou_id, budget)?.is_none() {
                    return Err(BytecodeError::InvalidPouId(method.pou_id));
                }
            }
        }
        let start = entry.code_offset as usize;
        let end = start
            .checked_add(entry.code_length as usize)
            .ok_or_else(|| BytecodeError::from(RejectionReason::PouCodeOutOfBounds))?;
        if end > bodies.len() {
            return Err(BytecodeError::from(RejectionReason::PouCodeOutOfBounds));
        }
        let code = &bodies[start..end];
        budget.temporary(|budget| {
            let instructions = decode_instructions(
                code,
                &mut decoded_instruction_count,
                budget,
                |instruction, budget| {
                    validate_instruction_operands(tables, entry, instruction, budget)
                },
            )?;
            // The legacy-call rejection follows complete decoding/jump checks, as before.
            for instruction in &instructions {
                budget.work(1)?;
                if instruction.opcode == 0x05 {
                    return Err(BytecodeError::from(RejectionReason::LegacyCall));
                }
            }
            budget.temporary(|budget| validate_reference_escape(tables, &instructions, budget))?;
            budget.temporary(|budget| validate_owner_contract(tables, &instructions, budget))?;
            budget.temporary(|budget| {
                validate_stack_shape(tables, &instructions, code.len(), budget)
            })?;
            budget.temporary(|budget| validate_const_compat(tables, &instructions, budget))?;
            budget.temporary(|budget| {
                validate_param_direction_calls(tables, entry, &instructions, budget)
            })?;
            Ok(())
        })?;
    }
    Ok(())
}

pub(super) fn validate_pou_local_ref_partition(
    tables: &ValidationContext<'_>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut previous_end = 0;
    for &(start, end, _) in &tables.local_ranges {
        budget.work(1)?;
        if start < previous_end {
            return Err(RejectionReason::PouLocalRefRangesOverlap.into());
        }
        previous_end = end;
    }
    let mut owners = Vec::new();
    for &(start, end, _) in &tables.local_ranges {
        let mut owner_id = None;
        for reference in &tables.ref_table.entries[start as usize..end as usize] {
            budget.work(1)?;
            match owner_id {
                Some(expected) if expected != reference.owner_id => {
                    return Err(RejectionReason::PouLocalRefRangeContainsMultipleFrameOwners.into())
                }
                None => owner_id = Some(reference.owner_id),
                Some(_) => {}
            }
        }
        if let Some(owner) = owner_id {
            budget.push(&mut owners, owner)?;
        }
    }
    budget.sort_by(&mut owners, |a, b, _| Ok(a.cmp(b)))?;
    for pair in owners.windows(2) {
        budget.work(1)?;
        if pair[0] == pair[1] {
            return Err(RejectionReason::PouLocalRefRangesShareAFrameOwner.into());
        }
    }
    Ok(())
}

pub(super) fn validate_instruction_operands(
    tables: &ValidationContext<'_>,
    pou: &PouEntry,
    instruction: &Instruction,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let opcode = instruction.opcode;
    match opcode {
        0x00 | 0x01 | 0x06 | 0x11 | 0x12 | 0x13 | 0x25 | 0x31 | 0x32 | 0x33 | 0x40 | 0x41
        | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x48 | 0x49 | 0x4C | 0x50 | 0x51 | 0x52
        | 0x53 | 0x54 | 0x55 => {}
        0x02..=0x04 => {}
        0x05 => {
            let pou_id = instruction.operand(0);
            if tables.pou(pou_id, budget)?.is_none() {
                return Err(BytecodeError::InvalidPouId(pou_id));
            }
        }

        0x09 => {
            let kind = instruction.operand(0);
            let symbol_idx = instruction.operand(1);
            let arg_count = instruction.operand(2);
            if kind > 3 {
                return Err(BytecodeError::from(
                    RejectionReason::CallNativeKindOutOfRange,
                ));
            }
            if symbol_idx as usize >= tables.strings.entries.len() {
                return Err(BytecodeError::InvalidIndex {
                    kind: "native symbol".into(),
                    index: symbol_idx,
                });
            }
            if arg_count as usize > crate::bytecode::BYTECODE_MAX_NATIVE_ARGUMENTS {
                return Err(BytecodeError::from(
                    RejectionReason::CallNativeArgCountOutOfRange,
                ));
            }
        }
        0x10 => {
            let const_idx = instruction.operand(0);
            ensure_const_index(tables.const_pool, const_idx)?;
        }
        0x20..=0x22 => {
            let ref_idx = instruction.operand(0);
            ensure_pou_ref_operand(tables.ref_table, pou, ref_idx)?;
        }
        0x23 | 0x24 => {}
        0x30 => {
            let name_idx = instruction.operand(0);
            ensure_string_index(tables.strings, name_idx)?;
        }
        0x60 => {
            let type_id = instruction.operand(0);
            ensure_type_index(tables.types, type_id)?;
        }
        0x61 => {}
        0x62 | 0x63 => {
            let operand = instruction.operand(0);
            validate_partial_access_operand(operand)?;
        }
        0x64 => {
            let type_id = instruction.operand(0);
            ensure_type_index(tables.types, type_id)?;
            let entry = &tables.types.entries[type_id as usize];
            if !matches!(entry.kind, TypeKind::Reference | TypeKind::Interface) {
                return Err(BytecodeError::from(
                    RejectionReason::ReferenceAttemptExpectsReferenceOrInterfaceTargetType,
                ));
            }
        }
        0x70 => {}
        _ => return Err(BytecodeError::InvalidOpcode(opcode)),
    }
    Ok(())
}

pub(super) fn validate_pou_local_ref_range(
    ref_table: &RefTable,
    pou: &PouEntry,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let start = pou.local_ref_start;
    let end = start
        .checked_add(pou.local_ref_count)
        .ok_or_else(|| BytecodeError::from(RejectionReason::PouLocalRefRangeOverflow))?;
    if end as usize > ref_table.entries.len() {
        return Err(BytecodeError::from(
            RejectionReason::PouLocalRefRangeOutOfBounds,
        ));
    }
    budget.work((end - start) as usize)?;
    for ref_idx in start..end {
        let ref_entry = &ref_table.entries[ref_idx as usize];
        if ref_entry.location != RefLocation::Local {
            return Err(BytecodeError::from(
                RejectionReason::PouLocalRefRangeContainsNonLocalRef,
            ));
        }
        if !ref_entry.segments.is_empty() {
            return Err(BytecodeError::from(
                RejectionReason::PouLocalRefRangeContainsPathRef,
            ));
        }
        if ref_entry.offset != ref_idx.saturating_sub(start) {
            return Err(BytecodeError::from(
                RejectionReason::PouLocalRefRangeContainsNonContiguousLocalOffset,
            ));
        }
    }
    Ok(())
}

pub(super) fn pou_local_owner(ref_table: &RefTable, pou: &PouEntry) -> Option<u32> {
    if pou.local_ref_count == 0 {
        return None;
    }
    ref_table
        .entries
        .get(pou.local_ref_start as usize)
        .map(|entry| entry.owner_id)
}

pub(super) fn ensure_pou_ref_operand(
    ref_table: &RefTable,
    pou: &PouEntry,
    ref_idx: u32,
) -> Result<(), BytecodeError> {
    ensure_ref_index(ref_table, ref_idx)?;
    let ref_entry = &ref_table.entries[ref_idx as usize];
    if ref_entry.location == RefLocation::Local {
        let end = pou
            .local_ref_start
            .checked_add(pou.local_ref_count)
            .ok_or_else(|| BytecodeError::from(RejectionReason::PouLocalRefRangeOverflow))?;
        if ref_idx < pou.local_ref_start || ref_idx >= end {
            if !ref_entry.segments.is_empty()
                && pou_local_owner(ref_table, pou) == Some(ref_entry.owner_id)
                && ref_entry.offset < pou.local_ref_count
            {
                return Ok(());
            }
            return Err(BytecodeError::from(
                RejectionReason::LocalRefOutsidePouLocalRange,
            ));
        }
    }
    Ok(())
}

pub(super) fn unsupported_runtime_opcode_name(opcode: u8) -> Option<&'static str> {
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

pub(super) fn validate_partial_access_operand(operand: u32) -> Result<(), BytecodeError> {
    if (operand & !0x3FF) != 0 {
        return Err(BytecodeError::from(
            RejectionReason::PartialAccessOperandOutOfRange,
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
        return Err(BytecodeError::from(
            RejectionReason::PartialAccessIndexOutOfRange,
        ));
    }
    Ok(())
}
