use crate::bytecode::{
    BytecodeError, PouIndex, RefTable, BYTECODE_MAX_INSTRUCTIONS, BYTECODE_MAX_LOCALS_PER_POU,
    BYTECODE_MAX_PARAMETERS_PER_POU, BYTECODE_MAX_REFERENCES,
};
use trust_runtime_core::vm::VM_MAX_OPERAND_STACK;

pub(super) fn validate_declared_resource_limits(
    ref_table: &RefTable,
    pou_index: &PouIndex,
) -> Result<(), BytecodeError> {
    validate_count(
        ref_table.entries.len(),
        BYTECODE_MAX_REFERENCES,
        "REF_TABLE entries",
    )?;
    for pou in &pou_index.entries {
        validate_count(
            pou.local_ref_count as usize,
            BYTECODE_MAX_LOCALS_PER_POU,
            "POU local references",
        )?;
        validate_count(
            pou.params.len(),
            BYTECODE_MAX_PARAMETERS_PER_POU,
            "POU parameters",
        )?;
    }
    Ok(())
}

pub(super) fn charge_decoded_instruction(total: &mut usize) -> Result<(), BytecodeError> {
    let next = total.checked_add(1).ok_or_else(resource_limit_error)?;
    validate_count(
        next,
        BYTECODE_MAX_INSTRUCTIONS,
        "decoded module instructions",
    )?;
    *total = next;
    Ok(())
}

pub(super) fn validate_operand_stack_depth(depth: usize) -> Result<(), BytecodeError> {
    validate_count(depth, VM_MAX_OPERAND_STACK, "operand stack values")
}

fn validate_count(observed: usize, maximum: usize, resource: &str) -> Result<(), BytecodeError> {
    if observed > maximum {
        return Err(BytecodeError::InvalidSection(
            format!("{resource} exceed fixed resource limit").into(),
        ));
    }
    Ok(())
}

fn resource_limit_error() -> BytecodeError {
    BytecodeError::InvalidSection("decoded module instruction count overflow".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_limit_accepts_boundary_and_rejects_next_instruction() {
        let mut total = BYTECODE_MAX_INSTRUCTIONS - 1;
        charge_decoded_instruction(&mut total).expect("fixed instruction boundary must pass");
        assert_eq!(total, BYTECODE_MAX_INSTRUCTIONS);
        assert!(charge_decoded_instruction(&mut total).is_err());
    }

    #[test]
    fn stack_limit_accepts_boundary_and_rejects_next_value() {
        validate_operand_stack_depth(VM_MAX_OPERAND_STACK).expect("fixed stack boundary must pass");
        assert!(validate_operand_stack_depth(VM_MAX_OPERAND_STACK + 1).is_err());
    }
}
