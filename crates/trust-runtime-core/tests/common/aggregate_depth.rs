//! Aggregate nesting is an admitted data bound, not the invocation-stack limit.
use super::*;
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, construction_demands, RejectionReason, TypeData, TypeEntry,
    TypeKind, BYTECODE_MAX_TYPE_NESTING,
};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{ArrayValue, RefSegment, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn aggregate_module(depth: usize, aliases: bool) -> BytecodeModule {
    let mut module = default_constant_bounds::owned_default();
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        panic!("types")
    };
    let mut ty = types.entries.len() as u32;
    types.entries.push(TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id: 7,
            max_length: 0,
        },
    });
    for _ in 0..depth {
        let child = ty;
        ty = types.entries.len() as u32;
        types.entries.push(TypeEntry {
            kind: if aliases {
                TypeKind::Alias
            } else {
                TypeKind::Array
            },
            name_idx: None,
            data: if aliases {
                TypeData::Alias {
                    target_type_id: child,
                }
            } else {
                TypeData::Array {
                    elem_type_id: child,
                    dims: vec![(0, 0)],
                }
            },
        });
    }
    types.offsets = compute_type_offsets_for_entries(&types.entries).unwrap();
    let SectionData::ConstPool(constants) = module.section_mut(SectionId::ConstPool).unwrap()
    else {
        panic!("constants")
    };
    constants.entries.clear();
    let SectionData::StorageLayout(layout) = module.section_mut(SectionId::StorageLayout).unwrap()
    else {
        panic!("layout")
    };
    layout.entries[0].type_id = Some(ty);
    layout.entries[0].default_const_idx = None;
    layout.entries[0].construction_nodes = if aliases { 1 } else { depth as u32 + 1 };
    layout.entries[0].retain = 1;
    let SectionData::VarMeta(meta) = module.section_mut(SectionId::VarMeta).unwrap() else {
        panic!("variable metadata")
    };
    for entry in &mut meta.entries {
        entry.type_id = ty;
        entry.init_const_idx = None;
    }
    meta.entries[1].retain = 1;
    module
}

fn nested_value(depth: usize, leaf: i16) -> Value {
    let mut value = Value::Int(leaf);
    for _ in 0..depth {
        value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![value],
            vec![(0, 0)],
        )));
    }
    value
}

#[test]
fn admitted_aggregate_uses_64_edges_independently_of_four_call_frames() {
    for depth in [8, BYTECODE_MAX_TYPE_NESTING] {
        let module = aggregate_module(depth, false);
        module.validate().expect("the full aggregate is admitted");
        let prepared = PreparedModule::from_bytes(
            &module.encode().unwrap(),
            PreparationLimits {
                max_call_depth: 4,
                ..PreparationLimits::default()
            },
        )
        .unwrap();
        let mut state = prepared
            .instantiate(0)
            .expect("one initializer constructs a deep aggregate");
        assert_eq!(
            state.storage().get_global("payload"),
            Some(&nested_value(depth, 0))
        );
        state
            .write_global("payload", nested_value(depth, 23))
            .unwrap();
        let root = state.storage().ref_for_global("payload").unwrap();
        assert_eq!(
            state.read_reference(&root).unwrap(),
            nested_value(depth, 23)
        );
        let path = vec![RefSegment::Index(vec![0]); depth];
        state
            .write_named_path("payload", &path, Value::Int(31))
            .unwrap();
        assert_eq!(
            state.storage().get_global("payload"),
            Some(&nested_value(depth, 31))
        );
        let snapshot = state.retain_snapshot().unwrap();
        state
            .write_global("payload", nested_value(depth, 99))
            .unwrap();
        state.apply_retain_snapshot(&snapshot).unwrap();
        state.restart(RestartMode::Warm).unwrap();
        assert_eq!(
            state.storage().get_global("payload"),
            Some(&nested_value(depth, 31))
        );
        assert_eq!(
            state.write_global("payload", nested_value(BYTECODE_MAX_TYPE_NESTING + 1, 7)),
            Err(trust_runtime_core::error::RuntimeError::TypeMismatch)
        );
        assert_eq!(
            state.storage().get_global("payload"),
            Some(&nested_value(depth, 31))
        );
        assert_eq!(state.fault(), None);
    }
}

#[test]
fn alias_normalization_matches_the_admitted_64_edge_type_boundary() {
    let module = aggregate_module(BYTECODE_MAX_TYPE_NESTING, true);
    module
        .validate()
        .expect("source-free admission accepts 64 alias edges from a depth-zero root");
    let prepared = PreparedModule::from_bytes(
        &module.encode().unwrap(),
        PreparationLimits {
            max_call_depth: 4,
            ..PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.write_global("payload", Value::SInt(17)).unwrap();
    assert_eq!(state.storage().get_global("payload"), Some(&Value::Int(17)));
}

#[test]
fn the_sixty_fifth_structural_edge_is_rejected_before_instantiation() {
    for aliases in [false, true] {
        let module = aggregate_module(BYTECODE_MAX_TYPE_NESTING + 1, aliases);
        assert_eq!(
            module.validate(),
            Err(BytecodeError::from(
                RejectionReason::ConstructionDemandOverflow
            ))
        );
        assert!(PreparedModule::from_bytes(
            &module.encode().unwrap(),
            PreparationLimits::default()
        )
        .is_err());
    }
}

#[test]
fn previously_cached_suffixes_cannot_hide_a_deeper_declaration_path() {
    for depth in [BYTECODE_MAX_TYPE_NESTING, BYTECODE_MAX_TYPE_NESTING + 1] {
        let mut module = aggregate_module(depth, true);
        let SectionData::StorageLayout(layout) =
            module.section_mut(SectionId::StorageLayout).unwrap()
        else {
            panic!("layout")
        };
        let declaration = layout.entries[0].clone();
        let last = declaration.type_id.unwrap();
        let first = last - depth as u32;
        // Exercise the actual producer/admission demand computation. Each root
        // primes the memo for the following root's complete suffix.
        layout.entries = (first..=last)
            .map(|ty| {
                let mut declaration = declaration.clone();
                declaration.type_id = Some(ty);
                declaration
            })
            .collect();
        let result = construction_demands(module.view(), ValidationLimits::default());
        if depth == BYTECODE_MAX_TYPE_NESTING {
            assert_eq!(result.unwrap(), vec![1; depth + 1]);
        } else {
            assert_eq!(
                result,
                Err(BytecodeError::from(
                    RejectionReason::ConstructionDemandOverflow
                ))
            );
        }
    }
}
