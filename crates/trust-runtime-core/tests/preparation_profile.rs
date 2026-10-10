//! Admission regressions consume the actual saved artifact without a compiler.
use trust_runtime_core::bytecode::{BytecodeModule, SectionData, SectionId, StorageRole};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};
const BYTES: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
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
    assert!(PreparedModule::from_decoded(
        &raw,
        PreparationLimits {
            max_preparation_bytes: baseline.bytes - 1,
            ..limits
        }
    )
    .is_err());
    assert!(PreparedModule::from_decoded(
        &raw,
        PreparationLimits {
            max_preparation_work: baseline.work - 1,
            ..limits
        }
    )
    .is_err());
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
    assert!(PreparedModule::from_bytes(
        BYTES,
        PreparationLimits {
            max_preparation_bytes: usage.bytes - 1,
            ..limits
        }
    )
    .is_err());
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
