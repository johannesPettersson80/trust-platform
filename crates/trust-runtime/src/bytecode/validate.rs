//! Bytecode validation.

#![allow(missing_docs)]

use std::collections::{HashMap, HashSet};

use super::reader::BytecodeReader;
use super::{
    BytecodeError, BytecodeModule, ConstEntry, ConstPool, DebugMap, IoMap, ParamEntry, PouEntry,
    PouIndex, PouKind, RefLocation, RefSegment, RefTable, ResourceMeta, RetainInit, SectionData,
    SectionId, StringTable, TypeData, TypeEntry, TypeKind, TypeTable, VarMeta,
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK,
};

mod resource_limits;

use resource_limits::{
    charge_decoded_instruction, validate_declared_resource_limits, validate_operand_stack_depth,
};

include!("validate/module_validate.rs");
include!("validate/tables_consts.rs");
include!("validate/pou_and_instr.rs");
include!("validate/reference_escape.rs");
include!("validate/owner_contract.rs");
include!("validate/stack_shape.rs");
include!("validate/const_compat.rs");
include!("validate/param_direction.rs");
include!("validate/call_target.rs");
include!("validate/resource_io.rs");
include!("validate/meta_debug.rs");

#[cfg(test)]
mod tests {
    use super::{is_numeric_primitive, validate_partial_access_operand};

    #[test]
    fn numeric_primitive_and_partial_access_domains_are_closed() {
        for primitive in 6..=15 {
            assert!(
                is_numeric_primitive(primitive),
                "primitive {primitive} must be numeric"
            );
        }
        for primitive in [0, 1, 5, 16, u16::MAX] {
            assert!(
                !is_numeric_primitive(primitive),
                "primitive {primitive} must remain outside the numeric domain"
            );
        }

        for operand in [
            0x0000, 0x003F, 0x0100, 0x0107, 0x0200, 0x0203, 0x0300, 0x0301,
        ] {
            validate_partial_access_operand(operand)
                .unwrap_or_else(|error| panic!("valid operand {operand:#x}: {error}"));
        }
        for operand in [0x0040, 0x0108, 0x0204, 0x0302, 0x0400] {
            assert!(
                validate_partial_access_operand(operand).is_err(),
                "invalid operand {operand:#x} must fail"
            );
        }
    }
}
