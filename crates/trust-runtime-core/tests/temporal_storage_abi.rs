//! The saved artifact selects layout; changing the producer default cannot change old bytes.

use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, BytecodeError, BytecodeModule, BytecodeVersion, ConstEntry,
    Field, RejectionReason, SectionData, SectionId, TypeData, TypeEntry, TypeKind, TypeTable,
    HEADER_FLAG_CRC32, HEADER_FLAG_LEGACY_TEMPORAL32,
};
use trust_runtime_core::value::{
    size_of_value_with_abi_and_visitor, ArrayValue, DateTimeValue, DateValue, Duration,
    LDateTimeValue, LDateValue, LTimeOfDayValue, SizeOfError, StorageAbi, StructValue,
    TimeOfDayValue, Value,
};
use trust_runtime_core::vm::{sizeof_type_from_table_with_abi, PreparationLimits, PreparedModule};

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

fn type_size(types: &TypeTable, id: u32, abi: StorageAbi) -> u64 {
    sizeof_type_from_table_with_abi(types, id, abi, &mut |_| Ok(())).unwrap()
}

fn value_size(value: &Value, abi: StorageAbi) -> Result<u64, SizeOfError> {
    size_of_value_with_abi_and_visitor(
        value,
        abi,
        &mut |_| Err(SizeOfError::UnsupportedType),
        &mut |_| Ok(None),
        &mut |_| Ok(()),
    )
}

#[test]
fn saved_two_point_zero_bytes_keep_their_abi_and_round_trip_unchanged() {
    let module = BytecodeModule::decode(OLD_ARTIFACT).unwrap();
    assert_eq!(module.version, BytecodeVersion::SOURCE_FREE_V2);
    assert_eq!(module.storage_abi().unwrap(), StorageAbi::LegacyTemporal32);
    assert_eq!(module.encode().unwrap(), OLD_ARTIFACT);
    assert_eq!(
        PreparedModule::from_bytes(OLD_ARTIFACT, PreparationLimits::default())
            .unwrap()
            .storage_abi(),
        StorageAbi::LegacyTemporal32,
    );
}

#[test]
fn current_and_contextual_migration_headers_select_an_explicit_abi() {
    for (version, flags, abi) in [
        (
            BytecodeVersion::SOURCE_FREE_V2,
            HEADER_FLAG_CRC32,
            StorageAbi::LegacyTemporal32,
        ),
        (
            BytecodeVersion::SOURCE_FREE,
            HEADER_FLAG_CRC32,
            StorageAbi::UniformTemporal64,
        ),
        (
            BytecodeVersion::SOURCE_FREE,
            HEADER_FLAG_CRC32 | HEADER_FLAG_LEGACY_TEMPORAL32,
            StorageAbi::LegacyTemporal32,
        ),
    ] {
        let module = saved_artifact(version, flags);
        let bytes = module.encode().unwrap();
        assert_eq!(module.storage_abi().unwrap(), abi);
        let decoded = BytecodeModule::decode(&bytes).unwrap();
        assert_eq!(decoded.storage_abi().unwrap(), abi);
        let from_bytes = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
        let from_object =
            PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
        assert_eq!(from_bytes.storage_abi(), abi);
        assert_eq!(from_object.storage_abi(), abi);
        assert!(decoded
            .disassemble()
            .unwrap()
            .starts_with(&format!("STBC {}.{}", version.major, version.minor)));
    }
    for version in [BytecodeVersion::new(1, 0), BytecodeVersion::LEGACY] {
        assert_eq!(version.storage_abi().unwrap(), StorageAbi::LegacyTemporal32);
    }
    for version in [
        BytecodeVersion::new(1, 2),
        BytecodeVersion::new(2, 1),
        BytecodeVersion::new(3, 1),
        BytecodeVersion::new(4, 0),
    ] {
        assert!(matches!(
            version.storage_abi(),
            Err(BytecodeError::UnsupportedVersion { .. })
        ));
    }
}

#[test]
fn all_temporal_types_and_array_strides_use_the_artifact_abi() {
    let mut types = TypeTable {
        offsets: vec![],
        entries: (16..=23).map(primitive).collect(),
    };
    types.entries.push(TypeEntry {
        kind: TypeKind::Alias,
        name_idx: None,
        data: TypeData::Alias { target_type_id: 0 },
    });
    types.entries.push(TypeEntry {
        kind: TypeKind::Array,
        name_idx: None,
        data: TypeData::Array {
            elem_type_id: 8,
            dims: vec![(-2, 0), (4, 5)],
        },
    });
    for id in 0..8 {
        assert_eq!(type_size(&types, id, StorageAbi::UniformTemporal64), 8);
        assert_eq!(
            type_size(&types, id, StorageAbi::LegacyTemporal32),
            if id % 2 == 0 { 4 } else { 8 }
        );
    }
    assert_eq!(type_size(&types, 9, StorageAbi::UniformTemporal64), 48);
    assert_eq!(type_size(&types, 9, StorageAbi::LegacyTemporal32), 24);

    for ticks in [i64::MIN, -1, i64::from(u32::MAX) + 1, i64::MAX] {
        let values = [
            Value::Time(Duration::from_nanos(ticks)),
            Value::LTime(Duration::from_nanos(ticks)),
            Value::Date(DateValue::new(ticks)),
            Value::LDate(LDateValue::new(ticks)),
            Value::Tod(TimeOfDayValue::new(ticks)),
            Value::LTod(LTimeOfDayValue::new(ticks)),
            Value::Dt(DateTimeValue::new(ticks)),
            Value::Ldt(LDateTimeValue::new(ticks)),
        ];
        for (id, value) in values.iter().enumerate() {
            assert_eq!(value_size(value, StorageAbi::UniformTemporal64), Ok(8));
            assert_eq!(
                value_size(value, StorageAbi::LegacyTemporal32),
                Ok(if id % 2 == 0 { 4 } else { 8 })
            );
        }
        let array = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![values[0].clone(); 6],
            vec![(-2, 0), (4, 5)],
        )));
        assert_eq!(value_size(&array, StorageAbi::UniformTemporal64), Ok(48));
        assert_eq!(value_size(&array, StorageAbi::LegacyTemporal32), Ok(24));
    }
}

#[test]
fn constant_payloads_remain_signed_eight_bytes_in_both_formats() {
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
        let values = [-1i64, i64::from(u32::MAX) + 1, i64::MIN, i64::MAX];
        for temporal in 0..8 {
            for value in values {
                constants.entries.push(ConstEntry {
                    type_id: first_type + temporal,
                    payload: value.to_le_bytes().to_vec(),
                });
            }
        }
        let bytes = module.encode().unwrap();
        let decoded = BytecodeModule::decode(&bytes).unwrap();
        let Some(SectionData::ConstPool(constants)) = decoded.section(SectionId::ConstPool) else {
            panic!("constants")
        };
        for (index, entry) in constants.entries[first_constant as usize..]
            .iter()
            .enumerate()
        {
            assert_eq!(entry.payload, values[index % values.len()].to_le_bytes());
        }
        let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
        for (index, expected) in values.into_iter().enumerate() {
            assert_eq!(
                prepared.constant(first_constant + index as u32),
                Some(&Value::Time(Duration::from_nanos(expected)))
            );
        }
    }
}

#[test]
fn relative_layout_size_includes_gaps_and_aliases_only_in_the_current_abi() {
    let mut types = TypeTable {
        offsets: vec![],
        entries: vec![primitive(16), primitive(1)],
    };
    for kind in [TypeKind::LocatedStruct, TypeKind::OverlappingStruct] {
        types.entries.push(TypeEntry {
            kind,
            name_idx: None,
            data: TypeData::Struct {
                fields: vec![
                    Field {
                        name_idx: 0,
                        type_id: 1,
                        bit_offset: Some(3),
                    },
                    Field {
                        name_idx: 1,
                        type_id: 0,
                        bit_offset: Some(64),
                    },
                ],
            },
        });
    }
    types.entries.push(TypeEntry {
        kind: TypeKind::Array,
        name_idx: None,
        data: TypeData::Array {
            elem_type_id: 2,
            dims: vec![(1, 3)],
        },
    });
    for id in [2, 3] {
        assert_eq!(type_size(&types, id, StorageAbi::UniformTemporal64), 16);
        assert_eq!(type_size(&types, id, StorageAbi::LegacyTemporal32), 5);
    }
    assert_eq!(type_size(&types, 4, StorageAbi::UniformTemporal64), 48);
    assert_eq!(type_size(&types, 4, StorageAbi::LegacyTemporal32), 15);
    let value = Value::Struct(std::sync::Arc::new(StructValue::from_canonical_parts(
        "Located".into(),
        [
            ("bit".into(), Value::Bool(false)),
            ("time".into(), Value::Time(Duration::from_nanos(-1))),
        ]
        .into_iter()
        .collect(),
    )));
    for (abi, expected) in [
        (StorageAbi::UniformTemporal64, 16),
        (StorageAbi::LegacyTemporal32, 5),
    ] {
        let size: Result<u64, SizeOfError> = size_of_value_with_abi_and_visitor(
            &value,
            abi,
            &mut |_| Err(SizeOfError::UnsupportedType),
            &mut |name| {
                assert_eq!(name, "Located");
                Ok(Some(16))
            },
            &mut |_| Ok(()),
        );
        assert_eq!(size, Ok(expected));
    }
}

#[test]
fn relative_wire_records_cannot_be_encoded_or_decoded_as_an_older_format() {
    for kind in [TypeKind::OverlappingStruct, TypeKind::LocatedStruct] {
        let mut module = saved_artifact(BytecodeVersion::SOURCE_FREE, HEADER_FLAG_CRC32);
        let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable)
        else {
            panic!("strings")
        };
        let name = strings.entries.len() as u32;
        strings
            .entries
            .extend(["TemporalLayout".into(), "time".into()]);
        let Some(SectionData::TypeTable(types)) = module.section_mut(SectionId::TypeTable) else {
            panic!("types")
        };
        let temporal = types.entries.len() as u32;
        types.entries.push(primitive(16));
        types.entries.push(TypeEntry {
            kind,
            name_idx: Some(name),
            data: TypeData::Struct {
                fields: vec![Field {
                    name_idx: name + 1,
                    type_id: temporal,
                    bit_offset: Some(8),
                }],
            },
        });
        types.offsets = compute_type_offsets_for_entries(&types.entries).unwrap();
        let bytes = module.encode().unwrap();
        PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
        for version in [BytecodeVersion::LEGACY, BytecodeVersion::SOURCE_FREE_V2] {
            module.version = version;
            let reason = BytecodeError::from(RejectionReason::InvalidTypeKind);
            assert_eq!(module.encode().unwrap_err(), reason);
            let mut raw_older = module.clone();
            if version.major == 1 {
                // Typed construction sections themselves are not 1.x records.
                // Remove them so this object assertion reaches the type gate.
                raw_older
                    .sections
                    .retain(|section| section.id < SectionId::StorageLayout.as_raw());
            }
            assert_eq!(raw_older.validate().unwrap_err(), reason);
            // Relabel only the version and give the file the older format's valid
            // checksum (section table onward), then prove the reader itself
            // refuses the new required type record. A plain relabel fails the
            // 3.0 whole-file checksum (stbc_header_integrity.rs).
            let mut downgraded = bytes.clone();
            downgraded[4..6].copy_from_slice(&version.major.to_le_bytes());
            downgraded[6..8].copy_from_slice(&version.minor.to_le_bytes());
            let table = u32::from_le_bytes(downgraded[16..20].try_into().unwrap()) as usize;
            let checksum = trust_runtime_core::crc32::checksum(&downgraded[table..]);
            downgraded[20..24].copy_from_slice(&checksum.to_le_bytes());
            assert_eq!(BytecodeModule::decode(&downgraded).unwrap_err(), reason);
        }
    }
}
