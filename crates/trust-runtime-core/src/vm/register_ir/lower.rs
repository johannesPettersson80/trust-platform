use super::*;

pub(super) mod decode;
mod fuse;
mod verify;

pub use self::decode::{collect_block_leaders, compute_block_entry_stack_depths, decode_pou};
use self::decode::{
    jump_target_pc, operand_i32, operand_native_call, operand_u32, pc_to_block_target,
};
use self::fuse::fuse_register_block_instructions_with_cost_steps;
pub use self::fuse::is_cmp_binary_op;
#[cfg(feature = "test-support")]
pub use self::fuse::{
    fuse_register_block_instructions, fuse_register_block_instructions_with_costs,
    instruction_reads_register,
};
pub use self::verify::verify_register_program;

fn canonical_stack_register(slot: u32) -> RegisterId {
    RegisterId(slot)
}

pub fn normalize_stack_for_block_exit(
    next_register: &mut u32,
    instructions: &mut Vec<RegisterInstr>,
    stack: &[RegisterId],
    protected: Option<RegisterId>,
) -> Result<Option<RegisterId>, RuntimeError> {
    let mut protected = protected;
    if let Some(register) = protected {
        let clobbered = stack.iter().enumerate().any(|(slot, src)| {
            let dest = canonical_stack_register(slot as u32);
            dest == register && *src != register
        });
        if clobbered {
            let temp = alloc_register(next_register);
            instructions.push(RegisterInstr::Move {
                src: register,
                dest: temp,
            });
            protected = Some(temp);
        }
    }

    let mut pending = Vec::<Option<(RegisterId, RegisterId)>>::new();
    let mut destinations = BTreeMap::<RegisterId, usize>::new();
    let mut uses = BTreeMap::<RegisterId, usize>::new();
    let mut source_positions = BTreeMap::<RegisterId, Vec<usize>>::new();
    let mut live = BTreeSet::<usize>::new();
    for (slot, src) in stack.iter().copied().enumerate() {
        let dest = canonical_stack_register(slot as u32);
        if src == dest {
            continue;
        }
        let index = pending.len();
        pending.push(Some((src, dest)));
        destinations.insert(dest, index);
        live.insert(index);
        *uses.entry(src).or_insert(0) += 1;
        source_positions.entry(src).or_default().push(index);
    }
    let mut ready = live
        .iter()
        .copied()
        .filter(|index| pending[*index].is_some_and(|(_, dest)| !uses.contains_key(&dest)))
        .collect::<BTreeSet<_>>();
    let mut scratch = None;
    while !live.is_empty() {
        let index = if let Some(index) = ready.pop_first() {
            index
        } else {
            *live.first().ok_or(RuntimeError::InvalidExecutionState)?
        };
        let (src, dest) = pending
            .get_mut(index)
            .and_then(Option::take)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        live.remove(&index);
        destinations.remove(&dest);
        let count = uses
            .get_mut(&src)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        *count = count
            .checked_sub(1)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if *count == 0 {
            uses.remove(&src);
            source_positions.remove(&src);
            if let Some(index) = destinations.get(&src) {
                ready.insert(*index);
            }
        }
        if !uses.contains_key(&dest) {
            instructions.push(RegisterInstr::Move { src, dest });
            continue;
        }
        // A cycle has no safe destination. Save one source and replace every
        // outstanding use through its pre-indexed positions, then requeue its
        // original destination after the remaining moves, exactly as before.
        let temp = *scratch.get_or_insert_with(|| alloc_register(next_register));
        instructions.push(RegisterInstr::Move { src, dest: temp });
        let positions = source_positions.remove(&src).unwrap_or_default();
        let mut rewritten = Vec::new();
        for position in positions {
            if let Some((other_src, _)) = pending.get_mut(position).and_then(Option::as_mut)
                && *other_src == src
            {
                *other_src = temp;
                rewritten.push(position);
            }
        }
        uses.remove(&src);
        if let Some(index) = destinations.get(&src) {
            ready.insert(*index);
        }
        let new_index = pending.len();
        pending.push(Some((temp, dest)));
        live.insert(new_index);
        destinations.insert(dest, new_index);
        rewritten.push(new_index);
        uses.insert(temp, rewritten.len());
        source_positions.insert(temp, rewritten);
    }
    Ok(protected)
}

pub fn lower_pou_to_register_ir(
    module: &VmModule,
    pou_id: u32,
) -> Result<RegisterProgram, RuntimeError> {
    let pou = module
        .pou(pou_id)
        .ok_or_else(|| invalid_bytecode(format!("vm missing pou id {pou_id}")))?;
    let decoded = decode_pou(module, pou.code_start, pou.code_end)?;
    let leaders = collect_block_leaders(&decoded, pou.code_start, pou.code_end)?;
    let entry_stack_depths =
        compute_block_entry_stack_depths(&decoded, &leaders, pou.code_start, pou.code_end)?;
    let mut start_to_block = BTreeMap::new();
    for (idx, start) in leaders.iter().copied().enumerate() {
        start_to_block.insert(start, idx as u32);
    }

    let max_entry_stack_depth = entry_stack_depths.values().copied().max().unwrap_or(0);
    let mut next_register = max_entry_stack_depth;
    let mut blocks = Vec::with_capacity(leaders.len());
    for (idx, start_pc) in leaders.iter().copied().enumerate() {
        let end_pc = leaders.get(idx + 1).copied().unwrap_or(pou.code_end);
        let entry_stack_depth = entry_stack_depths.get(&start_pc).copied().unwrap_or(0);
        let mut stack = (0..entry_stack_depth)
            .map(canonical_stack_register)
            .collect::<Vec<_>>();
        let mut opaque_mode = false;
        let mut instructions = Vec::new();
        let mut instruction_costs = Vec::new();
        let mut pending_instruction_cost = 0usize;
        let first = decoded.partition_point(|instruction| instruction.pc < start_pc);
        let last = decoded.partition_point(|instruction| instruction.pc < end_pc);
        let decoded_block = &decoded[first..last];
        let bytecode_instruction_count = decoded_block.len();

        for instr in decoded_block {
            pending_instruction_cost = pending_instruction_cost.saturating_add(1);
            let instruction_start = instructions.len();
            if opaque_mode {
                instructions.push(RegisterInstr::VmFallback {
                    opcode: instr.opcode,
                    operands: instr.owned_operands(),
                });
                instruction_costs.resize(instructions.len(), 0);
                instruction_costs[instruction_start] = pending_instruction_cost;
                pending_instruction_cost = 0;
                continue;
            }
            match instr.opcode {
                0x00 => instructions.push(RegisterInstr::Nop),
                0x01 => instructions.push(RegisterInstr::ForStepZero),
                0x02 => {
                    let offset = operand_i32(instr)?;
                    let target_pc =
                        jump_target_pc(instr.next_pc, offset, pou.code_start, pou.code_end)?;
                    let target = pc_to_block_target(target_pc, pou.code_end, &start_to_block)?;
                    normalize_stack_for_block_exit(
                        &mut next_register,
                        &mut instructions,
                        &stack,
                        None,
                    )?;
                    instructions.push(RegisterInstr::Jump { target });
                }
                0x03 | 0x04 => {
                    let cond = pop_stack(&mut stack, instr.opcode)?;
                    let offset = operand_i32(instr)?;
                    let target_pc =
                        jump_target_pc(instr.next_pc, offset, pou.code_start, pou.code_end)?;
                    let target = pc_to_block_target(target_pc, pou.code_end, &start_to_block)?;
                    let cond = normalize_stack_for_block_exit(
                        &mut next_register,
                        &mut instructions,
                        &stack,
                        Some(cond),
                    )?
                    .unwrap_or(cond);
                    instructions.push(RegisterInstr::JumpIf {
                        cond,
                        jump_if_true: instr.opcode == 0x03,
                        target,
                    });
                }
                0x06 => instructions.push(RegisterInstr::Return),
                0x09 => {
                    let (kind, symbol_idx, arg_count) = operand_native_call(instr)?;
                    let arg_count = usize::try_from(arg_count).map_err(|_| {
                        invalid_bytecode("register-ir lowering arg_count overflow on CALL_NATIVE")
                    })?;
                    if stack.len() < arg_count {
                        return Err(invalid_bytecode(
                            "register-ir lowering stack underflow on CALL_NATIVE",
                        ));
                    }
                    let split = stack.len() - arg_count;
                    let args = stack.split_off(split);
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::CallNative {
                        kind,
                        symbol_idx,
                        args,
                        dest,
                    });
                }
                0x10 => {
                    let const_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadConst { dest, const_idx });
                }
                0x11 => {
                    let top = stack.last().copied().ok_or_else(|| {
                        invalid_bytecode("register-ir lowering stack underflow on DUP")
                    })?;
                    stack.push(top);
                }
                0x12 => {
                    let _ = pop_stack(&mut stack, instr.opcode)?;
                }
                0x13 => {
                    if stack.len() < 2 {
                        return Err(invalid_bytecode(
                            "register-ir lowering stack underflow on SWAP",
                        ));
                    }
                    let len = stack.len();
                    stack.swap(len - 1, len - 2);
                }
                0x20 => {
                    let ref_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadRef { dest, ref_idx });
                }
                0x21 => {
                    let src = pop_stack(&mut stack, instr.opcode)?;
                    let ref_idx = operand_u32(instr)?;
                    instructions.push(RegisterInstr::StoreRef { ref_idx, src });
                }
                0x22 => {
                    let ref_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadRefAddr { dest, ref_idx });
                }
                0x25 => {
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadNull { dest });
                }
                0x23 => {
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadSelf { dest });
                }
                0x24 => {
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadSuper { dest });
                }
                0x30 => {
                    let field_idx = operand_u32(instr)?;
                    let base = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::RefField {
                        base,
                        field_idx,
                        dest,
                    });
                }
                0x31 => {
                    let index = pop_stack(&mut stack, instr.opcode)?;
                    let base = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::RefIndex { base, index, dest });
                }
                0x32 => {
                    let reference = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::LoadDynamic { reference, dest });
                }
                0x33 => {
                    let value = pop_stack(&mut stack, instr.opcode)?;
                    let reference = pop_stack(&mut stack, instr.opcode)?;
                    instructions.push(RegisterInstr::StoreDynamic { reference, value });
                }
                0x40 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Add,
                    instr.opcode,
                )?,
                0x41 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Sub,
                    instr.opcode,
                )?,
                0x42 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Mul,
                    instr.opcode,
                )?,
                0x43 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Div,
                    instr.opcode,
                )?,
                0x44 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Mod,
                    instr.opcode,
                )?,
                0x45 => lower_unary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    UnaryOp::Neg,
                    instr.opcode,
                )?,
                0x46 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::And,
                    instr.opcode,
                )?,
                0x47 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Or,
                    instr.opcode,
                )?,
                0x48 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Xor,
                    instr.opcode,
                )?,
                0x49 => lower_unary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    UnaryOp::Not,
                    instr.opcode,
                )?,
                0x50 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Eq,
                    instr.opcode,
                )?,
                0x51 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Ne,
                    instr.opcode,
                )?,
                0x52 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Lt,
                    instr.opcode,
                )?,
                0x53 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Le,
                    instr.opcode,
                )?,
                0x54 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Gt,
                    instr.opcode,
                )?,
                0x55 => lower_binary(
                    &mut next_register,
                    &mut stack,
                    &mut instructions,
                    BinaryOp::Ge,
                    instr.opcode,
                )?,
                0x60 => {
                    let type_idx = operand_u32(instr)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::SizeOfType { type_idx, dest });
                }
                0x61 => {
                    let src = pop_stack(&mut stack, instr.opcode)?;
                    let dest = alloc_register(&mut next_register);
                    stack.push(dest);
                    instructions.push(RegisterInstr::SizeOfValue { src, dest });
                }
                _ => {
                    instructions.push(RegisterInstr::VmFallback {
                        opcode: instr.opcode,
                        operands: instr.owned_operands(),
                    });
                    opaque_mode = true;
                }
            }
            if instructions.len() > instruction_start {
                instruction_costs.resize(instructions.len(), 0);
                instruction_costs[instruction_start] = pending_instruction_cost;
                pending_instruction_cost = 0;
            }
        }
        if pending_instruction_cost > 0 {
            instructions.push(RegisterInstr::Nop);
            instruction_costs.push(pending_instruction_cost);
        }

        let terminates_control_flow = instructions.last().is_some_and(|instruction| {
            matches!(
                instruction,
                RegisterInstr::Jump { .. }
                    | RegisterInstr::JumpIf { .. }
                    | RegisterInstr::Return
                    | RegisterInstr::ForStepZero
            )
        });
        if !terminates_control_flow {
            normalize_stack_for_block_exit(&mut next_register, &mut instructions, &stack, None)?;
            instruction_costs.resize(instructions.len(), 0);
        }

        let fused =
            fuse_register_block_instructions_with_cost_steps(&instructions, &instruction_costs)?;
        blocks.push(RegisterBlock {
            id: idx as u32,
            start_pc,
            end_pc,
            entry_stack_depth,
            bytecode_instruction_count,
            instruction_costs: fused.costs,
            instruction_cost_steps: fused.cost_steps,
            instructions: fused.instructions,
        });
    }

    let lowered = RegisterProgram {
        pou_id,
        entry_block: 0,
        max_registers: next_register,
        blocks,
    };
    verify_register_program(&lowered)?;
    Ok(lowered)
}

fn alloc_register(next_register: &mut u32) -> RegisterId {
    let reg = RegisterId(*next_register);
    *next_register = next_register.saturating_add(1);
    reg
}

fn pop_stack(stack: &mut Vec<RegisterId>, opcode: u8) -> Result<RegisterId, RuntimeError> {
    stack.pop().ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir lowering stack underflow while decoding opcode 0x{opcode:02X}",
        ))
    })
}

fn lower_unary(
    next_register: &mut u32,
    stack: &mut Vec<RegisterId>,
    instructions: &mut Vec<RegisterInstr>,
    op: UnaryOp,
    opcode: u8,
) -> Result<(), RuntimeError> {
    let src = pop_stack(stack, opcode)?;
    let dest = alloc_register(next_register);
    stack.push(dest);
    instructions.push(RegisterInstr::Unary { op, src, dest });
    Ok(())
}

fn lower_binary(
    next_register: &mut u32,
    stack: &mut Vec<RegisterId>,
    instructions: &mut Vec<RegisterInstr>,
    op: BinaryOp,
    opcode: u8,
) -> Result<(), RuntimeError> {
    let right = pop_stack(stack, opcode)?;
    let left = pop_stack(stack, opcode)?;
    let dest = alloc_register(next_register);
    stack.push(dest);
    instructions.push(RegisterInstr::Binary {
        op,
        left,
        right,
        dest,
    });
    Ok(())
}
