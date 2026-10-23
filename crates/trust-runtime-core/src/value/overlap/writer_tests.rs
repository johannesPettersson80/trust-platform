//! Native assertions for physical storage, atomic faults and destination-indexed work.
use super::*;
use crate::bytecode::{EnumVariant, Field, TypeEntry};
use crate::collections::OrderedMap;
use crate::value::{
    write_value_path_checked, ArrayValue, DateTimeValue, Duration, EnumValue, StorageAbi,
};
use alloc::{boxed::Box, vec};

fn primitive(id: u16) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id: id,
            max_length: 0,
        },
    }
}
fn structure(fields: &[(u32, u32, u32)]) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::OverlappingStruct,
        name_idx: None,
        data: TypeData::Struct {
            fields: fields
                .iter()
                .map(|&(name_idx, type_id, offset)| Field {
                    name_idx,
                    type_id,
                    bit_offset: Some(offset),
                })
                .collect(),
        },
    }
}
fn prepare(
    entries: Vec<TypeEntry>,
    names: &[&str],
    abi: StorageAbi,
) -> Vec<Option<Arc<PhysicalLayout>>> {
    let types = TypeTable {
        offsets: Vec::new(),
        entries,
    };
    let names: Vec<_> = names.iter().map(SmolStr::new).collect();
    prepare_layouts(&types, &names, abi, &mut |_, _| Ok(())).unwrap()
}
fn value(name: &str, fields: &[(&str, Value)], layout: Option<Arc<PhysicalLayout>>) -> Value {
    let fields = fields
        .iter()
        .map(|(name, value)| (SmolStr::new(name), value.clone()))
        .collect::<OrderedMap<_, _>>();
    Value::Struct(Arc::new(
        StructValue::from_canonical_parts(name.into(), fields)
            .initialize_physical_layout(layout)
            .unwrap(),
    ))
}
fn path(name: &str) -> Vec<RefSegment> {
    vec![RefSegment::Field(name.into())]
}
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Struct(value) = value else {
        panic!("structure")
    };
    value.field(name).unwrap()
}
fn bytes(value: &Value) -> &[u8] {
    let Value::Struct(value) = value else {
        panic!("structure")
    };
    &value.physical.as_ref().unwrap().bytes
}

#[test]
fn hundred_field_write_charges_only_its_patch_and_aliases() {
    use alloc::format;
    let names: Vec<_> = (0..100).map(|i| format!("f{i:03}")).collect();
    let labels: Vec<_> = names.iter().map(|name| name.as_str()).collect();
    let members: Vec<_> = (0..100).map(|i| (i, 0, i * 32)).collect();
    let layout = prepare(
        vec![primitive(4), structure(&members)],
        &labels,
        StorageAbi::UniformTemporal64,
    );
    let fields: Vec<_> = labels.iter().map(|name| (*name, Value::DWord(0))).collect();
    let mut target = value("Hundred", &fields, layout[1].clone());
    let Value::Struct(structure) = &target else {
        unreachable!()
    };
    let cost = structure
        .physical_write_cost(&path("f050"))
        .unwrap()
        .unwrap();
    assert!(cost.work < 4096, "single-field cost {}", cost.work);
    assert_eq!(cost.allocation, 4 + core::mem::size_of::<(usize, Value)>());
    assert!(
        write_value_path_checked(&mut target, &path("f050"), Value::DWord(0x12345678)).unwrap()
    );
    for (i, name) in labels.iter().enumerate() {
        assert_eq!(
            field(&target, name),
            &Value::DWord(if i == 50 { 0x12345678 } else { 0 })
        );
    }
}

#[test]
fn bool_patch_preserves_neighbor_bits_and_refreshes_byte_alias() {
    let layouts = prepare(
        vec![
            primitive(1),
            primitive(2),
            structure(&[(0, 1, 0), (1, 0, 1), (2, 0, 2)]),
        ],
        &["byte", "neighbor", "selected"],
        StorageAbi::UniformTemporal64,
    );
    let mut target = value(
        "Bits",
        &[
            ("byte", Value::Byte(0)),
            ("neighbor", Value::Bool(true)),
            ("selected", Value::Bool(false)),
        ],
        layouts[2].clone(),
    );
    assert!(write_value_path_checked(&mut target, &path("selected"), Value::Bool(true)).unwrap());
    assert_eq!(field(&target, "neighbor"), &Value::Bool(true));
    assert_eq!(field(&target, "byte"), &Value::Byte(6));
    assert_eq!(bytes(&target), &[6]);
}

fn nested_layouts(array: bool) -> Vec<Option<Arc<PhysicalLayout>>> {
    let mut entries = vec![
        primitive(2),
        primitive(5),
        structure(&[(0, 0, 0), (1, 0, 16)]),
    ];
    if array {
        entries.push(TypeEntry {
            kind: TypeKind::Array,
            name_idx: None,
            data: TypeData::Array {
                elem_type_id: 2,
                dims: vec![(4, 5)],
            },
        });
        entries.push(structure(&[(2, 1, 0), (3, 3, 0)]));
    } else {
        entries.push(structure(&[(2, 1, 0), (3, 2, 0)]));
    }
    prepare(
        entries,
        &["low", "high", "whole", "inner"],
        StorageAbi::UniformTemporal64,
    )
}
fn inner(layout: Option<Arc<PhysicalLayout>>) -> Value {
    value(
        "Inner",
        &[("low", Value::Byte(0)), ("high", Value::Byte(0))],
        layout,
    )
}

#[test]
fn nested_copy_preserves_hidden_gap_bits_in_parent_and_child() {
    let layouts = nested_layouts(false);
    let mut source = value(
        "Outer",
        &[
            ("whole", Value::LWord(0)),
            ("inner", inner(layouts[2].clone())),
        ],
        layouts[3].clone(),
    );
    assert!(write_value_path_checked(
        &mut source,
        &path("whole"),
        Value::LWord(0x88776655_DDCCBBAA)
    )
    .unwrap());
    let copied = field(&source, "inner").clone();
    assert_eq!(bytes(&copied), &[0xAA, 0xBB, 0xCC]);
    let mut target = value(
        "Outer",
        &[
            ("whole", Value::LWord(0)),
            ("inner", inner(layouts[2].clone())),
        ],
        layouts[3].clone(),
    );
    assert!(write_value_path_checked(
        &mut target,
        &path("whole"),
        Value::LWord(0x11223344_99000000)
    )
    .unwrap());
    assert!(write_value_path_checked(&mut target, &path("inner"), copied).unwrap());
    assert_eq!(field(&target, "whole"), &Value::LWord(0x11223344_99CCBBAA));
    assert_eq!(bytes(field(&target, "inner")), &[0xAA, 0xBB, 0xCC]);
    let leaf = vec![
        RefSegment::Field("inner".into()),
        RefSegment::Field("low".into()),
    ];
    assert!(write_value_path_checked(&mut target, &leaf, Value::Byte(0x11)).unwrap());
    assert_eq!(field(&target, "whole"), &Value::LWord(0x11223344_99CCBB11));
}

#[test]
fn whole_array_copy_preserves_each_child_gap_and_alias_stride() {
    let layouts = nested_layouts(true);
    let array = || {
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![inner(layouts[2].clone()), inner(layouts[2].clone())],
            vec![(4, 5)],
        )))
    };
    let mut source = value(
        "Outer",
        &[("whole", Value::LWord(0)), ("inner", array())],
        layouts[4].clone(),
    );
    assert!(write_value_path_checked(
        &mut source,
        &path("whole"),
        Value::LWord(0x9988_665544_332211)
    )
    .unwrap());
    let copied = field(&source, "inner").clone();
    let mut target = value(
        "Outer",
        &[("whole", Value::LWord(0)), ("inner", array())],
        layouts[4].clone(),
    );
    assert!(write_value_path_checked(
        &mut target,
        &path("whole"),
        Value::LWord(0xBBAA_000000_000000)
    )
    .unwrap());
    assert!(write_value_path_checked(&mut target, &path("inner"), copied).unwrap());
    assert_eq!(field(&target, "whole"), &Value::LWord(0xBBAA_665544_332211));
    let leaf = vec![
        RefSegment::Field("inner".into()),
        RefSegment::Index(vec![5]),
        RefSegment::Field("high".into()),
    ];
    assert!(write_value_path_checked(&mut target, &leaf, Value::Byte(0x77)).unwrap());
    assert_eq!(field(&target, "whole"), &Value::LWord(0xBBAA_775544_332211));
}

#[test]
fn aggregate_copy_preserves_disjoint_opaque_logical_fields() {
    let mut text = primitive(24);
    let TypeData::Primitive { max_length, .. } = &mut text.data else {
        unreachable!()
    };
    *max_length = 8;
    let layouts = prepare(
        vec![primitive(2), text, structure(&[(0, 0, 0), (1, 1, 8)])],
        &["byte", "text"],
        StorageAbi::UniformTemporal64,
    );
    let mut target = value(
        "Opaque",
        &[
            ("byte", Value::Byte(0)),
            ("text", Value::String("old".into())),
        ],
        layouts[2].clone(),
    );
    let incoming = value(
        "Opaque",
        &[
            ("byte", Value::Byte(7)),
            ("text", Value::String("new".into())),
        ],
        layouts[2].clone(),
    );
    assert!(write_value_path_checked(&mut target, &[], incoming).unwrap());
    assert_eq!(field(&target, "text"), &Value::String("new".into()));
    assert!(write_value_path_checked(&mut target, &path("byte"), Value::Byte(9)).unwrap());
    assert_eq!(field(&target, "text"), &Value::String("new".into()));
    assert!(
        write_value_path_checked(&mut target, &path("text"), Value::String("next".into())).unwrap()
    );
    assert_eq!(field(&target, "byte"), &Value::Byte(9));
}

#[test]
fn uniform_temporal_storage_roundtrips_negative_and_large_time_and_dt() {
    for (id, temporal) in [
        (16, Value::Time(Duration::from_nanos(-5_000_000_001))),
        (22, Value::Dt(DateTimeValue::new(1_700_000_000_123))),
    ] {
        let layouts = prepare(
            vec![
                primitive(id),
                primitive(5),
                structure(&[(0, 0, 0), (1, 1, 0)]),
            ],
            &["time", "bits"],
            StorageAbi::UniformTemporal64,
        );
        let zero = match temporal {
            Value::Time(_) => Value::Time(Duration::ZERO),
            _ => Value::Dt(DateTimeValue::new(0)),
        };
        let mut target = value(
            "Temporal",
            &[("time", zero), ("bits", Value::LWord(0))],
            layouts[2].clone(),
        );
        assert!(write_value_path_checked(&mut target, &path("time"), temporal.clone()).unwrap());
        assert_eq!(field(&target, "time"), &temporal);
        let raw = scalar::bits(&temporal).unwrap();
        assert_eq!(field(&target, "bits"), &Value::LWord(raw));
        assert_eq!(bytes(&target), &raw.to_le_bytes());
    }
}

#[test]
fn legacy_temporal_narrowing_refuses_before_aliases_or_backing_change() {
    for (id, bad) in [
        (16, Value::Time(Duration::from_nanos(5_000_000_000))),
        (22, Value::Dt(DateTimeValue::new(-3_000_000_000))),
    ] {
        let layouts = prepare(
            vec![
                primitive(id),
                primitive(4),
                structure(&[(0, 0, 0), (1, 1, 0)]),
            ],
            &["time", "bits"],
            StorageAbi::LegacyTemporal32,
        );
        let zero = match bad {
            Value::Time(_) => Value::Time(Duration::ZERO),
            _ => Value::Dt(DateTimeValue::new(0)),
        };
        let mut target = value(
            "Legacy",
            &[("time", zero), ("bits", Value::DWord(0))],
            layouts[2].clone(),
        );
        let before = target.clone();
        let before_bytes = bytes(&target).to_vec();
        let error = write_value_path_checked(&mut target, &path("time"), bad).unwrap_err();
        assert!(matches!(error, RuntimeError::Overflow));
        assert_eq!(target, before);
        assert_eq!(bytes(&target), before_bytes);
        assert_eq!(
            error.stable_code(),
            crate::error::StableErrorCode::RuntimeOverflow
        );
        let negative = if id == 16 {
            Value::Time(Duration::from_nanos(-1))
        } else {
            Value::Dt(DateTimeValue::new(-1))
        };
        assert!(write_value_path_checked(&mut target, &path("time"), negative.clone()).unwrap());
        assert_eq!(field(&target, "time"), &negative);
        assert_eq!(field(&target, "bits"), &Value::DWord(u32::MAX));
    }
}

#[test]
fn alias_enum_and_subrange_faults_are_atomic_and_keep_their_identity() {
    let enumeration = TypeEntry {
        kind: TypeKind::Enum,
        name_idx: None,
        data: TypeData::Enum {
            base_type_id: 0,
            variants: vec![
                EnumVariant {
                    name_idx: 2,
                    value: 0,
                },
                EnumVariant {
                    name_idx: 3,
                    value: 1,
                },
            ],
        },
    };
    let layouts = prepare(
        vec![
            primitive(10),
            enumeration,
            structure(&[(0, 0, 0), (1, 1, 0)]),
        ],
        &["raw", "choice", "zero", "one"],
        StorageAbi::UniformTemporal64,
    );
    let mut target = value(
        "EnumOverlay",
        &[
            ("raw", Value::USInt(0)),
            (
                "choice",
                Value::Enum(Box::new(EnumValue::from_canonical_parts(
                    "Choice".into(),
                    "zero".into(),
                    0,
                ))),
            ),
        ],
        layouts[2].clone(),
    );
    let before = target.clone();
    let error = write_value_path_checked(&mut target, &path("raw"), Value::USInt(2)).unwrap_err();
    assert!(matches!(error, RuntimeError::TypeMismatch));
    assert_eq!(
        error.stable_code(),
        crate::error::StableErrorCode::RuntimeTypeMismatch
    );
    assert_eq!(target, before);
    assert_eq!(bytes(&target), &[0]);

    let range = TypeEntry {
        kind: TypeKind::Subrange,
        name_idx: None,
        data: TypeData::Subrange {
            base_type_id: 0,
            lower: 0,
            upper: 10,
        },
    };
    let layouts = prepare(
        vec![primitive(7), range, structure(&[(0, 0, 0), (1, 1, 0)])],
        &["raw", "bounded"],
        StorageAbi::UniformTemporal64,
    );
    let mut target = value(
        "RangeOverlay",
        &[("raw", Value::Int(0)), ("bounded", Value::Int(0))],
        layouts[2].clone(),
    );
    let before = target.clone();
    let error = write_value_path_checked(&mut target, &path("raw"), Value::Int(11)).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::SubrangeViolation {
            value: 11,
            lower: 0,
            upper: 10
        }
    ));
    assert_eq!(
        error.stable_code(),
        crate::error::StableErrorCode::RuntimeSubrangeViolation
    );
    assert_eq!(target, before);
    assert_eq!(bytes(&target), &[0, 0]);
}

#[test]
fn checked_memory_writer_keeps_physical_fault_instead_of_null_reference() {
    let range = TypeEntry {
        kind: TypeKind::Subrange,
        name_idx: None,
        data: TypeData::Subrange {
            base_type_id: 0,
            lower: 0,
            upper: 10,
        },
    };
    let layouts = prepare(
        vec![primitive(7), range, structure(&[(0, 0, 0), (1, 1, 0)])],
        &["raw", "bounded"],
        StorageAbi::UniformTemporal64,
    );
    let original = value(
        "RangeOverlay",
        &[("raw", Value::Int(0)), ("bounded", Value::Int(0))],
        layouts[2].clone(),
    );
    let mut storage = crate::memory::VariableStorage::default();
    storage.set_global("probe", original.clone());
    let mut reference = storage.ref_for_global("probe").unwrap();
    reference.path = path("raw");
    let error = storage
        .write_by_ref_ref_checked(&reference, Value::Int(11))
        .unwrap_err();
    assert!(matches!(error, RuntimeError::SubrangeViolation { .. }));
    assert_eq!(storage.get_global("probe"), Some(&original));
}

#[test]
fn mixed_case_selected_path_uses_the_same_prepared_alias_owner() {
    let layouts = nested_layouts(false);
    let mut target = value(
        "Outer",
        &[
            ("whole", Value::LWord(0)),
            ("inner", inner(layouts[2].clone())),
        ],
        layouts[3].clone(),
    );
    let mixed = vec![
        RefSegment::Field("InNeR".into()),
        RefSegment::Field("LoW".into()),
    ];
    let Value::Struct(structure) = &target else {
        unreachable!()
    };
    assert!(structure.physical_write_cost(&mixed).unwrap().is_some());
    assert!(write_value_path_checked(&mut target, &mixed, Value::Byte(0xAB)).unwrap());
    assert_eq!(field(&target, "whole"), &Value::LWord(0xAB));
    assert_eq!(bytes(field(&target, "inner")), &[0xAB, 0, 0]);
}

#[test]
fn retained_logical_fields_cannot_silently_change_when_temporal_abi_changes() {
    let records = || {
        vec![
            primitive(16),
            primitive(5),
            structure(&[(0, 0, 0), (1, 1, 0)]),
        ]
    };
    let legacy = prepare(records(), &["time", "bits"], StorageAbi::LegacyTemporal32);
    let current = prepare(records(), &["time", "bits"], StorageAbi::UniformTemporal64);
    let logical = StructValue::from_canonical_parts(
        "Retained".into(),
        OrderedMap::from_iter([
            ("time".into(), Value::Time(Duration::from_nanos(-1))),
            ("bits".into(), Value::LWord(u64::from(u32::MAX))),
        ]),
    );
    let installed = logical
        .clone()
        .with_physical_layout(legacy[2].clone())
        .unwrap();
    let before = installed.clone();
    let candidate = logical.with_physical_layout(current[2].clone());
    assert!(matches!(candidate, Err(RuntimeError::TypeMismatch)));
    assert_eq!(installed, before);
    assert_eq!(
        installed.field("time"),
        Some(&Value::Time(Duration::from_nanos(-1)))
    );
    assert_eq!(
        installed.physical.as_ref().unwrap().bytes,
        u64::from(u32::MAX).to_le_bytes()
    );
    let mut installed = installed;
    assert!(matches!(
        installed.rebind_physical_layout(current[2].clone()),
        Err(RuntimeError::TypeMismatch)
    ));
    assert_eq!(installed, before);
}

#[test]
fn physical_compatibility_compares_encodings_not_only_width_or_abi_label() {
    let records = || {
        vec![
            primitive(4),
            primitive(2),
            structure(&[(0, 0, 0), (1, 1, 16)]),
        ]
    };
    let legacy = prepare(records(), &["whole", "part"], StorageAbi::LegacyTemporal32);
    let current = prepare(records(), &["whole", "part"], StorageAbi::UniformTemporal64);
    assert!(legacy[2]
        .as_ref()
        .unwrap()
        .storage_compatible(current[2].as_ref().unwrap()));
    let mut target = value(
        "Plain",
        &[("whole", Value::DWord(0)), ("part", Value::Byte(0))],
        legacy[2].clone(),
    );
    assert!(
        write_value_path_checked(&mut target, &path("whole"), Value::DWord(0xDDCCBBAA)).unwrap()
    );
    let Value::Struct(target) = &mut target else {
        unreachable!()
    };
    Arc::make_mut(target)
        .rebind_physical_layout(current[2].clone())
        .unwrap();
    assert_eq!(
        target.physical.as_ref().unwrap().bytes,
        [0xAA, 0xBB, 0xCC, 0xDD]
    );
    let time = prepare(
        vec![primitive(16), structure(&[(0, 0, 0)])],
        &["whole"],
        StorageAbi::LegacyTemporal32,
    );
    let integer = prepare(
        vec![primitive(8), structure(&[(0, 0, 0)])],
        &["whole"],
        StorageAbi::LegacyTemporal32,
    );
    assert!(!time[1]
        .as_ref()
        .unwrap()
        .storage_compatible(integer[1].as_ref().unwrap()));
}

#[test]
fn strict_attachment_and_internal_copy_preserve_nan_payloads_and_signed_zero() {
    let layouts = prepare(
        vec![
            primitive(14),
            primitive(4),
            structure(&[(0, 0, 0), (1, 1, 0)]),
        ],
        &["real", "bits"],
        StorageAbi::UniformTemporal64,
    );
    for raw in [0x7FC0_1234_u32, 0x8000_0000] {
        let logical = StructValue::from_canonical_parts(
            "IEEE".into(),
            OrderedMap::from_iter([
                ("real".into(), Value::Real(f32::from_bits(raw))),
                ("bits".into(), Value::DWord(raw)),
            ]),
        );
        let source = Value::Struct(Arc::new(
            logical.with_physical_layout(layouts[2].clone()).unwrap(),
        ));
        let mut target = value(
            "IEEE",
            &[("real", Value::Real(0.0)), ("bits", Value::DWord(0))],
            layouts[2].clone(),
        );
        assert!(write_value_path_checked(&mut target, &[], source).unwrap());
        let Value::Real(real) = field(&target, "real") else {
            unreachable!()
        };
        assert_eq!(real.to_bits(), raw);
        assert_eq!(bytes(&target), &raw.to_le_bytes());
        if real.is_nan() {
            assert_eq!(
                crate::stdlib::StandardLibrary::new()
                    .call("IS_VALID", &[Value::Real(*real)])
                    .unwrap(),
                Value::Bool(false)
            );
        }
    }
    let inconsistent = StructValue::from_canonical_parts(
        "IEEE".into(),
        OrderedMap::from_iter([
            ("real".into(), Value::Real(-0.0)),
            ("bits".into(), Value::DWord(0)),
        ]),
    );
    assert!(matches!(
        inconsistent.with_physical_layout(layouts[2].clone()),
        Err(RuntimeError::TypeMismatch)
    ));
}

#[test]
fn staged_alias_policy_can_reject_nonfinite_without_changing_internal_ieee_rules() {
    let layouts = prepare(
        vec![
            primitive(14),
            primitive(4),
            structure(&[(0, 0, 0), (1, 1, 0)]),
        ],
        &["real", "bits"],
        StorageAbi::UniformTemporal64,
    );
    let original = value(
        "IEEE",
        &[("real", Value::Real(0.0)), ("bits", Value::DWord(0))],
        layouts[2].clone(),
    );
    let mut storage = crate::memory::VariableStorage::default();
    storage.set_global("probe", original.clone());
    let mut reference = storage.ref_for_global("probe").unwrap();
    reference.path = path("BiTs");
    let input = Value::DWord(0x7FC0_0000);
    let plan = storage
        .prepare_by_ref_parts_write(
            reference.location,
            reference.offset,
            &reference.path,
            &input,
        )
        .unwrap()
        .unwrap();
    let error = plan
        .validate_aliases(&mut |value| match value {
            Value::Real(value) if !value.is_finite() => Err(RuntimeError::NonFiniteValue),
            _ => Ok(()),
        })
        .unwrap_err();
    assert!(matches!(error, RuntimeError::NonFiniteValue));
    assert_eq!(storage.get_global("probe"), Some(&original));
    assert!(storage.write_by_ref_ref_checked(&reference, input).unwrap());
    reference.path = path("ReAl");
    let Some(Value::Real(real)) = storage.read_by_ref_ref(&reference) else {
        unreachable!()
    };
    assert_eq!(real.to_bits(), 0x7FC0_0000);
}

#[test]
fn inconsistent_raw_aggregate_assignment_is_refused_before_any_alias_publication() {
    let layouts = prepare(
        vec![primitive(4), structure(&[(0, 0, 0), (1, 0, 0)])],
        &["first", "second"],
        StorageAbi::UniformTemporal64,
    );
    let mut target = value(
        "Pair",
        &[("first", Value::DWord(7)), ("second", Value::DWord(7))],
        layouts[1].clone(),
    );
    let fields = [
        ("first".into(), Value::DWord(9)),
        ("second".into(), Value::DWord(10)),
    ]
    .into_iter()
    .collect();
    let input = Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Pair".into(),
        fields,
    )));
    assert_eq!(
        write_value_path_checked(&mut target, &[], input),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(field(&target, "first"), &Value::DWord(7));
    assert_eq!(field(&target, "second"), &Value::DWord(7));
    assert_eq!(bytes(&target), &7_u32.to_le_bytes());
}

#[test]
fn aggregate_work_bound_includes_sparse_gap_clearing_and_backing_copy() {
    let layouts = prepare(
        vec![primitive(2), structure(&[(0, 0, 0), (1, 0, 65528)])],
        &["first", "last"],
        StorageAbi::UniformTemporal64,
    );
    let target = value(
        "Sparse",
        &[("first", Value::Byte(0)), ("last", Value::Byte(0))],
        layouts[1].clone(),
    );
    let Value::Struct(target) = target else {
        panic!("structure")
    };
    let cost = target.physical_write_cost(&[]).unwrap().unwrap();
    assert!(
        cost.work >= 2 * 65536,
        "whole backing clear and copy cannot be charged only by two leaves"
    );
    assert!(
        cost.allocation >= 8192,
        "patch includes all sparse gap bytes"
    );
}

#[test]
fn nested_array_scalar_cost_does_not_scan_earlier_sibling_children() {
    fn probe(count: i64) -> Value {
        let layouts = prepare(
            vec![
                primitive(2),
                structure(&[(0, 0, 0), (1, 0, 16)]),
                TypeEntry {
                    kind: TypeKind::Array,
                    name_idx: None,
                    data: TypeData::Array {
                        elem_type_id: 1,
                        dims: vec![(0, count - 1)],
                    },
                },
                structure(&[(2, 2, 0)]),
                structure(&[(3, 3, 0)]),
            ],
            &["low", "high", "items", "nested"],
            StorageAbi::UniformTemporal64,
        );
        let elements = (0..count)
            .map(|_| {
                value(
                    "Element",
                    &[("low", Value::Byte(0)), ("high", Value::Byte(0))],
                    layouts[1].clone(),
                )
            })
            .collect();
        let array = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            elements,
            vec![(0, count - 1)],
        )));
        let nested = value("Nested", &[("items", array)], layouts[3].clone());
        value("Outer", &[("nested", nested)], layouts[4].clone())
    }
    fn selected(index: i64) -> Vec<RefSegment> {
        vec![
            RefSegment::Field("nested".into()),
            RefSegment::Field("items".into()),
            RefSegment::Index(vec![index]),
            RefSegment::Field("low".into()),
        ]
    }
    let small = probe(4);
    let mut large = probe(512);
    let Value::Struct(small_root) = &small else {
        panic!("structure")
    };
    let Value::Struct(large_root) = &large else {
        panic!("structure")
    };
    let first = large_root
        .physical_write_cost(&selected(0))
        .unwrap()
        .unwrap();
    let last = large_root
        .physical_write_cost(&selected(511))
        .unwrap()
        .unwrap();
    let smaller = small_root
        .physical_write_cost(&selected(3))
        .unwrap()
        .unwrap();
    assert_eq!(
        first.work, last.work,
        "position among siblings cannot add linear work"
    );
    assert_eq!(first.allocation, last.allocation);
    assert_eq!(
        large_root
            .physical_affected_children(&selected(511))
            .unwrap()
            .count(),
        2,
        "only the enclosing structure and selected array element intersect"
    );
    assert!(
        last.work < smaller.work + 4096,
        "sibling growth adds logarithmic index work"
    );
    assert!(write_value_path_checked(&mut large, &selected(511), Value::Byte(9)).unwrap());
    assert_eq!(
        read_value_path_borrowed(&large, &selected(0)),
        Some(&Value::Byte(0))
    );
    assert_eq!(
        read_value_path_borrowed(&large, &selected(511)),
        Some(&Value::Byte(9))
    );
}

#[test]
fn enum_subrange_base_accepts_valid_members_and_refuses_raw_aliases_atomically() {
    let range = TypeEntry {
        kind: TypeKind::Subrange,
        name_idx: None,
        data: TypeData::Subrange {
            base_type_id: 0,
            lower: 1,
            upper: 3,
        },
    };
    let enumeration = TypeEntry {
        kind: TypeKind::Enum,
        name_idx: None,
        data: TypeData::Enum {
            base_type_id: 1,
            variants: vec![
                EnumVariant {
                    name_idx: 2,
                    value: 1,
                },
                EnumVariant {
                    name_idx: 3,
                    value: 3,
                },
            ],
        },
    };
    let layouts = prepare(
        vec![
            primitive(10),
            range,
            enumeration,
            structure(&[(0, 0, 0), (1, 2, 0)]),
        ],
        &["raw", "choice", "one", "three"],
        StorageAbi::UniformTemporal64,
    );
    let mut target = value(
        "BoundedEnum",
        &[
            ("raw", Value::USInt(1)),
            (
                "choice",
                Value::Enum(Box::new(EnumValue::from_canonical_parts(
                    "Choice".into(),
                    "one".into(),
                    1,
                ))),
            ),
        ],
        layouts[3].clone(),
    );
    assert!(write_value_path_checked(&mut target, &path("raw"), Value::USInt(3)).unwrap());
    assert_eq!(
        field(&target, "choice"),
        &Value::Enum(Box::new(EnumValue::from_canonical_parts(
            "Choice".into(),
            "three".into(),
            3
        )))
    );
    let before = target.clone();
    assert_eq!(
        write_value_path_checked(&mut target, &path("raw"), Value::USInt(2)),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(target, before);
    assert_eq!(bytes(&target), &[3]);
    let error = write_value_path_checked(&mut target, &path("raw"), Value::USInt(4)).unwrap_err();
    assert_eq!(
        error,
        RuntimeError::SubrangeViolation {
            value: 4,
            lower: 1,
            upper: 3
        }
    );
    assert_eq!(
        error.stable_code(),
        crate::error::StableErrorCode::RuntimeSubrangeViolation
    );
    assert_eq!(target, before);
    assert_eq!(bytes(&target), &[3]);
}
