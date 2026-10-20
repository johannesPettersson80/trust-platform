//! Artifact declaration/root construction and ordered lifecycle commits.
use super::super::super::construction::values::construct_intrinsic_value;
use super::*;
use crate::bytecode::{InitializationStage, InitializationTrigger};
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::MemoryLocation;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn construct_resource(
        &mut self,
        retained: Option<&EngineState<'_>>,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        // Every global holds its declared default from the moment the store is reserved (R3), and a
        // retained global gets its value when the import below copies it. A program root holds no
        // storage: it names the instance its root reserves here.
        // Reserve all persistent identities before executing any default. This does
        // not expose uninitialized members through the initializer visibility view.
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            if let Some(pou) = root.template_pou_id {
                self.data.construction.roots[index] = Some(self.reserve_root_instance(index, pou)?);
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
                if !self.set_instance_parent(derived, Some(instance)) {
                    return Err(RuntimeError::InvalidExecutionState);
                }
                self.data
                    .construction
                    .claimed_roots
                    .insert(root_index(index)?);
            } else {
                let declaration = &prepared.layout.entries[root.declaration_idx as usize];
                if declaration.role == StorageRole::ProgramRoot {
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

    /// Reserve an instance of template `pou` owned by the activation `lifetime`, or a persistent instance
    /// when `lifetime` is `None`. A persistent instance has a construction root (C1-9d2b-1, R5): the cell
    /// store places it in its root's range, so a persistent instance without a root is refused there
    /// (see [`Self::reserve_root_instance`]).
    pub(in crate::vm::engine) fn reserve_instance(
        &mut self,
        pou: u32,
        lifetime: Option<FrameId>,
    ) -> Result<InstanceId, RuntimeError> {
        self.reserve_instance_at(pou, lifetime, None)
    }

    /// Reserve the persistent instance the construction root at index `root` holds, of template `pou`
    /// (R2, R5): on the cell store it is recorded in the root's own range, with an identity from the
    /// store's one counter.
    pub(in crate::vm::engine) fn reserve_root_instance(
        &mut self,
        root: usize,
        pou: u32,
    ) -> Result<InstanceId, RuntimeError> {
        self.reserve_instance_at(pou, None, Some(root))
    }

    fn reserve_instance_at(
        &mut self,
        pou: u32,
        lifetime: Option<FrameId>,
        root: Option<usize>,
    ) -> Result<InstanceId, RuntimeError> {
        self.charge_work_units(1)?;
        self.charge_constructed_value()?;
        let name = self.prepared.vm.pou_name(pou).ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static("unknown instance template"))
        })?;
        // A persistent instance lives in its root's range. An instance with no owner and no root has no
        // range to live in, so it is refused before an identity is issued (R5): the construction paths
        // that create one are the parent of an instance that has no inheritance-parent root and a
        // retained object that no root of the replacement names.
        let persistent_cells = lifetime.is_none();
        // An instance a call owns lies on the activation stack, above the frame that runs (MEM-12;
        // C1-11c). One that an entry or construction owns outside any call stays in the `Value` half:
        // the stack reserves room for the calls of the deepest chain only (C1-19e).
        let on_stack = lifetime.is_some() && self.data.store.holds_frames();
        if persistent_cells && root.is_none() {
            return Err(RuntimeError::InitFailed {
                owner: name.into(),
                variable: smol_str::SmolStr::new_static("<instance>"),
                error: smol_str::SmolStr::new_static(
                    "a persistent instance without a construction root has no cell range",
                ),
            });
        }
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
        // A persistent instance is recorded in its table and one on the activation stack in the records
        // reserved for it, neither of which grows.
        let bytes = if persistent_cells || on_stack {
            0
        } else {
            self.data
                .storage
                .instance_insertion_demand()
                .ok_or_else(|| RuntimeError::Overflow)?
                .0
        };
        self.charge_allocation_bytes(bytes)?;
        // The lifetime check's visited set is reserved for the live instance count; it grows here,
        // once per created instance, never during a scan of an unchanged program (spec 34 MEM-09).
        let live_instances = self
            .live_instance_count()
            .checked_add(1)
            .ok_or(RuntimeError::Overflow)?;
        self.data
            .resources
            .bookkeeping
            .reserve_lifetime_visited(live_instances)?;
        let instance = match root {
            Some(index) => self.create_root_instance(index, pou)?,
            None if on_stack => self.create_call_instance(pou)?,
            None => self.try_create_instance(name)?,
        };
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
            self.remove_instance(instance);
            self.data.construction.instance_templates.remove(&instance);
            self.data.lifetimes.instance_lifetimes.remove(&instance);
            return Err(error);
        }
        // The members of a persistent instance on the cell store hold their declared defaults and
        // constants from the moment the store was constructed (R3), and writing the empty value over
        // them would erase them. An instance of the `Value` half and a per-call instance name every
        // member here; on the activation stack the members start at zero and take their declared
        // constants.
        if persistent_cells {
            return Ok(instance);
        }
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            let declaration = &prepared.layout.entries[id as usize];
            let name = self
                .prepared
                .vm
                .strings
                .get(declaration.name_idx as usize)
                .map(SmolStr::new)
                .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
            let value = declaration
                .default_const_idx
                .and_then(|index| self.prepared.vm.consts.get(index as usize))
                .cloned()
                .unwrap_or(Value::Null);
            self.write_instance_var(instance, name, value);
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
            // A typed instance with no root is owned by a live activation: both callers pass one
            // (`values.rs` under an active initializer, `entry.rs` after activating its scope). A
            // persistent instance needs its root's range in the cell store (O8), so an ownerless
            // instance is refused before anything is issued.
            None => {
                let owner = lifetime.ok_or(RuntimeError::InvalidExecutionState)?;
                self.reserve_instance(pou, Some(owner))?
            }
        };
        let mut current = instance;
        let mut current_pou = pou;
        for _ in 0..crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            let Some(parent_pou) = self.prepared.vm.parent_pou_ids.get(&current_pou).copied()
            else {
                break;
            };
            let parent = match self.instance_parent(current).ok().flatten() {
                Some(parent) => parent,
                None => {
                    let owner = self
                        .data
                        .lifetimes
                        .instance_lifetimes
                        .get(&current)
                        .flatten();
                    // A persistent instance on the cell store has no range for a parent that no
                    // inheritance-parent root names, so `reserve_instance` refuses it there (R5).
                    let parent = self.reserve_instance(parent_pou, owner)?;
                    if !self.set_instance_parent(current, Some(parent)) {
                        return Err(RuntimeError::InvalidExecutionState);
                    }
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
        if let Some(parent) = self.instance_parent(instance).ok().flatten() {
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
        let has_explicit = self.prepared.has_explicit_action(id);
        let mark_once = entry.once != InitializationOnce::None
            && (entry.stage == InitializationStage::Explicit || !has_explicit);
        let once_marker = mark_once.then_some((declaration, once_owner));
        // A frame variable's array, structure or text is staged and committed in place on the flat
        // store (C1-13).
        if let Some(frame) = frame.as_deref_mut()
            && self.run_declaration_in_place(
                id,
                declaration,
                instance,
                frame,
                depth,
                once_marker,
            )?
        {
            return Ok(());
        }
        // The default of a global's or a persistent instance variable's array, structure or text
        // is built in the variable's own bytes, so that no value of it is built (C1-22).
        if self.run_persistent_in_place(
            id,
            declaration,
            instance,
            frame.as_deref_mut(),
            depth,
            once_marker,
        )? {
            return Ok(());
        }
        let value = self.evaluate_initializer(id, frame.as_deref_mut(), instance, depth)?;
        self.commit_declaration_with_once(declaration, instance, frame, value, once_marker)
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
            instance = self.instance_parent(id).ok().flatten();
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
            let slot = declared.slot as usize;
            // A slot on the activation stack is written there (C1-11); which one is known before the
            // once marker borrows the construction state.
            let on_stack = frame
                .as_deref()
                .and_then(|frame| self.frame_slot_activation(frame, slot));
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
            match on_stack {
                Some(id) => {
                    let mut store = super::super::store_access::store_writer(
                        &mut self.data.storage,
                        &mut self.data.store,
                        self.prepared,
                    );
                    if !StoreMutate::write_slot(&mut store, MemoryLocation::Local(id), slot, value)
                    {
                        return Err(RuntimeError::InvalidExecutionState);
                    }
                }
                None => {
                    let Some(target) = frame.locals.get_mut(slot) else {
                        return Err(RuntimeError::InvalidExecutionState);
                    };
                    *target = value;
                }
            }
            if let Some(prepared_once_marker) = prepared_once_marker {
                prepared_once_marker.publish();
            }
            return Ok(());
        }
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
        // The exclusive marker reservations above borrow the lifecycle tables: only the storage
        // and store fields are free for the destination write. It goes through the store the engine
        // holds, so a cell arm receives the declared value (R2).
        let mut writer = super::super::store_access::store_writer(
            &mut self.data.storage,
            &mut self.data.store,
            self.prepared,
        );
        match declared.owner {
            StorageOwner::Global => {
                // The store refuses a value its declared type does not hold.
                if !StoreMutate::set_global(&mut writer, name, value) {
                    return Err(RuntimeError::InvalidExecutionState);
                }
            }
            StorageOwner::Instance => {
                let instance = instance.ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                if !StoreMutate::write_instance_var(&mut writer, instance, name, value) {
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

    /// Run the frame actions of `frame`'s declarations, phase by phase. `return_supplied` says
    /// whether the caller supplied the return value at slot zero, read before the frame moved its
    /// values onto the activation stack (C1-11).
    pub(in crate::vm::engine) fn initialize_frame_declarations(
        &mut self,
        _module: &VmModule,
        frame: &mut VmFrame,
        depth: u32,
        return_supplied: bool,
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
            // Admission fixes the return declaration at slot zero; its presence was read before
            // Default runs, so a newly initialized value is not taken for a caller-supplied return.
            let return_supplied = phase == InitializationPhase::Return && return_supplied;
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
        // The walk's set is one of the reserved lists, so a call's promotion
        // allocates nothing once the engine is warm (spec 34 MEM-09; C1-11c).
        let mut instances = self.data.resources.bookkeeping.take_visited();
        let promoted = self
            .collect_owned_instance_graph(value, 0, &mut instances)
            .and_then(|()| self.promote_instances(&instances, source, destination));
        self.data.resources.bookkeeping.return_visited(instances);
        promoted
    }

    /// Give each of `instances` that `source` owns to `destination`.
    fn promote_instances(
        &mut self,
        instances: &InstanceSet,
        source: FrameId,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        for instance in instances.iter() {
            if self.data.lifetimes.instance_lifetimes.get(&instance) == Some(Some(source)) {
                // An instance on the activation stack cannot outlive every activation: a persistent
                // instance needs its root's range (R5), which the compiler gives every instance a
                // global or a program holds.
                if destination.is_none() && !self.data.store.is_root_instance(instance) {
                    let owner = self
                        .data
                        .construction
                        .instance_templates
                        .get(&instance)
                        .and_then(|&pou| self.prepared.vm.pou_name(pou))
                        .unwrap_or_default();
                    return Err(RuntimeError::InitFailed {
                        owner: owner.into(),
                        variable: smol_str::SmolStr::new_static("<instance>"),
                        error: smol_str::SmolStr::new_static(
                            "a persistent instance without a construction root has no cell range",
                        ),
                    });
                }
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

    /// The owned instances of one stored member (C1-9d1, C1-9d2a): an instance, or the walk of an
    /// aggregate value. A cell aggregate is walked by its layout, its members in declaration order and
    /// its elements in flat order, as the `Value` tree is walked.
    fn collect_owned_member(
        &self,
        view: ValueView<'_>,
        depth: usize,
        visited: &mut InstanceSet,
    ) -> Result<(), RuntimeError> {
        match view {
            ValueView::Value(value) => {
                return self.collect_owned_instance_graph(value, depth, visited);
            }
            // A program root's instance is walked as the instance value it names.
            ValueView::Instance(instance) => {
                return self.collect_owned_instance_graph(
                    &Value::Instance(instance),
                    depth,
                    visited,
                );
            }
            ValueView::Cells { .. } | ValueView::Bits { .. } => {}
        }
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        if let Some(instance) = view.instance()? {
            return self.collect_owned_instance_graph(&Value::Instance(instance), depth, visited);
        }
        let Some(count) = view.cell_child_count()? else {
            return Ok(());
        };
        for ordinal in 0..count {
            let child = view.cell_child(ordinal)?;
            self.collect_owned_member(child, depth + 1, visited)?;
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
                // Conservative B-tree entry plus possible destination owner entry. The set
                // is reused from the reserved lists and charged as one that grows from empty.
                self.charge_allocation_bytes(8 * core::mem::size_of::<InstanceId>())?;
                self.charge_sorted_growth(visited.fresh_insertion_demand(*instance))?;
                visited.insert(*instance)?;
                let members = self
                    .instance_member_count(*instance)
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                if let Some(parent) = self
                    .instance_parent(*instance)
                    .map_err(|_| RuntimeError::InvalidExecutionState)?
                {
                    self.collect_owned_instance_graph(
                        &Value::Instance(parent),
                        depth + 1,
                        visited,
                    )?;
                }
                // Each member is read through its slot, in declaration order.
                for slot in 0..members {
                    let view = self
                        .read_slot(MemoryLocation::Instance(*instance), slot)
                        .map_err(|_| RuntimeError::InvalidExecutionState)?;
                    self.collect_owned_member(view, depth + 1, visited)?;
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
        let owns_any = self
            .data
            .lifetimes
            .owned_instances
            .take(owner)
            .any(|id| self.data.lifetimes.instance_lifetimes.get(&id) == Some(Some(owner)));
        if !owns_any {
            return;
        }
        let lifetimes = &self.data.lifetimes.instance_lifetimes;
        // The release goes through the store's writer, so that the store records it in the one counter
        // that issued the identities (specification 34 §4.3.1; C1-9d2b-2, R4).
        let mut writer = super::super::store_access::store_writer(
            &mut self.data.storage,
            &mut self.data.store,
            self.prepared,
        );
        StoreMutate::retain_instances(&mut writer, |id| lifetimes.get(&id) != Some(Some(owner)));
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

#[cfg(test)]
mod typed_instance_refusal_tests {
    use super::*;
    use crate::vm::PreparationLimits;

    #[test]
    fn typed_instance_without_root_or_owner_is_refused_before_it_is_issued() {
        // Specification 34 §6.2, O8: a persistent instance has its root's cell range. With no root
        // and no owning activation, the typed path is refused and issues no identity. A path that
        // created the instance unowned would leave the instance count one higher.
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            PreparationLimits::default(),
        )
        .unwrap();
        let pou = prepared
            .layout
            .entries
            .iter()
            .find(|entry| entry.owner == StorageOwner::Instance && entry.type_id.is_some())
            .and_then(|entry| entry.owner_pou_id)
            .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let instances = data.storage.instances().len();
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        assert_eq!(
            state.construct_typed_instance(pou, false),
            Err(RuntimeError::InvalidExecutionState)
        );
        assert_eq!(state.data.storage.instances().len(), instances);
    }
}
