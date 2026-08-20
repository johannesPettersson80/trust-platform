fn validate_owner_contract(
    ref_table: &RefTable,
    _pou: &PouEntry,
    code: &[u8],
) -> Result<(), BytecodeError> {
    let mut reader = BytecodeReader::new(code);
    let mut instance_owners = HashSet::new();
    while reader.remaining() > 0 {
        let opcode = reader.read_u8()?;
        match opcode {
            0x00 | 0x01 | 0x06 | 0x11 | 0x12 | 0x13 | 0x23 | 0x24 | 0x25 | 0x31 | 0x32
            | 0x33 | 0x40..=0x49 | 0x4C | 0x50..=0x55 | 0x61 => {}
            0x02..=0x05 | 0x07 | 0x10 | 0x30 | 0x60 | 0x62..=0x64 | 0x70 => {
                let _ = reader.read_u32()?;
            }
            0x08 => {
                let _interface_type_id = reader.read_u32()?;
                let _slot = reader.read_u32()?;
            }
            0x09 => {
                let _kind = reader.read_u32()?;
                let _symbol_idx = reader.read_u32()?;
                let _arg_count = reader.read_u32()?;
            }
            0x20..=0x22 => {
                let ref_idx = reader.read_u32()?;
                if let Some(entry) = ref_table.entries.get(ref_idx as usize) {
                    if entry.location == RefLocation::Instance {
                        instance_owners.insert(entry.owner_id);
                        if instance_owners.len() > 1 {
                            return Err(BytecodeError::InvalidSection(
                                "POU body references multiple instance owners".into(),
                            ));
                        }
                    }
                }
            }
            _ => return Err(BytecodeError::InvalidOpcode(opcode)),
        }
    }
    Ok(())
}
