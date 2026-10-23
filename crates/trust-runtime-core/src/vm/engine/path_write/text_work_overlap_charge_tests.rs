//! TIME-08: a write through a STRUCT OVERLAP physical layout charges exactly the layout write
//! cost of the spec's STRUCT OVERLAP paragraph, and allocates its P bytes plus one leaf entry
//! per affected leaf. The expected values are the hand computations in
//! `value/overlap/text_work_overlap_cost_tests.rs`.
use super::*;
use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
use crate::collections::OrderedMap;
use crate::error::ExecutionLimitKind;
use crate::value::{StorageAbi, StructValue};
use alloc::{format, sync::Arc, vec, vec::Vec};
use smol_str::SmolStr;

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
}

/// A structure of DWORD fields (primitive id 4) at the given bit offsets. With `with_layout`, it
/// carries the physical layout of an overlapping structure; without it, it is an ordinary value.
/// Each leaf must hold the value variant of its primitive id, so the leaves are `Value::DWord`.
fn located(fields: Vec<(SmolStr, u32)>, with_layout: bool) -> Value {
    let names: Vec<SmolStr> = fields.iter().map(|(name, _)| name.clone()).collect();
    let types = TypeTable {
        entries: vec![
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 4,
                    max_length: 0,
                },
            },
            TypeEntry {
                kind: TypeKind::OverlappingStruct,
                name_idx: None,
                data: TypeData::Struct {
                    fields: fields
                        .iter()
                        .enumerate()
                        .map(|(index, (_, offset))| Field {
                            name_idx: index as u32,
                            type_id: 0,
                            bit_offset: Some(*offset),
                        })
                        .collect(),
                },
            },
        ],
        ..Default::default()
    };
    let layouts = crate::value::overlap::prepare_layouts(
        &types,
        &names,
        StorageAbi::UniformTemporal64,
        &mut |_, _| Ok(()),
    )
    .unwrap();
    let layout = if with_layout {
        layouts[1].clone()
    } else {
        None
    };
    let members = fields
        .iter()
        .map(|(name, _)| (name.clone(), Value::DWord(0)))
        .collect::<OrderedMap<_, _>>();
    let structure = StructValue::from_canonical_parts("Pair".into(), members)
        .initialize_physical_layout(layout)
        .unwrap();
    Value::Struct(Arc::new(structure))
}

fn two_leaves() -> Value {
    located(vec![("a".into(), 0), ("bb".into(), 32)], true)
}

#[test]
fn single_leaf_physical_write_charges_the_formula_value() {
    // 446 units and 4 + size_of::<(usize, Value)>() bytes, as worked out in the overlap tests.
    let prepared = fixture();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let root = two_leaves();
    let path = [RefSegment::Field("a".into())];
    let value = Value::DWord(1);

    state.data.resources.work_budget.reset(445);
    state.data.resources.construction_bytes.set(0);
    let error = state.charge_path_write(&root, &path, &value).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 446,
            limit: 445,
        }
    ));

    state.data.resources.work_budget.reset(446);
    state.charge_path_write(&root, &path, &value).unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        4 + core::mem::size_of::<(usize, Value)>()
    );
}

#[test]
fn whole_structure_physical_write_charges_the_formula_value() {
    // 979 units and 8 + 2 × size_of::<(usize, Value)>() bytes. The input has no layout, so it
    // is not a shared-layout assignment that would be free.
    let prepared = fixture();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let root = two_leaves();
    let input = located(vec![("a".into(), 0), ("bb".into(), 32)], false);

    state.data.resources.work_budget.reset(978);
    state.data.resources.construction_bytes.set(0);
    let error = state.charge_path_write(&root, &[], &input).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 979,
            limit: 978,
        }
    ));

    state.data.resources.work_budget.reset(979);
    state.charge_path_write(&root, &[], &input).unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        8 + 2 * core::mem::size_of::<(usize, Value)>()
    );
}

#[test]
fn hundred_field_physical_write_charges_the_formula_value() {
    // 693 units and 4 + size_of::<(usize, Value)>() bytes for "f050" among 100 DWORD leaves.
    let prepared = fixture();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let fields: Vec<(SmolStr, u32)> = (0..100u32)
        .map(|index| (SmolStr::new(format!("f{index:03}")), index * 32))
        .collect();
    let root = located(fields, true);
    let path = [RefSegment::Field("f050".into())];
    let value = Value::DWord(1);

    state.data.resources.work_budget.reset(692);
    state.data.resources.construction_bytes.set(0);
    let error = state.charge_path_write(&root, &path, &value).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 693,
            limit: 692,
        }
    ));

    state.data.resources.work_budget.reset(693);
    state.charge_path_write(&root, &path, &value).unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        4 + core::mem::size_of::<(usize, Value)>()
    );
}
