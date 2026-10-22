//! Transactional transfer of live retained globals into replacement storage.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole, TypeData};
use crate::memory::layout::CELL_BYTES;
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::MemoryLocation;
use alloc::sync::Arc;

mod cells;

/// The declared name of the layout entry `declaration` (C1-9d1).
fn member_name(prepared: &PreparedModule, declaration: u32) -> &str {
    let entry = prepared.storage_declaration_at(declaration as usize);
    &prepared.vm.strings[entry.name_idx as usize]
}

struct RetainedGraph<'s, 'p> {
    source: &'s EngineState<'p>,
    identities: BTreeMap<u32, u32>,
    copied: BTreeSet<u32>,
    initialized: BTreeMap<u32, Vec<u32>>,
    once: BTreeMap<u32, Vec<u32>>,
    root_indices: BTreeMap<u32, Vec<u32>>,
}

fn index_marks(marks: &LifecycleMarks) -> Result<BTreeMap<u32, Vec<u32>>, RuntimeError> {
    let mut index: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (declaration, owner) in marks.iter() {
        let Some(owner) = owner else {
            continue;
        };
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
            initialized: index_marks(&source.data.construction.initialized_declarations)?,
            once: index_marks(&source.data.construction.once)?,
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
                graph.identities.insert(old.0, new.0);
                let indices = graph.root_indices.entry(new.0).or_default();
                indices.try_reserve(1).map_err(|_| RuntimeError::Overflow)?;
                indices.push(root_index(index)?);
            }
        }
        for (index, declaration) in source.prepared.storage_declarations().enumerate() {
            if declaration.owner != StorageOwner::Global
                || declaration.role != StorageRole::Variable
                || !declaration.is_retained()
            {
                continue;
            }
            let name = &source.prepared.vm.strings[declaration.name_idx as usize];
            let view = source
                .read_global(name)
                .ok_or(RuntimeError::NullReference)?;
            // On the flat store the copy is charged from the source's cells, as `value` charges it,
            // and made in place at the publication below; nothing builds a value (C1-14b).
            // The generation the candidate's references take, read before its tables are reserved.
            let generation = StoreAccess::generation(&*self);
            let value = if graph.copies_in_place(view)? {
                self.charge_allocation_bytes(self.view_clone_charge(&view)?)?;
                graph.owned_cells(self, view, 0)?;
                None
            } else {
                // `value` charges the copy once (TIME-08); the owned copy is not charged again.
                let value = view.materialize()?;
                Some(graph.value(self, declaration.type_id, &value, 0)?)
            };
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
            match value {
                Some(value) => {
                    // The marker reservation borrows the lifecycle tables; the destination write needs
                    // only the storage and store fields, and goes through the store the engine holds.
                    let mut writer = super::store_access::store_writer(
                        &mut self.data.storage,
                        &mut self.data.store,
                        self.prepared,
                    );
                    if !StoreMutate::set_global(&mut writer, name, value) {
                        return Err(RuntimeError::InvalidExecutionState);
                    }
                }
                None => {
                    let at = self
                        .prepared
                        .global_range(declaration_index)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    graph.write_cells(&mut self.data.store, self.prepared, generation, view, at)?;
                }
            }
            self.data
                .construction
                .retained_globals
                .insert(declaration_index)?;
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
                // The copy is charged once, by `value`, for each copied leaf (TIME-08), so the
                // remap adds no charge. Arc::make_mut detaches shared storage, so remapping never
                // mutates source state.
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
        source
            .instance_type_name(old)
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
            self.identities.insert(old.0, new.0);
            new
        };
        if self.copied.contains(&old.0) {
            return Ok(new);
        }
        self.copied.insert(old.0);
        // The members are the template's declarations, in declaration order: the record holds exactly
        // these (C1-9d1, R2), so the count and the names come from the layout, not the record's map.
        let pou = *source
            .data
            .construction
            .instance_templates
            .get(&old)
            .ok_or(RuntimeError::NullReference)?;
        let members = source
            .prepared
            .declarations(StorageOwner::Instance, Some(pou));
        // Four cells for each copied variable entry; each value's copy is charged by `value`.
        target.charge_allocation_bytes(
            members
                .len()
                .checked_mul(4 * CELL_BYTES)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        // Reserve every field before recursion so cycles can bind native/private slots too.
        for &declaration in members {
            let name = member_name(source.prepared, declaration);
            if target.read_instance_var(new, name).is_none() {
                StoreMutate::write_instance_var(target, new, name, Value::Null);
            }
        }
        let parent = source
            .instance_parent(old)
            .map_err(|_| RuntimeError::NullReference)?;
        for &declaration in members {
            let name = member_name(source.prepared, declaration);
            let view = source
                .read_instance_var(old, name)
                .ok_or(RuntimeError::NullReference)?;
            // Type metadata is identical in the mapped candidate. Charge its
            // fresh restart budget, never the source state's previous scan fuel.
            let ty = match target.ref_for_instance(new, name) {
                Some(reference) => target.reference_type(reference.as_view())?,
                None => None,
            };
            // On the flat store the member is copied from the source's cells into the
            // candidate's, charged as its value's copy is (C1-14b).
            if self.copies_in_place(view)? {
                target.charge_allocation_bytes(target.view_clone_charge(&view)?)?;
                self.owned_cells(target, view, depth + 1)?;
                let Some(ValueView::Cells { range: at, .. }) = target.read_instance_var(new, name)
                else {
                    return Err(RuntimeError::NullReference);
                };
                let generation = StoreAccess::generation(&*target);
                self.write_cells(
                    &mut target.data.store,
                    target.prepared,
                    generation,
                    view,
                    at,
                )?;
                continue;
            }
            let value = self.value(target, ty, &view.materialize()?, depth + 1)?;
            if !StoreMutate::write_instance_var(target, new, name, value) {
                return Err(RuntimeError::NullReference);
            }
        }
        let parent = parent
            .map(|parent| self.instance(target, parent, depth + 1))
            .transpose()?;
        if !target.set_instance_parent(new, parent) {
            return Err(RuntimeError::NullReference);
        }
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
        // Only a reference of the source generation names source state.
        if reference.generation != StoreAccess::generation(self.source) {
            return Err(RuntimeError::ReferenceLifetime);
        }
        // A flat reference is carried by the variable and path that hold its target (C1-11d).
        if matches!(reference.location, MemoryLocation::Flat { .. }) {
            *reference = self.source.path_form(reference);
        }
        match reference.location {
            MemoryLocation::Global => {
                let name = self
                    .source
                    .global_name(reference.offset as usize)
                    .ok_or(RuntimeError::NullReference)?;
                reference.offset = target
                    .ref_for_global(name)
                    .ok_or(RuntimeError::NullReference)?
                    .offset;
            }
            MemoryLocation::Instance(old) => {
                let name = self
                    .source
                    .instance_member_name(old, reference.offset as usize)
                    .ok_or(RuntimeError::NullReference)?;
                // A reference to an artifact root preserves binding, not ownership.
                let new = self.binding(target, old, depth + 1)?;
                let mapped = target
                    .ref_for_instance(new, &name)
                    .ok_or(RuntimeError::NullReference)?;
                reference.location = mapped.location;
                reference.offset = mapped.offset;
            }
            MemoryLocation::Io(_)
            | MemoryLocation::Local(_)
            | MemoryLocation::Retain
            | MemoryLocation::Flat { .. } => return Err(RuntimeError::ReferenceLifetime),
        }
        // The transferred reference names the candidate's state: this is the checked
        // rebinding of a retained reference into the new generation (spec 34 §11.3).
        reference.generation = StoreAccess::generation(&*target);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use smol_str::SmolStr;

    #[test]
    fn retained_import_marker_refusal_precedes_global_and_retained_publication() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let (declaration, name) = prepared
            .storage_declarations()
            .enumerate()
            .find(|(_, entry)| {
                entry.owner == StorageOwner::Global && entry.role == StorageRole::Variable
            })
            .map(|(index, entry)| {
                (
                    u32::try_from(index).expect("fixture declaration fits u32"),
                    SmolStr::new(&prepared.vm.strings[entry.name_idx as usize]),
                )
            })
            .expect("GPIO fixture has a global variable");
        let mut edited = prepared.storage_declaration_at(declaration as usize);
        edited.retain = 1;
        prepared.set_declaration(declaration as usize, edited);
        let mut source_data =
            EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        crate::vm::engine::test_store::set_global(
            &mut EngineState::borrow(&mut source_data, &prepared, &services::LOGICAL_ONLY),
            &name,
            Value::Int(17),
        );
        let source = EngineState::borrow(&mut source_data, &prepared, &services::LOGICAL_ONLY);
        let mut target_data =
            EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let previous = {
            let mut target =
                EngineState::borrow(&mut target_data, &prepared, &services::LOGICAL_ONLY);
            crate::vm::engine::test_store::set_global(&mut target, &name, Value::Int(99));
            crate::vm::engine::test_store::global(&target, &name)
                .expect("constructed retained global")
        };
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
        crate::vm::engine::test_store::set_global(
            &mut EngineState::borrow(&mut probe_data, &prepared, &services::LOGICAL_ONLY),
            &name,
            Value::Int(99),
        );
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
            crate::vm::engine::test_store::set_global(
                &mut EngineState::borrow(&mut candidate_data, &prepared, &services::LOGICAL_ONLY),
                &name,
                Value::Int(99),
            );
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
                crate::vm::engine::test_store::global(&candidate, &name),
                Some(Value::Int(99)),
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
        assert_eq!(
            crate::vm::engine::test_store::global(&target, &name),
            Some(previous.clone())
        );
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
        fn observe(state: &EngineState<'_>) -> Observed {
            let plant = crate::vm::engine::test_store::global_instance(state, "Plant");
            (
                crate::vm::engine::test_store::global(state, "probe_depth"),
                crate::vm::engine::test_store::instance_var(state, plant, "depth_result"),
                state.data.images.outputs.clone(),
                state.data.images.memory.clone(),
                state.data.now,
                state.data.fault.is_some(),
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
                .storage_declarations()
                .position(|entry| {
                    entry.owner == StorageOwner::Global && entry.role == StorageRole::Variable
                })
                .expect("GPIO fixture has a global variable");
            let mut edited = prepared.storage_declaration_at(declaration);
            edited.retain = 1;
            prepared.set_declaration(declaration, edited);
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
            let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
            crate::vm::engine::test_store::set_global(&mut state, "probe_depth", Value::Int(99));
            let before = observe(&state);
            match state.restart(RestartMode::Warm) {
                Ok(()) => {
                    assert_eq!(
                        crate::vm::engine::test_store::global(&state, "probe_depth"),
                        Some(Value::Int(99)),
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
                        observe(&state),
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
        let mut source = EngineState::borrow(
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
        let program = crate::vm::engine::test_store::global_instance(&source, "Plant");
        let Some(Value::Instance(counter)) =
            crate::vm::engine::test_store::instance_var(&source, program, "counter")
        else {
            panic!("counter")
        };
        // `value` starts at INT#3 in both stores; the source's 9 shows the copy.
        assert!(StoreMutate::write_instance_var(
            &mut source,
            counter,
            "value",
            Value::Int(9)
        ));
        let source = source;
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
        // On the flat store every persistent instance lies in its construction root's range, so the
        // candidate's instances are its roots', to which a warm restart maps the source's before it
        // copies (`import_retained_globals`).
        for (old, new) in source
            .data
            .construction
            .roots
            .iter()
            .zip(&target.data.construction.roots)
        {
            if let (Some(old), Some(new)) = (old, new) {
                graph.identities.insert(old.0, new.0);
            }
        }
        let copied = graph.instance(&mut target, counter, 0).unwrap();
        assert_eq!(
            crate::vm::engine::test_store::instance_var(&target, copied, "value"),
            Some(Value::Int(9))
        );
        // TIME-08 VERIFY: the copied `history` array costs 1 + 3 elements (COPY); the
        // scalar fields and the reference type queries (bookkeeping) cost nothing.
        assert!(target.data.resources.work_budget.remaining() < before);
        assert_eq!(source.data.resources.work_budget.remaining(), 0);
    }
    #[test]
    fn compact_retained_indexes_preserve_global_separation_full_ids_and_declaration_order() {
        let mut marks = LifecycleMarks::default();
        marks
            .insert((u16::MAX as u32, Some(InstanceId(u32::MAX))))
            .unwrap();
        marks.insert((1, Some(InstanceId(0)))).unwrap();
        marks.insert((0, None)).unwrap();
        marks.insert((0, Some(InstanceId(u32::MAX)))).unwrap();
        let index = index_marks(&marks).unwrap();
        assert_eq!(index.keys().copied().collect::<Vec<_>>(), [0, u32::MAX]);
        assert_eq!(index[&0], [1]);
        assert_eq!(index[&u32::MAX], [0, u16::MAX as u32]);
        assert!(
            marks.contains(&(0, None)).unwrap(),
            "indexing must not consume source marks"
        );
    }

    /// Spec 34 TIME-08 copy: an imported retained global is charged its copy once. A WSTRING of
    /// 64 units costs 64 x 2 = 128 bytes, and the same import of an empty WSTRING differs only by
    /// that copy. Charging the copy twice (once by `value`, once more after the import) would give
    /// 256.
    #[test]
    fn import_retained_global_charges_its_copy_once() {
        // A retained `probe : WSTRING`, declared beside the cell fixture's globals.
        let prepared = {
            let mut builder = crate::memory::cell_access::fixture::Builder::new();
            let wstring = builder.ty(crate::memory::cell_access::fixture::primitive(25, 0));
            builder.retained("probe", wstring);
            builder.prepare()
        };
        let imported_bytes = |value: Value| -> usize {
            let mut source_data =
                EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
            crate::vm::engine::test_store::set_global(
                &mut EngineState::borrow(&mut source_data, &prepared, &services::LOGICAL_ONLY),
                "probe",
                value,
            );
            let source = EngineState::borrow(&mut source_data, &prepared, &services::LOGICAL_ONLY);
            let mut target_data =
                EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
            target_data.construction.initialized_declarations = Default::default();
            target_data.resources.construction_bytes.set(0);
            target_data
                .resources
                .work_budget
                .reset(prepared.limits.max_work);
            let mut target =
                EngineState::borrow(&mut target_data, &prepared, &services::LOGICAL_ONLY);
            target.import_retained_globals(&source).unwrap();
            target.data.resources.construction_bytes.get()
        };
        let empty = imported_bytes(Value::WString(String::new()));
        let copied = imported_bytes(Value::WString("A".repeat(64)));
        assert_eq!(copied - empty, 128);
    }
}
