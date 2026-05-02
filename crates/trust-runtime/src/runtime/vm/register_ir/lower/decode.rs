use super::*;

const MAX_INLINE_OPERAND_BYTES: usize = 12;
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

pub(in crate::runtime::vm::register_ir) fn compute_block_entry_stack_depths(
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

pub(in crate::runtime::vm::register_ir) fn decode_pou(
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
        if operand_len > MAX_INLINE_OPERAND_BYTES {
            return Err(invalid_bytecode(format!(
                "register-ir decode opcode 0x{opcode:02X} operand length {operand_len} exceeds inline storage"
            )));
        }
        let mut operands = [0_u8; MAX_INLINE_OPERAND_BYTES];
        operands[..operand_len].copy_from_slice(&module.code[(pc + 1)..next_pc]);
        decoded.push(DecodedInstr {
            pc,
            next_pc,
            opcode,
            operand_len,
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

pub(in crate::runtime::vm::register_ir) fn collect_block_leaders(
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

pub(super) fn jump_target_pc(
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

pub(super) fn pc_to_block_target(
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

pub(super) fn operand_u32(instr: &DecodedInstr) -> Result<u32, RuntimeError> {
    if instr.operands().len() != 4 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 4-byte operand",
            instr.opcode
        )));
    }
    operand_u32_slice(instr, 0)
}

pub(super) fn operand_native_call(instr: &DecodedInstr) -> Result<(u32, u32, u32), RuntimeError> {
    if instr.operands().len() != 12 {
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

pub(super) fn operand_i32(instr: &DecodedInstr) -> Result<i32, RuntimeError> {
    let operands = instr.operands();
    if operands.len() != 4 {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} expected 4-byte operand",
            instr.opcode
        )));
    }
    let bytes = [operands[0], operands[1], operands[2], operands[3]];
    Ok(i32::from_le_bytes(bytes))
}

fn operand_u32_slice(instr: &DecodedInstr, offset: usize) -> Result<u32, RuntimeError> {
    let end = offset.saturating_add(4);
    let operands = instr.operands();
    if operands.len() < end {
        return Err(invalid_bytecode(format!(
            "register-ir opcode 0x{:02X} missing operand bytes at offset {offset}",
            instr.opcode
        )));
    }
    let bytes = [
        operands[offset],
        operands[offset + 1],
        operands[offset + 2],
        operands[offset + 3],
    ];
    Ok(u32::from_le_bytes(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime::vm::register_ir) struct DecodedInstr {
    pub(super) pc: usize,
    pub(super) next_pc: usize,
    pub(super) opcode: u8,
    pub(super) operand_len: usize,
    pub(super) operands: [u8; MAX_INLINE_OPERAND_BYTES],
}

impl DecodedInstr {
    fn operands(&self) -> &[u8] {
        &self.operands[..self.operand_len]
    }

    pub(super) fn owned_operands(&self) -> Vec<u8> {
        self.operands().to_vec()
    }
}
