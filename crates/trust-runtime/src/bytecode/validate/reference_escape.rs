#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefProvenance {
    Unknown,
    LocalFrame,
    LongerLived,
    InstanceRoot,
}

fn validate_reference_escape(
    ref_table: &RefTable,
    _pou: &PouEntry,
    code: &[u8],
) -> Result<(), BytecodeError> {
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
                let _ = reader.read_i32()?;
                let _ = pop_ref_provenance(&mut stack);
            }
            0x09 => {
                let _kind = reader.read_u32()?;
                let _symbol_idx = reader.read_u32()?;
                let arg_count = reader.read_u32()?;
                for _ in 0..arg_count {
                    let _ = pop_ref_provenance(&mut stack);
                }
                stack.push(RefProvenance::Unknown);
            }
            0x10 | 0x60 => {
                let _ = reader.read_u32()?;
                stack.push(RefProvenance::Unknown);
            }
            0x11 => {
                let top = stack.last().copied().unwrap_or(RefProvenance::Unknown);
                stack.push(top);
            }
            0x12 => {
                let _ = pop_ref_provenance(&mut stack);
            }
            0x13 => {
                if stack.len() < 2 {
                    stack.push(RefProvenance::Unknown);
                    stack.push(RefProvenance::Unknown);
                }
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            0x20 => {
                let _ = reader.read_u32()?;
                stack.push(RefProvenance::Unknown);
            }
            0x21 => {
                let ref_idx = reader.read_u32()?;
                let value = pop_ref_provenance(&mut stack);
                reject_local_ref_persistence(ref_table, ref_idx, value)?;
            }
            0x22 => {
                let ref_idx = reader.read_u32()?;
                stack.push(ref_provenance_for_ref(ref_table, ref_idx));
            }
            0x23 | 0x24 => {
                stack.push(RefProvenance::InstanceRoot);
            }
            0x25 => {
                stack.push(RefProvenance::Unknown);
            }
            0x30 => {
                let _name_idx = reader.read_u32()?;
                let base = pop_ref_provenance(&mut stack);
                stack.push(match base {
                    RefProvenance::LocalFrame => RefProvenance::LocalFrame,
                    RefProvenance::LongerLived | RefProvenance::InstanceRoot => {
                        RefProvenance::LongerLived
                    }
                    RefProvenance::Unknown => RefProvenance::Unknown,
                });
            }
            0x31 => {
                let _index = pop_ref_provenance(&mut stack);
                let base = pop_ref_provenance(&mut stack);
                stack.push(match base {
                    RefProvenance::LocalFrame => RefProvenance::LocalFrame,
                    RefProvenance::LongerLived => RefProvenance::LongerLived,
                    RefProvenance::Unknown | RefProvenance::InstanceRoot => RefProvenance::Unknown,
                });
            }
            0x32 | 0x61 | 0x62 => {
                let _ = pop_ref_provenance(&mut stack);
                if opcode == 0x62 {
                    let _operand = reader.read_u32()?;
                }
                stack.push(RefProvenance::Unknown);
            }
            0x33 => {
                let value = pop_ref_provenance(&mut stack);
                let reference = pop_ref_provenance(&mut stack);
                if value == RefProvenance::LocalFrame && reference != RefProvenance::LocalFrame {
                    return Err(BytecodeError::InvalidSection(
                        "frame-local reference cannot be stored through non-local reference"
                            .into(),
                    ));
                }
            }
            0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x50..=0x55 => {
                let _right = pop_ref_provenance(&mut stack);
                let _left = pop_ref_provenance(&mut stack);
                stack.push(RefProvenance::Unknown);
            }
            0x45 | 0x49 => {
                let _ = pop_ref_provenance(&mut stack);
                stack.push(RefProvenance::Unknown);
            }
            0x63 => {
                let _operand = reader.read_u32()?;
                let _value = pop_ref_provenance(&mut stack);
                let _target = pop_ref_provenance(&mut stack);
                stack.push(RefProvenance::Unknown);
            }
            _ => return Err(BytecodeError::InvalidOpcode(opcode)),
        }
    }
    Ok(())
}

fn pop_ref_provenance(stack: &mut Vec<RefProvenance>) -> RefProvenance {
    stack.pop().unwrap_or(RefProvenance::Unknown)
}

fn ref_provenance_for_ref(ref_table: &RefTable, ref_idx: u32) -> RefProvenance {
    match ref_table.entries.get(ref_idx as usize) {
        Some(entry) if entry.location == RefLocation::Local => RefProvenance::LocalFrame,
        Some(_) => RefProvenance::LongerLived,
        None => RefProvenance::Unknown,
    }
}

fn reject_local_ref_persistence(
    ref_table: &RefTable,
    ref_idx: u32,
    value: RefProvenance,
) -> Result<(), BytecodeError> {
    if value != RefProvenance::LocalFrame {
        return Ok(());
    }
    let Some(entry) = ref_table.entries.get(ref_idx as usize) else {
        return Ok(());
    };
    if entry.location == RefLocation::Local {
        return Ok(());
    }
    Err(BytecodeError::InvalidSection(
        "frame-local reference cannot be stored to longer-lived storage".into(),
    ))
}
