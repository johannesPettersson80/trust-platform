use super::*;

pub(super) fn validate_owner_contract(
    tables: &ValidationContext<'_>,
    instructions: &[Instruction],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut owner = None;
    for instruction in instructions {
        budget.work(1)?;
        if matches!(instruction.opcode, 0x20..=0x22) {
            if let Some(entry) = tables
                .ref_table
                .entries
                .get(instruction.operand(0) as usize)
            {
                if entry.location == RefLocation::Instance {
                    if owner.is_some_and(|value| value != entry.owner_id) {
                        return Err(BytecodeError::from(RejectionReason::MultipleInstanceOwners));
                    }
                    owner = Some(entry.owner_id);
                }
            }
        }
    }
    Ok(())
}
