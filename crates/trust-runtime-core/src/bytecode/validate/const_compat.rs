use super::*;

pub(super) fn validate_const_compat(
    tables: &ValidationContext<'_>,
    instructions: &[Instruction],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let const_pool = tables.const_pool;
    if tables.var_meta.is_none() {
        return Ok(());
    }
    let mut stack = Vec::new();
    for instruction in instructions {
        budget.work(1)?;
        let opcode = instruction.opcode;
        match opcode {
            0x10 => {
                let const_idx = instruction.operand(0);
                let entry = const_pool.entries.get(const_idx as usize).ok_or(
                    BytecodeError::InvalidIndex {
                        kind: "const".into(),
                        index: const_idx,
                    },
                )?;
                budget.push(&mut stack, Some(entry.type_id))?;
            }
            0x21 => {
                let value_type = pop_const_type(&mut stack);
                validate_const_store(tables, value_type, instruction.operand(0), budget)?;
            }
            0x64 => {
                pop_const_type(&mut stack);
                budget.push(&mut stack, Some(instruction.operand(0)))?;
            }
            _ => apply_unknown_effect(instruction, &mut stack, None, budget)?,
        }
    }
    Ok(())
}

pub(super) fn pop_const_type(stack: &mut Vec<Option<u32>>) -> Option<u32> {
    stack.pop().flatten()
}

pub(super) fn validate_const_store(
    tables: &ValidationContext<'_>,
    value_type: Option<u32>,
    ref_idx: u32,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let Some(value_type) = value_type else {
        return Ok(());
    };
    ensure_ref_index(tables.ref_table, ref_idx)?;
    let Some(target_type) = tables.ref_type(ref_idx, budget)? else {
        return Ok(());
    };
    if assignment_type_compatible(tables.types, value_type, target_type, budget)? {
        return Ok(());
    }
    Err(BytecodeError::from(
        RejectionReason::ConstantTypeIsIncompatibleWithStoreRefTarget,
    ))
}

pub(super) fn assignment_type_compatible(
    types: &TypeTable,
    value_type: u32,
    target_type: u32,
    budget: &mut ValidationBudget,
) -> Result<bool, BytecodeError> {
    if value_type == target_type {
        return Ok(true);
    }
    let value_prim = resolved_primitive_id(types, value_type, budget)?;
    let target_prim = resolved_primitive_id(types, target_type, budget)?;
    Ok(match (value_prim, target_prim) {
        (Some(1), Some(1)) => true,
        (Some(1), Some(_)) | (Some(_), Some(1)) => false,
        (Some(value), Some(target))
            if is_numeric_primitive(value) && is_numeric_primitive(target) =>
        {
            true
        }
        _ => true,
    })
}

pub(super) fn resolved_primitive_id(
    types: &TypeTable,
    mut type_id: u32,
    budget: &mut ValidationBudget,
) -> Result<Option<u16>, BytecodeError> {
    for _ in 0..=crate::bytecode::BYTECODE_MAX_CONST_NESTING {
        budget.work(1)?;
        let entry = types
            .entries
            .get(type_id as usize)
            .ok_or(BytecodeError::InvalidIndex {
                kind: "type".into(),
                index: type_id,
            })?;
        match &entry.data {
            TypeData::Primitive { prim_id, .. } => return Ok(Some(*prim_id)),
            TypeData::Alias { target_type_id }
            | TypeData::Subrange {
                base_type_id: target_type_id,
                ..
            } => type_id = *target_type_id,
            _ => return Ok(None),
        }
    }
    Err(BytecodeError::from(
        RejectionReason::TypeReferenceRecursionOverflow,
    ))
}

pub(super) fn is_numeric_primitive(prim_id: u16) -> bool {
    matches!(prim_id, 6..=15)
}
