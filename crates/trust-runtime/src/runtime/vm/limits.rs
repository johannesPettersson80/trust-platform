pub(super) use trust_runtime_core::vm::hosted::materialization_limits::{
    checked_local_count, validate_materialization_limits,
};
#[cfg(test)]
mod tests {
    use super::*;

    use crate::bytecode::{
        ParamEntry, PouEntry, PouIndex, PouKind, RefEntry, RefLocation, RefTable,
        BYTECODE_MAX_LOCALS_PER_POU, BYTECODE_MAX_PARAMETERS_PER_POU, BYTECODE_MAX_REFERENCES,
    };
    use crate::error::StableErrorCode;
    use trust_hir::TypeId;
    use trust_runtime_core::vm::VmTrap;

    #[test]
    fn vm_materialization_accepts_each_fixed_limit_exactly() {
        let refs = RefTable {
            entries: vec![reference(); BYTECODE_MAX_REFERENCES],
        };
        let pous = PouIndex {
            entries: vec![pou(
                BYTECODE_MAX_LOCALS_PER_POU as u32,
                BYTECODE_MAX_PARAMETERS_PER_POU,
            )],
        };

        assert!(validate_materialization_limits(&refs, &pous).is_ok());
        assert_eq!(
            checked_local_count(BYTECODE_MAX_LOCALS_PER_POU as u32).unwrap(),
            BYTECODE_MAX_LOCALS_PER_POU
        );
    }

    #[test]
    fn vm_materialization_rejects_first_reference_excess() {
        let refs = RefTable {
            entries: vec![reference(); BYTECODE_MAX_REFERENCES + 1],
        };
        let error = validate_materialization_limits(&refs, &PouIndex::default())
            .expect_err("first reference above the fixed limit must reject");

        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(error.to_string().contains("REF_TABLE entries"));
    }

    #[test]
    fn vm_materialization_rejects_first_local_and_parameter_excess() {
        let local_error = validate_materialization_limits(
            &RefTable::default(),
            &PouIndex {
                entries: vec![pou(BYTECODE_MAX_LOCALS_PER_POU as u32 + 1, 0)],
            },
        )
        .expect_err("first local above the fixed limit must reject");
        assert_eq!(local_error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(local_error.to_string().contains("local references"));

        let parameter_error = validate_materialization_limits(
            &RefTable::default(),
            &PouIndex {
                entries: vec![pou(0, BYTECODE_MAX_PARAMETERS_PER_POU + 1)],
            },
        )
        .expect_err("first parameter above the fixed limit must reject");
        assert_eq!(
            parameter_error.stable_code(),
            StableErrorCode::VmBytecodeDecode
        );
        assert!(parameter_error.to_string().contains("parameters"));
    }

    #[test]
    fn vm_checked_local_count_rejects_first_excess_with_decode_identity() {
        let error = checked_local_count(BYTECODE_MAX_LOCALS_PER_POU as u32 + 1)
            .expect_err("first local above the fixed limit must reject");

        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(
            matches!(error, VmTrap::BytecodeDecode(message) if message.contains("local references"))
        );
    }

    fn reference() -> RefEntry {
        RefEntry {
            location: RefLocation::Global,
            owner_id: 0,
            offset: 0,
            segments: Vec::new(),
        }
    }

    fn pou(local_ref_count: u32, parameter_count: usize) -> PouEntry {
        PouEntry {
            id: 1,
            name_idx: 0,
            kind: PouKind::Function,
            code_offset: 0,
            code_length: 0,
            local_ref_start: 0,
            local_ref_count,
            return_type_id: None,
            owner_pou_id: None,
            params: vec![
                ParamEntry {
                    name_idx: 0,
                    type_id: TypeId::DINT.0,
                    direction: 0,
                    default_const_idx: None,
                };
                parameter_count
            ],
            class_meta: None,
        }
    }
}
