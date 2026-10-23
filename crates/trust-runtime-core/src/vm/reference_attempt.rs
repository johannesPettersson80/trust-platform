use crate::bytecode::{TypeData, TypeKind};
use crate::value::Value;
use alloc::string::String;

use super::dispatch_refs::dynamic_ref_type;
use super::errors::VmTrap;
use super::frames::VmFrame;
use super::module::VmModule;

pub fn apply_reference_attempt(
    runtime: &impl super::context::ReferenceContext,
    module: &VmModule,
    frame: &VmFrame,
    value: Value,
    target_type_idx: u32,
) -> Result<Value, VmTrap> {
    let target_type_idx = resolved_type_idx(module, target_type_idx)
        .map_err(VmTrap::Runtime)?
        .ok_or_else(|| {
            VmTrap::Runtime(super::module::invalid_bytecode(
                "invalid attempt target type",
            ))
        })?;
    let target = module
        .types
        .entries
        .get(target_type_idx as usize)
        .ok_or_else(|| {
            VmTrap::Runtime(super::module::invalid_bytecode(
                "invalid attempt target type",
            ))
        })?;

    match (&target.kind, &target.data) {
        (TypeKind::Reference, TypeData::Reference { target_type_id }) => {
            apply_typed_reference_attempt(runtime, module, frame, value, *target_type_id)
        }
        (TypeKind::Interface, TypeData::Interface { .. }) => {
            apply_interface_attempt(runtime, module, value, target_type_idx)
        }
        _ => Err(VmTrap::Runtime(super::module::invalid_bytecode(
            "REFERENCE_ATTEMPT target is not reference-like",
        ))),
    }
}

fn apply_typed_reference_attempt(
    runtime: &impl super::context::ReferenceContext,
    module: &VmModule,
    frame: &VmFrame,
    value: Value,
    target_value_type_idx: u32,
) -> Result<Value, VmTrap> {
    let Value::Reference(reference) = value else {
        return Ok(Value::Reference(None));
    };
    let Some(reference) = reference else {
        return Ok(Value::Reference(None));
    };
    let Some(actual_type_idx) = dynamic_ref_type(runtime, module, frame, &reference)? else {
        return Ok(Value::Reference(None));
    };
    if runtime_type_is_compatible(module, actual_type_idx, target_value_type_idx)
        .map_err(VmTrap::Runtime)?
    {
        Ok(Value::Reference(Some(reference)))
    } else {
        Ok(Value::Reference(None))
    }
}

fn apply_interface_attempt(
    runtime: &impl super::context::ReferenceContext,
    module: &VmModule,
    value: Value,
    target_interface_type_idx: u32,
) -> Result<Value, VmTrap> {
    let Value::Instance(instance_id) = value else {
        return Ok(Value::Null);
    };
    let Some(instance) = runtime.storage().get_instance(instance_id) else {
        return Ok(Value::Null);
    };
    let type_name = instance.type_name.as_str();
    runtime
        .before_reference_path_growth(type_name.len(), type_name.len())
        .map_err(VmTrap::Runtime)?;
    let mut key = String::new();
    key.try_reserve_exact(type_name.len())
        .map_err(|_| VmTrap::Runtime(crate::error::RuntimeError::PreparationLimit))?;
    key.push_str(type_name);
    key.make_ascii_uppercase();
    let Some(pou_id) = module
        .class_ids
        .get(key.as_str())
        .or_else(|| module.function_block_ids.get(key.as_str()))
        .copied()
    else {
        return Ok(Value::Null);
    };
    if pou_implements_interface(module, pou_id, target_interface_type_idx)
        .map_err(VmTrap::Runtime)?
    {
        Ok(Value::Instance(instance_id))
    } else {
        Ok(Value::Null)
    }
}

pub(crate) fn runtime_type_is_compatible(
    module: &VmModule,
    mut actual: u32,
    mut target: u32,
) -> Result<bool, crate::error::RuntimeError> {
    // Alias/reference/array paths each have one successor. Walk them without
    // recursion or allocations; distinct cyclic paths fail closed at this bound.
    // Type-structure comparisons are bookkeeping under TIME-08 and charge nothing.
    let limit = module
        .types
        .entries
        .len()
        .saturating_mul(2)
        .saturating_add(1);
    let mut structural = false;
    for _ in 0..limit {
        let (Some(a), Some(t)) = (
            module.types.entries.get(actual as usize),
            module.types.entries.get(target as usize),
        ) else {
            return Ok(false);
        };
        if actual == target {
            return Ok(true);
        }
        if let TypeData::Alias { target_type_id } = a.data {
            actual = target_type_id;
            continue;
        }
        if let TypeData::Alias { target_type_id } = t.data {
            target = target_type_id;
            continue;
        }
        match (&a.data, &t.data) {
            (TypeData::Pou { pou_id: actual_pou }, TypeData::Pou { pou_id: target_pou })
                if !structural =>
            {
                let mut current = *actual_pou;
                for _ in 0..=module.pou_by_id.len() {
                    if current == *target_pou {
                        return Ok(true);
                    }
                    let Some(parent) = module.parent_pou_ids.get(&current) else {
                        return Ok(false);
                    };
                    current = *parent;
                }
                return Ok(false);
            }
            (TypeData::Pou { pou_id }, TypeData::Interface { .. }) if !structural => {
                let mut current = *pou_id;
                for _ in 0..=module.pou_by_id.len() {
                    if let Some(interfaces) = module.interface_type_ids_by_pou.get(&current)
                        && interfaces.contains(&target)
                    {
                        return Ok(true);
                    }
                    let Some(parent) = module.parent_pou_ids.get(&current) else {
                        return Ok(false);
                    };
                    current = *parent;
                }
                return Ok(false);
            }
            _ => {}
        }
        // Display/lookup labels do not change structural primitive, array or
        // reference types. Nominal aggregate kinds still require the same ID.
        match (&a.data, &t.data) {
            (
                TypeData::Primitive {
                    prim_id: a,
                    max_length: al,
                },
                TypeData::Primitive {
                    prim_id: t,
                    max_length: tl,
                },
            ) => return Ok(a == t && al == tl),
            (
                TypeData::Array {
                    elem_type_id: a,
                    dims: ad,
                },
                TypeData::Array {
                    elem_type_id: t,
                    dims: td,
                },
            ) => {
                if ad != td {
                    return Ok(false);
                }
                structural = true;
                actual = *a;
                target = *t;
            }
            (
                TypeData::Reference { target_type_id: a },
                TypeData::Reference { target_type_id: t },
            ) => {
                structural = true;
                actual = *a;
                target = *t;
            }
            (
                TypeData::Subrange {
                    base_type_id: a,
                    lower: al,
                    upper: au,
                },
                TypeData::Subrange {
                    base_type_id: t,
                    lower: tl,
                    upper: tu,
                },
            ) => {
                if al != tl || au != tu {
                    return Ok(false);
                }
                structural = true;
                actual = *a;
                target = *t;
            }
            // Structures, unions and enums retain their declared identity.
            _ => return Ok(false),
        }
    }
    Ok(false)
}

fn resolved_type_idx(
    module: &VmModule,
    mut type_idx: u32,
) -> Result<Option<u32>, crate::error::RuntimeError> {
    for _ in 0..module.types.entries.len().saturating_add(1) {
        let Some(entry) = module.types.entries.get(type_idx as usize) else {
            return Ok(None);
        };
        if let TypeData::Alias { target_type_id } = &entry.data {
            type_idx = *target_type_id;
        } else {
            return Ok(Some(type_idx));
        }
    }
    Ok(None)
}

fn pou_implements_interface(
    module: &VmModule,
    mut pou_id: u32,
    interface_type_idx: u32,
) -> Result<bool, crate::error::RuntimeError> {
    for _ in 0..module.pou_by_id.len().saturating_add(1) {
        if let Some(interfaces) = module.interface_type_ids_by_pou.get(&pou_id)
            && interfaces.contains(&interface_type_idx)
        {
            return Ok(true);
        }
        let Some(parent) = module.parent_pou_ids.get(&pou_id).copied() else {
            return Ok(false);
        };
        pou_id = parent;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{PouKind, TypeEntry, TypeTable};
    use crate::error::{ExecutionLimitKind, RuntimeError};
    use crate::memory::{MemoryLocation, VariableStorage};
    use crate::value::{write_value_path, RefSegment};
    use crate::vm::{budget::ExecutionBudget, context::ReferenceContext, VmFrame, VmTrap};
    use alloc::{format, vec};

    fn module(entries: alloc::vec::Vec<TypeEntry>) -> VmModule {
        // This private comparison fixture is not an admission or execution token.
        let prepared = super::super::PreparedModule::from_bytes(
            include_bytes!(
                "../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            super::super::PreparationLimits::default(),
        )
        .unwrap();
        let mut module = prepared.vm;
        module.types = TypeTable {
            offsets: vec![],
            entries,
        };
        module.strings = vec!["First".into(), "Second".into()];
        module
    }

    fn entry(data: TypeData) -> TypeEntry {
        let kind = match data {
            TypeData::Primitive { .. } => TypeKind::Primitive,
            TypeData::Array { .. } => TypeKind::Array,
            TypeData::Reference { .. } => TypeKind::Reference,
            TypeData::Alias { .. } => TypeKind::Alias,
            TypeData::Struct { .. } => TypeKind::Struct,
            _ => panic!("test type"),
        };
        TypeEntry {
            kind,
            name_idx: None,
            data,
        }
    }

    #[test]
    fn compatible_repeated_types_require_exact_shape_and_nominal_members() {
        let mut module = module(vec![
            entry(TypeData::Primitive {
                prim_id: 7,
                max_length: 0,
            }),
            entry(TypeData::Primitive {
                prim_id: 8,
                max_length: 0,
            }),
            entry(TypeData::Array {
                elem_type_id: 0,
                dims: vec![(0, 2)],
            }),
            entry(TypeData::Array {
                elem_type_id: 0,
                dims: vec![(0, 2)],
            }),
            entry(TypeData::Array {
                elem_type_id: 0,
                dims: vec![(1, 3)],
            }),
            entry(TypeData::Array {
                elem_type_id: 1,
                dims: vec![(0, 2)],
            }),
            entry(TypeData::Reference { target_type_id: 2 }),
            entry(TypeData::Reference { target_type_id: 3 }),
            entry(TypeData::Primitive {
                prim_id: 24,
                max_length: 4,
            }),
            entry(TypeData::Primitive {
                prim_id: 24,
                max_length: 8,
            }),
            entry(TypeData::Struct { fields: vec![] }),
            entry(TypeData::Struct { fields: vec![] }),
            entry(TypeData::Alias { target_type_id: 3 }),
        ]);
        // The earlier anonymous type loses its generated registry name when a
        // later identical declaration reuses that name (the real producer case).
        module.types.entries[3].name_idx = Some(0);
        assert_eq!(runtime_type_is_compatible(&module, 2, 3), Ok(true));
        assert_eq!(runtime_type_is_compatible(&module, 6, 7), Ok(true));
        assert_eq!(runtime_type_is_compatible(&module, 2, 12), Ok(true));
        for (actual, target) in [(2, 4), (2, 5), (8, 9), (10, 11), (6, 2)] {
            assert_eq!(
                runtime_type_is_compatible(&module, actual, target),
                Ok(false)
            );
        }
    }

    #[test]
    fn differently_named_structures_do_not_become_the_same_declared_target() {
        let mut first = entry(TypeData::Struct { fields: vec![] });
        first.name_idx = Some(0);
        let mut second = first.clone();
        second.name_idx = Some(1);
        let module = module(vec![
            entry(TypeData::Primitive {
                prim_id: 7,
                max_length: 0,
            }),
            first,
            second,
        ]);
        assert_eq!(runtime_type_is_compatible(&module, 1, 2), Ok(false));
    }

    #[test]
    fn nominal_inheritance_does_not_make_nested_reference_targets_covariant() {
        let mut module = module(vec![
            TypeEntry {
                kind: TypeKind::Class,
                name_idx: Some(0),
                data: TypeData::Pou { pou_id: 10 },
            },
            TypeEntry {
                kind: TypeKind::Class,
                name_idx: Some(1),
                data: TypeData::Pou { pou_id: 20 },
            },
            entry(TypeData::Reference { target_type_id: 0 }),
            entry(TypeData::Reference { target_type_id: 1 }),
        ]);
        module.parent_pou_ids.insert(20, 10);
        assert_eq!(runtime_type_is_compatible(&module, 1, 0), Ok(true));
        assert_eq!(runtime_type_is_compatible(&module, 0, 1), Ok(false));
        assert_eq!(runtime_type_is_compatible(&module, 3, 2), Ok(false));
    }

    #[test]
    fn cyclic_type_paths_fail_closed_at_their_bound() {
        let module = module(vec![
            entry(TypeData::Reference { target_type_id: 0 }),
            entry(TypeData::Reference { target_type_id: 1 }),
        ]);
        // TIME-08: type-structure comparisons are bookkeeping and charge nothing; the
        // distinct cyclic path still stops at its iteration bound and is rejected.
        assert_eq!(runtime_type_is_compatible(&module, 0, 1), Ok(false));
    }

    struct AttemptContext {
        storage: VariableStorage,
        budget: ExecutionBudget,
    }

    impl AttemptContext {
        fn new(limit: usize) -> Self {
            Self {
                storage: VariableStorage::new(),
                budget: ExecutionBudget::new(limit),
            }
        }
    }

    impl ReferenceContext for AttemptContext {
        fn before_reference_path_growth(
            &self,
            _bytes: usize,
            work: usize,
        ) -> Result<(), RuntimeError> {
            self.budget.charge(work).map(|_| ())
        }
        fn check_reference_write(
            &self,
            _reference: crate::value::ValueRefView<'_>,
            _value: &Value,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }
        fn check_reference_read(
            &self,
            _reference: crate::value::ValueRefView<'_>,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }
        fn initializer_reference<'a>(
            &self,
            _initializer_id: u32,
            _path: &'a [RefSegment],
        ) -> Result<crate::value::ValueRefView<'a>, VmTrap> {
            Err(VmTrap::NullReference)
        }
        fn storage(&self) -> &VariableStorage {
            &self.storage
        }
        fn storage_mut(&mut self) -> &mut VariableStorage {
            &mut self.storage
        }
        fn write_typed_storage(
            &mut self,
            location: MemoryLocation,
            offset: usize,
            path: &[RefSegment],
            value: Value,
        ) -> Result<bool, RuntimeError> {
            Ok(self
                .storage
                .write_by_ref_parts(location, offset, path, value))
        }
        fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
            write_value_path(root, path, value)
        }
    }

    fn interface_attempt_module() -> VmModule {
        let mut module = module(vec![TypeEntry {
            kind: TypeKind::Interface,
            name_idx: None,
            data: TypeData::Interface { methods: vec![] },
        }]);
        module
            .types
            .entries
            .push(entry(TypeData::Alias { target_type_id: 0 }));
        for id in [8, 9, 10] {
            module.pou_by_id.insert(
                id,
                crate::vm::module::VmPouEntry {
                    kind: PouKind::FunctionBlock,
                    name: format!("FB{id}").into(),
                    code_start: 0,
                    code_end: 0,
                    local_ref_start: 0,
                    local_ref_count: 0,
                    primary_instance_owner: None,
                },
            );
        }
        module.parent_pou_ids.insert(10, 9);
        module.parent_pou_ids.insert(9, 8);
        module.class_ids.insert("DRIVE".into(), 10);
        module.interface_type_ids_by_pou.insert(8, vec![0]);
        module
    }

    fn empty_frame() -> VmFrame {
        VmFrame {
            activation: None,
            pou_id: None,
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            locals: vec![],
            parameter_values_present: vec![],
            runtime_instance: None,
            instance_owner: None,
        }
    }

    #[test]
    fn interface_attempt_charges_only_its_copied_type_name() {
        let module = interface_attempt_module();
        let mut context = AttemptContext::new(10);
        let instance = context.storage.create_instance("drive");
        // TIME-08: alias resolution, ancestry and interface visits are bookkeeping. The only
        // charge is the copy of the 5-character type name, so the attempt costs 5 units.
        assert_eq!(
            apply_reference_attempt(
                &context,
                &module,
                &empty_frame(),
                Value::Instance(instance),
                1,
            )
            .unwrap(),
            Value::Instance(instance)
        );
        assert_eq!(context.budget.remaining(), 5);

        context.budget.reset(4);
        let failure = apply_reference_attempt(
            &context,
            &module,
            &empty_frame(),
            Value::Instance(instance),
            1,
        )
        .unwrap_err()
        .into_runtime_error();
        assert!(matches!(
            failure,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                requested: 5,
                used: 0,
                limit: 4,
            }
        ));

        context.budget.reset(5);
        assert_eq!(
            apply_reference_attempt(
                &context,
                &module,
                &empty_frame(),
                Value::Instance(instance),
                1,
            )
            .unwrap(),
            Value::Instance(instance)
        );
        assert_eq!(context.budget.remaining(), 0);
    }
}

#[cfg(test)]
#[path = "reference_attempt/work_allowance_tests.rs"]
mod work_allowance_tests;
