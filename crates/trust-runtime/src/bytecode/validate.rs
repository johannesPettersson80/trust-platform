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
    use super::{
        is_numeric_primitive, validate_const_payload, validate_partial_access_operand,
        BytecodeError, ConstEntry, TypeData, TypeEntry, TypeKind, TypeTable,
    };
    use crate::bytecode::Field;

    fn primitive_int() -> TypeEntry {
        TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 7,
                max_length: 0,
            },
        }
    }

    fn alias_chain(alias_count: usize) -> TypeTable {
        let mut entries = vec![primitive_int()];
        for target_type_id in 0..alias_count {
            entries.push(TypeEntry {
                kind: TypeKind::Alias,
                name_idx: None,
                data: TypeData::Alias {
                    target_type_id: target_type_id as u32,
                },
            });
        }
        TypeTable {
            offsets: vec![],
            entries,
        }
    }

    fn const_entry(type_id: u32, payload: Vec<u8>) -> ConstEntry {
        ConstEntry { type_id, payload }
    }

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

    #[test]
    fn const_payload_validation_rejects_nesting_beyond_fixed_limit() {
        let at_limit = alias_chain(64);
        validate_const_payload(&at_limit, &const_entry(64, 7_i16.to_le_bytes().to_vec()))
            .expect("64 nested constant type references must remain valid");

        let over_limit = alias_chain(65);
        let error =
            validate_const_payload(&over_limit, &const_entry(65, 7_i16.to_le_bytes().to_vec()))
                .expect_err("65 nested constant type references must fail closed");
        assert!(error.to_string().contains("const type recursion overflow"));
    }

    #[test]
    fn const_payload_validation_rejects_cyclic_type_before_stack_exhaustion() {
        let types = TypeTable {
            offsets: vec![],
            entries: vec![TypeEntry {
                kind: TypeKind::Alias,
                name_idx: None,
                data: TypeData::Alias { target_type_id: 0 },
            }],
        };
        let error = validate_const_payload(&types, &const_entry(0, vec![]))
            .expect_err("cyclic constant type must fail closed");
        assert!(error.to_string().contains("const type recursion overflow"));
    }

    #[test]
    fn const_payload_validation_accepts_empty_wildcard_array_frame() {
        let types = TypeTable {
            offsets: vec![],
            entries: vec![
                primitive_int(),
                TypeEntry {
                    kind: TypeKind::Array,
                    name_idx: None,
                    data: TypeData::Array {
                        elem_type_id: 0,
                        dims: vec![(0, i64::MAX)],
                    },
                },
            ],
        };
        validate_const_payload(&types, &const_entry(1, 0_u32.to_le_bytes().to_vec()))
            .expect("wildcard ARRAY default must retain an empty frame");
    }

    #[test]
    fn const_payload_validation_rejects_malformed_aggregate_frames() {
        let types = TypeTable {
            offsets: vec![],
            entries: vec![
                primitive_int(),
                TypeEntry {
                    kind: TypeKind::Array,
                    name_idx: None,
                    data: TypeData::Array {
                        elem_type_id: 0,
                        dims: vec![(1, 2)],
                    },
                },
                TypeEntry {
                    kind: TypeKind::Struct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: vec![Field {
                            name_idx: 0,
                            type_id: 0,
                        }],
                    },
                },
            ],
        };

        let array_count =
            validate_const_payload(&types, &const_entry(1, 1_u32.to_le_bytes().to_vec()))
                .expect_err("array count that disagrees with dimensions must fail");
        assert!(array_count
            .to_string()
            .contains("array constant count mismatch"));

        let mut truncated = 1_u32.to_le_bytes().to_vec();
        truncated.extend_from_slice(&3_u32.to_le_bytes());
        truncated.extend_from_slice(&7_i16.to_le_bytes());
        assert!(matches!(
            validate_const_payload(&types, &const_entry(2, truncated)),
            Err(BytecodeError::UnexpectedEof)
        ));

        let mut trailing = 1_u32.to_le_bytes().to_vec();
        trailing.extend_from_slice(&3_u32.to_le_bytes());
        trailing.extend_from_slice(&7_i16.to_le_bytes());
        trailing.push(0);
        let error = validate_const_payload(&types, &const_entry(2, trailing))
            .expect_err("trailing child bytes must fail");
        assert!(error.to_string().contains("const child payload length"));
    }

    #[test]
    fn const_payload_validation_rejects_non_null_reference_index() {
        let types = TypeTable {
            offsets: vec![],
            entries: vec![
                primitive_int(),
                TypeEntry {
                    kind: TypeKind::Reference,
                    name_idx: None,
                    data: TypeData::Reference { target_type_id: 0 },
                },
            ],
        };
        validate_const_payload(&types, &const_entry(1, u32::MAX.to_le_bytes().to_vec()))
            .expect("NULL reference constant must remain valid");
        let error = validate_const_payload(&types, &const_entry(1, 0_u32.to_le_bytes().to_vec()))
            .expect_err("live reference identity must not be accepted as a constant");
        assert!(error
            .to_string()
            .contains("REFERENCE const payload must encode NULL"));
    }
}
