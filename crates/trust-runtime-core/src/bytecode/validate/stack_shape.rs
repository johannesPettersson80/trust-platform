use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StackShape {
    Unknown,
    Bool,
    Numeric,
    Reference,
    StagingReference,
    MaybeStagingReference,
    FrameReference,
    Instance,
}

pub(super) fn validate_stack_shape(
    tables: &ValidationContext<'_>,
    instructions: &[Instruction],
    code_len: usize,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    validate_stack_shape_inner(tables, instructions, code_len, None, budget)
}

pub(super) fn validate_initializer_stack_shape(
    tables: &ValidationContext<'_>,
    instructions: &[Instruction],
    code_len: usize,
    result_is_local: bool,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    validate_stack_shape_inner(
        tables,
        instructions,
        code_len,
        Some(result_is_local),
        budget,
    )
}

fn validate_stack_shape_inner(
    tables: &ValidationContext<'_>,
    instructions: &[Instruction],
    code_len: usize,
    result_is_local: Option<bool>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    if instructions.is_empty() {
        return Ok(());
    }
    let mut is_leader = Vec::new();
    budget.reserve(&mut is_leader, instructions.len())?;
    budget.work(instructions.len())?;
    is_leader.resize(instructions.len(), false);
    is_leader[0] = true;
    for (index, instruction) in instructions.iter().enumerate() {
        budget.work(1)?;
        if let Some(pc) = instruction.jump_target()? {
            if pc != code_len {
                is_leader[instruction_index(instructions, pc, budget)?] = true;
            }
        }
        if matches!(instruction.opcode, 0x02..=0x04 | 0x06) && index + 1 < instructions.len() {
            is_leader[index + 1] = true;
        }
    }
    let mut leaders = Vec::new();
    budget.work(is_leader.len())?;
    for (index, leader) in is_leader.iter().enumerate() {
        if *leader {
            budget.push(&mut leaders, index)?;
        }
    }
    // Only block entries retain stacks; a straight-line instruction has no snapshot.
    let mut states: Vec<Option<Vec<StackShape>>> = Vec::new();
    budget.reserve(&mut states, leaders.len())?;
    budget.work(leaders.len())?;
    states.resize_with(leaders.len(), || None);
    states[0] = Some(Vec::new());
    let mut work = Vec::new();
    budget.push(&mut work, 0)?;
    let entries = FlowEntries {
        instructions,
        leaders: &leaders,
        code_len,
    };
    let mut stack = Vec::new();
    while let Some(block) = work.pop() {
        budget.copy(
            &mut stack,
            states[block]
                .as_ref()
                .ok_or(RejectionReason::MissingBlockEntryState)?,
        )?;
        let mut index = leaders[block];
        loop {
            budget.work(1)?;
            let instruction = &instructions[index];
            apply_stack_instruction(tables, instruction, &mut stack, result_is_local, budget)?;
            if matches!(instruction.opcode, 0x02..=0x04) {
                // Target is queued before fallthrough; LIFO visits fallthrough first.
                let target = instruction
                    .jump_target()?
                    .ok_or(BytecodeError::InvalidOpcode(instruction.opcode))?;
                enqueue_stack_state(target, &stack, &entries, &mut states, &mut work, budget)?;
                if instruction.opcode != 0x02 {
                    enqueue_stack_state(
                        instruction.next_pc()?,
                        &stack,
                        &entries,
                        &mut states,
                        &mut work,
                        budget,
                    )?;
                }
                break;
            }
            if instruction.opcode == 0x06 {
                break;
            }
            index += 1;
            if index == instructions.len() || is_leader[index] {
                enqueue_stack_state(
                    instruction.next_pc()?,
                    &stack,
                    &entries,
                    &mut states,
                    &mut work,
                    budget,
                )?;
                break;
            }
        }
    }
    Ok(())
}

fn instruction_index(
    instructions: &[Instruction],
    pc: usize,
    budget: &mut ValidationBudget,
) -> Result<usize, BytecodeError> {
    budget
        .search_by(instructions, |instruction, _| {
            Ok((instruction.pc as usize).cmp(&pc))
        })?
        .map_err(|_| BytecodeError::InvalidJumpTarget(i32::try_from(pc).unwrap_or(i32::MAX)))
}

fn apply_stack_instruction(
    tables: &ValidationContext<'_>,
    instr: &Instruction,
    stack: &mut Vec<StackShape>,
    result_is_local: Option<bool>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let types = tables.types;
    let const_pool = tables.const_pool;
    let opcode = instr.opcode;
    match opcode {
        0x00..=0x02 => {}
        0x03 | 0x04 => {
            let condition = pop_stack_shape(stack, opcode)?;
            if !matches!(condition, StackShape::Bool | StackShape::Unknown) {
                return Err(BytecodeError::from(
                    RejectionReason::ConditionalJumpExpectsBoolOperand,
                ));
            }
        }
        0x05 | 0x70 => {}
        0x06 => {
            if !stack.is_empty() {
                return Err(BytecodeError::from(
                    RejectionReason::PouBodyLeavesValuesOnOperandStack,
                ));
            }
            return Ok(());
        }
        0x09 => {
            let arg_count = instr.operand(2);
            budget.work(arg_count as usize)?;
            if result_is_local.is_some() {
                budget.temporary(|budget| {
                    let symbol = tables
                        .strings
                        .entries
                        .get(instr.operand(1) as usize)
                        .ok_or(RejectionReason::InvalidInitializerNativeArguments)?;
                    let arguments =
                        super::param_direction::parse_native_symbol_args(symbol, budget)?
                            .filter(|args| args.args.len() == arg_count as usize)
                            .ok_or(RejectionReason::InvalidInitializerNativeArguments)?;
                    for arg in arguments.args.iter().rev() {
                        let value = pop_stack_shape(stack, opcode)?;
                        if arg.is_target {
                            require_staging_destination(value)?;
                        } else {
                            reject_staging(value)?;
                        }
                    }
                    Ok(())
                })?;
            } else {
                for _ in 0..arg_count {
                    reject_staging(pop_stack_shape(stack, opcode)?)?;
                }
            }
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x10 => {
            let const_idx = instr.operand(0);
            let shape = const_stack_shape(types, const_pool, const_idx, budget)?;
            budget.push_stack(stack, shape)?;
        }
        0x11 => {
            let top = stack
                .last()
                .copied()
                .ok_or_else(|| BytecodeError::from(RejectionReason::OperandStackUnderflowOnDup))?;
            budget.push_stack(stack, top)?;
        }
        0x12 => {
            let _ = pop_stack_shape(stack, opcode)?;
        }
        0x13 => {
            if stack.len() < 2 {
                return Err(BytecodeError::from(
                    RejectionReason::OperandStackUnderflowOnSwap,
                ));
            }
            let len = stack.len();
            stack.swap(len - 1, len - 2);
        }
        0x20..=0x25 | 0x30..=0x33 => {
            apply_reference_stack_instruction(tables, instr, stack, result_is_local, budget)?;
        }
        0x40..=0x44 | 0x4C => {
            let right = pop_stack_shape(stack, opcode)?;
            reject_staging(right)?;
            let left = pop_stack_shape(stack, opcode)?;
            reject_staging(left)?;
            if matches!(left, StackShape::Bool) || matches!(right, StackShape::Bool) {
                return Err(BytecodeError::from(
                    RejectionReason::ArithmeticOpcodeExpectsNumericOperands,
                ));
            }
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x45 | 0x49 => {
            reject_staging(pop_stack_shape(stack, opcode)?)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x46..=0x48 | 0x50..=0x55 => {
            reject_staging(pop_stack_shape(stack, opcode)?)?;
            reject_staging(pop_stack_shape(stack, opcode)?)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        crate::bytecode::opcodes::DEFAULT_VALUE
        | crate::bytecode::opcodes::DEFAULT_TYPED
        | crate::bytecode::opcodes::ARRAY_NEW
        | crate::bytecode::opcodes::STRUCT_NEW => {
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        crate::bytecode::opcodes::COERCE_INIT_VALUE
        | crate::bytecode::opcodes::APPLY_INIT_VALUE => {
            let value = pop_stack_shape(stack, opcode)?;
            reject_staging(value)?;
            budget.push_stack(stack, value)?;
        }
        crate::bytecode::opcodes::ARRAY_SET | crate::bytecode::opcodes::STRUCT_SET => {
            let value = pop_stack_shape(stack, opcode)?;
            let aggregate = pop_stack_shape(stack, opcode)?;
            reject_staging(value)?;
            reject_staging(aggregate)?;
            let shape =
                if value == StackShape::FrameReference || aggregate == StackShape::FrameReference {
                    StackShape::FrameReference
                } else {
                    StackShape::Unknown
                };
            budget.push_stack(stack, shape)?;
        }
        0x60 => {
            budget.push_stack(stack, StackShape::Numeric)?;
        }
        0x61 => {
            reject_staging(pop_stack_shape(stack, opcode)?)?;
            budget.push_stack(stack, StackShape::Numeric)?;
        }
        0x62 => {
            reject_staging(pop_stack_shape(stack, opcode)?)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x63 => {
            reject_staging(pop_stack_shape(stack, opcode)?)?;
            reject_staging(pop_stack_shape(stack, opcode)?)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x64 => {
            let value = pop_stack_shape(stack, opcode)?;
            reject_staging(value)?;
            if !matches!(
                value,
                StackShape::Reference
                    | StackShape::StagingReference
                    | StackShape::MaybeStagingReference
                    | StackShape::FrameReference
                    | StackShape::Instance
                    | StackShape::Unknown
            ) {
                return Err(BytecodeError::from(
                    RejectionReason::ReferenceAttemptExpectsReferenceInterfaceInstanceOrNullOperand,
                ));
            }
            budget.push_stack(
                stack,
                if value == StackShape::FrameReference {
                    value
                } else {
                    StackShape::Unknown
                },
            )?;
        }
        _ => return Err(BytecodeError::InvalidOpcode(opcode)),
    }
    Ok(())
}

/// Reference shapes retain frame/staging lifetime information through field access.
fn apply_reference_stack_instruction(
    tables: &ValidationContext<'_>,
    instr: &Instruction,
    stack: &mut Vec<StackShape>,
    result_is_local: Option<bool>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let opcode = instr.opcode;
    match opcode {
        0x20 => {
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x21 => {
            let value = pop_stack_shape(stack, opcode)?;
            reject_staging(value)?;
            let location = tables.ref_table.entries[instr.operand(0) as usize].location;
            if value == StackShape::FrameReference
                && location != RefLocation::Local
                && !(location == RefLocation::InitializerResult && result_is_local == Some(true))
            {
                return Err(
                    RejectionReason::FrameLocalReferenceCannotBeStoredToLongerLivedStorage.into(),
                );
            }
        }
        0x22 => {
            let location = tables.ref_table.entries[instr.operand(0) as usize].location;
            let shape = match location {
                RefLocation::InitializerResult => StackShape::StagingReference,
                RefLocation::Local if result_is_local.is_some() => StackShape::FrameReference,
                _ => StackShape::Reference,
            };
            budget.push_stack(stack, shape)?;
        }
        0x23 | 0x24 => {
            budget.push_stack(stack, StackShape::Instance)?;
        }
        0x25 => {
            budget.push_stack(stack, StackShape::Reference)?;
        }
        0x30 => {
            let base = pop_stack_shape(stack, opcode)?;
            if !matches!(
                base,
                StackShape::Reference
                    | StackShape::StagingReference
                    | StackShape::MaybeStagingReference
                    | StackShape::FrameReference
                    | StackShape::Instance
                    | StackShape::Unknown
            ) {
                return Err(BytecodeError::from(
                    RejectionReason::FieldReferenceExpectsReferenceOrInstanceOperand,
                ));
            }
            budget.push_stack(
                stack,
                match base {
                    StackShape::StagingReference
                    | StackShape::MaybeStagingReference
                    | StackShape::FrameReference => base,
                    _ => StackShape::Reference,
                },
            )?;
        }
        0x31 => {
            let index = pop_stack_shape(stack, opcode)?;
            let base = pop_stack_shape(stack, opcode)?;
            if !matches!(index, StackShape::Numeric | StackShape::Unknown) {
                return Err(BytecodeError::from(
                    RejectionReason::IndexedReferenceExpectsNumericIndexOperand,
                ));
            }
            if !matches!(
                base,
                StackShape::Reference
                    | StackShape::StagingReference
                    | StackShape::MaybeStagingReference
                    | StackShape::FrameReference
                    | StackShape::Unknown
            ) {
                return Err(BytecodeError::from(
                    RejectionReason::IndexedReferenceExpectsReferenceOperand,
                ));
            }
            budget.push_stack(
                stack,
                match base {
                    StackShape::StagingReference
                    | StackShape::MaybeStagingReference
                    | StackShape::FrameReference => base,
                    _ => StackShape::Reference,
                },
            )?;
        }
        0x32 => {
            let reference = pop_stack_shape(stack, opcode)?;
            if !matches!(
                reference,
                StackShape::Reference
                    | StackShape::StagingReference
                    | StackShape::MaybeStagingReference
                    | StackShape::FrameReference
                    | StackShape::Unknown
            ) {
                return Err(BytecodeError::from(
                    RejectionReason::DynamicLoadExpectsReferenceOperand,
                ));
            }
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x33 => {
            let value = pop_stack_shape(stack, opcode)?;
            reject_staging(value)?;
            let reference = pop_stack_shape(stack, opcode)?;
            if result_is_local.is_some() {
                require_staging_destination(reference)?;
            }
            if value == StackShape::FrameReference
                && reference != StackShape::FrameReference
                && !(reference == StackShape::StagingReference && result_is_local == Some(true))
            {
                return Err(
                    RejectionReason::FrameLocalReferenceCannotBeStoredThroughNonLocalReference
                        .into(),
                );
            }
            if !matches!(
                reference,
                StackShape::Reference
                    | StackShape::StagingReference
                    | StackShape::MaybeStagingReference
                    | StackShape::FrameReference
                    | StackShape::Unknown
            ) {
                return Err(BytecodeError::from(
                    RejectionReason::DynamicStoreExpectsReferenceOperand,
                ));
            }
        }
        _ => return Err(BytecodeError::InvalidOpcode(opcode)),
    }
    Ok(())
}

struct FlowEntries<'a> {
    instructions: &'a [Instruction],
    leaders: &'a [usize],
    code_len: usize,
}

fn enqueue_stack_state(
    pc: usize,
    stack: &[StackShape],
    entries: &FlowEntries<'_>,
    states: &mut [Option<Vec<StackShape>>],
    work: &mut Vec<usize>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    validate_operand_stack_depth(stack.len())?;
    if pc == entries.code_len {
        if !stack.is_empty() {
            return Err(BytecodeError::from(
                RejectionReason::PouBodyLeavesValuesOnOperandStack,
            ));
        }
        return Ok(());
    }
    let index = instruction_index(entries.instructions, pc, budget)?;
    let block = budget
        .search_by(entries.leaders, |leader, _| Ok(leader.cmp(&index)))?
        .map_err(|_| BytecodeError::InvalidJumpTarget(i32::try_from(pc).unwrap_or(i32::MAX)))?;
    let changed = if let Some(current) = states[block].as_mut() {
        if current.len() != stack.len() {
            return Err(BytecodeError::from(
                RejectionReason::InconsistentOperandStackDepthAtControlFlowMerge,
            ));
        }
        budget.work(stack.len())?;
        let mut changed = false;
        for (current, incoming) in current.iter_mut().zip(stack) {
            let joined = if *current == *incoming {
                *current
            } else if matches!(
                (*current, *incoming),
                (
                    StackShape::StagingReference | StackShape::MaybeStagingReference,
                    _
                ) | (
                    _,
                    StackShape::StagingReference | StackShape::MaybeStagingReference
                )
            ) {
                StackShape::MaybeStagingReference
            } else if matches!(
                (*current, *incoming),
                (StackShape::FrameReference, _) | (_, StackShape::FrameReference)
            ) {
                StackShape::FrameReference
            } else {
                StackShape::Unknown
            };
            if joined != *current {
                *current = joined;
                changed = true;
            }
        }
        changed
    } else {
        let mut state = Vec::new();
        budget.copy(&mut state, stack)?;
        states[block] = Some(state);
        true
    };
    if changed {
        budget.push(work, block)?;
    }
    Ok(())
}

pub(super) fn pop_stack_shape(
    stack: &mut Vec<StackShape>,
    opcode: u8,
) -> Result<StackShape, BytecodeError> {
    stack.pop().ok_or_else(|| {
        BytecodeError::InvalidSection(
            format!("operand stack underflow while decoding opcode 0x{opcode:02X}").into(),
        )
    })
}

pub(super) fn const_stack_shape(
    types: &TypeTable,
    const_pool: &ConstPool,
    const_idx: u32,
    budget: &mut ValidationBudget,
) -> Result<StackShape, BytecodeError> {
    let entry = const_pool
        .entries
        .get(const_idx as usize)
        .ok_or(BytecodeError::InvalidIndex {
            kind: "const".into(),
            index: const_idx,
        })?;
    type_stack_shape(types, entry.type_id, budget)
}

fn type_stack_shape(
    types: &TypeTable,
    mut type_id: u32,
    budget: &mut ValidationBudget,
) -> Result<StackShape, BytecodeError> {
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
            TypeData::Primitive { prim_id, .. } => {
                return Ok(match prim_id {
                    1 => StackShape::Bool,
                    6..=15 => StackShape::Numeric,
                    _ => StackShape::Unknown,
                })
            }
            TypeData::Alias { target_type_id }
            | TypeData::Subrange {
                base_type_id: target_type_id,
                ..
            } => type_id = *target_type_id,
            TypeData::Reference { .. } => return Ok(StackShape::Reference),
            TypeData::Pou { .. } => return Ok(StackShape::Instance),
            _ => return Ok(StackShape::Unknown),
        }
    }
    Err(BytecodeError::from(
        RejectionReason::TypeReferenceRecursionOverflow,
    ))
}

fn reject_staging(value: StackShape) -> Result<(), BytecodeError> {
    if matches!(
        value,
        StackShape::StagingReference | StackShape::MaybeStagingReference
    ) {
        Err(RejectionReason::InitializerReferenceEscape.into())
    } else {
        Ok(())
    }
}

fn require_staging_destination(value: StackShape) -> Result<(), BytecodeError> {
    if value == StackShape::StagingReference {
        Ok(())
    } else {
        Err(RejectionReason::InitializerWriteOutsideStaging.into())
    }
}

#[cfg(test)]
mod initializer_tests;
