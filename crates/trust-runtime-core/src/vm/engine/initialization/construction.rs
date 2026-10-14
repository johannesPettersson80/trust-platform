//! Artifact declaration/root construction and ordered lifecycle commits.
use super::super::super::construction::values::construct_intrinsic_value;
use super::*;
use crate::bytecode::{InitializationStage, InitializationTrigger};

impl EngineState<'_> {
    pub(in crate::vm::engine) fn construct_resource(
        &mut self,
        retained: Option<&EngineState<'_>>,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        for &id in prepared.declarations(StorageOwner::Global, None) {
            let declaration = &prepared.layout.entries[id as usize];
            let name = prepared
                .vm
                .strings
                .get(declaration.name_idx as usize)
                .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
            self.data.storage.set_global(
                name.clone(),
                self.data
                    .construction
                    .retained_globals
                    .get(&id)
                    .cloned()
                    .unwrap_or(Value::Null),
            );
        }
        // Reserve all persistent identities before executing any default. This does
        // not expose uninitialized members through the initializer visibility view.
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            if let Some(pou) = root.template_pou_id {
                self.data.construction.roots[index] = Some(self.reserve_instance(pou, None)?);
            }
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let Some(instance) = self.data.construction.roots[index] else {
                continue;
            };
            if root.is_inheritance_parent() {
                let derived = root
                    .parent_root_idx
                    .and_then(|index| {
                        self.data
                            .construction
                            .roots
                            .get(index as usize)
                            .copied()
                            .flatten()
                    })
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                self.data
                    .storage
                    .get_instance_mut(derived)
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?
                    .parent = Some(instance);
                self.data
                    .construction
                    .claimed_roots
                    .insert(root_index(index)?);
            } else {
                let declaration = &prepared.layout.entries[root.declaration_idx as usize];
                if declaration.role == StorageRole::ProgramRoot {
                    let name = prepared.vm.strings[declaration.name_idx as usize].clone();
                    self.data
                        .storage
                        .set_global(name, Value::Instance(instance));
                    self.data
                        .construction
                        .claimed_roots
                        .insert(root_index(index)?);
                }
            }
        }

        if let Some(source) = retained {
            self.import_retained_globals(source)?;
        }

        for &id in prepared.resource_actions() {
            let entry = &prepared.initializers.entries[id as usize];
            if entry
                .declaration_idx
                .is_some_and(|index| self.data.construction.retained_globals.contains_key(&index))
            {
                continue;
            }
            self.run_declaration_action(id, None, None, 0)?;
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            if root.parent_root_idx.is_none()
                && prepared.layout.entries[root.declaration_idx as usize].role
                    == StorageRole::ProgramRoot
                && let Some(instance) = self.data.construction.roots[index]
            {
                self.initialize_instance(instance, false)?;
            }
        }
        self.apply_configuration_actions()?;
        if !self.data.construction.after_restart {
            self.seed_task_inputs()?;
        }
        if !self.data.construction.after_restart {
            for &id in prepared.module_static_actions() {
                self.run_declaration_action(id, None, None, 0)?;
            }
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn reserve_instance(
        &mut self,
        pou: u32,
        lifetime: Option<FrameId>,
    ) -> Result<InstanceId, RuntimeError> {
        self.charge_work_units(1)?;
        self.charge_constructed_value()?;
        let name = self.prepared.vm.pou_name(pou).ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static("unknown instance template"))
        })?;
        // Preflight every owned default before issuing an instance identity.
        // A rejected clone must not strand an instance or its lifetime metadata.
        let prepared = self.prepared;
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            self.charge_work_units(1)?;
            if let Some(value) = prepared.layout.entries[id as usize]
                .default_const_idx
                .and_then(|index| prepared.vm.consts.get(index as usize))
            {
                self.before_value_clone(value)?;
            }
        }
        let (bytes, _) = self
            .data
            .storage
            .instance_insertion_demand()
            .ok_or_else(|| RuntimeError::Overflow)?;
        self.charge_allocation_bytes(bytes)?;
        let instance = self.data.storage.try_create_instance(name)?;
        // Each fallible metadata insertion is charged before mutation. If any
        // step fails, the newly issued instance is not exposed or stranded.
        let metadata = (|| {
            self.charge_sorted_growth(
                self.data
                    .construction
                    .instance_templates
                    .insertion_demand(instance),
            )?;
            self.data
                .construction
                .instance_templates
                .insert(instance, pou)?;
            self.charge_sorted_growth(
                self.data
                    .lifetimes
                    .instance_lifetimes
                    .insertion_demand(instance),
            )?;
            self.data
                .lifetimes
                .instance_lifetimes
                .insert(instance, lifetime)?;
            if let Some(owner) = lifetime {
                self.charge_sorted_growth(self.data.lifetimes.owned_instances.push_demand(owner))?;
                self.data.lifetimes.owned_instances.push(owner, instance)?;
            }
            Ok::<_, RuntimeError>(())
        })();
        if let Err(error) = metadata {
            self.data.storage.remove_instance(instance);
            self.data.construction.instance_templates.remove(&instance);
            self.data.lifetimes.instance_lifetimes.remove(&instance);
            return Err(error);
        }
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            let declaration = &prepared.layout.entries[id as usize];
            let name = self
                .prepared
                .vm
                .strings
                .get(declaration.name_idx as usize)
                .ok_or_else(|| RuntimeError::InvalidExecutionState)?
                .clone();
            let value = declaration
                .default_const_idx
                .and_then(|index| self.prepared.vm.consts.get(index as usize))
                .cloned()
                .unwrap_or(Value::Null);
            self.data.storage.set_instance_var(instance, name, value);
        }
        Ok(instance)
    }

    pub(in crate::vm::engine) fn construct_typed_instance(
        &mut self,
        pou: u32,
        intrinsic: bool,
    ) -> Result<InstanceId, RuntimeError> {
        let active = self.data.construction.initializers.last();
        let declaration = active.and_then(|active| active.entry.declaration_idx);
        let parent = active.and_then(|active| active.instance);
        let candidates = declaration.map_or(&[][..], |id| self.prepared.root_candidates(id, pou));
        let mut reserved = None;
        for &index in candidates {
            let key = root_index(index)?;
            let root = &self.prepared.roots.entries[index];
            let root_parent = root.parent_root_idx.and_then(|index| {
                self.data
                    .construction
                    .roots
                    .get(index as usize)
                    .copied()
                    .flatten()
            });
            if root_parent == parent && !self.data.construction.claimed_roots.contains(&key) {
                reserved = Some(index);
                break;
            }
        }
        let lifetime = active
            .map(|active| active.result)
            .or_else(|| self.data.storage.last_execution_activation());
        let instance = match reserved {
            Some(index) => {
                self.data
                    .construction
                    .claimed_roots
                    .insert(root_index(index)?);
                self.data.construction.roots[index]
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?
            }
            None => self.reserve_instance(pou, lifetime)?,
        };
        let mut current = instance;
        let mut current_pou = pou;
        for _ in 0..crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            let Some(parent_pou) = self.prepared.vm.parent_pou_ids.get(&current_pou).copied()
            else {
                break;
            };
            let parent = match self
                .data
                .storage
                .get_instance(current)
                .and_then(|instance| instance.parent)
            {
                Some(parent) => parent,
                None => {
                    let owner = self
                        .data
                        .lifetimes
                        .instance_lifetimes
                        .get(&current)
                        .flatten();
                    let parent = self.reserve_instance(parent_pou, owner)?;
                    self.data
                        .storage
                        .get_instance_mut(current)
                        .ok_or_else(|| RuntimeError::InvalidExecutionState)?
                        .parent = Some(parent);
                    parent
                }
            };
            current = parent;
            current_pou = parent_pou;
        }
        if self.prepared.vm.parent_pou_ids.contains_key(&current_pou) {
            return Err(RuntimeError::TypeMismatch);
        }
        self.initialize_instance(instance, intrinsic)?;
        Ok(instance)
    }

    pub(in crate::vm::engine) fn initialize_instance(
        &mut self,
        instance: InstanceId,
        intrinsic: bool,
    ) -> Result<(), RuntimeError> {
        if self
            .data
            .construction
            .initialized_instances
            .contains(&instance)
        {
            return Ok(());
        }
        if let Some(parent) = self
            .data
            .storage
            .get_instance(instance)
            .and_then(|instance| instance.parent)
        {
            self.initialize_instance(parent, intrinsic)?;
        }
        let pou = *self
            .data
            .construction
            .instance_templates
            .get(&instance)
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        let prepared = self.prepared;
        // Native-state/edge declarations without actions still own real fixed slots.
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            let declaration = &prepared.layout.entries[id as usize];
            if declaration.owner_pou_id == Some(pou)
                && matches!(
                    declaration.role,
                    StorageRole::NativeState | StorageRole::EdgePhase
                )
            {
                let marker = (id, Some(instance));
                let plan = self
                    .data
                    .construction
                    .initialized_declarations
                    .insertion_plan(marker)?;
                self.charge_sorted_growth(Ok(plan.demand()))?;
                let prepared_marker = self
                    .data
                    .construction
                    .initialized_declarations
                    .prepare_insert(plan)?;
                prepared_marker.publish();
            }
        }
        for &id in prepared.instance_actions(pou) {
            let entry = &prepared.initializers.entries[id as usize];
            if intrinsic {
                if entry.stage == InitializationStage::Explicit {
                    continue;
                }
                let declaration = entry
                    .declaration_idx
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                let type_id = prepared.layout.entries[declaration as usize]
                    .type_id
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                let prepared = self.prepared;
                let value = construct_intrinsic_value(&prepared.vm.types, self, type_id)?;
                self.commit_declaration(declaration, Some(instance), None, value)?;
            } else {
                let depth = self
                    .data
                    .construction
                    .initializers
                    .last()
                    .map_or(0, |active| active.depth.saturating_add(1));
                self.run_declaration_action(id, Some(instance), None, depth)?;
            }
        }
        self.charge_sorted_growth(
            self.data
                .construction
                .initialized_instances
                .insertion_demand(instance),
        )?;
        self.data
            .construction
            .initialized_instances
            .insert(instance)?;
        Ok(())
    }

    pub(in crate::vm::engine) fn run_declaration_action(
        &mut self,
        id: u32,
        instance: Option<InstanceId>,
        mut frame: Option<&mut VmFrame>,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::DeclarationAction,
        );
        let Some(entry) = self.prepared.initializers.entries.get(id as usize).cloned() else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let Some(declaration) = entry.declaration_idx else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let instance = self.physical_declaration_instance(declaration, instance)?;
        let once_owner = if entry.once == InitializationOnce::Instance {
            instance
        } else {
            None
        };
        if entry.once != InitializationOnce::None {
            let marker = (declaration, once_owner);
            if self.data.construction.once.contains(&marker)? {
                return Ok(());
            }
        }
        let value = self.evaluate_initializer(id, frame.as_deref_mut(), instance, depth)?;
        let has_explicit = self.prepared.has_explicit_action(id);
        let mark_once = entry.once != InitializationOnce::None
            && (entry.stage == InitializationStage::Explicit || !has_explicit);
        self.commit_declaration_with_once(
            declaration,
            instance,
            frame,
            value,
            mark_once.then_some((declaration, once_owner)),
        )
    }

    fn physical_declaration_instance(
        &self,
        declaration: u32,
        mut instance: Option<InstanceId>,
    ) -> Result<Option<InstanceId>, RuntimeError> {
        let Some(entry) = self.prepared.layout.entries.get(declaration as usize) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if entry.owner != StorageOwner::Instance {
            return Ok(instance);
        }
        let owner = entry
            .owner_pou_id
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        let template = self
            .prepared
            .method_owners
            .get(&owner)
            .copied()
            .unwrap_or(owner);
        for _ in 0..=self.prepared.vm.pou_by_id.len() {
            let id = instance.ok_or_else(|| RuntimeError::InvalidExecutionState)?;
            if self.data.construction.instance_templates.get(&id) == Some(&template) {
                return Ok(Some(id));
            }
            instance = self
                .data
                .storage
                .get_instance(id)
                .and_then(|value| value.parent);
        }
        Err(RuntimeError::InvalidExecutionState)
    }

    pub(in crate::vm::engine) fn commit_declaration(
        &mut self,
        declaration: u32,
        instance: Option<InstanceId>,
        frame: Option<&mut VmFrame>,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.commit_declaration_with_once(declaration, instance, frame, value, None)
    }

    fn commit_declaration_with_once(
        &mut self,
        declaration: u32,
        instance: Option<InstanceId>,
        frame: Option<&mut VmFrame>,
        value: Value,
        once_marker: Option<(u32, Option<InstanceId>)>,
    ) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::InitializerCommit,
        );
        let Some(declared) = self.prepared.layout.entries.get(declaration as usize) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let Some(name) = self.prepared.vm.strings.get(declared.name_idx as usize) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        // Frame visibility is activation-local. Persistent initialized markers
        // are consulted for instance storage, never for local declaration IDs.
        if declared.owner == StorageOwner::Frame {
            self.check_value_lifetime(&value, frame.as_ref().and_then(|frame| frame.activation))?;
            let prepared_once_marker = if let Some(marker) = once_marker {
                let plan = self.data.construction.once.insertion_plan(marker)?;
                self.charge_allocation_bytes(plan.demand().0)?;
                Some(self.data.construction.once.prepare_insert(plan)?)
            } else {
                None
            };
            let Some(frame) = frame else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            let Some(slot) = frame.locals.get_mut(declared.slot as usize) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            *slot = value;
            if let Some(prepared_once_marker) = prepared_once_marker {
                prepared_once_marker.publish();
            }
            return Ok(());
        }
        // Persistent destinations still retain their name before lifetime checks.
        let name = name.clone();
        let destination = if declared.owner == StorageOwner::Instance {
            instance.and_then(|id| self.data.lifetimes.instance_lifetimes.get(&id).flatten())
        } else {
            None
        };
        self.check_value_lifetime(&value, destination)?;
        let initialized_marker = (declaration, instance);
        // Admit every marker before reserving either directory or publishing the
        // destination value. Keep once-marker accounting first as before, with
        // lifetime validation now preceding both marker admissions.
        let once_plan = once_marker
            .map(|marker| self.data.construction.once.insertion_plan(marker))
            .transpose()?;
        if let Some(plan) = once_plan {
            self.charge_allocation_bytes(plan.demand().0)?;
        }
        let initialized_plan = self
            .data
            .construction
            .initialized_declarations
            .insertion_plan(initialized_marker)?;
        self.charge_allocation_bytes(initialized_plan.demand().0)?;
        let prepared_once_marker = if let Some(plan) = once_plan {
            Some(self.data.construction.once.prepare_insert(plan)?)
        } else {
            None
        };
        let prepared_marker = self
            .data
            .construction
            .initialized_declarations
            .prepare_insert(initialized_plan)?;
        match declared.owner {
            StorageOwner::Global => self.data.storage.set_global(name, value),
            StorageOwner::Instance => {
                if !self.data.storage.set_instance_var(
                    instance.ok_or_else(|| RuntimeError::InvalidExecutionState)?,
                    name,
                    value,
                ) {
                    return Err(RuntimeError::InvalidExecutionState);
                }
            }
            StorageOwner::Frame => return Err(RuntimeError::InvalidExecutionState),
        }
        prepared_marker.publish();
        if let Some(prepared_once_marker) = prepared_once_marker {
            prepared_once_marker.publish();
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn initialize_frame_declarations(
        &mut self,
        _module: &VmModule,
        frame: &mut VmFrame,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        let Some(owner) = frame.pou_id else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let prepared = self.prepared;
        let mut frame_plan_index = None;
        let mut frame_plan_resolved = false;
        for phase in [
            InitializationPhase::Parameter,
            InitializationPhase::Return,
            InitializationPhase::Static,
            InitializationPhase::Frame,
        ] {
            let trigger =
                if phase == InitializationPhase::Static && self.data.construction.after_restart {
                    InitializationTrigger::AfterRestart
                } else {
                    InitializationTrigger::Ordinary
                };
            if !frame_plan_resolved {
                frame_plan_index = prepared.frame_action_plan_index(owner);
                frame_plan_resolved = true;
            }
            let actions = frame_plan_index.map_or(&[][..], |index| {
                prepared.frame_actions_at(index, phase, trigger)
            });
            #[cfg(all(feature = "std", feature = "test-support"))]
            let mut query_log = crate::vm::diagnostic_timing::begin_frame_query(
                owner,
                phase,
                trigger,
                actions.len(),
            );
            // Admission fixes the return declaration at slot zero. Snapshot its
            // presence before Default runs: rereading it before Explicit would
            // mistake the newly initialized value for a caller-supplied return.
            let return_supplied = phase == InitializationPhase::Return
                && frame
                    .locals
                    .first()
                    .is_some_and(|value| !matches!(value, Value::Null));
            for &id in actions {
                let supplied = prepared.initializers.entries[id as usize]
                    .declaration_idx
                    .is_some_and(|declaration| match phase {
                        InitializationPhase::Parameter => {
                            let slot = prepared.layout.entries[declaration as usize].slot as usize;
                            frame.parameter_values_present.get(slot) == Some(&true)
                        }
                        InitializationPhase::Return => return_supplied,
                        _ => false,
                    });
                if supplied {
                    #[cfg(all(feature = "std", feature = "test-support"))]
                    query_log.supplied_skip();
                    continue;
                }
                #[cfg(all(feature = "std", feature = "test-support"))]
                query_log.action_attempt();
                if !self.apply_direct_frame_default(id, frame, depth)? {
                    self.run_declaration_action(id, frame.runtime_instance, Some(frame), depth)?;
                }
            }
            #[cfg(all(feature = "std", feature = "test-support"))]
            query_log.complete();
        }
        Ok(())
    }
}

#[cfg(test)]
mod declaration_commit_tests;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn apply_configuration_actions(
        &mut self,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        for &id in prepared.configuration_actions() {
            let entry = &prepared.initializers.entries[id as usize];
            let value = self.evaluate_initializer(id, None, None, 0)?;
            self.commit_configuration_target(entry, value)?;
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn promote_constructed_instances(
        &mut self,
        value: &Value,
        source: FrameId,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        // Traverse borrowed payloads first. Cloning aggregate fields here would
        // allocate before charging and duplicate large arrays merely to find IDs.
        let mut instances = InstanceSet::default();
        self.collect_owned_instance_graph(value, 0, &mut instances)?;
        for instance in instances {
            if self.data.lifetimes.instance_lifetimes.get(&instance) == Some(Some(source)) {
                // Reserve the new owner's list before changing lifetime. Rollback
                // restores existing lifetimes without allocating or using fuel.
                self.charge_sorted_growth(
                    self.data
                        .lifetimes
                        .instance_lifetimes
                        .insertion_demand(instance),
                )?;
                if let Some(owner) = destination {
                    self.charge_sorted_growth(
                        self.data.lifetimes.owned_instances.push_demand(owner),
                    )?;
                    self.data.lifetimes.owned_instances.push(owner, instance)?;
                }
                self.data
                    .lifetimes
                    .instance_lifetimes
                    .restore(instance, destination);
            }
        }
        Ok(())
    }

    fn collect_owned_instance_graph(
        &self,
        value: &Value,
        depth: usize,
        visited: &mut InstanceSet,
    ) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match value {
            Value::Instance(instance) => {
                if visited.contains(instance) {
                    return Ok(());
                }
                // Conservative B-tree entry plus possible destination owner entry.
                self.charge_allocation_bytes(8 * core::mem::size_of::<InstanceId>())?;
                self.charge_sorted_growth(visited.insertion_demand(*instance))?;
                visited.insert(*instance)?;
                let data = self
                    .data
                    .storage
                    .get_instance(*instance)
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                if let Some(parent) = data.parent {
                    self.collect_owned_instance_graph(
                        &Value::Instance(parent),
                        depth + 1,
                        visited,
                    )?;
                }
                for value in data.variables.values() {
                    self.collect_owned_instance_graph(value, depth + 1, visited)?;
                }
            }
            Value::Array(array) => {
                for value in array.elements() {
                    self.collect_owned_instance_graph(value, depth + 1, visited)?;
                }
            }
            Value::Struct(structure) => {
                for value in structure.fields().values() {
                    self.collect_owned_instance_graph(value, depth + 1, visited)?;
                }
            }
            // References alias storage; they never transfer ownership of it.
            _ => {}
        }
        Ok(())
    }

    /// Retirement is bookkeeping (TIME-08) and charges no work. Its retained-scan
    /// demand check stays, so an unrepresentable scan is still refused.
    pub(in crate::vm::engine) fn remove_owned_instances(
        &mut self,
        owner: FrameId,
    ) -> Result<(), RuntimeError> {
        self.remove_owned_instances_unchecked(owner);
        Ok(())
    }

    // Terminal cleanup must complete even after a refused check. Check at the
    // fallible boundary before calling this; preserve the original fault.
    pub(in crate::vm::engine) fn remove_owned_instances_unchecked(&mut self, owner: FrameId) {
        let mut removed = self.data.lifetimes.owned_instances.take(owner);
        if !removed.any(|id| self.data.lifetimes.instance_lifetimes.get(&id) == Some(Some(owner))) {
            return;
        }
        let lifetimes = &self.data.lifetimes.instance_lifetimes;
        self.data
            .storage
            .retain_instances(|id| lifetimes.get(&id) != Some(Some(owner)));
        self.data
            .construction
            .initialized_declarations
            .retain(|(_, id)| id.is_none_or(|id| lifetimes.get(&id) != Some(Some(owner))));
        self.data
            .construction
            .once
            .retain(|(_, id)| id.is_none_or(|id| lifetimes.get(&id) != Some(Some(owner))));
        // The same live-owner predicate governs values and side metadata. One
        // retain pass per collection avoids repeated shifting during retirement.
        self.data
            .construction
            .instance_templates
            .retain(|id| lifetimes.get(&id) != Some(Some(owner)));
        self.data
            .construction
            .initialized_instances
            .retain(|id| lifetimes.get(&id) != Some(Some(owner)));
        self.data
            .lifetimes
            .instance_lifetimes
            .retain(|_, lifetime| lifetime != Some(owner));
    }
}
