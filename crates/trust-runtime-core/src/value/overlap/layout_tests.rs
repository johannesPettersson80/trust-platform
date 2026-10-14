use super::*;
use crate::bytecode::{EnumVariant, Field, TypeEntry};
use alloc::vec;

fn entry(kind: TypeKind, data: TypeData) -> TypeEntry {
    TypeEntry {
        kind,
        name_idx: None,
        data,
    }
}

fn primitive(id: u16) -> TypeEntry {
    entry(
        TypeKind::Primitive,
        TypeData::Primitive {
            prim_id: id,
            max_length: 0,
        },
    )
}

fn field(name: u32, ty: u32, offset: Option<u32>) -> Field {
    Field {
        name_idx: name,
        type_id: ty,
        bit_offset: offset,
    }
}

fn structure(kind: TypeKind, fields: Vec<Field>) -> TypeEntry {
    entry(kind, TypeData::Struct { fields })
}

fn table(entries: Vec<TypeEntry>) -> TypeTable {
    TypeTable {
        offsets: Vec::new(),
        entries,
    }
}

fn layouts(types: &TypeTable) -> Result<Vec<Option<Arc<PhysicalLayout>>>, RuntimeError> {
    prepare_layouts(
        types,
        &["a".into(), "b".into(), "c".into()],
        crate::value::StorageAbi::default(),
        &mut |_, _| Ok(()),
    )
}

#[test]
fn physical_extent_uses_the_same_64_edge_boundary_as_admission() {
    let mut types = table(vec![primitive(1)]);
    for depth in 1..=BYTECODE_MAX_TYPE_NESTING + 1 {
        types.entries.push(entry(
            TypeKind::Alias,
            TypeData::Alias {
                target_type_id: depth as u32 - 1,
            },
        ));
        let extent = type_extent_bits(
            &types,
            depth as u32,
            crate::value::StorageAbi::default(),
            &mut |_, _| Ok(()),
        );
        if depth <= BYTECODE_MAX_TYPE_NESTING {
            assert_eq!(extent, Ok(1));
        } else {
            assert_eq!(extent, Err(RuntimeError::TypeMismatch));
        }
    }
}

#[test]
fn located_bools_use_bits_while_arrays_use_byte_strides() {
    for last in [1, 7] {
        let types = table(vec![
            primitive(1),
            structure(
                TypeKind::LocatedStruct,
                vec![field(0, 0, Some(0)), field(1, 0, Some(last))],
            ),
        ]);
        assert_eq!(
            type_extent_bits(
                &types,
                1,
                crate::value::StorageAbi::default(),
                &mut |_, _| Ok(())
            )
            .unwrap(),
            8
        );
        assert!(layouts(&types).unwrap().iter().all(Option::is_none));
    }
    let types = table(vec![
        primitive(1),
        entry(
            TypeKind::Array,
            TypeData::Array {
                elem_type_id: 0,
                dims: vec![(1, 2)],
            },
        ),
    ]);
    assert_eq!(
        type_extent_bits(
            &types,
            1,
            crate::value::StorageAbi::default(),
            &mut |_, _| Ok(())
        )
        .unwrap(),
        16
    );
}

#[test]
fn located_layout_requires_every_location_and_rejects_scalar_misalignment_and_intersection() {
    for fields in [
        vec![field(0, 0, None), field(1, 0, Some(0))],
        vec![field(0, 0, Some(1))],
        vec![field(0, 0, Some(0)), field(1, 0, Some(8))],
    ] {
        let types = table(vec![
            primitive(3),
            structure(TypeKind::LocatedStruct, fields),
        ]);
        assert!(type_extent_bits(
            &types,
            1,
            crate::value::StorageAbi::default(),
            &mut |_, _| Ok(())
        )
        .is_err());
        assert!(layouts(&types).is_err());
    }
}

#[test]
fn ordinary_located_strings_and_unions_remain_logical_with_complete_extents() {
    let types = table(vec![
        primitive(2),
        primitive(3),
        entry(
            TypeKind::Primitive,
            TypeData::Primitive {
                prim_id: 24,
                max_length: 8,
            },
        ),
        entry(
            TypeKind::Union,
            TypeData::Union {
                fields: vec![field(0, 0, None), field(1, 1, None)],
            },
        ),
        structure(
            TypeKind::LocatedStruct,
            vec![field(0, 2, Some(0)), field(1, 3, Some(64))],
        ),
    ]);
    assert_eq!(
        type_extent_bits(
            &types,
            3,
            crate::value::StorageAbi::default(),
            &mut |_, _| Ok(())
        )
        .unwrap(),
        16
    );
    assert_eq!(
        type_extent_bits(
            &types,
            4,
            crate::value::StorageAbi::default(),
            &mut |_, _| Ok(())
        )
        .unwrap(),
        80
    );
    assert!(layouts(&types).unwrap().iter().all(Option::is_none));
}

#[test]
fn only_actual_opaque_intersections_refuse_overlapping_structures() {
    let mut types = table(vec![
        primitive(3),
        entry(
            TypeKind::Primitive,
            TypeData::Primitive {
                prim_id: 25,
                max_length: 4,
            },
        ),
        structure(
            TypeKind::OverlappingStruct,
            vec![field(0, 1, Some(0)), field(1, 0, Some(64))],
        ),
    ]);
    assert!(layouts(&types).unwrap()[2].is_some());
    let TypeData::Struct { fields } = &mut types.entries[2].data else {
        unreachable!()
    };
    fields[1].bit_offset = Some(48);
    assert!(layouts(&types).is_err());
}

#[test]
fn physical_enums_use_the_declared_integer_base_and_keep_range_constraints() {
    for (base, value, signed) in [(7, -1, true), (11, 200, false)] {
        let types = table(vec![
            primitive(base),
            entry(
                TypeKind::Enum,
                TypeData::Enum {
                    base_type_id: 0,
                    variants: vec![EnumVariant { name_idx: 0, value }],
                },
            ),
            structure(TypeKind::OverlappingStruct, vec![field(0, 1, Some(0))]),
        ]);
        let prepared = layouts(&types).unwrap();
        let leaf = &prepared[2].as_ref().unwrap().leaves[0];
        assert_eq!(leaf.width, 16);
        assert_eq!(leaf.enumeration.as_ref().unwrap().signed, signed);
    }
    let types = table(vec![
        primitive(7),
        entry(
            TypeKind::Subrange,
            TypeData::Subrange {
                base_type_id: 0,
                lower: 0,
                upper: 10,
            },
        ),
        structure(TypeKind::OverlappingStruct, vec![field(0, 1, Some(0))]),
    ]);
    let prepared = layouts(&types).unwrap();
    assert_eq!(prepared[2].as_ref().unwrap().leaves[0].range, Some((0, 10)));
}

#[test]
fn nested_array_structures_register_each_child_backing_path() {
    let types = table(vec![
        primitive(3),
        structure(TypeKind::Struct, vec![field(0, 0, None)]),
        entry(
            TypeKind::Array,
            TypeData::Array {
                elem_type_id: 1,
                dims: vec![(4, 5)],
            },
        ),
        structure(TypeKind::OverlappingStruct, vec![field(0, 2, Some(0))]),
    ]);
    let prepared = layouts(&types).unwrap();
    let root = prepared[3].as_ref().unwrap();
    assert_eq!(root.bits, 32);
    assert_eq!(root.children.len(), 2);
    for (child, (ordinal, offset)) in root.children.iter().zip([(4, 0), (5, 16)]) {
        assert_eq!(
            child.path,
            vec![
                RefSegment::Field("a".into()),
                RefSegment::Index(vec![ordinal])
            ]
        );
        assert_eq!(child.offset, offset);
        assert_eq!(child.bits, 16);
    }
}

#[test]
fn index_path_copy_charges_deep_payload_before_allocation() {
    let source = vec![
        RefSegment::Field("a".into()),
        RefSegment::Index(vec![1, 2, 3]),
    ];
    let mut allocated = 0;
    let copied = clone_path(&source, &mut |bytes, _| {
        allocated += bytes;
        Ok(())
    })
    .unwrap();
    assert_eq!(copied, source);
    assert_eq!(
        allocated,
        2 * size_of::<RefSegment>() + 3 * size_of::<i64>()
    );
    let error = clone_path(&source, &mut |_, _| Err(RuntimeError::Overflow));
    assert_eq!(error, Err(RuntimeError::Overflow));
}

#[test]
fn multidimensional_leaf_expansion_stops_at_the_admitted_work_or_storage_limit() {
    use crate::error::ExecutionLimitKind;
    let types = table(vec![
        primitive(7),
        entry(
            TypeKind::Array,
            TypeData::Array {
                elem_type_id: 0,
                dims: vec![(1, 100), (1, 100)],
            },
        ),
        structure(TypeKind::OverlappingStruct, vec![field(0, 1, Some(0))]),
    ]);
    for (kind, limit) in [
        (ExecutionLimitKind::AllocationBytes, 4096),
        (ExecutionLimitKind::Work, 64),
    ] {
        let mut used = 0usize;
        let result = prepare_layouts(
            &types,
            &["a".into()],
            crate::value::StorageAbi::default(),
            &mut |bytes, work| {
                let requested = if kind == ExecutionLimitKind::AllocationBytes {
                    bytes
                } else {
                    work
                };
                if requested > limit - used {
                    return Err(RuntimeError::ExecutionLimit {
                        kind,
                        used,
                        requested,
                        limit,
                    });
                }
                used += requested;
                Ok(())
            },
        );
        assert!(matches!(result, Err(RuntimeError::ExecutionLimit {
            kind: actual, used: accepted, requested, limit: admitted,
        }) if actual == kind && accepted == used && admitted == limit && requested > limit - used));
        assert!(used <= limit);
    }
}

#[test]
fn shared_layout_walk_preserves_each_budget_failure_boundary() {
    let types = table(vec![
        primitive(7),
        structure(
            TypeKind::OverlappingStruct,
            vec![field(0, 0, Some(0)), field(1, 0, Some(0))],
        ),
    ]);
    let strings = ["a".into(), "b".into()];
    let mut charges = Vec::new();
    let completed = prepare_layouts(
        &types,
        &strings,
        crate::value::StorageAbi::default(),
        &mut |bytes, work| {
            charges.push((bytes, work));
            Ok(())
        },
    )
    .unwrap();
    assert!(completed[1].is_some());
    assert!(!charges.is_empty());

    // Validation and materialization use different captured budget owners. Every
    // callback rejection must stop this same shared traversal at that boundary.
    for stop in 0..charges.len() {
        let mut observed = Vec::new();
        let result = prepare_layouts(
            &types,
            &strings,
            crate::value::StorageAbi::default(),
            &mut |bytes, work| {
                observed.push((bytes, work));
                if observed.len() == stop + 1 {
                    Err(RuntimeError::PreparationLimit)
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result, Err(RuntimeError::PreparationLimit));
        assert_eq!(observed, charges[..=stop]);
    }
}
