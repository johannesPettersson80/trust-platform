use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StackShape {
    Unknown,
    Bool,
    Numeric,
    Reference,
    Instance,
}

pub(super) fn validate_stack_shape(
    tables: &ValidationContext<'_>,
    instructions: &[Instruction],
    code_len: usize,
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
            apply_stack_instruction(tables, instruction, &mut stack, budget)?;
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
            for _ in 0..arg_count {
                let _ = pop_stack_shape(stack, opcode)?;
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
        0x20 => {
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x21 => {
            let _value = pop_stack_shape(stack, opcode)?;
        }
        0x22 => {
            budget.push_stack(stack, StackShape::Reference)?;
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
                StackShape::Reference | StackShape::Instance | StackShape::Unknown
            ) {
                return Err(BytecodeError::from(
                    RejectionReason::FieldReferenceExpectsReferenceOrInstanceOperand,
                ));
            }
            budget.push_stack(stack, StackShape::Reference)?;
        }
        0x31 => {
            let index = pop_stack_shape(stack, opcode)?;
            let base = pop_stack_shape(stack, opcode)?;
            if !matches!(index, StackShape::Numeric | StackShape::Unknown) {
                return Err(BytecodeError::from(
                    RejectionReason::IndexedReferenceExpectsNumericIndexOperand,
                ));
            }
            if !matches!(base, StackShape::Reference | StackShape::Unknown) {
                return Err(BytecodeError::from(
                    RejectionReason::IndexedReferenceExpectsReferenceOperand,
                ));
            }
            budget.push_stack(stack, StackShape::Reference)?;
        }
        0x32 => {
            let reference = pop_stack_shape(stack, opcode)?;
            if !matches!(reference, StackShape::Reference | StackShape::Unknown) {
                return Err(BytecodeError::from(
                    RejectionReason::DynamicLoadExpectsReferenceOperand,
                ));
            }
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x33 => {
            let _value = pop_stack_shape(stack, opcode)?;
            let reference = pop_stack_shape(stack, opcode)?;
            if !matches!(reference, StackShape::Reference | StackShape::Unknown) {
                return Err(BytecodeError::from(
                    RejectionReason::DynamicStoreExpectsReferenceOperand,
                ));
            }
        }
        0x40..=0x44 | 0x4C => {
            let right = pop_stack_shape(stack, opcode)?;
            let left = pop_stack_shape(stack, opcode)?;
            if matches!(left, StackShape::Bool) || matches!(right, StackShape::Bool) {
                return Err(BytecodeError::from(
                    RejectionReason::ArithmeticOpcodeExpectsNumericOperands,
                ));
            }
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x45 | 0x49 => {
            let _ = pop_stack_shape(stack, opcode)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x46..=0x48 | 0x50..=0x55 => {
            let _right = pop_stack_shape(stack, opcode)?;
            let _left = pop_stack_shape(stack, opcode)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x60 => {
            budget.push_stack(stack, StackShape::Numeric)?;
        }
        0x61 => {
            let _value = pop_stack_shape(stack, opcode)?;
            budget.push_stack(stack, StackShape::Numeric)?;
        }
        0x62 => {
            let _target = pop_stack_shape(stack, opcode)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x63 => {
            let _value = pop_stack_shape(stack, opcode)?;
            let _target = pop_stack_shape(stack, opcode)?;
            budget.push_stack(stack, StackShape::Unknown)?;
        }
        0x64 => {
            let value = pop_stack_shape(stack, opcode)?;
            if !matches!(
                value,
                StackShape::Reference | StackShape::Instance | StackShape::Unknown
            ) {
                return Err(BytecodeError::from(
                    RejectionReason::ReferenceAttemptExpectsReferenceInterfaceInstanceOrNullOperand,
                ));
            }
            budget.push_stack(stack, StackShape::Unknown)?;
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
            if *current != *incoming && *current != StackShape::Unknown {
                *current = StackShape::Unknown;
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
