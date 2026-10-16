//! Native assertions for durable physical aggregate checkpoints.
use super::*;
use crate::{
    bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable},
    collections::OrderedMap,
    value::{overlap::prepare_layouts, StorageAbi},
};
use alloc::boxed::Box;
use alloc::{format, sync::Arc, vec};

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

fn checkpoint_layout(abi: StorageAbi) -> Arc<PhysicalLayout> {
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(4),
            primitive(8),
            TypeEntry {
                kind: TypeKind::OverlappingStruct,
                name_idx: None,
                data: TypeData::Struct {
                    fields: vec![
                        Field {
                            name_idx: 0,
                            type_id: 0,
                            bit_offset: Some(0),
                        },
                        Field {
                            name_idx: 1,
                            type_id: 1,
                            bit_offset: Some(0),
                        },
                    ],
                },
            },
        ],
    };
    let names = ["raw".into(), "signed".into()];
    prepare_layouts(&types, &names, abi, &mut |_, _| Ok(()))
        .unwrap()
        .pop()
        .flatten()
        .unwrap()
}

#[test]
fn signed_narrow_leaf_checkpoint_compares_only_its_storage_width() {
    let layout = checkpoint_layout(StorageAbi::UniformTemporal64);
    let logical = StructValue::from_canonical_parts(
        "SignedOverlay".into(),
        OrderedMap::from_iter([
            ("raw".into(), Value::DWord((-17_i32) as u32)),
            ("signed".into(), Value::DInt(-17)),
        ]),
    )
    .with_physical_layout(Some(layout))
    .unwrap();
    let checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();

    let restored =
        StructValue::from_canonical_parts("SignedOverlay".into(), logical.fields().clone())
            .with_physical_checkpoint(&checkpoint)
            .unwrap();
    assert_eq!(
        restored.encode_physical_checkpoint().unwrap(),
        Some(checkpoint)
    );
}

#[test]
fn checkpoint_preserves_an_admitted_empty_field_name() {
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(8),
            TypeEntry {
                kind: TypeKind::OverlappingStruct,
                name_idx: None,
                data: TypeData::Struct {
                    fields: vec![Field {
                        name_idx: 0,
                        type_id: 0,
                        bit_offset: Some(0),
                    }],
                },
            },
        ],
    };
    let names = ["".into()];
    let layout = prepare_layouts(
        &types,
        &names,
        StorageAbi::UniformTemporal64,
        &mut |_, _| Ok(()),
    )
    .unwrap()
    .pop()
    .flatten()
    .unwrap();
    let logical = StructValue::from_canonical_parts(
        "EmptyNameOverlay".into(),
        OrderedMap::from_iter([("".into(), Value::DInt(7))]),
    )
    .with_physical_layout(Some(layout))
    .unwrap();
    let checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    let restored =
        StructValue::from_canonical_parts("EmptyNameOverlay".into(), logical.fields().clone())
            .with_physical_checkpoint(&checkpoint)
            .unwrap();
    assert_eq!(
        restored.encode_physical_checkpoint().unwrap(),
        Some(checkpoint)
    );
}

#[test]
fn physical_checkpoint_rejects_logical_backing_disagreement() {
    let layout = checkpoint_layout(StorageAbi::UniformTemporal64);
    let logical = StructValue::from_canonical_parts(
        "SignedOverlay".into(),
        OrderedMap::from_iter([
            ("raw".into(), Value::DWord(7)),
            ("signed".into(), Value::DInt(7)),
        ]),
    )
    .with_physical_layout(Some(layout))
    .unwrap();
    let mut checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    *checkpoint.last_mut().unwrap() ^= 1;
    let serialized =
        StructValue::from_canonical_parts("SignedOverlay".into(), logical.fields().clone());
    assert!(serialized.with_physical_checkpoint(&checkpoint).is_err());
}

#[test]
fn tiny_header_cannot_reserve_from_a_forged_large_record_count() {
    let logical = StructValue::from_canonical_parts(
        "SignedOverlay".into(),
        OrderedMap::from_iter([
            ("raw".into(), Value::DWord(7)),
            ("signed".into(), Value::DInt(7)),
        ]),
    );
    let mut checkpoint = vec![b'P', b'H', b'V', b'1', 1];
    checkpoint.extend_from_slice(&32_u32.to_le_bytes());
    checkpoint.extend_from_slice(&u32::MAX.to_le_bytes());
    checkpoint.extend_from_slice(&0_u32.to_le_bytes());
    assert!(logical.with_physical_checkpoint(&checkpoint).is_err());
}

#[test]
fn malformed_leaf_width_and_path_tags_are_rejected() {
    let layout = checkpoint_layout(StorageAbi::UniformTemporal64);
    let logical = StructValue::from_canonical_parts(
        "SignedOverlay".into(),
        OrderedMap::from_iter([
            ("raw".into(), Value::DWord(7)),
            ("signed".into(), Value::DInt(7)),
        ]),
    )
    .with_physical_layout(Some(layout))
    .unwrap();
    let checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    // The first leaf starts at byte 17. Its first path segment tag follows
    // the path count, and its width follows the field name and bit offset.
    assert_eq!(checkpoint[21], 1);
    let mut malformed_path = checkpoint.clone();
    malformed_path[21] = u8::MAX;
    assert!(logical
        .clone()
        .with_physical_checkpoint(&malformed_path)
        .is_err());
    let mut malformed_width = checkpoint;
    malformed_width[33] = 0;
    assert!(logical.with_physical_checkpoint(&malformed_width).is_err());
}

#[test]
fn nested_checkpoint_requires_the_child_layout_header_to_match() {
    fn layouts(inner_high_offset: u32) -> Vec<Option<Arc<PhysicalLayout>>> {
        let types = TypeTable {
            offsets: Vec::new(),
            entries: vec![
                primitive(4),
                primitive(2),
                TypeEntry {
                    kind: TypeKind::LocatedStruct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: vec![
                            Field {
                                name_idx: 1,
                                type_id: 1,
                                bit_offset: Some(0),
                            },
                            Field {
                                name_idx: 2,
                                type_id: 1,
                                bit_offset: Some(inner_high_offset),
                            },
                        ],
                    },
                },
                TypeEntry {
                    kind: TypeKind::OverlappingStruct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: vec![
                            Field {
                                name_idx: 0,
                                type_id: 0,
                                bit_offset: Some(0),
                            },
                            Field {
                                name_idx: 3,
                                type_id: 2,
                                bit_offset: Some(0),
                            },
                        ],
                    },
                },
            ],
        };
        let names = ["raw".into(), "low".into(), "high".into(), "inner".into()];
        prepare_layouts(
            &types,
            &names,
            StorageAbi::UniformTemporal64,
            &mut |_, _| Ok(()),
        )
        .unwrap()
    }

    let admitted = layouts(16);
    let alternate = layouts(8);
    let child = StructValue::from_canonical_parts(
        "Inner".into(),
        OrderedMap::from_iter([
            ("low".into(), Value::Byte(1)),
            ("high".into(), Value::Byte(0)),
        ]),
    )
    .with_physical_layout(admitted[2].clone())
    .unwrap();
    let parent = StructValue::from_canonical_parts(
        "Outer".into(),
        OrderedMap::from_iter([
            ("raw".into(), Value::DWord(1)),
            ("inner".into(), Value::Struct(Arc::new(child))),
        ]),
    )
    .with_physical_layout(admitted[3].clone())
    .unwrap();
    let parent_checkpoint = parent.encode_physical_checkpoint().unwrap().unwrap();

    let mismatched_child = StructValue::from_canonical_parts(
        "Inner".into(),
        OrderedMap::from_iter([
            ("low".into(), Value::Byte(1)),
            ("high".into(), Value::Byte(0)),
        ]),
    )
    .with_physical_layout(alternate[2].clone())
    .unwrap();
    let logical = StructValue::from_canonical_parts(
        "Outer".into(),
        OrderedMap::from_iter([
            ("raw".into(), Value::DWord(1)),
            ("inner".into(), Value::Struct(Arc::new(mismatched_child))),
        ]),
    );
    assert!(logical
        .with_physical_checkpoint(&parent_checkpoint)
        .is_err());
}

#[test]
fn aliases_can_share_one_storage_bit_without_an_invented_leaf_limit() {
    let names = (0..9)
        .map(|index| SmolStr::new(format!("flag{index}")))
        .collect::<Vec<_>>();
    let fields = names
        .iter()
        .enumerate()
        .map(|(index, _)| Field {
            name_idx: index as u32,
            type_id: 0,
            bit_offset: Some(0),
        })
        .collect();
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(1),
            TypeEntry {
                kind: TypeKind::OverlappingStruct,
                name_idx: None,
                data: TypeData::Struct { fields },
            },
        ],
    };
    let layout = prepare_layouts(
        &types,
        &names,
        StorageAbi::UniformTemporal64,
        &mut |_, _| Ok(()),
    )
    .unwrap()[1]
        .clone()
        .unwrap();
    assert!(layout.leaves.len() > layout.bits.div_ceil(8) as usize * 8);
    let fields = names
        .iter()
        .cloned()
        .map(|name| (name, Value::Bool(true)))
        .collect();
    let logical = StructValue::from_canonical_parts("Flags".into(), fields)
        .with_physical_layout(Some(layout))
        .unwrap();
    let checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    assert!(
        StructValue::from_canonical_parts("Flags".into(), logical.fields().clone())
            .with_physical_checkpoint(&checkpoint)
            .is_ok()
    );
}

#[test]
fn opaque_only_physical_layout_can_encode_and_restore_with_no_scalar_leaves() {
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 24,
                    max_length: 8,
                },
            },
            TypeEntry {
                kind: TypeKind::OverlappingStruct,
                name_idx: None,
                data: TypeData::Struct {
                    fields: vec![Field {
                        name_idx: 0,
                        type_id: 0,
                        bit_offset: Some(0),
                    }],
                },
            },
        ],
    };
    let layout = prepare_layouts(
        &types,
        &["label".into()],
        StorageAbi::UniformTemporal64,
        &mut |_, _| Ok(()),
    )
    .unwrap()[1]
        .clone()
        .unwrap();
    assert!(layout.leaves.is_empty());
    let logical = StructValue::from_canonical_parts(
        "Opaque".into(),
        OrderedMap::from_iter([("label".into(), Value::String("kept".into()))]),
    )
    .with_physical_layout(Some(layout))
    .unwrap();
    let checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    assert!(
        StructValue::from_canonical_parts("Opaque".into(), logical.fields().clone())
            .with_physical_checkpoint(&checkpoint)
            .is_ok()
    );
}

#[test]
fn zero_extent_physical_parent_and_empty_child_roundtrip() {
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            TypeEntry {
                kind: TypeKind::Struct,
                name_idx: None,
                data: TypeData::Struct { fields: Vec::new() },
            },
            TypeEntry {
                kind: TypeKind::OverlappingStruct,
                name_idx: None,
                data: TypeData::Struct {
                    fields: vec![Field {
                        name_idx: 0,
                        type_id: 0,
                        bit_offset: Some(0),
                    }],
                },
            },
        ],
    };
    let layouts = prepare_layouts(
        &types,
        &["empty".into()],
        StorageAbi::UniformTemporal64,
        &mut |_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(layouts[0].as_ref().unwrap().bits, 0);
    assert_eq!(layouts[1].as_ref().unwrap().bits, 0);
    let empty = StructValue::from_canonical_parts("Empty".into(), OrderedMap::default())
        .with_physical_layout(layouts[0].clone())
        .unwrap();
    let logical = StructValue::from_canonical_parts(
        "Parent".into(),
        OrderedMap::from_iter([("empty".into(), Value::Struct(Arc::new(empty)))]),
    )
    .with_physical_layout(layouts[1].clone())
    .unwrap();
    let checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    let restored = StructValue::from_canonical_parts("Parent".into(), logical.fields().clone())
        .with_physical_checkpoint(&checkpoint)
        .unwrap();
    assert_eq!(
        restored.encode_physical_checkpoint().unwrap(),
        Some(checkpoint)
    );
}

#[test]
fn checkpoint_leaf_path_count_beyond_the_structural_depth_refuses_the_checkpoint() {
    let layout = checkpoint_layout(StorageAbi::UniformTemporal64);
    let logical = StructValue::from_canonical_parts(
        "SignedOverlay".into(),
        OrderedMap::from_iter([
            ("raw".into(), Value::DWord(7)),
            ("signed".into(), Value::DInt(7)),
        ]),
    )
    .with_physical_layout(Some(layout))
    .unwrap();
    let mut checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    // The first leaf's path count is the u32 at byte 17; its single segment follows at byte 21.
    assert_eq!(&checkpoint[17..21], &1u32.to_le_bytes()[..]);
    checkpoint[17..21].copy_from_slice(&65u32.to_le_bytes());
    let restored =
        StructValue::from_canonical_parts("SignedOverlay".into(), logical.fields().clone());
    // A stored path longer than the structural bound makes the checkpoint invalid, and spec 34
    // RETAIN-02 admits only a complete, valid checkpoint. The count reader refuses 65 > 64 before
    // any segment is read (checkpoint.rs:101, 437-441). MEM-06's TypeMismatch governs value-copy
    // traversal, not decoding stored checkpoint bytes.
    assert_eq!(
        restored.with_physical_checkpoint(&checkpoint).err(),
        Some(RuntimeError::Overflow)
    );
}

#[test]
fn checkpoint_preserves_array_rank_above_type_path_depth() {
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            primitive(1),
            TypeEntry {
                kind: TypeKind::Array,
                name_idx: None,
                data: TypeData::Array {
                    elem_type_id: 0,
                    dims: vec![(0, 0); 65],
                },
            },
            TypeEntry {
                kind: TypeKind::OverlappingStruct,
                name_idx: None,
                data: TypeData::Struct {
                    fields: vec![Field {
                        name_idx: 0,
                        type_id: 1,
                        bit_offset: Some(0),
                    }],
                },
            },
        ],
    };
    let layout = prepare_layouts(
        &types,
        &["payload".into()],
        StorageAbi::UniformTemporal64,
        &mut |_, _| Ok(()),
    )
    .unwrap()[2]
        .clone()
        .unwrap();
    let payload = Value::Array(Box::new(crate::value::ArrayValue::from_canonical_parts(
        vec![Value::Bool(true)],
        vec![(0, 0); 65],
    )));
    let logical = StructValue::from_canonical_parts(
        "Ranked".into(),
        OrderedMap::from_iter([("payload".into(), payload)]),
    )
    .with_physical_layout(Some(layout))
    .unwrap();
    let checkpoint = logical.encode_physical_checkpoint().unwrap().unwrap();
    let restored = StructValue::from_canonical_parts("Ranked".into(), logical.fields().clone())
        .with_physical_checkpoint(&checkpoint)
        .unwrap();
    assert_eq!(
        restored.encode_physical_checkpoint().unwrap(),
        Some(checkpoint)
    );
}
