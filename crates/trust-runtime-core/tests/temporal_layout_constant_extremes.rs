//! Each temporal constant-pool payload decodes to the value variant of its own type at the signed
//! 64-bit extremes, in the 2.0 and 3.0 artifact layouts. Spec 12 §6.3: TIME and LTIME payloads are
//! i64 nanoseconds, DATE, TOD and DT are i64 profile ticks, and LDATE, LTOD and LDT are i64
//! nanoseconds. Spec 12 §4.1: constant payloads stay eight bytes in both formats.
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, BytecodeModule, BytecodeVersion, ConstEntry, SectionData,
    SectionId, TypeData, TypeEntry, TypeKind, HEADER_FLAG_CRC32,
};
use trust_runtime_core::value::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, Value,
};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const OLD_ARTIFACT: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

fn primitive(prim_id: u16) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id,
            max_length: 0,
        },
    }
}

fn saved_artifact(version: BytecodeVersion, flags: u32) -> BytecodeModule {
    let mut module = BytecodeModule::decode(OLD_ARTIFACT).unwrap();
    module.version = version;
    module.flags = flags;
    module
}

/// The value that a constant of temporal type `16 + index` must decode to.
fn expected(index: u32, value: i64) -> Value {
    match index {
        0 => Value::Time(Duration::from_nanos(value)),
        1 => Value::LTime(Duration::from_nanos(value)),
        2 => Value::Date(DateValue::new(value)),
        3 => Value::LDate(LDateValue::new(value)),
        4 => Value::Tod(TimeOfDayValue::new(value)),
        5 => Value::LTod(LTimeOfDayValue::new(value)),
        6 => Value::Dt(DateTimeValue::new(value)),
        _ => Value::Ldt(LDateTimeValue::new(value)),
    }
}

#[test]
fn every_temporal_constant_decodes_to_its_own_variant_at_the_64_bit_extremes() {
    let values = [i64::MIN, -1, i64::from(u32::MAX) + 1, i64::MAX];
    for version in [
        BytecodeVersion::SOURCE_FREE_V2,
        BytecodeVersion::SOURCE_FREE,
    ] {
        let mut module = saved_artifact(version, HEADER_FLAG_CRC32);
        let Some(SectionData::TypeTable(types)) = module.section_mut(SectionId::TypeTable) else {
            panic!("types")
        };
        let first_type = types.entries.len() as u32;
        types.entries.extend((16..=23).map(primitive));
        types.offsets = compute_type_offsets_for_entries(&types.entries).unwrap();
        let Some(SectionData::ConstPool(constants)) = module.section_mut(SectionId::ConstPool)
        else {
            panic!("constants")
        };
        let first_constant = constants.entries.len() as u32;
        for temporal in 0..8 {
            for value in values {
                constants.entries.push(ConstEntry {
                    type_id: first_type + temporal,
                    payload: value.to_le_bytes().to_vec(),
                });
            }
        }
        let bytes = module.encode().unwrap();
        let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
        for temporal in 0..8 {
            for (offset, value) in values.into_iter().enumerate() {
                let index = first_constant + temporal * 4 + offset as u32;
                assert_eq!(
                    prepared.constant(index),
                    Some(&expected(temporal, value)),
                    "{version:?} type {}",
                    16 + temporal
                );
            }
        }
    }
}
