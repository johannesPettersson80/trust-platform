//! Transactional transfer of live retained globals into replacement storage.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole, TypeData};
use crate::memory::MemoryLocation;
use alloc::sync::Arc;
use smol_str::SmolStr;

struct RetainedGraph<'s, 'p> {
    source: &'s EngineState<'p>,
    identities: BTreeMap<u32, u32>,
    copied: BTreeSet<u32>,
    initialized: BTreeMap<u32, Vec<u32>>,
    once: BTreeMap<u32, Vec<u32>>,
    root_indices: BTreeMap<u32, Vec<u32>>,
}

fn index_marks(
    target: &EngineState<'_>,
    marks: &LifecycleMarks,
) -> Result<BTreeMap<u32, Vec<u32>>, RuntimeError> {
    let mut index: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (declaration, owner) in marks.iter() {
        let Some(owner) = owner else {
            continue;
        };
        target.charge_allocation_bytes(
            core::mem::size_of::<(InstanceId, Vec<u32>)>() * 4 + 2 * core::mem::size_of::<u32>(),
        )?;
        let declarations = index.entry(owner.0).or_default();
        declarations
            .try_reserve(1)
            .map_err(|_| RuntimeError::Overflow)?;
        declarations.push(declaration);
    }
    Ok(index)
}

impl EngineState<'_> {
    pub(in crate::vm::engine) fn import_retained_globals(
        &mut self,
        source: &EngineState<'_>,
    ) -> Result<(), RuntimeError> {
        let mut graph = RetainedGraph {
            source,
            identities: BTreeMap::new(),
            copied: BTreeSet::new(),
            initialized: index_marks(self, &source.data.construction.initialized_declarations)?,
            once: index_marks(self, &source.data.construction.once)?,
            root_indices: BTreeMap::new(),
        };
        for (index, (old, new)) in source
            .data
            .construction
            .roots
            .iter()
            .zip(&self.data.construction.roots)
            .enumerate()
        {
            if let (Some(old), Some(new)) = (old, new) {
                self.charge_allocation_bytes(core::mem::size_of::<(InstanceId, InstanceId)>() * 4)?;
                graph.identities.insert(old.0, new.0);
                self.charge_allocation_bytes(
                    core::mem::size_of::<(InstanceId, Vec<usize>)>() * 4
                        + 2 * core::mem::size_of::<usize>(),
                )?;
                let indices = graph.root_indices.entry(new.0).or_default();
                indices.try_reserve(1).map_err(|_| RuntimeError::Overflow)?;
                indices.push(root_index(index)?);
            }
        }
        for (index, declaration) in source.prepared.layout.entries.iter().enumerate() {
            if declaration.owner != StorageOwner::Global
                || declaration.role != StorageRole::Variable
                || !declaration.is_retained()
            {
                continue;
            }
            let name = &source.prepared.vm.strings[declaration.name_idx as usize];
            let value = source
                .data
                .storage
                .get_global(name)
                .ok_or(RuntimeError::NullReference)?;
            let value = graph.value(self, declaration.type_id, value, 0)?;
            self.charge_allocation_bytes(self.value_clone_charge(&value, 0)?)?;
            let declaration_index = root_index(index)?;
            let (growth_bytes, _) = self
                .data
                .construction
                .retained_globals
                .growth_charge(declaration_index)?;
            self.charge_allocation_bytes(RetainedGlobals::insertion_charge().max(growth_bytes))?;
            let marker = (declaration_index, None);
            let marker_plan = self
                .data
                .construction
                .initialized_declarations
                .insertion_plan(marker)?;
            let (marker_bytes, _) = marker_plan.demand();
            self.charge_allocation_bytes(marker_bytes)?;
            let prepared_marker = self
                .data
                .construction
                .initialized_declarations
                .prepare_insert(marker_plan)?;
            // All work/allocation admission and marker reservation precede the
            // candidate's persistent value/retained-state publication.
            self.data.storage.set_global(name.clone(), value.clone());
            self.data
                .construction
                .retained_globals
                .insert(declaration_index, value)?;
            prepared_marker.publish();
        }
        Ok(())
    }
}

impl RetainedGraph<'_, '_> {
    fn step(depth: usize) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        Ok(())
    }

    fn value(
        &mut self,
        target: &mut EngineState<'_>,
        type_id: Option<u32>,
        value: &Value,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        target.charge_allocation_bytes(target.value_clone_charge(value, depth)?)?;
        self.owned_value(target, type_id, value.clone(), depth)
    }

    fn owned_value(
        &mut self,
        target: &mut EngineState<'_>,
        type_id: Option<u32>,
        mut value: Value,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        Self::step(depth)?;
        let source = self.source;
        let resolved_type = type_id.and_then(|id| {
            super::super::type_policy::resolved_alias_type(&source.prepared.vm.types, id, 0)
        });
        let ty = resolved_type.and_then(|id| source.prepared.vm.types.entries.get(id as usize));
        match &mut value {
            Value::Instance(id)
                if ty.is_some_and(|entry| matches!(entry.data, TypeData::Interface { .. })) =>
            {
                *id = self.binding(target, *id, depth + 1)?;
            }
            Value::Instance(id) => *id = self.instance(target, *id, depth + 1)?,
            Value::Reference(Some(reference)) => self.reference(target, reference, depth + 1)?,
            Value::Array(array) => {
                let element_type = match ty.map(|entry| &entry.data) {
                    Some(TypeData::Array { elem_type_id, .. }) => Some(*elem_type_id),
                    _ => return Err(RuntimeError::TypeMismatch),
                };
                for element in array.elements_mut() {
                    let original = core::mem::replace(element, Value::Null);
                    *element = self.owned_value(target, element_type, original, depth + 1)?;
                }
            }
            Value::Struct(structure) => {
                // Arc clone alone is cheap, but remapping must never mutate source state.
                if Arc::strong_count(structure) > 1 || Arc::weak_count(structure) > 0 {
                    target.charge_allocation_bytes(
                        core::mem::size_of::<crate::value::StructValue>()
                            .checked_add(2 * core::mem::size_of::<usize>())
                            .and_then(|bytes| bytes.checked_add(structure.physical_byte_len()))
                            .and_then(|bytes| {
                                structure
                                    .fields()
                                    .len()
                                    .checked_mul(core::mem::size_of::<(SmolStr, Value)>() * 4)
                                    .and_then(|fields| bytes.checked_add(fields))
                            })
                            .ok_or(RuntimeError::Overflow)?,
                    )?;
                    for field in structure.fields().values() {
                        target.charge_allocation_bytes(
                            target.value_clone_charge(field, depth + 1)?,
                        )?;
                    }
                }
                let type_id = resolved_type.ok_or(RuntimeError::TypeMismatch)?;
                let structure = Arc::make_mut(structure);
                structure.remap_retained_identities(|name, old| {
                    let field_type = source
                        .prepared
                        .type_member(type_id, name)
                        .ok_or(RuntimeError::TypeMismatch)?;
                    self.owned_value(target, Some(field_type), old, depth + 1)
                })?;
                let layout = target.prepared.vm.physical_layout(type_id);
                if !structure.has_physical_layout(layout.as_ref()) {
                    if let Some(layout) = &layout {
                        target.charge_work_units(layout.construction_work()?)?;
                    }
                    structure.rebind_physical_layout(layout)?;
                }
            }
            _ => {}
        }
        Ok(value)
    }

    fn instance(
        &mut self,
        target: &mut EngineState<'_>,
        old: InstanceId,
        depth: usize,
    ) -> Result<InstanceId, RuntimeError> {
        Self::step(depth)?;
        if self
            .source
            .data
            .lifetimes
            .instance_lifetimes
            .get(&old)
            .is_some_and(|owner| owner.is_some())
        {
            return Err(RuntimeError::ReferenceLifetime);
        }
        let source = self.source;
        let data = source
            .data
            .storage
            .get_instance(old)
            .ok_or(RuntimeError::NullReference)?;
        let new = if let Some(new) = self.identities.get(&old.0) {
            InstanceId(*new)
        } else {
            let pou = *self
                .source
                .data
                .construction
                .instance_templates
                .get(&old)
                .ok_or(RuntimeError::NullReference)?;
            let new = target.reserve_instance(pou, None)?;
            target.charge_allocation_bytes(core::mem::size_of::<(InstanceId, InstanceId)>() * 4)?;
            self.identities.insert(old.0, new.0);
            new
        };
        if self.copied.contains(&old.0) {
            return Ok(new);
        }
        target.charge_allocation_bytes(core::mem::size_of::<InstanceId>() * 4)?;
        self.copied.insert(old.0);
        target.charge_allocation_bytes(
            data.variables
                .len()
                .checked_mul(core::mem::size_of::<Value>() * 4)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        // Reserve every field before recursion so cycles can bind native/private slots too.
        for name in data.variables.keys() {
            if target.data.storage.get_instance_var(new, name).is_none() {
                target
                    .data
                    .storage
                    .set_instance_var(new, name.clone(), Value::Null);
            }
        }
        let parent = data.parent;
        let names = data.variables.keys().cloned().collect::<Vec<_>>();
        for name in names {
            let old_value = source
                .data
                .storage
                .get_instance_var(old, &name)
                .ok_or(RuntimeError::NullReference)?;
            // Type metadata is identical in the mapped candidate. Charge its
            // fresh restart budget, never the source state's previous scan fuel.
            let ty = match target.data.storage.ref_for_instance(new, &name) {
                Some(reference) => target.reference_type(reference.as_view())?,
                None => None,
            };
            let value = self.value(target, ty, old_value, depth + 1)?;
            if !target.data.storage.set_instance_var(new, name, value) {
                return Err(RuntimeError::NullReference);
            }
        }
        let parent = parent
            .map(|parent| self.instance(target, parent, depth + 1))
            .transpose()?;
        target
            .data
            .storage
            .get_instance_mut(new)
            .ok_or(RuntimeError::NullReference)?
            .parent = parent;
        target.charge_allocation_bytes(core::mem::size_of::<InstanceId>() * 4)?;
        target.charge_sorted_growth(
            target
                .data
                .construction
                .initialized_instances
                .insertion_demand(new),
        )?;
        target.data.construction.initialized_instances.insert(new)?;
        if let Some(declarations) = self.initialized.get(&old.0) {
            for &declaration in declarations {
                let marker = (declaration, Some(new));
                let plan = target
                    .data
                    .construction
                    .initialized_declarations
                    .insertion_plan(marker)?;
                target.charge_sorted_growth(Ok(plan.demand()))?;
                let prepared_marker = target
                    .data
                    .construction
                    .initialized_declarations
                    .prepare_insert(plan)?;
                prepared_marker.publish();
            }
        }
        if let Some(declarations) = self.once.get(&old.0) {
            for &declaration in declarations {
                let marker = (declaration, Some(new));
                let plan = target.data.construction.once.insertion_plan(marker)?;
                target.charge_sorted_growth(Ok(plan.demand()))?;
                let prepared_marker = target.data.construction.once.prepare_insert(plan)?;
                prepared_marker.publish();
            }
        }
        if let Some(indices) = self.root_indices.get(&new.0) {
            for &index in indices {
                target.charge_allocation_bytes(core::mem::size_of::<usize>() * 4)?;
                target.data.construction.claimed_roots.insert(index);
            }
        }
        Ok(new)
    }

    fn binding(
        &mut self,
        target: &mut EngineState<'_>,
        old: InstanceId,
        depth: usize,
    ) -> Result<InstanceId, RuntimeError> {
        Self::step(depth)?;
        match self.identities.get(&old.0) {
            Some(new) => Ok(InstanceId(*new)),
            // A persistent dynamic object has no artifact root to reconstruct.
            // Keep it alive if its remaining retained link is a reference/interface.
            None => self.instance(target, old, depth + 1),
        }
    }

    fn reference(
        &mut self,
        target: &mut EngineState<'_>,
        reference: &mut ValueRef,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        Self::step(depth)?;
        match reference.location {
            MemoryLocation::Global => {
                let (name, _) = self
                    .source
                    .data
                    .storage
                    .globals()
                    .get_index(reference.offset)
                    .ok_or(RuntimeError::NullReference)?;
                reference.offset = target
                    .data
                    .storage
                    .ref_for_global(name)
                    .ok_or(RuntimeError::NullReference)?
                    .offset;
            }
            MemoryLocation::Instance(old) => {
                let data = self
                    .source
                    .data
                    .storage
                    .get_instance(old)
                    .ok_or(RuntimeError::NullReference)?;
                let name = data
                    .variables
                    .get_index(reference.offset)
                    .ok_or(RuntimeError::NullReference)?
                    .0
                    .clone();
                // A reference to an artifact root preserves binding, not ownership.
                let new = self.binding(target, old, depth + 1)?;
                let mapped = target
                    .data
                    .storage
                    .ref_for_instance(new, &name)
                    .ok_or(RuntimeError::NullReference)?;
                reference.location = mapped.location;
                reference.offset = mapped.offset;
            }
            MemoryLocation::Io(_) | MemoryLocation::Local(_) | MemoryLocation::Retain => {
                return Err(RuntimeError::ReferenceLifetime)
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_import_marker_refusal_precedes_global_and_retained_publication() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let (declaration, name) = prepared
            .layout
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| {
                entry.owner == StorageOwner::Global && entry.role == StorageRole::Variable
            })
            .map(|(index, entry)| {
                (
                    u32::try_from(index).expect("fixture declaration fits u32"),
                    prepared.vm.strings[entry.name_idx as usize].clone(),
                )
            })
            .expect("GPIO fixture has a global variable");
        prepared.layout.entries[declaration as usize].retain = 1;
        let mut source_data =
            EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        source_data.storage.set_global(name.clone(), Value::Int(17));
        let source = EngineState::borrow(&mut source_data, &prepared, &services::LOGICAL_ONLY);
        let mut target_data =
            EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        target_data.storage.set_global(name.clone(), Value::Int(99));
        let previous = target_data
            .storage
            .get_global(&name)
            .expect("constructed retained global")
            .clone();
        assert_ne!(
            previous,
            Value::Int(17),
            "source and target differ before import"
        );
        target_data.construction.initialized_declarations = Default::default();
        // Measure the import's complete allocation on an identical target, then
        // allow one byte less so its last admission, the marker, is refused.
        let mut probe_data =
            EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        probe_data.storage.set_global(name.clone(), Value::Int(99));
        probe_data.construction.initialized_declarations = Default::default();
        probe_data.resources.construction_bytes.set(0);
        let mut probe = EngineState::borrow(&mut probe_data, &prepared, &services::LOGICAL_ONLY);
        probe.import_retained_globals(&source).unwrap();
        let required = probe.data.resources.construction_bytes.get();
        // The single boundary above is one sample; every smaller allowance must also
        // refuse at some admission and publish neither the global nor its marker. The
        // import writes into the replacement candidate, which commits only after a full
        // replacement succeeds and is discarded on refusal, so a partial import inside
        // a refused candidate is not observable by the live owner (spec 34 RETAIN-02).
        for allowance in 0..required {
            let mut candidate_data =
                EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
            candidate_data
                .storage
                .set_global(name.clone(), Value::Int(99));
            candidate_data.construction.initialized_declarations = Default::default();
            candidate_data
                .resources
                .construction_bytes
                .set(prepared.limits.max_construction_bytes - allowance);
            let capacity = candidate_data
                .construction
                .initialized_declarations
                .capacity_state_for_test();
            let mut candidate =
                EngineState::borrow(&mut candidate_data, &prepared, &services::LOGICAL_ONLY);
            assert!(
                matches!(
                    candidate.import_retained_globals(&source),
                    Err(RuntimeError::ExecutionLimit {
                        kind: crate::error::ExecutionLimitKind::AllocationBytes,
                        ..
                    })
                ),
                "allowance {allowance} must refuse the import with AllocationBytes"
            );
            assert_eq!(
                candidate.data.storage.get_global(&name),
                Some(&Value::Int(99)),
                "allowance {allowance} published the global"
            );
            assert!(
                !candidate
                    .data
                    .construction
                    .retained_globals
                    .contains_key(&declaration),
                "allowance {allowance} published the retained value"
            );
            assert_eq!(
                candidate
                    .data
                    .construction
                    .initialized_declarations
                    .capacity_state_for_test(),
                capacity,
                "allowance {allowance} grew the marker table"
            );
        }
        target_data
            .resources
            .construction_bytes
            .set(prepared.limits.max_construction_bytes - (required - 1));
        let marker_capacity_before = target_data
            .construction
            .initialized_declarations
            .capacity_state_for_test();
        let mut target = EngineState::borrow(&mut target_data, &prepared, &services::LOGICAL_ONLY);

        assert!(matches!(
            target.import_retained_globals(&source),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            })
        ));
        assert_eq!(target.data.storage.get_global(&name), Some(&previous));
        assert!(!target
            .data
            .construction
            .retained_globals
            .contains_key(&declaration));
        assert_eq!(
            target
                .data
                .construction
                .initialized_declarations
                .capacity_state_for_test(),
            marker_capacity_before
        );
    }

    #[test]
    fn warm_restart_transfer_refusal_above_construction_preserves_the_owner() {
        use crate::retain::RestartMode;

        type Observed = (
            Option<Value>,
            Option<Value>,
            Vec<u8>,
            Vec<u8>,
            Duration,
            bool,
        );
        fn observe(data: &EngineData) -> Observed {
            let plant = match data.storage.get_global("Plant") {
                Some(Value::Instance(instance)) => *instance,
                other => panic!("program root Plant: {other:?}"),
            };
            (
                data.storage.get_global("probe_depth").cloned(),
                data.storage
                    .get_instance_var(plant, "depth_result")
                    .cloned(),
                data.images.outputs.clone(),
                data.images.memory.clone(),
                data.now,
                data.fault.is_some(),
            )
        }
        let retained_fixture = || {
            let mut prepared = PreparedModule::from_bytes(
                include_bytes!(
                    "../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"
                ),
                crate::vm::PreparationLimits::default(),
            )
            .unwrap();
            let declaration = prepared
                .layout
                .entries
                .iter()
                .position(|entry| {
                    entry.owner == StorageOwner::Global && entry.role == StorageRole::Variable
                })
                .expect("GPIO fixture has a global variable");
            prepared.layout.entries[declaration].retain = 1;
            prepared
        };
        // Construction under a limit below these bytes fails before any restart runs. The
        // sweep therefore covers only the limits from this charge upward: a restart
        // allocation point that is charged below it is outside this test.
        let live_bytes = {
            let prepared = retained_fixture();
            let data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
            data.resources.construction_bytes.get()
        };
        // Raise the admitted construction limit one byte at a time until the warm
        // candidate, including its retained-graph import, is admitted. Every smaller
        // limit must refuse with the exact error and leave the live owner unchanged.
        let mut refusals = 0;
        let mut limit = live_bytes;
        loop {
            let mut prepared = retained_fixture();
            prepared.limits.max_construction_bytes = limit;
            let mut data =
                EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
            data.storage.set_global("probe_depth", Value::Int(99));
            let before = observe(&data);
            let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
            match state.restart(RestartMode::Warm) {
                Ok(()) => {
                    assert_eq!(
                        state.data.storage.get_global("probe_depth"),
                        Some(&Value::Int(99)),
                        "limit {limit} admitted a restart that did not transfer the global"
                    );
                    break;
                }
                Err(error) => {
                    assert!(
                        matches!(
                            error,
                            RuntimeError::ExecutionLimit {
                                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                                limit: observed,
                                ..
                            } if observed == limit
                        ),
                        "limit {limit} must refuse with the exact AllocationBytes limit: {error:?}"
                    );
                    assert_eq!(
                        observe(&*state.data),
                        before,
                        "limit {limit} changed the live owner"
                    );
                    refusals += 1;
                }
            }
            limit += 1;
            assert!(
                limit < live_bytes + 65_536,
                "the warm restart transfer was never admitted"
            );
        }
        assert!(refusals > 0, "the sweep must observe refused transfers");
    }

    #[test]
    fn retained_structure_remap_keeps_gap_bits_and_rebinds_the_candidate_layout() {
        use crate::bytecode::{BytecodeModule, Field, SectionData, SectionId, TypeEntry, TypeKind};
        use crate::collections::OrderedMap;
        use crate::value::{read_value_path_borrowed, write_value_path, RefSegment, StructValue};

        let mut artifact = BytecodeModule::decode(include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"
        ))
        .unwrap();
        artifact.version = crate::bytecode::BytecodeVersion::SOURCE_FREE;
        let SectionData::StringTable(strings) =
            artifact.section_mut(SectionId::StringTable).unwrap()
        else {
            panic!("strings")
        };
        let first_name = strings.entries.len() as u32;
        strings.entries.extend(
            [
                "RetainedInner",
                "RetainedOuter",
                "low",
                "high",
                "whole",
                "inner",
            ]
            .map(SmolStr::from),
        );
        let SectionData::TypeTable(types) = artifact.section_mut(SectionId::TypeTable).unwrap()
        else {
            panic!("types")
        };
        let byte = types.entries.len() as u32;
        types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 2,
                max_length: 0,
            },
        });
        let dword = types.entries.len() as u32;
        types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 4,
                max_length: 0,
            },
        });
        let inner = types.entries.len() as u32;
        types.entries.push(TypeEntry {
            kind: TypeKind::OverlappingStruct,
            name_idx: Some(first_name),
            data: TypeData::Struct {
                fields: vec![
                    Field {
                        name_idx: first_name + 2,
                        type_id: byte,
                        bit_offset: Some(0),
                    },
                    Field {
                        name_idx: first_name + 3,
                        type_id: byte,
                        bit_offset: Some(16),
                    },
                ],
            },
        });
        let outer = types.entries.len() as u32;
        types.entries.push(TypeEntry {
            kind: TypeKind::OverlappingStruct,
            name_idx: Some(first_name + 1),
            data: TypeData::Struct {
                fields: vec![
                    Field {
                        name_idx: first_name + 4,
                        type_id: dword,
                        bit_offset: Some(0),
                    },
                    Field {
                        name_idx: first_name + 5,
                        type_id: inner,
                        bit_offset: Some(0),
                    },
                ],
            },
        });
        let bytes = artifact.encode().unwrap();
        let source_prepared =
            PreparedModule::from_bytes(&bytes, crate::vm::PreparationLimits::default()).unwrap();
        let target_prepared =
            PreparedModule::from_bytes(&bytes, crate::vm::PreparationLimits::default()).unwrap();
        let blank = |prepared: &PreparedModule| {
            let value = StructValue::from_canonical_parts(
                "RetainedInner".into(),
                OrderedMap::from_iter([
                    ("low".into(), Value::Byte(0)),
                    ("high".into(), Value::Byte(0)),
                ]),
            )
            .with_physical_layout(prepared.vm.physical_layout(inner))
            .unwrap();
            let value = StructValue::from_canonical_parts(
                "RetainedOuter".into(),
                OrderedMap::from_iter([
                    ("whole".into(), Value::DWord(0)),
                    ("inner".into(), Value::Struct(Arc::new(value))),
                ]),
            )
            .with_physical_layout(prepared.vm.physical_layout(outer))
            .unwrap();
            Value::Struct(Arc::new(value))
        };
        let mut original = blank(&source_prepared);
        assert!(write_value_path(
            &mut original,
            &[RefSegment::Field("whole".into())],
            Value::DWord(0xDDCC_BBAA)
        ));
        let retained =
            read_value_path_borrowed(&original, &[RefSegment::Field("inner".into())]).unwrap();
        let mut source_data =
            EngineState::construct_data(&source_prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let source =
            EngineState::borrow(&mut source_data, &source_prepared, &services::LOGICAL_ONLY);
        let mut target_data =
            EngineState::construct_data(&target_prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let mut target =
            EngineState::borrow(&mut target_data, &target_prepared, &services::LOGICAL_ONLY);
        let mut graph = RetainedGraph {
            source: &source,
            identities: BTreeMap::new(),
            copied: BTreeSet::new(),
            initialized: BTreeMap::new(),
            once: BTreeMap::new(),
            root_indices: BTreeMap::new(),
        };
        let copied = graph.value(&mut target, Some(inner), retained, 0).unwrap();
        let Value::Struct(copied_structure) = &copied else {
            panic!("retained structure")
        };
        assert!(copied_structure
            .has_physical_layout(target_prepared.vm.physical_layout(inner).as_ref()));
        let mut restored = blank(&target_prepared);
        assert!(write_value_path(
            &mut restored,
            &[RefSegment::Field("whole".into())],
            Value::DWord(0x9900_0000)
        ));
        assert!(write_value_path(
            &mut restored,
            &[RefSegment::Field("inner".into())],
            copied
        ));
        assert_eq!(
            read_value_path_borrowed(&restored, &[RefSegment::Field("whole".into())]),
            Some(&Value::DWord(0x99CC_BBAA))
        );
        assert_eq!(
            read_value_path_borrowed(&original, &[RefSegment::Field("whole".into())]),
            Some(&Value::DWord(0xDDCC_BBAA))
        );
        // The shared writer remains live after restore; aliases still update.
        assert!(write_value_path(
            &mut restored,
            &[
                RefSegment::Field("inner".into()),
                RefSegment::Field("low".into())
            ],
            Value::Byte(0x11)
        ));
        assert_eq!(
            read_value_path_borrowed(&restored, &[RefSegment::Field("whole".into())]),
            Some(&Value::DWord(0x99CC_BB11))
        );
        // Engineering assignment can receive a coherent value from a different
        // prepared module. Normalizing its fields must retain the same gap bits.
        let assigned = target
            .normalize_assignment_value(inner, retained.clone())
            .unwrap();
        let mut destination = blank(&target_prepared);
        assert!(write_value_path(
            &mut destination,
            &[RefSegment::Field("whole".into())],
            Value::DWord(0x9900_0000)
        ));
        assert!(write_value_path(
            &mut destination,
            &[RefSegment::Field("inner".into())],
            assigned
        ));
        assert_eq!(
            read_value_path_borrowed(&destination, &[RefSegment::Field("whole".into())]),
            Some(&Value::DWord(0x99CC_BBAA))
        );
    }

    #[test]
    fn retained_object_copy_consumes_candidate_fuel_not_old_scan_fuel() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut source_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let source = EngineState::borrow(
            &mut source_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let mut target_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut target = EngineState::borrow(
            &mut target_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let Some(Value::Instance(program)) = source.data.storage.get_global("Plant") else {
            panic!("program")
        };
        let Some(Value::Instance(counter)) =
            source.data.storage.get_instance_var(*program, "counter")
        else {
            panic!("counter")
        };
        source.data.resources.work_budget.reset(0);
        let before = target.data.resources.work_budget.remaining();
        let mut graph = RetainedGraph {
            source: &source,
            identities: BTreeMap::new(),
            copied: BTreeSet::new(),
            initialized: BTreeMap::new(),
            once: BTreeMap::new(),
            root_indices: BTreeMap::new(),
        };
        let copied = graph.instance(&mut target, *counter, 0).unwrap();
        assert_eq!(
            target.data.storage.get_instance_var(copied, "value"),
            Some(&Value::Int(3))
        );
        // TIME-08 VERIFY: the copied `history` array costs 1 + 3 elements (COPY); the
        // scalar fields and the reference type queries (bookkeeping) cost nothing.
        assert!(target.data.resources.work_budget.remaining() < before);
        assert_eq!(source.data.resources.work_budget.remaining(), 0);
    }
    #[test]
    fn compact_retained_indexes_preserve_global_separation_full_ids_and_declaration_order() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut target_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let target = EngineState::borrow(
            &mut target_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let mut marks = LifecycleMarks::default();
        marks
            .insert((u16::MAX as u32, Some(InstanceId(u32::MAX))))
            .unwrap();
        marks.insert((1, Some(InstanceId(0)))).unwrap();
        marks.insert((0, None)).unwrap();
        marks.insert((0, Some(InstanceId(u32::MAX)))).unwrap();
        let index = index_marks(&target, &marks).unwrap();
        assert_eq!(index.keys().copied().collect::<Vec<_>>(), [0, u32::MAX]);
        assert_eq!(index[&0], [1]);
        assert_eq!(index[&u32::MAX], [0, u16::MAX as u32]);
        assert!(
            marks.contains(&(0, None)).unwrap(),
            "indexing must not consume source marks"
        );
    }
}
