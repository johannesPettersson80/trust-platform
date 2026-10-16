//! Bytecode-table SIZEOF of located and overlapping structures at bit and byte boundaries (spec
//! 12 §4.1). The 3.0 uniform layout measures the complete relative extent, rounded up to whole
//! bytes, and refuses a non-BOOL field that is not byte-aligned. The 1.1/2.0 layout keeps the
//! historic sum of field sizes and ignores the relative offsets. Relative offsets are u32 bit
//! locations in TYPE_TABLE, so the largest cases here use the largest representable offsets.
use trust_runtime_core::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
use trust_runtime_core::value::StorageAbi;
use trust_runtime_core::vm::sizeof_type_from_table_with_abi;

// Table ids 0, 1 and 2 are BOOL, TIME and LTIME. The cases below are appended from id 3.
const BOOL: u32 = 0;
const TIME: u32 = 1;
const LTIME: u32 = 2;

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

fn structure(kind: TypeKind, fields: &[(u32, Option<u32>)]) -> TypeEntry {
    TypeEntry {
        kind,
        name_idx: None,
        data: TypeData::Struct {
            fields: fields
                .iter()
                .enumerate()
                .map(|(index, &(type_id, bit_offset))| Field {
                    name_idx: index as u32,
                    type_id,
                    bit_offset,
                })
                .collect(),
        },
    }
}

fn table(extra: Vec<TypeEntry>) -> TypeTable {
    let mut entries = vec![primitive(1), primitive(16), primitive(17)];
    entries.extend(extra);
    TypeTable {
        offsets: vec![],
        entries,
    }
}

fn size(types: &TypeTable, id: u32, abi: StorageAbi) -> u64 {
    sizeof_type_from_table_with_abi(types, id, abi, &mut |_| Ok(())).unwrap()
}

fn refused(types: &TypeTable, id: u32, abi: StorageAbi) -> bool {
    sizeof_type_from_table_with_abi(types, id, abi, &mut |_| Ok(())).is_err()
}

#[test]
fn located_extents_round_up_to_whole_bytes_only_in_the_uniform_layout() {
    let types = table(vec![
        // 3: BOOL at bit 71 (byte 8, bit 7) ends at bit 72, so nine bytes.
        structure(TypeKind::LocatedStruct, &[(BOOL, Some(71))]),
        // 4: BOOL at bit 63 ends exactly at bit 64, so eight bytes.
        structure(TypeKind::LocatedStruct, &[(BOOL, Some(63))]),
        // 5: TIME at byte 8 leaves a gap in bytes 0 to 7 and ends at bit 128. Legacy sums 4.
        structure(TypeKind::LocatedStruct, &[(TIME, Some(64))]),
        // 6: TIME and LTIME share byte 0, so the uniform extent is 8. Legacy sums 4 + 8.
        structure(
            TypeKind::OverlappingStruct,
            &[(TIME, Some(0)), (LTIME, Some(0))],
        ),
        // 7: TIME at bit 4 is not byte-aligned, which the uniform layout refuses.
        structure(TypeKind::LocatedStruct, &[(TIME, Some(4))]),
        // 8: BOOL at bit 3 and LTIME at byte 8 end at bit 128. Legacy sums 1 + 8.
        structure(
            TypeKind::LocatedStruct,
            &[(BOOL, Some(3)), (LTIME, Some(64))],
        ),
    ]);
    assert_eq!(size(&types, 3, StorageAbi::UniformTemporal64), 9);
    assert_eq!(size(&types, 3, StorageAbi::LegacyTemporal32), 1);
    assert_eq!(size(&types, 4, StorageAbi::UniformTemporal64), 8);
    assert_eq!(size(&types, 4, StorageAbi::LegacyTemporal32), 1);
    assert_eq!(size(&types, 5, StorageAbi::UniformTemporal64), 16);
    assert_eq!(size(&types, 5, StorageAbi::LegacyTemporal32), 4);
    assert_eq!(size(&types, 6, StorageAbi::UniformTemporal64), 8);
    assert_eq!(size(&types, 6, StorageAbi::LegacyTemporal32), 12);
    assert!(refused(&types, 7, StorageAbi::UniformTemporal64));
    assert_eq!(size(&types, 7, StorageAbi::LegacyTemporal32), 4);
    assert_eq!(size(&types, 8, StorageAbi::UniformTemporal64), 16);
    assert_eq!(size(&types, 8, StorageAbi::LegacyTemporal32), 9);
}

#[test]
fn located_extents_stay_exact_at_the_largest_representable_bit_offsets() {
    let types = table(vec![
        // 3: TIME at bit 4294967288 (byte 536870911) ends at bit 4294967352, which is 536870919
        // bytes. The extent is computed in 64 bits, so it does not wrap at 2^32.
        structure(TypeKind::LocatedStruct, &[(TIME, Some(4_294_967_288))]),
        // 4: BOOL at bit u32::MAX ends at bit 2^32, which is 536870912 bytes exactly.
        structure(TypeKind::LocatedStruct, &[(BOOL, Some(u32::MAX))]),
        // 5: TIME at bit u32::MAX is not byte-aligned, so the uniform layout refuses it.
        structure(TypeKind::LocatedStruct, &[(TIME, Some(u32::MAX))]),
    ]);
    assert_eq!(size(&types, 3, StorageAbi::UniformTemporal64), 536_870_919);
    assert_eq!(size(&types, 3, StorageAbi::LegacyTemporal32), 4);
    assert_eq!(size(&types, 4, StorageAbi::UniformTemporal64), 536_870_912);
    assert_eq!(size(&types, 4, StorageAbi::LegacyTemporal32), 1);
    assert!(refused(&types, 5, StorageAbi::UniformTemporal64));
    assert_eq!(size(&types, 5, StorageAbi::LegacyTemporal32), 4);
}

#[test]
fn array_of_located_elements_strides_by_the_complete_extent_of_each_element() {
    let types = table(vec![
        // 3: TIME at bit 64 is 16 bytes uniform and 4 bytes legacy.
        structure(TypeKind::LocatedStruct, &[(TIME, Some(64))]),
        // 4: ARRAY[-1..1] OF element 3, three elements.
        TypeEntry {
            kind: TypeKind::Array,
            name_idx: None,
            data: TypeData::Array {
                elem_type_id: 3,
                dims: vec![(-1, 1)],
            },
        },
    ]);
    assert_eq!(size(&types, 4, StorageAbi::UniformTemporal64), 48);
    assert_eq!(size(&types, 4, StorageAbi::LegacyTemporal32), 12);
}
