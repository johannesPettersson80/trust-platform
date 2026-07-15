use crate::bytecode::{
    PouIndex, RefTable, BYTECODE_MAX_LOCALS_PER_POU, BYTECODE_MAX_PARAMETERS_PER_POU,
    BYTECODE_MAX_REFERENCES,
};
use crate::error::RuntimeError;

use super::errors::VmTrap;

pub(super) fn validate_materialization_limits(
    ref_table: &RefTable,
    pou_index: &PouIndex,
) -> Result<(), RuntimeError> {
    if ref_table.entries.len() > BYTECODE_MAX_REFERENCES {
        return Err(invalid_bytecode(
            "REF_TABLE entries exceed fixed resource limit",
        ));
    }
    for pou in &pou_index.entries {
        if pou.local_ref_count as usize > BYTECODE_MAX_LOCALS_PER_POU {
            return Err(invalid_bytecode(
                "POU local references exceed fixed resource limit",
            ));
        }
        if pou.params.len() > BYTECODE_MAX_PARAMETERS_PER_POU {
            return Err(invalid_bytecode(
                "POU parameters exceed fixed resource limit",
            ));
        }
    }
    Ok(())
}

pub(super) fn checked_local_count(local_ref_count: u32) -> Result<usize, VmTrap> {
    let local_count = usize::try_from(local_ref_count).map_err(|_| {
        VmTrap::BytecodeDecode("POU local reference count does not fit host usize".into())
    })?;
    if local_count > BYTECODE_MAX_LOCALS_PER_POU {
        return Err(VmTrap::BytecodeDecode(
            "POU local references exceed fixed resource limit".into(),
        ));
    }
    Ok(local_count)
}

fn invalid_bytecode(message: &'static str) -> RuntimeError {
    RuntimeError::InvalidBytecode(message.into())
}
