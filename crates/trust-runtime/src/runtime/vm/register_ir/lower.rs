use super::*;

fn canonical_stack_register(slot: u32) -> RegisterId {
    RegisterId(slot)
}

fn pop_stack_depth(depth: &mut u32, opcode: u8) -> Result<(), RuntimeError> {
    if *depth == 0 {
        return Err(invalid_bytecode(format!(
            "register-ir lowering stack underflow while decoding opcode 0x{opcode:02X}",
        )));
    }
    *depth -= 1;
    Ok(())
}

fn propagate_block_entry_stack_depth(
    entry_depths: &mut [Option<u32>],
    block_index: usize,
    depth: u32,
    worklist: &mut VecDeque<usize>,
) -> Result<(), RuntimeError> {
    match entry_depths.get_mut(block_index) {
        Some(slot @ None) => {
            *slot = Some(depth);
            worklist.push_back(block_index);
        }
        Some(Some(existing)) if *existing != depth => {
            return Err(invalid_bytecode(format!(
                "register-ir inconsistent block-entry stack depth for block {block_index}: {existing} vs {depth}",
            )));
        }
        Some(Some(_)) => {}
        None => {
            return Err(invalid_bytecode(format!(
                "register-ir missing block {block_index} while propagating stack depth",
            )));
        }
    }
    Ok(())
}

fn apply_decoded_stack_effect(depth: &mut u32, instr: &DecodedInstr) -> Result<(), RuntimeError> {
    match instr.opcode {
        0x00 | 0x01 | 0x02 | 0x05 | 0x06 | 0x07 | 0x08 | 0x70 => {}
        0x03 | 0x04 | 0x12 => pop_stack_depth(depth, instr.opcode)?,
        0x10 | 0x20 | 0x22 | 0x23 | 0x24 | 0x25 | 0x60 => {
            *depth = depth.saturating_add(1);
        }
        0x09 => {
            let (_, _, arg_count) = operand_native_call(instr)?;
            for _ in 0..arg_count {
                pop_stack_depth(depth, instr.opcode)?;
            }
            *depth = depth.saturating_add(1);
        }
        0x11 => {
            if *depth == 0 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on DUP",
                ));
            }
            *depth = depth.saturating_add(1);
        }
        0x13 => {
            if *depth < 2 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on SWAP",
                ));
            }
        }
        0x14 => {
            if *depth < 3 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on ROT3",
                ));
            }
        }
        0x15 => {
            if *depth < 4 {
                return Err(invalid_bytecode(
                    "register-ir lowering stack underflow on ROT4",
                ));
            }
        }
        0x16 | 0x30 | 0x32 | 0x45 | 0x49 | 0x61 | 0x62 => {
            if *depth == 0 {
                return Err(invalid_bytecode(format!(
                    "register-ir lowering stack underflow while decoding opcode 0x{:02X}",
                    instr.opcode,
                )));
            }
        }
        0x21 => pop_stack_depth(depth, instr.opcode)?,
        0x31 | 0x40..=0x44 | 0x46..=0x48 | 0x4A..=0x4E | 0x50..=0x55 | 0x63 => {
            pop_stack_depth(depth, instr.opcode)?;
        }
        0x33 => {
            pop_stack_depth(depth, instr.opcode)?;
            pop_stack_depth(depth, instr.opcode)?;
        }
        _ => {
            return Err(invalid_bytecode(format!(
                "register-ir unsupported stack-effect analysis for opcode 0x{:02X}",
                instr.opcode,
            )));
        }
    }
    Ok(())
}

fn compute_block_entry_stack_depths(
    decoded: &[DecodedInstr],
    leaders: &[usize],
    code_start: usize,
    code_end: usize,
) -> Result<HashMap<usize, u32>, RuntimeError> {
    let mut start_to_index = HashMap::new();
    for (index, start_pc) in leaders.iter().copied().enumerate() {
        start_to_index.insert(start_pc, index);
    }

    let mut entry_depths = vec![None; leaders.len()];
    let mut worklist = VecDeque::new();
    if !leaders.is_empty() {
        entry_depths[0] = Some(0);
        worklist.push_back(0);
    }

    while let Some(block_index) = worklist.pop_front() {
        let start_pc = leaders[block_index];
        let end_pc = leaders.get(block_index + 1).copied().unwrap_or(code_end);
        let mut depth = entry_depths[block_index].unwrap_or(0);
        let mut terminated = false;

        for instr in decoded
            .iter()
            .filter(|instr| instr.pc >= start_pc && instr.pc < end_pc)
        {
            match instr.opcode {
                0x02 => {
                    let offset = operand_i32(instr)?;
                    let target_pc = jump_target_pc(instr.next_pc, offset, code_start, code_end)?;
                    if target_pc < code_end {
                        let target_index = *start_to_index.get(&target_pc).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "register-ir jump target {target_pc} is not a block leader"
                            ))
                        })?;
                        propagate_block_entry_stack_depth(
                            &mut entry_depths,
                            target_index,
                            depth,
                            &mut worklist,
                        )?;
                    }
                    terminated = true;
                    break;
                }
                0x03 | 0x04 => {
                    pop_stack_depth(&mut depth, instr.opcode)?;
                    let offset = operand_i32(instr)?;
                    let target_pc = jump_target_pc(instr.next_pc, offset, code_start, code_end)?;
                    if target_pc < code_end {
                        let target_index = *start_to_index.get(&target_pc).ok_or_else(|| {
                            invalid_bytecode(format!(
                                "register-ir jump target {target_pc} is not a block leader"
                            ))
                        })?;
                        propagate_block_entry_stack_depth(
                            &mut entry_depths,
                            target_index,
                            depth,
                            &mut worklist,
                        )?;
                    }
                    if instr.next_pc < code_end {
                        let fallthrough_index =
                            *start_to_index.get(&instr.next_pc).ok_or_else(|| {
                                invalid_bytecode(format!(
                                    "register-ir fallthrough target {} is not a block leader",
                                    instr.next_pc,
                                ))
                            })?;
                        propagate_block_entry_stack_depth(
                            &mut entry_depths,
                            fallthrough_index,
                            depth,
                            &mut worklist,
                        )?;
                    }
                    terminated = true;
                    break;
                }
                0x06 => {
                    terminated = true;
                    break;
                }
                _ => apply_decoded_stack_effect(&mut depth, instr)?,
            }
        }

        if !terminated {
            if let Some(next_start_pc) = leaders.get(block_index + 1).copied() {
                let next_index = *start_to_index.get(&next_start_pc).ok_or_else(|| {
                    invalid_bytecode(format!(
                        "register-ir fallthrough target {next_start_pc} is not a block leader",
                    ))
                })?;
                propagate_block_entry_stack_depth(
                    &mut entry_depths,
                    next_index,
                    depth,
                    &mut worklist,
                )?;
            }
        }
    }

    let mut resolved = HashMap::new();
    for (index, start_pc) in leaders.iter().copied().enumerate() {
        if let Some(depth) = entry_depths[index] {
            resolved.insert(start_pc, depth);
        }
    }
    Ok(resolved)
}

fn normalize_stack_for_block_exit(
    next_register: &mut u32,
    instructions: &mut Vec<RegisterInstr>,
    stack: &[RegisterId],
    protected: Option<RegisterId>,
) -> Option<RegisterId> {
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

    let mut pending = stack
        .iter()
        .enumerate()
        .filter_map(|(slot, src)| {
            let dest = canonical_stack_register(slot as u32);
            (*src != dest).then_some((*src, dest))
        })
        .collect::<Vec<_>>();
    let mut scratch = None;

    while !pending.is_empty() {
        if let Some(index) = pending
            .iter()
            .position(|(_, dest)| !pending.iter().any(|(other_src, _)| *other_src == *dest))
        {
            let (src, dest) = pending.remove(index);
            instructions.push(RegisterInstr::Move { src, dest });
            continue;
        }

        let (src, dest) = pending.remove(0);
        let temp = *scratch.get_or_insert_with(|| alloc_register(next_register));
        instructions.push(RegisterInstr::Move { src, dest: temp });
        for (other_src, _) in &mut pending {
            if *other_src == src {
                *other_src = temp;
            }
        }
        pending.push((temp, dest));
    }

    protected
}

pub(super) fn lower_pou_to_register_ir(
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
    let mut start_to_block = HashMap::new();
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

        for instr in decoded
            .iter()
            .filter(|instr| instr.pc >= start_pc && instr.pc < end_pc)
        {
            if opaque_mode {
                instructions.push(RegisterInstr::VmFallback {
                    opcode: instr.opcode,
                    operands: instr.operands.clone(),
                });
                continue;
            }

            match instr.opcode {
                0x00 => instructions.push(RegisterInstr::Nop),
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
                    );
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
                    )
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
                        operands: instr.operands.clone(),
                    });
                    opaque_mode = true;
                }
            }
        }

        let terminates_control_flow = instructions.last().is_some_and(|instruction| {
            matches!(
                instruction,
                RegisterInstr::Jump { .. } | RegisterInstr::JumpIf { .. } | RegisterInstr::Return
            )
        });
        if !terminates_control_flow {
            normalize_stack_for_block_exit(&mut next_register, &mut instructions, &stack, None);
        }

        let instructions = fuse_register_block_instructions(&instructions);
        blocks.push(RegisterBlock {
            id: idx as u32,
            start_pc,
            end_pc,
            entry_stack_depth,
            instructions,
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

fn fuse_register_block_instructions(instructions: &[RegisterInstr]) -> Vec<RegisterInstr> {
    if instructions.len() < 4 {
        return instructions.to_vec();
    }
    let mut fused = Vec::with_capacity(instructions.len());
    let mut index = 0usize;
    while index < instructions.len() {
        if let Some((instruction, consumed)) = try_fuse_instruction_window(instructions, index) {
            fused.push(instruction);
            index += consumed;
            continue;
        }
        fused.push(instructions[index].clone());
        index += 1;
    }
    fused
}

fn try_fuse_instruction_window(
    instructions: &[RegisterInstr],
    index: usize,
) -> Option<(RegisterInstr, usize)> {
    if index + 3 >= instructions.len() {
        return None;
    }

    if let (
        RegisterInstr::LoadRef {
            dest: left_reg,
            ref_idx: left_ref_idx,
        },
        RegisterInstr::LoadRef {
            dest: right_reg,
            ref_idx: right_ref_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == left_reg
            && right == right_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *left_reg)
            && !register_used_after(instructions, index + 4, *right_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryRefToRef {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadRef {
            dest: left_reg,
            ref_idx: left_ref_idx,
        },
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == left_reg
            && right == const_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *left_reg)
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryRefConstToRef {
                    op: *op,
                    left_ref_idx: *left_ref_idx,
                    const_idx: *const_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::LoadRef {
            dest: right_reg,
            ref_idx: right_ref_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::StoreRef { ref_idx, src },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if left == const_reg
            && right == right_reg
            && src == dest
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *right_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::BinaryConstRefToRef {
                    op: *op,
                    const_idx: *const_idx,
                    right_ref_idx: *right_ref_idx,
                    dest_ref_idx: *ref_idx,
                },
                4,
            ));
        }
    }

    if let (
        RegisterInstr::LoadRef {
            dest: ref_reg,
            ref_idx,
        },
        RegisterInstr::LoadConst {
            dest: const_reg,
            const_idx,
        },
        RegisterInstr::Binary {
            op,
            left,
            right,
            dest,
        },
        RegisterInstr::JumpIf {
            cond,
            jump_if_true,
            target,
        },
    ) = (
        &instructions[index],
        &instructions[index + 1],
        &instructions[index + 2],
        &instructions[index + 3],
    ) {
        if is_cmp_binary_op(*op)
            && left == ref_reg
            && right == const_reg
            && cond == dest
            && !register_used_after(instructions, index + 4, *ref_reg)
            && !register_used_after(instructions, index + 4, *const_reg)
            && !register_used_after(instructions, index + 4, *dest)
        {
            return Some((
                RegisterInstr::CmpRefConstJumpIf {
                    op: *op,
                    ref_idx: *ref_idx,
                    const_idx: *const_idx,
                    jump_if_true: *jump_if_true,
                    target: *target,
                },
                4,
            ));
        }
    }

    None
}

fn register_used_after(
    instructions: &[RegisterInstr],
    start_index: usize,
    register: RegisterId,
) -> bool {
    instructions[start_index..]
        .iter()
        .any(|instruction| instruction_reads_register(instruction, register))
}

fn instruction_reads_register(instruction: &RegisterInstr, register: RegisterId) -> bool {
    match instruction {
        RegisterInstr::CallNative { args, .. } => args.contains(&register),
        RegisterInstr::SizeOfValue { src, .. } => *src == register,
        RegisterInstr::RefField { base, .. } => *base == register,
        RegisterInstr::RefIndex { base, index, .. } => *base == register || *index == register,
        RegisterInstr::LoadDynamic { reference, .. } => *reference == register,
        RegisterInstr::StoreDynamic { reference, value } => {
            *reference == register || *value == register
        }
        RegisterInstr::Unary { src, .. } => *src == register,
        RegisterInstr::Binary { left, right, .. } => *left == register || *right == register,
        RegisterInstr::StoreRef { src, .. } => *src == register,
        RegisterInstr::JumpIf { cond, .. } => *cond == register,
        _ => false,
    }
}

pub(super) fn is_cmp_binary_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    )
}

pub(super) fn verify_register_program(program: &RegisterProgram) -> Result<(), RuntimeError> {
    if program.blocks.is_empty() {
        return Err(invalid_bytecode("register-ir program has no blocks"));
    }
    let known_blocks = program
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<HashSet<_>>();
    if !known_blocks.contains(&program.entry_block) {
        return Err(invalid_bytecode(format!(
            "register-ir entry block {} missing",
            program.entry_block
        )));
    }

    for block in &program.blocks {
        let mut defined = (0..block.entry_stack_depth)
            .map(RegisterId)
            .collect::<BTreeSet<_>>();
        for instr in &block.instructions {
            match instr {
                RegisterInstr::LoadConst { dest, .. }
                | RegisterInstr::LoadRef { dest, .. }
                | RegisterInstr::LoadRefAddr { dest, .. }
                | RegisterInstr::LoadNull { dest }
                | RegisterInstr::LoadSelf { dest }
                | RegisterInstr::LoadSuper { dest }
                | RegisterInstr::SizeOfType { dest, .. } => {
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Move { src, dest } => {
                    verify_src(src, &defined)?;
                    verify_move_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::SizeOfValue { src, dest } => {
                    verify_src(src, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::RefField { base, dest, .. } => {
                    verify_src(base, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::RefIndex {
                    base, index, dest, ..
                } => {
                    verify_src(base, &defined)?;
                    verify_src(index, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::LoadDynamic { reference, dest } => {
                    verify_src(reference, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::StoreDynamic { reference, value } => {
                    verify_src(reference, &defined)?;
                    verify_src(value, &defined)?;
                }
                RegisterInstr::CallNative { args, dest, .. } => {
                    for arg in args {
                        verify_src(arg, &defined)?;
                    }
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Unary { src, dest, .. } => {
                    verify_src(src, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::Binary {
                    left, right, dest, ..
                } => {
                    verify_src(left, &defined)?;
                    verify_src(right, &defined)?;
                    verify_dest(dest, program.max_registers, &mut defined)?;
                }
                RegisterInstr::CmpRefConstJumpIf { target, .. } => {
                    verify_target(target, &known_blocks)?;
                }
                RegisterInstr::StoreRef { src, .. } => {
                    verify_src(src, &defined)?;
                }
                RegisterInstr::Jump { target } => verify_target(target, &known_blocks)?,
                RegisterInstr::JumpIf { cond, target, .. } => {
                    verify_src(cond, &defined)?;
                    verify_target(target, &known_blocks)?;
                }
                RegisterInstr::BinaryRefToRef { .. }
                | RegisterInstr::BinaryRefConstToRef { .. }
                | RegisterInstr::BinaryConstRefToRef { .. }
                | RegisterInstr::Nop
                | RegisterInstr::Return
                | RegisterInstr::VmFallback { .. } => {}
            }
        }
    }

    Ok(())
}

fn verify_dest(
    dest: &RegisterId,
    max_registers: u32,
    defined: &mut BTreeSet<RegisterId>,
) -> Result<(), RuntimeError> {
    if dest.index() >= max_registers {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} out of bounds (max={max_registers})",
            dest.index()
        )));
    }
    if !defined.insert(*dest) {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} redefined in block",
            dest.index()
        )));
    }
    Ok(())
}

fn verify_move_dest(
    dest: &RegisterId,
    max_registers: u32,
    defined: &mut BTreeSet<RegisterId>,
) -> Result<(), RuntimeError> {
    if dest.index() >= max_registers {
        return Err(invalid_bytecode(format!(
            "register-ir destination register {} out of bounds (max={max_registers})",
            dest.index()
        )));
    }
    defined.insert(*dest);
    Ok(())
}

fn verify_src(src: &RegisterId, defined: &BTreeSet<RegisterId>) -> Result<(), RuntimeError> {
    if !defined.contains(src) {
        return Err(invalid_bytecode(format!(
            "register-ir source register {} used before definition",
            src.index()
        )));
    }
    Ok(())
}

fn verify_target(target: &BlockTarget, known_blocks: &HashSet<u32>) -> Result<(), RuntimeError> {
    if let BlockTarget::Block(id) = target {
        if !known_blocks.contains(id) {
            return Err(invalid_bytecode(format!(
                "register-ir unknown block target {id}",
            )));
        }
    }
    Ok(())
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

fn decode_pou(
    module: &VmModule,
    code_start: usize,
    code_end: usize,
) -> Result<Vec<DecodedInstr>, RuntimeError> {
    let mut decoded = Vec::new();
    let mut pc = code_start;
    while pc < code_end {
        let opcode = module.code.get(pc).copied().ok_or_else(|| {
            invalid_bytecode("register-ir decode instruction fetch out of bounds")
        })?;
        let operand_len = opcode_operand_len_for_lowering(opcode).ok_or_else(|| {
            invalid_bytecode(format!("register-ir decode invalid opcode 0x{opcode:02X}"))
        })?;
        let next_pc = pc + 1 + operand_len;
        if next_pc > code_end {
            return Err(invalid_bytecode(
                "register-ir decode unexpected end of input while reading operands",
            ));
        }
        let operands = module.code[(pc + 1)..next_pc].to_vec();
        decoded.push(DecodedInstr {
            pc,
            next_pc,
            opcode,
            operands,
        });
        pc = next_pc;
    }
    Ok(decoded)
}

fn opcode_operand_len_for_lowering(opcode: u8) -> Option<usize> {
    opcode_operand_len(opcode).or(match opcode {
        0x25 => Some(0),
        _ => None,
    })
}

fn collect_block_leaders(
    decoded: &[DecodedInstr],
    code_start: usize,
    code_end: usize,
) -> Result<Vec<usize>, RuntimeError> {
    let mut leaders = BTreeSet::new();
    leaders.insert(code_start);
    for instr in decoded {
        if let 0x02..=0x04 = instr.opcode {
            let offset = operand_i32(instr)?;
            let target = jump_target_pc(instr.next_pc, offset, code_start, code_end)?;
            if target < code_end {
                leaders.insert(target);
            }
            if instr.opcode != 0x02 && instr.next_pc < code_end {
                leaders.insert(instr.next_pc);
            }
        }
    }
    Ok(leaders.into_iter().collect())
}

fn jump_target_pc(
    pc_after_operand: usize,
    offset: i32,
    code_start: usize,
    code_end: usize,
) -> Result<usize, RuntimeError> {
    let base = pc_after_operand as i64;
    let target = base + i64::from(offset);
    if target < code_start as i64 || target > code_end as i64 {
        return Err(invalid_bytecode(format!(
            "register-ir invalid jump target {target}",
        )));
    }
    Ok(target as usize)
}

fn pc_to_block_target(
    target_pc: usize,
    code_end: usize,
    start_to_block: &HashMap<usize, u32>,
) -> Result<BlockTarget, RuntimeError> {
    if target_pc == code_end {
        return Ok(BlockTarget::Exit);
    }
    let id = start_to_block.get(&target_pc).copied().ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir jump target {target_pc} is not a block leader"
        ))
    })?;
    Ok(BlockTarget::Block(id))
}

fn operand_u32(instr: &DecodedInstr) -> Result<u32, RuntimeError> {
    if instr.operands.len() != 4 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 4-byte operand",
            instr.opcode
        )));
    }
    operand_u32_slice(instr, 0)
}

fn operand_native_call(instr: &DecodedInstr) -> Result<(u32, u32, u32), RuntimeError> {
    if instr.operands.len() != 12 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 12-byte operand",
            instr.opcode
        )));
    }
    Ok((
        operand_u32_slice(instr, 0)?,
        operand_u32_slice(instr, 4)?,
        operand_u32_slice(instr, 8)?,
    ))
}

fn operand_i32(instr: &DecodedInstr) -> Result<i32, RuntimeError> {
    if instr.operands.len() != 4 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 4-byte operand",
            instr.opcode
        )));
    }
    let bytes = [
        instr.operands[0],
        instr.operands[1],
        instr.operands[2],
        instr.operands[3],
    ];
    Ok(i32::from_le_bytes(bytes))
}

fn operand_u32_slice(instr: &DecodedInstr, offset: usize) -> Result<u32, RuntimeError> {
    let end = offset.saturating_add(4);
    if instr.operands.len() < end {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} missing operand bytes at offset {offset}",
            instr.opcode
        )));
    }
    let bytes = [
        instr.operands[offset],
        instr.operands[offset + 1],
        instr.operands[offset + 2],
        instr.operands[offset + 3],
    ];
    Ok(u32::from_le_bytes(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedInstr {
    pc: usize,
    next_pc: usize,
    opcode: u8,
    operands: Vec<u8>,
}
