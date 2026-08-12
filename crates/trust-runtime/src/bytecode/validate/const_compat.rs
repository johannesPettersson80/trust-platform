fn validate_const_compat(
    types: &TypeTable,
    const_pool: &ConstPool,
    ref_table: &RefTable,
    var_meta: Option<&VarMeta>,
    code: &[u8],
) -> Result<(), BytecodeError> {
    let Some(var_meta) = var_meta else {
        return Ok(());
    };
    let ref_types = ref_type_map(var_meta);
    let mut reader = BytecodeReader::new(code);
    let mut stack = Vec::new();
    while reader.remaining() > 0 {
        let opcode = reader.read_u8()?;
        match opcode {
            0x00 | 0x01 | 0x06 => {}
            0x02 | 0x05 | 0x70 => {
                let _ = reader.read_u32()?;
            }
            0x03 | 0x04 => {
                let _offset = reader.read_i32()?;
                let _ = pop_const_type(&mut stack);
            }
            0x09 => {
                let _kind = reader.read_u32()?;
                let _symbol_idx = reader.read_u32()?;
                let arg_count = reader.read_u32()?;
                for _ in 0..arg_count {
                    let _ = pop_const_type(&mut stack);
                }
                stack.push(None);
            }
            0x10 => {
                let const_idx = reader.read_u32()?;
                let entry =
                    const_pool
                        .entries
                        .get(const_idx as usize)
                        .ok_or(BytecodeError::InvalidIndex {
                            kind: "const".into(),
                            index: const_idx,
                        })?;
                stack.push(Some(entry.type_id));
            }
            0x11 => {
                let top = stack.last().copied().unwrap_or(None);
                stack.push(top);
            }
            0x12 => {
                let _ = pop_const_type(&mut stack);
            }
            0x13 => {
                if stack.len() < 2 {
                    stack.push(None);
                    stack.push(None);
                }
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            0x20 => {
                let _ref_idx = reader.read_u32()?;
                stack.push(None);
            }
            0x21 => {
                let ref_idx = reader.read_u32()?;
                let value_type = pop_const_type(&mut stack);
                validate_const_store(types, ref_table, &ref_types, value_type, ref_idx)?;
            }
            0x22 => {
                let _ref_idx = reader.read_u32()?;
                stack.push(None);
            }
            0x23..=0x25 => {
                stack.push(None);
            }
            0x30 | 0x60 | 0x62 => {
                let _ = reader.read_u32()?;
                if opcode == 0x30 || opcode == 0x62 {
                    let _ = pop_const_type(&mut stack);
                }
                stack.push(None);
            }
            0x31 => {
                let _index = pop_const_type(&mut stack);
                let _base = pop_const_type(&mut stack);
                stack.push(None);
            }
            0x32 | 0x45 | 0x49 | 0x61 => {
                let _ = pop_const_type(&mut stack);
                stack.push(None);
            }
            0x33 => {
                let _value = pop_const_type(&mut stack);
                let _reference = pop_const_type(&mut stack);
            }
            0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x50..=0x55 => {
                let _right = pop_const_type(&mut stack);
                let _left = pop_const_type(&mut stack);
                stack.push(None);
            }
            0x63 => {
                let _ = reader.read_u32()?;
                let _value = pop_const_type(&mut stack);
                let _target = pop_const_type(&mut stack);
                stack.push(None);
            }
            0x64 => {
                let target_type = reader.read_u32()?;
                let _value = pop_const_type(&mut stack);
                stack.push(Some(target_type));
            }
            _ => return Err(BytecodeError::InvalidOpcode(opcode)),
        }
    }
    Ok(())
}

fn ref_type_map(var_meta: &VarMeta) -> HashMap<u32, u32> {
    var_meta
        .entries
        .iter()
        .map(|entry| (entry.ref_idx, entry.type_id))
        .collect()
}

fn pop_const_type(stack: &mut Vec<Option<u32>>) -> Option<u32> {
    stack.pop().flatten()
}

fn validate_const_store(
    types: &TypeTable,
    ref_table: &RefTable,
    ref_types: &HashMap<u32, u32>,
    value_type: Option<u32>,
    ref_idx: u32,
) -> Result<(), BytecodeError> {
    let Some(value_type) = value_type else {
        return Ok(());
    };
    ensure_ref_index(ref_table, ref_idx)?;
    let Some(target_type) = ref_types.get(&ref_idx).copied() else {
        return Ok(());
    };
    if assignment_type_compatible(types, value_type, target_type)? {
        return Ok(());
    }
    Err(BytecodeError::InvalidSection(
        "constant type is incompatible with STORE_REF target".into(),
    ))
}

fn assignment_type_compatible(
    types: &TypeTable,
    value_type: u32,
    target_type: u32,
) -> Result<bool, BytecodeError> {
    if value_type == target_type {
        return Ok(true);
    }
    let value_prim = resolved_primitive_id(types, value_type)?;
    let target_prim = resolved_primitive_id(types, target_type)?;
    Ok(match (value_prim, target_prim) {
        (Some(1), Some(1)) => true,
        (Some(1), Some(_)) | (Some(_), Some(1)) => false,
        (Some(value), Some(target)) if is_numeric_primitive(value) && is_numeric_primitive(target) => {
            true
        }
        _ => true,
    })
}

fn resolved_primitive_id(types: &TypeTable, type_id: u32) -> Result<Option<u16>, BytecodeError> {
    let entry = types
        .entries
        .get(type_id as usize)
        .ok_or(BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: type_id,
        })?;
    match &entry.data {
        TypeData::Primitive { prim_id, .. } => Ok(Some(*prim_id)),
        TypeData::Alias { target_type_id }
        | TypeData::Subrange {
            base_type_id: target_type_id,
            ..
        } => resolved_primitive_id(types, *target_type_id),
        _ => Ok(None),
    }
}

fn is_numeric_primitive(prim_id: u16) -> bool {
    matches!(prim_id, 6..=15)
}
