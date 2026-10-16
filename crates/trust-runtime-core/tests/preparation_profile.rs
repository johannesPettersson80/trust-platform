//! Admission regressions consume the actual saved artifact without a compiler.
use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeModule, SectionData, SectionId, StorageRole,
    BYTECODE_MAX_CONTAINER_BYTES,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};
const BYTES: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
fn decoded() -> BytecodeModule {
    BytecodeModule::decode(BYTES).unwrap()
}
#[test]
fn decoded_and_byte_entry_points_enforce_the_same_artifact_bound() {
    let raw = decoded();
    let bytes = raw.encode().unwrap();
    assert_eq!(raw.encode_with_limit(bytes.len()).unwrap(), bytes);
    assert!(raw.encode_with_limit(bytes.len() - 1).is_err());
    let exact = PreparationLimits {
        max_artifact_bytes: bytes.len(),
        ..Default::default()
    };
    PreparedModule::from_bytes(&bytes, exact).unwrap();
    PreparedModule::from_decoded(&raw, exact).unwrap();
    let below = PreparationLimits {
        max_artifact_bytes: bytes.len() - 1,
        ..exact
    };
    assert!(PreparedModule::from_bytes(&bytes, below).is_err());
    assert!(PreparedModule::from_decoded(&raw, below).is_err());
}
#[test]
fn container_bound_is_reported_before_decode_work_is_charged() {
    // Spec 12 §4.6: the 64 MiB container bound precedes every decode charge, so a host budget
    // cannot turn an oversized artifact into a work-limit error.
    let oversized = vec![0u8; BYTECODE_MAX_CONTAINER_BYTES + 1];
    let expected =
        BytecodeError::InvalidHeader("encoded container exceeds fixed resource limit".into());
    assert_eq!(
        BytecodeModule::decode_with_limits(&oversized, usize::MAX, usize::MAX).unwrap_err(),
        expected
    );
    // This work budget is below the artifact's own length. Charging work first reported a work
    // limit here, so the fixed bound must be checked before any charge.
    assert_eq!(
        BytecodeModule::decode_with_limits(&oversized, usize::MAX, BYTECODE_MAX_CONTAINER_BYTES)
            .unwrap_err(),
        expected
    );
    let raised = PreparationLimits {
        max_artifact_bytes: 2 * BYTECODE_MAX_CONTAINER_BYTES,
        max_preparation_work: 2 * BYTECODE_MAX_CONTAINER_BYTES,
        ..Default::default()
    };
    assert_eq!(
        PreparedModule::from_bytes(&oversized, raised).unwrap_err(),
        RuntimeError::from(expected)
    );
}

#[test]
fn preparation_demand_has_exact_accounting_boundaries() {
    let raw = decoded();
    let baseline = PreparedModule::from_decoded(&raw, Default::default())
        .unwrap()
        .preparation_usage();
    assert!(baseline.bytes > 0 && baseline.work > 0);
    let limits = PreparationLimits {
        max_preparation_bytes: baseline.bytes,
        max_preparation_work: baseline.work,
        ..Default::default()
    };
    assert_eq!(
        PreparedModule::from_decoded(&raw, limits)
            .unwrap()
            .preparation_usage(),
        baseline
    );
    // The cumulative charge that crosses either limit is refused at budget.rs:43-46.
    assert_eq!(
        PreparedModule::from_decoded(
            &raw,
            PreparationLimits {
                max_preparation_bytes: baseline.bytes - 1,
                ..limits
            }
        )
        .unwrap_err(),
        RuntimeError::PreparationLimit
    );
    assert_eq!(
        PreparedModule::from_decoded(
            &raw,
            PreparationLimits {
                max_preparation_work: baseline.work - 1,
                ..limits
            }
        )
        .unwrap_err(),
        RuntimeError::PreparationLimit
    );
}
#[test]
fn unresolved_native_import_is_rejected_before_instantiation() {
    let mut raw = decoded();
    let Some(SectionData::StringTable(strings)) = raw.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let symbol = strings
        .entries
        .iter_mut()
        .find(|name| name.starts_with("DINT_TO_INT|"))
        .expect("fixture's native conversion");
    *symbol = symbol
        .replacen("DINT_TO_INT", "UNAPPROVED_NATIVE", 1)
        .into();
    raw.validated_source_free(Default::default())
        .expect("wire validity does not resolve native imports");
    assert!(PreparedModule::from_decoded(&raw, Default::default()).is_err());
}
#[test]
fn native_state_layout_is_an_execution_admission_contract() {
    let mut raw = decoded();
    let Some(SectionData::StringTable(strings)) = raw.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let alternate = strings
        .entries
        .iter()
        .position(|name| name == "result")
        .unwrap() as u32;
    let Some(SectionData::StorageLayout(layout)) = raw.section_mut(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let slot = layout
        .entries
        .iter_mut()
        .find(|entry| entry.role == StorageRole::NativeState)
        .expect("TON state");
    slot.name_idx = alternate;
    raw.validated_source_free(Default::default())
        .expect("wire shape is still valid");
    assert!(PreparedModule::from_decoded(&raw, Default::default()).is_err());
}
#[test]
fn validated_persistent_demand_cannot_exceed_profile_value_limit() {
    let raw = decoded();
    assert!(PreparedModule::from_decoded(
        &raw,
        PreparationLimits {
            max_construction_values: 1,
            ..Default::default()
        }
    )
    .is_err());
}
#[test]
fn a_second_resource_cannot_reclassify_its_programs_as_background_work() {
    let mut raw = decoded();
    let Some(SectionData::ResourceMeta(meta)) = raw.section_mut(SectionId::ResourceMeta) else {
        panic!("resources")
    };
    meta.resources.push(meta.resources[0].clone());
    assert!(PreparedModule::from_decoded(&raw, Default::default()).is_err());
}
#[test]
fn empty_string_slots_are_charged_before_decoder_reservation() {
    use trust_runtime_core::bytecode::{BytecodeVersion, Section, StringTable};
    let count = 1024usize;
    let mut raw = BytecodeModule::new(BytecodeVersion::SOURCE_FREE);
    raw.sections.push(Section {
        id: SectionId::StringTable.as_raw(),
        flags: 0,
        data: SectionData::StringTable(StringTable {
            entries: vec!["".into(); count],
        }),
    });
    let bytes = raw.encode().unwrap();
    let (_, usage) = BytecodeModule::decode_with_limits(&bytes, usize::MAX, usize::MAX).unwrap();
    let slots = count * core::mem::size_of::<smol_str::SmolStr>();
    assert!(
        slots > bytes.len(),
        "empty wire strings expand to owned slots"
    );
    assert!(usage.allocation_bytes >= slots);
    BytecodeModule::decode_with_limits(&bytes, usage.allocation_bytes, usage.work).unwrap();
    let error = BytecodeModule::decode_with_limits(&bytes, usage.allocation_bytes - 1, usage.work)
        .unwrap_err();
    assert_eq!(
        error.stable_code(),
        trust_runtime_core::error_code::StableErrorCode::BytecodeDecodeMemoryLimit
    );
    let error = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_preparation_bytes: slots - 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        error.stable_code()
            == trust_runtime_core::error_code::StableErrorCode::BytecodeDecodeMemoryLimit,
        "reservation must fail before missing-section validation"
    );
}
#[test]
fn byte_entry_cumulative_preparation_includes_decoded_tables() {
    let usage = PreparedModule::from_bytes(BYTES, Default::default())
        .unwrap()
        .preparation_usage();
    let limits = PreparationLimits {
        max_preparation_bytes: usage.bytes,
        max_preparation_work: usage.work,
        ..Default::default()
    };
    assert_eq!(
        PreparedModule::from_bytes(BYTES, limits)
            .unwrap()
            .preparation_usage(),
        usage
    );
    // The byte route refuses the same cumulative charge at budget.rs:43-46.
    assert_eq!(
        PreparedModule::from_bytes(
            BYTES,
            PreparationLimits {
                max_preparation_bytes: usage.bytes - 1,
                ..limits
            }
        )
        .unwrap_err(),
        RuntimeError::PreparationLimit
    );
}

#[test]
fn decoder_work_exhaustion_is_distinct_from_malformed_wire_input() {
    use trust_runtime_core::error_code::StableErrorCode;
    let (_, usage) = BytecodeModule::decode_with_limits(BYTES, usize::MAX, usize::MAX).unwrap();
    let error = BytecodeModule::decode_with_limits(BYTES, usize::MAX, usage.work - 1).unwrap_err();
    assert_eq!(
        error.stable_code(),
        StableErrorCode::BytecodeDecodeWorkLimit
    );
}

#[test]
fn execution_profile_rejection_is_not_a_bytecode_format_error() {
    use trust_runtime_core::error_code::StableErrorCode;
    let raw = decoded();
    let error = PreparedModule::from_decoded(
        &raw,
        PreparationLimits {
            max_call_depth: 0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeProfileUnsupported
    );
    let error = PreparedModule::from_bytes(
        BYTES,
        PreparationLimits {
            max_artifact_bytes: BYTES.len() - 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimePreparationLimit
    );
}

#[test]
fn call_depth_admission_is_exact_at_the_fixed_limit() {
    // Spec 12 §4.6 fixes 1,024 active call frames; prepared.rs:165-171 refuses 1,025.
    let raw = decoded();
    let at_limit = PreparationLimits {
        max_call_depth: 1024,
        ..Default::default()
    };
    PreparedModule::from_decoded(&raw, at_limit).unwrap();
    PreparedModule::from_bytes(BYTES, at_limit).unwrap();
    let above = PreparationLimits {
        max_call_depth: 1025,
        ..Default::default()
    };
    let refused = RuntimeError::ProfileUnsupported("invalid preparation profile limits".into());
    assert_eq!(
        PreparedModule::from_decoded(&raw, above).unwrap_err(),
        refused
    );
    assert_eq!(
        PreparedModule::from_bytes(BYTES, above).unwrap_err(),
        refused
    );
}

#[test]
fn zero_time_allowances_are_unsupported_on_both_routes() {
    // Spec 34 TIME-09; prepared.rs:165-171 refuses a zero scan or construction allowance.
    let raw = decoded();
    let unsupported = RuntimeError::ProfileUnsupported("invalid preparation profile limits".into());
    for limits in [
        PreparationLimits {
            max_work: 0,
            ..Default::default()
        },
        PreparationLimits {
            max_construction_work: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            PreparedModule::from_decoded(&raw, limits).unwrap_err(),
            unsupported
        );
        assert_eq!(
            PreparedModule::from_bytes(BYTES, limits).unwrap_err(),
            unsupported
        );
    }
}

#[test]
fn zero_construction_bytes_are_admitted_and_refuse_the_first_construction_charge() {
    // TIME-09 refuses only zero time allowances at preparation. A zero byte allowance is
    // admitted, and instantiation refuses its first construction charge at used 0
    // (engine/allocation.rs:32), before any value is constructed.
    let limits = PreparationLimits {
        max_construction_bytes: 0,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(BYTES, limits).unwrap();
    let error = prepared.instantiate(0).err();
    assert!(
        matches!(
            error,
            Some(RuntimeError::ExecutionLimit {
                kind: trust_runtime_core::error::ExecutionLimitKind::AllocationBytes,
                used: 0,
                requested,
                limit: 0,
            }) if requested > 0
        ),
        "unexpected construction refusal: {error:?}"
    );
}

#[test]
fn process_image_limit_is_exact_for_inputs_outputs_and_memory() {
    // gpio.st declares %IX0.0 and %QX0.0, so the admitted process image is non-empty.
    let raw = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::ResourceMeta(meta)) = raw.section(SectionId::ResourceMeta) else {
        panic!("resources")
    };
    let [resource] = meta.resources.as_slice() else {
        panic!("one resource")
    };
    let image = resource.inputs_size as usize
        + resource.outputs_size as usize
        + resource.memory_size as usize;
    assert!(image > 0);
    let exact = PreparationLimits {
        max_process_image_bytes: image,
        ..Default::default()
    };
    PreparedModule::from_decoded(&raw, exact).unwrap();
    PreparedModule::from_bytes(GPIO, exact).unwrap();
    let below = PreparationLimits {
        max_process_image_bytes: image - 1,
        ..Default::default()
    };
    // prepared.rs:275-276 compares the admitted image bytes with the profile limit.
    assert_eq!(
        PreparedModule::from_decoded(&raw, below).unwrap_err(),
        RuntimeError::PreparationLimit
    );
    assert_eq!(
        PreparedModule::from_bytes(GPIO, below).unwrap_err(),
        RuntimeError::PreparationLimit
    );
}
