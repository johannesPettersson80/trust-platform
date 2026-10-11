//! Byte transport is decoded once; struct-built input retains bounded encoding.
use trust_runtime_core::{
    bytecode::{BytecodeModule, SectionData, SectionId, ValidationLimits},
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
        assert!(prepare(PreparationLimits {
            max_preparation_bytes: usage.bytes - 1,
            ..exact
        })
        .is_err());
        assert!(prepare(PreparationLimits {
            max_preparation_work: usage.work - 1,
            ..exact
        })
        .is_err());
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
