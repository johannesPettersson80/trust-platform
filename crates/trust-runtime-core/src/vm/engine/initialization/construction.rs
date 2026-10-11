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
            self.charge_work_units(1)?;
            let name = prepared
                .vm
                .strings
                .get(declaration.name_idx as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            self.storage.set_global(
                name.clone(),
                self.construction
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
                self.construction.roots[index] = Some(self.reserve_instance(pou, None)?);
            }
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let Some(instance) = self.construction.roots[index] else {
                continue;
            };
            if root.is_inheritance_parent() {
                let derived = root
                    .parent_root_idx
                    .and_then(|index| {
                        self.construction
                            .roots
                            .get(index as usize)
                            .copied()
                            .flatten()
                    })
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                self.storage
                    .get_instance_mut(derived)
                    .ok_or(RuntimeError::InvalidExecutionState)?
                    .parent = Some(instance);
                self.construction.claimed_roots.insert(root_index(index)?);
            } else {
                let declaration = &prepared.layout.entries[root.declaration_idx as usize];
                if declaration.role == StorageRole::ProgramRoot {
                    let name = prepared.vm.strings[declaration.name_idx as usize].clone();
                    self.storage.set_global(name, Value::Instance(instance));
                    self.construction.claimed_roots.insert(root_index(index)?);
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
                .is_some_and(|index| self.construction.retained_globals.contains_key(&index))
            {
                continue;
            }
            self.run_declaration_action(id, None, None, 0)?;
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            if root.parent_root_idx.is_none()
                && prepared.layout.entries[root.declaration_idx as usize].role
                    == StorageRole::ProgramRoot
            {
                if let Some(instance) = self.construction.roots[index] {
                    self.initialize_instance(instance, false)?;
                }
            }
        }
        self.apply_configuration_actions()?;
        if !self.construction.after_restart {
            self.seed_task_inputs()?;
        }
        if !self.construction.after_restart {
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
        let (bytes, moved) = self
            .storage
            .instance_insertion_demand()
            .ok_or(RuntimeError::Overflow)?;
        self.charge_allocation_bytes(bytes)?;
        self.charge_work_units(moved)?;
        let instance = self.storage.try_create_instance(name)?;
        // Each fallible metadata insertion is charged before mutation. If any
        // step fails, the newly issued instance is not exposed or stranded.
        let metadata = (|| {
            self.charge_sorted_growth(
                self.construction
                    .instance_templates
                    .insertion_demand(instance),
            )?;
            self.construction.instance_templates.insert(instance, pou)?;
            self.charge_sorted_growth(
                self.lifetimes.instance_lifetimes.insertion_demand(instance),
            )?;
            self.lifetimes
                .instance_lifetimes
                .insert(instance, lifetime)?;
            if let Some(owner) = lifetime {
                self.charge_sorted_growth(self.lifetimes.owned_instances.push_demand(owner))?;
                self.lifetimes.owned_instances.push(owner, instance)?;
            }
            Ok::<_, RuntimeError>(())
        })();
        if let Err(error) = metadata {
            self.storage.remove_instance(instance);
            self.construction.instance_templates.remove(&instance);
            self.lifetimes.instance_lifetimes.remove(&instance);
            return Err(error);
        }
        let prepared = self.prepared;
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            self.charge_work_units(1)?;
            let declaration = &prepared.layout.entries[id as usize];
            let name = self
                .prepared
                .vm
                .strings
                .get(declaration.name_idx as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?
                .clone();
            let value = declaration
                .default_const_idx
                .and_then(|index| self.prepared.vm.consts.get(index as usize))
                .cloned()
                .unwrap_or(Value::Null);
            self.storage.set_instance_var(instance, name, value);
        }
        Ok(instance)
    }

    pub(in crate::vm::engine) fn construct_typed_instance(
        &mut self,
        pou: u32,
        intrinsic: bool,
    ) -> Result<InstanceId, RuntimeError> {
        let active = self.construction.initializers.last();
        let declaration = active.and_then(|active| active.entry.declaration_idx);
        let parent = active.and_then(|active| active.instance);
        let candidates = declaration.map_or(&[][..], |id| self.prepared.root_candidates(id, pou));
        self.charge_work_units(self.prepared.lookup_work().saturating_add(candidates.len()))?;
        let mut reserved = None;
        for &index in candidates {
            let key = root_index(index)?;
            let root = &self.prepared.roots.entries[index];
            let root_parent = root.parent_root_idx.and_then(|index| {
                self.construction
                    .roots
                    .get(index as usize)
                    .copied()
                    .flatten()
            });
            if root_parent == parent && !self.construction.claimed_roots.contains(&key) {
                reserved = Some(index);
                break;
            }
        }
        let lifetime = active
            .map(|active| active.result)
            .or_else(|| self.lifetimes.live_activations.last().copied());
        let instance = match reserved {
            Some(index) => {
                self.construction.claimed_roots.insert(root_index(index)?);
                self.construction.roots[index].ok_or(RuntimeError::InvalidExecutionState)?
            }
            None => self.reserve_instance(pou, lifetime)?,
        };
        let mut current = instance;
        let mut current_pou = pou;
        for _ in 0..self.prepared.limits.max_call_depth {
            let Some(parent_pou) = self.prepared.vm.parent_pou_ids.get(&current_pou).copied()
            else {
                break;
            };
            let parent = match self
                .storage
                .get_instance(current)
                .and_then(|instance| instance.parent)
            {
                Some(parent) => parent,
                None => {
                    let owner = self.lifetimes.instance_lifetimes.get(&current).flatten();
                    let parent = self.reserve_instance(parent_pou, owner)?;
                    self.storage
                        .get_instance_mut(current)
                        .ok_or(RuntimeError::InvalidExecutionState)?
                        .parent = Some(parent);
                    parent
                }
            };
            current = parent;
            current_pou = parent_pou;
        }
        if self.prepared.vm.parent_pou_ids.contains_key(&current_pou) {
            return Err(super::super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        self.initialize_instance(instance, intrinsic)?;
        Ok(instance)
    }

    pub(in crate::vm::engine) fn initialize_instance(
        &mut self,
        instance: InstanceId,
        intrinsic: bool,
    ) -> Result<(), RuntimeError> {
        if self.construction.initialized_instances.contains(&instance) {
            return Ok(());
        }
        self.charge_work_units(1)?;
        if let Some(parent) = self
            .storage
            .get_instance(instance)
            .and_then(|instance| instance.parent)
        {
            self.initialize_instance(parent, intrinsic)?;
        }
        let pou = *self
            .construction
            .instance_templates
            .get(&instance)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let prepared = self.prepared;
        // Native-state/edge declarations without actions still own real fixed slots.
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            let declaration = &prepared.layout.entries[id as usize];
            self.charge_work_units(1)?;
            if declaration.owner_pou_id == Some(pou)
                && matches!(
                    declaration.role,
                    StorageRole::NativeState | StorageRole::EdgePhase
                )
            {
                self.construction
                    .initialized_declarations
                    .insert((id, Some(instance)))?;
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
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let type_id = prepared.layout.entries[declaration as usize]
                    .type_id
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let value = construct_intrinsic_value(self, type_id)?;
                self.commit_declaration(declaration, Some(instance), None, value)?;
            } else {
                let depth = self
                    .construction
                    .initializers
                    .last()
                    .map_or(0, |active| active.depth.saturating_add(1));
                self.run_declaration_action(id, Some(instance), None, depth)?;
            }
        }
        self.charge_sorted_growth(
            self.construction
                .initialized_instances
                .insertion_demand(instance),
        )?;
        self.construction.initialized_instances.insert(instance)?;
        Ok(())
    }

    pub(in crate::vm::engine) fn run_declaration_action(
        &mut self,
        id: u32,
        instance: Option<InstanceId>,
        mut frame: Option<&mut VmFrame>,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        let entry = self
            .prepared
            .initializers
            .entries
            .get(id as usize)
            .cloned()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let declaration = entry
            .declaration_idx
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let instance = self.physical_declaration_instance(declaration, instance)?;
        let once_owner = if entry.once == InitializationOnce::Instance {
            instance
        } else {
            None
        };
        if entry.once != InitializationOnce::None
            && self
                .construction
                .once
                .contains(&(declaration, once_owner))?
        {
            return Ok(());
        }
        let value = self.evaluate_initializer(id, frame.as_deref_mut(), instance, depth)?;
        let has_explicit = self.prepared.has_explicit_action(id);
        let mark_once = entry.once != InitializationOnce::None
            && (entry.stage == InitializationStage::Explicit || !has_explicit);
        if mark_once {
            self.charge_sorted_growth(
                self.construction
                    .once
                    .insertion_demand((declaration, once_owner)),
            )?;
            self.construction
                .once
                .prepare_insert((declaration, once_owner))?;
        }
        self.commit_declaration(declaration, instance, frame, value)?;
        if mark_once {
            self.construction.once.insert((declaration, once_owner))?;
        }
        Ok(())
    }

    fn physical_declaration_instance(
        &self,
        declaration: u32,
        mut instance: Option<InstanceId>,
    ) -> Result<Option<InstanceId>, RuntimeError> {
        let entry = self
            .prepared
            .layout
            .entries
            .get(declaration as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if entry.owner != StorageOwner::Instance {
            return Ok(instance);
        }
        let owner = entry
            .owner_pou_id
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let template = self
            .prepared
            .method_owners
            .get(&owner)
            .copied()
            .unwrap_or(owner);
        for _ in 0..=self.prepared.vm.pou_by_id.len() {
            let id = instance.ok_or(RuntimeError::InvalidExecutionState)?;
            if self.construction.instance_templates.get(&id) == Some(&template) {
                return Ok(Some(id));
            }
            instance = self.storage.get_instance(id).and_then(|value| value.parent);
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
        let declared = self
            .prepared
            .layout
            .entries
            .get(declaration as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let name = self
            .prepared
            .vm
            .strings
            .get(declared.name_idx as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .clone();
        let destination = match declared.owner {
            StorageOwner::Frame => frame.as_ref().and_then(|frame| frame.activation),
            StorageOwner::Instance => {
                instance.and_then(|id| self.lifetimes.instance_lifetimes.get(&id).flatten())
            }
            StorageOwner::Global => None,
        };
        self.check_value_lifetime(&value, destination)?;
        self.charge_sorted_growth(
            self.construction
                .initialized_declarations
                .insertion_demand((declaration, instance)),
        )?;
        self.construction
            .initialized_declarations
            .prepare_insert((declaration, instance))?;
        match declared.owner {
            StorageOwner::Global => self.storage.set_global(name, value),
            StorageOwner::Instance => {
                if !self.storage.set_instance_var(
                    instance.ok_or(RuntimeError::InvalidExecutionState)?,
                    name,
                    value,
                ) {
                    return Err(RuntimeError::InvalidExecutionState);
                }
            }
            StorageOwner::Frame => {
                *frame
                    .ok_or(RuntimeError::InvalidExecutionState)?
                    .locals
                    .get_mut(declared.slot as usize)
                    .ok_or(RuntimeError::InvalidExecutionState)? = value;
            }
        }
        self.construction
            .initialized_declarations
            .insert((declaration, instance))?;
        Ok(())
    }

    pub(in crate::vm::engine) fn initialize_frame_declarations(
        &mut self,
        _module: &VmModule,
        frame: &mut VmFrame,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        let owner = frame.pou_id.ok_or(RuntimeError::InvalidExecutionState)?;
        let prepared = self.prepared;
        for phase in [
            InitializationPhase::Parameter,
            InitializationPhase::Return,
            InitializationPhase::Static,
            InitializationPhase::Frame,
        ] {
            let trigger = if phase == InitializationPhase::Static && self.construction.after_restart
            {
                InitializationTrigger::AfterRestart
            } else {
                InitializationTrigger::Ordinary
            };
            self.charge_work_units(prepared.lookup_work())?;
            let actions = prepared.actions(Some(owner), phase, trigger);
            let mut supplied = BTreeSet::new();
            if matches!(
                phase,
                InitializationPhase::Parameter | InitializationPhase::Return
            ) {
                for &id in actions {
                    if let Some(declaration) =
                        prepared.initializers.entries[id as usize].declaration_idx
                    {
                        let slot = prepared.layout.entries[declaration as usize].slot as usize;
                        if (phase == InitializationPhase::Parameter
                            && frame.parameter_values_present.get(slot) == Some(&true))
                            || (phase == InitializationPhase::Return
                                && frame
                                    .locals
                                    .get(slot)
                                    .is_some_and(|value| !matches!(value, Value::Null)))
                        {
                            supplied.insert(declaration);
                        }
                    }
                }
            }
            for &id in actions {
                if prepared.initializers.entries[id as usize]
                    .declaration_idx
                    .is_some_and(|declaration| supplied.contains(&declaration))
                {
                    continue;
                }
                self.run_declaration_action(id, frame.runtime_instance, Some(frame), depth)?;
            }
        }
        Ok(())
    }
}

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
            if self.lifetimes.instance_lifetimes.get(&instance) == Some(Some(source)) {
                // Reserve the new owner's list before changing lifetime. Rollback
                // restores existing lifetimes without allocating or using fuel.
                self.charge_sorted_growth(
                    self.lifetimes.instance_lifetimes.insertion_demand(instance),
                )?;
                if let Some(owner) = destination {
                    self.charge_sorted_growth(self.lifetimes.owned_instances.push_demand(owner))?;
                    self.lifetimes.owned_instances.push(owner, instance)?;
                }
                self.lifetimes
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
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth.min(128) {
            return Err(super::super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        match value {
            Value::Instance(instance) => {
                self.charge_work_units(visited.len().max(1).ilog2() as usize + 1)?;
                if visited.contains(instance) {
                    return Ok(());
                }
                // Conservative B-tree entry plus possible destination owner entry.
                self.charge_allocation_bytes(8 * core::mem::size_of::<InstanceId>())?;
                self.charge_sorted_growth(visited.insertion_demand(*instance))?;
                visited.insert(*instance)?;
                let data = self
                    .storage
                    .get_instance(*instance)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
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

    pub(in crate::vm::engine) fn charge_owned_retirement(
        &self,
        owner: FrameId,
    ) -> Result<(), RuntimeError> {
        let owned = self.lifetimes.owned_instances.count(owner);
        if owned == 0 {
            return Ok(());
        }
        let entries = self
            .storage
            .instances()
            .len()
            .saturating_add(self.construction.initialized_declarations.len())
            .saturating_add(self.construction.once.len())
            .saturating_add(self.construction.instance_templates.len())
            .saturating_add(self.construction.initialized_instances.len())
            .saturating_add(self.lifetimes.instance_lifetimes.len())
            .saturating_add(self.lifetimes.owned_instances.len());
        let lookup = 12 * (self.lifetimes.instance_lifetimes.len().max(1).ilog2() as usize + 1);
        self.charge_work_units(
            entries
                .saturating_add(owned.saturating_mul(4))
                .saturating_mul(lookup),
        )
    }

    pub(in crate::vm::engine) fn remove_owned_instances(
        &mut self,
        owner: FrameId,
    ) -> Result<(), RuntimeError> {
        let charged = self.charge_owned_retirement(owner);
        self.remove_owned_instances_unchecked(owner);
        charged
    }

    // Terminal cleanup must complete even after work/deadline failure. Charge at
    // the fallible boundary before calling this; preserve the original fault.
    pub(in crate::vm::engine) fn remove_owned_instances_unchecked(&mut self, owner: FrameId) {
        let mut removed = self.lifetimes.owned_instances.take(owner);
        if !removed.any(|id| self.lifetimes.instance_lifetimes.get(&id) == Some(Some(owner))) {
            return;
        }
        let lifetimes = &self.lifetimes.instance_lifetimes;
        self.storage
            .retain_instances(|id| lifetimes.get(&id) != Some(Some(owner)));
        self.construction
            .initialized_declarations
            .retain(|(_, id)| id.is_none_or(|id| lifetimes.get(&id) != Some(Some(owner))));
        self.construction
            .once
            .retain(|(_, id)| id.is_none_or(|id| lifetimes.get(&id) != Some(Some(owner))));
        // The same live-owner predicate governs values and side metadata. One
        // retain pass per collection avoids repeated shifting during retirement.
        self.construction
            .instance_templates
            .retain(|id| lifetimes.get(&id) != Some(Some(owner)));
        self.construction
            .initialized_instances
            .retain(|id| lifetimes.get(&id) != Some(Some(owner)));
        self.lifetimes
            .instance_lifetimes
            .retain(|_, lifetime| lifetime != Some(owner));
    }
}
