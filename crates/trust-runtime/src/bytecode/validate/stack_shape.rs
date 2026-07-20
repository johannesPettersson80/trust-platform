#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackShape {
    Unknown,
    Bool,
    Numeric,
    Reference,
    Instance,
}

fn validate_stack_shape(
    types: &TypeTable,
    const_pool: &ConstPool,
    code: &[u8],
) -> Result<(), BytecodeError> {
    let (instructions, index_by_pc) = decode_stack_instructions(code)?;
    let mut states = vec![None; instructions.len()];
    let mut work = Vec::new();
    if !instructions.is_empty() {
        states[0] = Some(Vec::new());
        work.push(0);
    }

    while let Some(index) = work.pop() {
        let Some(stack) = states[index].clone() else {
            continue;
        };
        let instr = &instructions[index];
        for (pc, stack) in apply_stack_instruction(types, const_pool, instr, stack)? {
            enqueue_stack_state(
                pc,
                stack,
                code.len(),
                &index_by_pc,
                &mut states,
                &mut work,
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct StackInstruction {
    next_pc: usize,
    opcode: u8,
    operand_u32: Option<u32>,
    jump_target: Option<usize>,
    native_arg_count: Option<u32>,
}

fn decode_stack_instructions(
    code: &[u8],
) -> Result<(Vec<StackInstruction>, HashMap<usize, usize>), BytecodeError> {
    let mut reader = BytecodeReader::new(code);
    let mut instructions = Vec::new();
    let mut index_by_pc = HashMap::new();
    while reader.remaining() > 0 {
        let pc = reader.pos();
        let opcode = reader.read_u8()?;
        let mut operand_u32 = None;
        let mut jump_target = None;
        let mut native_arg_count = None;
        match opcode {
            0x00 | 0x01 | 0x06 => {}
            0x02..=0x04 => {
                let offset = reader.read_i32()?;
                let target = pc as i32 + 1 + 4 + offset;
                if target < 0 || target as usize > code.len() {
                    return Err(BytecodeError::InvalidJumpTarget(target));
                }
                jump_target = Some(target as usize);
            }
            0x05 | 0x07 | 0x10 | 0x20..=0x22 | 0x30 | 0x60 | 0x62 | 0x63 | 0x70 => {
                operand_u32 = Some(reader.read_u32()?);
            }
            0x08 => {
                let _interface_type_id = reader.read_u32()?;
                let _slot = reader.read_u32()?;
            }
            0x09 => {
                let _kind = reader.read_u32()?;
                let _symbol_idx = reader.read_u32()?;
                native_arg_count = Some(reader.read_u32()?);
            }
            0x11 | 0x12 | 0x13 | 0x23 | 0x24 | 0x25 | 0x31 | 0x32 | 0x33 | 0x40..=0x49
            | 0x4C | 0x50..=0x55 | 0x61 => {}
            _ => return Err(BytecodeError::InvalidOpcode(opcode)),
        }
        let next_pc = reader.pos();
        index_by_pc.insert(pc, instructions.len());
        instructions.push(StackInstruction {
            next_pc,
            opcode,
            operand_u32,
            jump_target,
            native_arg_count,
        });
    }
    Ok((instructions, index_by_pc))
}

fn apply_stack_instruction(
    types: &TypeTable,
    const_pool: &ConstPool,
    instr: &StackInstruction,
    mut stack: Vec<StackShape>,
) -> Result<Vec<(usize, Vec<StackShape>)>, BytecodeError> {
    let opcode = instr.opcode;
    match opcode {
        0x00 | 0x01 => {}
        0x02 => return Ok(vec![(instr.jump_target.expect("decoded jump target"), stack)]),
        0x03 | 0x04 => {
            let condition = pop_stack_shape(&mut stack, opcode)?;
            if !matches!(condition, StackShape::Bool | StackShape::Unknown) {
                return Err(BytecodeError::InvalidSection(
                    "conditional jump expects BOOL operand".into(),
                ));
            }
            return Ok(vec![
                (instr.jump_target.expect("decoded jump target"), stack.clone()),
                (instr.next_pc, stack),
            ]);
        }
        0x05 | 0x70 => {}
        0x06 => {
            if !stack.is_empty() {
                return Err(BytecodeError::InvalidSection(
                    "POU body leaves values on operand stack".into(),
                ));
            }
            return Ok(Vec::new());
        }
        0x08 => {}
        0x09 => {
            let arg_count = instr.native_arg_count.expect("decoded arg count");
            for _ in 0..arg_count {
                let _ = pop_stack_shape(&mut stack, opcode)?;
            }
            stack.push(StackShape::Unknown);
        }
        0x10 => {
            let const_idx = instr.operand_u32.expect("decoded const operand");
            stack.push(const_stack_shape(types, const_pool, const_idx)?);
        }
        0x11 => {
            let top = stack.last().copied().ok_or_else(|| {
                BytecodeError::InvalidSection("operand stack underflow on DUP".into())
            })?;
            stack.push(top);
        }
        0x12 => {
            let _ = pop_stack_shape(&mut stack, opcode)?;
        }
        0x13 => {
            if stack.len() < 2 {
                return Err(BytecodeError::InvalidSection(
                    "operand stack underflow on SWAP".into(),
                ));
            }
            let len = stack.len();
            stack.swap(len - 1, len - 2);
        }
        0x20 => {
            stack.push(StackShape::Unknown);
        }
        0x21 => {
            let _value = pop_stack_shape(&mut stack, opcode)?;
        }
        0x22 => {
            stack.push(StackShape::Reference);
        }
        0x23 | 0x24 => {
            stack.push(StackShape::Instance);
        }
        0x25 => {
            stack.push(StackShape::Reference);
        }
        0x30 => {
            let base = pop_stack_shape(&mut stack, opcode)?;
            if !matches!(
                base,
                StackShape::Reference | StackShape::Instance | StackShape::Unknown
            ) {
                return Err(BytecodeError::InvalidSection(
                    "field reference expects reference or instance operand".into(),
                ));
            }
            stack.push(StackShape::Reference);
        }
        0x31 => {
            let index = pop_stack_shape(&mut stack, opcode)?;
            let base = pop_stack_shape(&mut stack, opcode)?;
            if !matches!(index, StackShape::Numeric | StackShape::Unknown) {
                return Err(BytecodeError::InvalidSection(
                    "indexed reference expects numeric index operand".into(),
                ));
            }
            if !matches!(base, StackShape::Reference | StackShape::Unknown) {
                return Err(BytecodeError::InvalidSection(
                    "indexed reference expects reference operand".into(),
                ));
            }
            stack.push(StackShape::Reference);
        }
        0x32 => {
            let reference = pop_stack_shape(&mut stack, opcode)?;
            if !matches!(reference, StackShape::Reference | StackShape::Unknown) {
                return Err(BytecodeError::InvalidSection(
                    "dynamic load expects reference operand".into(),
                ));
            }
            stack.push(StackShape::Unknown);
        }
        0x33 => {
            let _value = pop_stack_shape(&mut stack, opcode)?;
            let reference = pop_stack_shape(&mut stack, opcode)?;
            if !matches!(reference, StackShape::Reference | StackShape::Unknown) {
                return Err(BytecodeError::InvalidSection(
                    "dynamic store expects reference operand".into(),
                ));
            }
        }
        0x40..=0x44 | 0x4C => {
            let right = pop_stack_shape(&mut stack, opcode)?;
            let left = pop_stack_shape(&mut stack, opcode)?;
            if matches!(left, StackShape::Bool) || matches!(right, StackShape::Bool) {
                return Err(BytecodeError::InvalidSection(
                    "arithmetic opcode expects numeric operands".into(),
                ));
            }
            stack.push(StackShape::Unknown);
        }
        0x45 | 0x49 => {
            let _ = pop_stack_shape(&mut stack, opcode)?;
            stack.push(StackShape::Unknown);
        }
        0x46..=0x48 | 0x50..=0x55 => {
            let _right = pop_stack_shape(&mut stack, opcode)?;
            let _left = pop_stack_shape(&mut stack, opcode)?;
            stack.push(StackShape::Unknown);
        }
        0x60 => {
            stack.push(StackShape::Numeric);
        }
        0x61 => {
            let _value = pop_stack_shape(&mut stack, opcode)?;
            stack.push(StackShape::Numeric);
        }
        0x62 => {
            let _target = pop_stack_shape(&mut stack, opcode)?;
            stack.push(StackShape::Unknown);
        }
        0x63 => {
            let _value = pop_stack_shape(&mut stack, opcode)?;
            let _target = pop_stack_shape(&mut stack, opcode)?;
            stack.push(StackShape::Unknown);
        }
        _ => return Err(BytecodeError::InvalidOpcode(opcode)),
    }
    Ok(vec![(instr.next_pc, stack)])
}

fn enqueue_stack_state(
    pc: usize,
    stack: Vec<StackShape>,
    code_len: usize,
    index_by_pc: &HashMap<usize, usize>,
    states: &mut [Option<Vec<StackShape>>],
    work: &mut Vec<usize>,
) -> Result<(), BytecodeError> {
    validate_operand_stack_depth(stack.len())?;
    if pc == code_len {
        if !stack.is_empty() {
            return Err(BytecodeError::InvalidSection(
                "POU body leaves values on operand stack".into(),
            ));
        }
        return Ok(());
    }
    let Some(index) = index_by_pc.get(&pc).copied() else {
        return Err(BytecodeError::InvalidJumpTarget(pc as i32));
    };
    if merge_stack_state(&mut states[index], stack)? {
        work.push(index);
    }
    Ok(())
}

fn merge_stack_state(
    existing: &mut Option<Vec<StackShape>>,
    incoming: Vec<StackShape>,
) -> Result<bool, BytecodeError> {
    let Some(current) = existing else {
        *existing = Some(incoming);
        return Ok(true);
    };
    if current.len() != incoming.len() {
        return Err(BytecodeError::InvalidSection(
            "inconsistent operand stack depth at control-flow merge".into(),
        ));
    }
    let mut changed = false;
    for (current_shape, incoming_shape) in current.iter_mut().zip(incoming) {
        if *current_shape != incoming_shape && *current_shape != StackShape::Unknown {
            *current_shape = StackShape::Unknown;
            changed = true;
        }
    }
    Ok(changed)
}

fn pop_stack_shape(stack: &mut Vec<StackShape>, opcode: u8) -> Result<StackShape, BytecodeError> {
    stack.pop().ok_or_else(|| {
        BytecodeError::InvalidSection(
            format!("operand stack underflow while decoding opcode 0x{opcode:02X}").into(),
        )
    })
}

fn const_stack_shape(
    types: &TypeTable,
    const_pool: &ConstPool,
    const_idx: u32,
) -> Result<StackShape, BytecodeError> {
    let entry = const_pool
        .entries
        .get(const_idx as usize)
        .ok_or(BytecodeError::InvalidIndex {
            kind: "const".into(),
            index: const_idx,
        })?;
    type_stack_shape(types, entry.type_id)
}

fn type_stack_shape(types: &TypeTable, type_id: u32) -> Result<StackShape, BytecodeError> {
    let entry = types
        .entries
        .get(type_id as usize)
        .ok_or(BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: type_id,
        })?;
    match &entry.data {
        TypeData::Primitive { prim_id, .. } => Ok(match prim_id {
            1 => StackShape::Bool,
            6..=15 => StackShape::Numeric,
            _ => StackShape::Unknown,
        }),
        TypeData::Alias { target_type_id }
        | TypeData::Subrange {
            base_type_id: target_type_id,
            ..
        } => type_stack_shape(types, *target_type_id),
        TypeData::Reference { .. } => Ok(StackShape::Reference),
        TypeData::Pou { .. } => Ok(StackShape::Instance),
        _ => Ok(StackShape::Unknown),
    }
}
