use super::*;

/// One structurally decoded instruction, shared by all validation passes.
#[derive(Debug, Clone, Copy)]
pub(super) struct Instruction {
    pub(super) pc: u32,
    pub(super) opcode: u8,
    pub(super) operands: [u32; 3],
}

impl Instruction {
    pub(super) fn next_pc(&self) -> Result<usize, BytecodeError> {
        let width = crate::vm::opcode_operand_len(self.opcode)
            .ok_or(BytecodeError::InvalidOpcode(self.opcode))?;
        (self.pc as usize)
            .checked_add(1 + width)
            .ok_or_else(|| RejectionReason::CodePositionOverflow.into())
    }

    pub(super) fn jump_target(&self) -> Result<Option<usize>, BytecodeError> {
        if !matches!(self.opcode, 0x02..=0x04) {
            return Ok(None);
        }
        let pc = i32::try_from(self.pc).map_err(|_| RejectionReason::CodePositionOverflow)?;
        let target = checked_jump_target(pc, self.operand(0) as i32)?;
        Ok(Some(
            usize::try_from(target).map_err(|_| BytecodeError::InvalidJumpTarget(target))?,
        ))
    }

    pub(super) fn operand(&self, index: usize) -> u32 {
        self.operands[index]
    }
}

pub(super) fn decode_instructions(
    code: &[u8],
    count: &mut usize,
    budget: &mut ValidationBudget,
    mut validate_operand: impl FnMut(&Instruction, &mut ValidationBudget) -> Result<(), BytecodeError>,
) -> Result<Vec<Instruction>, BytecodeError> {
    let _ = i32::try_from(code.len()).map_err(|_| RejectionReason::CodePositionOverflow)?;
    let mut reader = BytecodeReader::new(code);
    let mut instructions = Vec::new();
    while reader.remaining() > 0 {
        charge_decoded_instruction(count)?;
        budget.work(1)?;
        let pc = reader.pos();
        let opcode = reader.read_u8()?;
        if let Some(name) = unsupported_runtime_opcode_name(opcode) {
            return Err(BytecodeError::InvalidSection(
                format!("unsupported runtime opcode {name} (0x{opcode:02X})").into(),
            ));
        }
        let width =
            crate::vm::opcode_operand_len(opcode).ok_or(BytecodeError::InvalidOpcode(opcode))?;
        let mut operands = [0; 3];
        let operand_count = match width {
            0 => 0,
            4 => 1,
            8 => 2,
            12 => 3,
            _ => return Err(BytecodeError::InvalidOpcode(opcode)),
        };
        for operand in &mut operands[..operand_count] {
            *operand = reader.read_u32()?;
        }
        let instruction = Instruction {
            pc: u32::try_from(pc).map_err(|_| RejectionReason::CodePositionOverflow)?,
            opcode,
            operands,
        };
        validate_operand(&instruction, budget)?;
        budget.push(&mut instructions, instruction)?;
    }
    for i in 0..instructions.len() {
        budget.work(1)?;
        if matches!(instructions[i].opcode, 0x02..=0x04) {
            let pc = i32::try_from(instructions[i].pc)
                .map_err(|_| RejectionReason::CodePositionOverflow)?;
            let target = checked_jump_target(pc, instructions[i].operand(0) as i32)?;
            if target < 0 || target as usize > code.len() {
                return Err(BytecodeError::InvalidJumpTarget(target));
            }
            if target as usize != code.len()
                && budget
                    .search_by(&instructions, |item, _| Ok(item.pc.cmp(&(target as u32))))?
                    .is_err()
            {
                return Err(BytecodeError::InvalidJumpTarget(target));
            }
        }
    }
    Ok(instructions)
}

#[cfg(test)]
mod tests {
    #[test]
    fn decoded_instruction_is_twenty_bytes_on_each_target() {
        assert_eq!(core::mem::size_of::<super::Instruction>(), 20);
    }
}
