//! Byte transport is decoded once; struct-built input retains bounded encoding.
use trust_runtime_core::{
    bytecode::{BytecodeError, BytecodeModule, SectionData, SectionId, ValidationLimits},
    error::RuntimeError,
    value::Duration,
    vm::{PreparationLimits, PreparedModule},
};

const FIXTURE: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

#[test]
fn routes_charge_only_their_transport_and_execute_identical_state() {
    let (raw, decoding) =
        BytecodeModule::decode_with_limits(FIXTURE, usize::MAX, usize::MAX).unwrap();
    let encoded = raw.encode().unwrap();
    assert_eq!(encoded, FIXTURE);
    let object = PreparedModule::from_decoded(&raw, PreparationLimits::default()).unwrap();
    let bytes = PreparedModule::from_bytes(FIXTURE, PreparationLimits::default()).unwrap();
    let object_usage = object.preparation_usage();
    let byte_usage = bytes.preparation_usage();
    // Both routes perform the same metadata preparation. The byte path pays
    // for real decoder demand; only the struct path pays serializer scratch.
    assert_eq!(
        byte_usage.bytes + encoded.len() * 4,
        object_usage.bytes + decoding.allocation_bytes
    );
    assert_eq!(
        byte_usage.work + encoded.len(),
        object_usage.work + decoding.work
    );
    let mut object_state = object.instantiate(0).unwrap();
    let mut byte_state = bytes.instantiate(0).unwrap();
    for millis in (0..=1000).step_by(10) {
        let now = Duration::from_millis(millis);
        object_state.execute_cycle(now).unwrap();
        byte_state.execute_cycle(now).unwrap();
        assert_eq!(
            object_state.storage().globals(),
            byte_state.storage().globals()
        );
        let left_instances = object_state.storage().instances();
        let right_instances = byte_state.storage().instances();
        assert_eq!(left_instances.len(), right_instances.len());
        for (id, left) in left_instances {
            let right = right_instances.get(id).unwrap();
            assert_eq!(left.type_name, right.type_name);
            assert_eq!(left.parent, right.parent);
            assert_eq!(left.variables, right.variables);
        }
        assert_eq!(object_state.outputs(), byte_state.outputs());
        let left = &object_state.task_states()[0];
        let right = &byte_state.task_states()[0];
        assert_eq!(
            (left.last_run, left.overrun_count),
            (right.last_run, right.overrun_count)
        );
    }
}

#[test]
fn both_routes_enforce_exact_cumulative_limits_and_full_validation() {
    let raw = BytecodeModule::decode(FIXTURE).unwrap();
    for byte_route in [false, true] {
        let prepare = |limits| {
            if byte_route {
                PreparedModule::from_bytes(FIXTURE, limits)
            } else {
                PreparedModule::from_decoded(&raw, limits)
            }
        };
        let usage = prepare(PreparationLimits::default())
            .unwrap()
            .preparation_usage();
        let exact = PreparationLimits {
            max_preparation_bytes: usage.bytes,
            max_preparation_work: usage.work,
            ..Default::default()
        };
        assert!(prepare(exact).is_ok());
        // The cumulative charge that crosses either limit is refused at budget.rs:43-46.
        assert_eq!(
            prepare(PreparationLimits {
                max_preparation_bytes: usage.bytes - 1,
                ..exact
            })
            .unwrap_err(),
            RuntimeError::PreparationLimit
        );
        assert_eq!(
            prepare(PreparationLimits {
                max_preparation_work: usage.work - 1,
                ..exact
            })
            .unwrap_err(),
            RuntimeError::PreparationLimit
        );
        assert!(prepare(PreparationLimits {
            max_artifact_bytes: FIXTURE.len() - 1,
            ..Default::default()
        })
        .is_err());
        assert!(prepare(PreparationLimits {
            max_call_depth: 0,
            ..Default::default()
        })
        .is_err());
        assert!(prepare(PreparationLimits {
            validation: ValidationLimits {
                max_work: 0,
                ..Default::default()
            },
            ..Default::default()
        })
        .is_err());
    }
    let mut corrupt = raw;
    let Some(SectionData::PouBodies(body)) = corrupt.section_mut(SectionId::PouBodies) else {
        panic!("fixture body");
    };
    body[0] = 0xff;
    let encoded = corrupt.encode().unwrap();
    let byte_error =
        PreparedModule::from_bytes(&encoded, PreparationLimits::default()).unwrap_err();
    let object_error =
        PreparedModule::from_decoded(&corrupt, PreparationLimits::default()).unwrap_err();
    assert_eq!(byte_error.stable_code(), object_error.stable_code());
}

#[test]
fn byte_artifact_bound_counts_original_padding_not_reserialized_length() {
    let mut padded = FIXTURE.to_vec();
    padded.extend_from_slice(&[0; 64]);
    let table_start = u32::from_le_bytes(padded[16..20].try_into().unwrap()) as usize;
    let checksum = trust_runtime_core::crc32::checksum(&padded[table_start..]);
    padded[20..24].copy_from_slice(&checksum.to_le_bytes());
    let decoded = BytecodeModule::decode(&padded).unwrap();
    assert_eq!(decoded.encode().unwrap().len(), FIXTURE.len());
    let short = PreparationLimits {
        max_artifact_bytes: FIXTURE.len(),
        ..Default::default()
    };
    assert!(PreparedModule::from_decoded(&decoded, short).is_ok());
    assert!(PreparedModule::from_bytes(&padded, short).is_err());
    assert!(PreparedModule::from_bytes(
        &padded,
        PreparationLimits {
            max_artifact_bytes: padded.len(),
            ..Default::default()
        }
    )
    .is_ok());
}

#[test]
fn metadata_accounting_does_not_replace_full_validator_rejection_codes() {
    let original = BytecodeModule::decode(FIXTURE).unwrap();
    let mut missing_strings = original.clone();
    missing_strings
        .sections
        .retain(|section| section.id != SectionId::StringTable.as_raw());

    let mut invalid_opcode = original.clone();
    let Some(SectionData::PouIndex(index)) = original.section(SectionId::PouIndex) else {
        panic!("fixture POU index")
    };
    let first = index
        .entries
        .iter()
        .find(|entry| entry.code_length > 0)
        .unwrap();
    let body_start = first.code_offset as usize;
    let Some(SectionData::PouBodies(code)) = invalid_opcode.section_mut(SectionId::PouBodies)
    else {
        panic!("fixture bodies")
    };
    code[body_start] = 0xff;

    let mut truncated = original.clone();
    let Some(SectionData::PouIndex(index)) = truncated.section_mut(SectionId::PouIndex) else {
        panic!("fixture POU index")
    };
    index
        .entries
        .iter_mut()
        .find(|entry| entry.id == first.id)
        .unwrap()
        .code_length = 1;
    let Some(SectionData::PouBodies(code)) = truncated.section_mut(SectionId::PouBodies) else {
        panic!("fixture bodies")
    };
    // A one-byte body containing LOAD_CONST has no room for its u32 operand.
    // The surrounding container and all section framing remain well formed.
    code[body_start] = 0x10;

    for (raw, expected) in [
        (
            missing_strings,
            BytecodeError::MissingSection("STRING_TABLE".into()),
        ),
        (invalid_opcode, BytecodeError::InvalidOpcode(0xff)),
        (truncated, BytecodeError::UnexpectedEof),
    ] {
        let rejected = raw.validate().unwrap_err();
        assert_eq!(rejected, expected);
        let bytes = raw.encode().unwrap();
        for result in [
            PreparedModule::from_bytes(&bytes, PreparationLimits::default()),
            PreparedModule::from_decoded(&raw, PreparationLimits::default()),
        ] {
            assert_eq!(result.unwrap_err().stable_code(), rejected.stable_code());
        }
    }
}

#[test]
fn pou_materialization_uses_admitted_names_and_refuses_invalid_identity_metadata() {
    let raw = BytecodeModule::decode(FIXTURE).unwrap();
    let Some(SectionData::PouIndex(index)) = raw.section(SectionId::PouIndex) else {
        panic!("fixture POU index");
    };
    let Some(SectionData::StringTable(strings)) = raw.section(SectionId::StringTable) else {
        panic!("fixture strings");
    };
    let prepared = PreparedModule::from_decoded(&raw, Default::default()).unwrap();
    for pou in &index.entries {
        assert_eq!(
            prepared.pou_name(pou.id),
            Some(strings.entries[pou.name_idx as usize].as_str())
        );
    }
    assert!(index.entries.len() >= 2);
    for bad_name in [false, true] {
        let mut forged = raw.clone();
        let Some(SectionData::PouIndex(index)) = forged.section_mut(SectionId::PouIndex) else {
            panic!("fixture POU index");
        };
        if bad_name {
            index.entries[0].name_idx = u32::MAX;
        } else {
            index.entries[1].id = index.entries[0].id;
        }
        // Both construction routes must reject this at the validator boundary;
        // removing redundant materialization storage does not relax admission.
        assert!(forged.validated_source_free(Default::default()).is_err());
        assert!(PreparedModule::from_decoded(&forged, Default::default()).is_err());
        let bytes = forged.encode().unwrap();
        assert!(PreparedModule::from_bytes(&bytes, Default::default()).is_err());
    }
}

#[cfg(feature = "std")]
#[test]
fn owned_and_artifact_retaining_routes_preserve_sections_faults_and_usage() {
    for fixture in [
        FIXTURE,
        &include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc")
            [..],
        &include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc")[..],
    ] {
        let mut raw = BytecodeModule::decode(fixture).unwrap();
        raw.sections.reverse();
        let bytes = raw.encode().unwrap();
        let limits = PreparationLimits::default();
        let owned = PreparedModule::from_bytes(&bytes, limits).unwrap();
        let (borrowed, retained) =
            PreparedModule::from_bytes_with_artifact(&bytes, limits).unwrap();
        assert_eq!(retained, raw);
        assert_eq!(retained.encode().unwrap(), bytes);
        assert_eq!(owned.preparation_usage(), borrowed.preparation_usage());
        assert_eq!(owned.type_table(), borrowed.type_table());
        assert_eq!(owned.strings(), borrowed.strings());
        assert_eq!(
            owned.storage_declarations(),
            borrowed.storage_declarations()
        );
        assert_eq!(owned.construction_roots(), borrowed.construction_roots());
        let mut left = owned.instantiate(0).unwrap();
        let mut right = borrowed.instantiate(0).unwrap();
        for millis in (0..=1000).step_by(10) {
            left.execute_cycle(Duration::from_millis(millis)).unwrap();
            right.execute_cycle(Duration::from_millis(millis)).unwrap();
            assert_eq!(left.storage().globals(), right.storage().globals());
            for (id, instance) in left.storage().instances() {
                let other = right.storage().get_instance(*id).unwrap();
                assert_eq!(instance.variables, other.variables);
                assert_eq!(instance.type_name, other.type_name);
                assert_eq!(instance.parent, other.parent);
            }
            assert_eq!(left.outputs(), right.outputs());
        }
        for limit in [
            PreparationLimits {
                max_preparation_bytes: owned.preparation_usage().bytes - 1,
                ..limits
            },
            PreparationLimits {
                max_preparation_work: owned.preparation_usage().work - 1,
                ..limits
            },
            PreparationLimits {
                max_process_image_bytes: 0,
                ..limits
            },
            PreparationLimits {
                validation: ValidationLimits {
                    max_work: 0,
                    ..limits.validation
                },
                ..limits
            },
        ] {
            let left = PreparedModule::from_bytes(&bytes, limit).map(|m| m.preparation_usage());
            let right = PreparedModule::from_bytes_with_artifact(&bytes, limit)
                .map(|(m, _)| m.preparation_usage());
            assert_eq!(left, right);
        }
        // Validator faults precede materialization on both byte routes.
        let Some(SectionData::PouBodies(code)) = raw.section_mut(SectionId::PouBodies) else {
            panic!("fixture code")
        };
        code[0] = 0xff;
        let malformed = raw.encode().unwrap();
        let left = PreparedModule::from_bytes(&malformed, limits).unwrap_err();
        let right = PreparedModule::from_bytes_with_artifact(&malformed, limits).unwrap_err();
        assert_eq!(left, right);
    }
}
