use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefProvenance {
    Unknown,
    LocalFrame,
    LongerLived,
    InstanceRoot,
}

pub(super) fn validate_reference_escape(
    tables: &ValidationContext<'_>,
    instructions: &[Instruction],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let ref_table = tables.ref_table;

    let mut stack = Vec::new();
    for instruction in instructions {
        budget.work(1)?;
        let opcode = instruction.opcode;
        match opcode {
            0x21 => {
                let ref_idx = instruction.operand(0);
                let value = pop_ref_provenance(&mut stack);
                reject_local_ref_persistence(ref_table, ref_idx, value)?;
            }
            0x22 => {
                let ref_idx = instruction.operand(0);
                budget.push(&mut stack, ref_provenance_for_ref(ref_table, ref_idx))?;
            }
            0x23 | 0x24 => {
                budget.push(&mut stack, RefProvenance::InstanceRoot)?;
            }
            0x30 => {
                let base = pop_ref_provenance(&mut stack);
                budget.push(
                    &mut stack,
                    match base {
                        RefProvenance::LocalFrame => RefProvenance::LocalFrame,
                        RefProvenance::LongerLived | RefProvenance::InstanceRoot => {
                            RefProvenance::LongerLived
                        }
                        RefProvenance::Unknown => RefProvenance::Unknown,
                    },
                )?;
            }
            0x31 => {
                let _index = pop_ref_provenance(&mut stack);
                let base = pop_ref_provenance(&mut stack);
                budget.push(
                    &mut stack,
                    match base {
                        RefProvenance::LocalFrame => RefProvenance::LocalFrame,
                        RefProvenance::LongerLived => RefProvenance::LongerLived,
                        RefProvenance::Unknown | RefProvenance::InstanceRoot => {
                            RefProvenance::Unknown
                        }
                    },
                )?;
            }
            0x33 => {
                let value = pop_ref_provenance(&mut stack);
                let reference = pop_ref_provenance(&mut stack);
                if value == RefProvenance::LocalFrame && reference != RefProvenance::LocalFrame {
                    return Err(BytecodeError::from(
                        RejectionReason::FrameLocalReferenceCannotBeStoredThroughNonLocalReference,
                    ));
                }
            }
            0x64 => {
                let value = pop_ref_provenance(&mut stack);
                budget.push(&mut stack, value)?;
            }
            _ => apply_unknown_effect(instruction, &mut stack, RefProvenance::Unknown, budget)?,
        }
    }
    Ok(())
}

pub(super) fn pop_ref_provenance(stack: &mut Vec<RefProvenance>) -> RefProvenance {
    stack.pop().unwrap_or(RefProvenance::Unknown)
}

pub(super) fn ref_provenance_for_ref(ref_table: &RefTable, ref_idx: u32) -> RefProvenance {
    match ref_table.entries.get(ref_idx as usize) {
        Some(entry) if entry.location == RefLocation::Local => RefProvenance::LocalFrame,
        Some(_) => RefProvenance::LongerLived,
        None => RefProvenance::Unknown,
    }
}

pub(super) fn reject_local_ref_persistence(
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
    Err(BytecodeError::from(
        RejectionReason::FrameLocalReferenceCannotBeStoredToLongerLivedStorage,
    ))
}
