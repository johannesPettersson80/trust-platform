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

    #[test]
    fn instruction_counter_overflow_is_fail_closed_and_preserves_total() {
        let mut total = usize::MAX;
        let error = charge_decoded_instruction(&mut total)
            .expect_err("counter overflow must fail before mutation");
        assert_eq!(total, usize::MAX);
        assert!(error
            .to_string()
            .contains("decoded module instruction count overflow"));
    }

    #[test]
    fn declared_resource_limits_accept_empty_tables_and_reject_first_excess() {
        validate_declared_resource_limits(&RefTable::default(), &PouIndex::default())
            .expect("empty declared tables must fit fixed resource limits");
        validate_count(
            BYTECODE_MAX_REFERENCES,
            BYTECODE_MAX_REFERENCES,
            "REF_TABLE entries",
        )
        .expect("the exact reference boundary must pass");

        let error = validate_count(
            BYTECODE_MAX_REFERENCES + 1,
            BYTECODE_MAX_REFERENCES,
            "REF_TABLE entries",
        )
        .expect_err("the first reference above the boundary must fail");
        assert!(error
            .to_string()
            .contains("REF_TABLE entries exceed fixed resource limit"));
    }
}
