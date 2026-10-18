//! Private initializer transactions for existing FB member override destinations.
use super::*;
use crate::bytecode::{InitializationPhase, InitializationStage, InitializationTarget, TypeData};
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::{InstanceData, MemoryLocation};
use crate::value::ValueRef;

/// The value a seed copies: the configuration action's selected reference, or the
/// value the declaration's global, instance variable or lexical frame local holds
/// (specification 34 MEM-09). It is copied out of storage, as the seed needs an owned value.
fn seed_source_value<S: StoreAccess>(
    storage: &S,
    configured: Option<&ValueRef>,
    name: &str,
    owner: StorageOwner,
    instance: Option<InstanceId>,
    frame_value: Option<&Value>,
) -> Result<Value, RuntimeError> {
    if let Some(reference) = configured {
        let view = storage
            .read_reference(reference.as_view())
            .map_err(|_| RuntimeError::InvalidExecutionState)?;
        return view.materialize();
    }
    match owner {
        StorageOwner::Global => storage
            .read_global(name)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .materialize(),
        StorageOwner::Instance => instance
            .and_then(|id| storage.read_instance_var(id, name))
            .ok_or(RuntimeError::InvalidExecutionState)?
            .materialize(),
        StorageOwner::Frame => frame_value
            .cloned()
            .ok_or(RuntimeError::InvalidExecutionState),
    }
}

pub(super) struct InitializerSeed {
    pub(super) value: Value,
    pub(super) backups: Vec<(InstanceId, InstanceData)>,
    pub(super) writable_instances: InstanceSet,
}
impl InitializerSeed {
    fn empty() -> Self {
        Self {
            value: Value::Null,
            backups: Vec::new(),
            writable_instances: InstanceSet::default(),
        }
    }
}

impl EngineState<'_> {
    pub(super) fn initializer_seed(
        &mut self,
        entry: &InitializerEntry,
        instance: Option<InstanceId>,
        lexical: Option<&VmFrame>,
    ) -> Result<InitializerSeed, RuntimeError> {
        if entry.stage != InitializationStage::Explicit {
            return Ok(InitializerSeed::empty());
        }
        let prepared = self.prepared;
        let Some(declaration) = entry
            .declaration_idx
            .and_then(|id| prepared.layout.entries.get(id as usize))
        else {
            return Ok(InitializerSeed::empty());
        };
        // A configuration action selects a persistent path, which may reach a
        // scalar inside an aggregate declaration. Seed the selected destination,
        // not its containing declaration or an absent lexical instance.
        let configured = if entry.phase == InitializationPhase::Configuration
            && entry.target_kind == InitializationTarget::Reference
        {
            Some(
                self.persistent_io_reference(
                    entry
                        .target_idx
                        .ok_or(RuntimeError::InvalidExecutionState)?,
                )?,
            )
        } else {
            None
        };
        let selected_type = match &configured {
            Some(reference) => self.reference_type(reference.as_view())?,
            None => declaration.type_id,
        };
        let Some(type_id) = selected_type.and_then(|id| {
            super::super::super::type_policy::resolved_alias_type(&self.prepared.vm.types, id, 0)
        }) else {
            return Ok(InitializerSeed::empty());
        };
        if !matches!(
            self.prepared.vm.types.entries[type_id as usize].data,
            TypeData::Pou { .. }
                | TypeData::Struct { .. }
                | TypeData::Array { .. }
                | TypeData::Union { .. }
        ) {
            return Ok(InitializerSeed::empty());
        }
        let name = &self.prepared.vm.strings[declaration.name_idx as usize];
        if let Some(reference) = &configured {
            self.check_read(reference.as_view())?;
        }
        let frame_value = lexical.and_then(|frame| frame.locals.get(declaration.slot as usize));
        let value = seed_source_value(
            &*self,
            configured.as_ref(),
            name,
            declaration.owner,
            instance,
            frame_value,
        )?;
        if matches!(
            self.prepared.vm.types.entries[type_id as usize].data,
            TypeData::Pou { .. }
        ) && !matches!(value, Value::Instance(_))
        {
            return Err(RuntimeError::TypeMismatch);
        }
        // Discover ownership from the declared type, never through references or
        // interface handles. Charge the staged value before cloning its payload.
        let mut pending = PendingInstances::new(self.data.resources.bookkeeping.take_pending());
        let mut visited = self.data.resources.bookkeeping.take_visited();
        let mut backups = self.data.resources.bookkeeping.take_backups();
        let mut visited_charged = 0usize;
        let mut visited_count = 0usize;
        let mut backups_charged = 0usize;
        let value = (|| -> Result<Value, RuntimeError> {
            self.owned_value_instances(type_id, &value, &mut pending, 0)?;
            self.charge_allocation_bytes(self.value_clone_charge(&value, 0)?)?;
            while let Some(raw_id) = pending.pop() {
                let id = InstanceId(raw_id);
                // Charge the sorted growth a set growing on demand would make, so the reused
                // reserved set changes no charge (spec 34 TIME-08, census N3).
                let bytes = if visited.contains(&id) || visited_count < visited_charged {
                    0
                } else {
                    let capacity = visited_charged
                        .checked_mul(2)
                        .ok_or(RuntimeError::Overflow)?
                        .max(4);
                    visited_charged = capacity;
                    capacity
                        .checked_mul(core::mem::size_of::<u32>())
                        .ok_or(RuntimeError::Overflow)?
                };
                self.charge_sorted_growth(Ok((bytes, 0)))?;
                if !visited.insert(id)? {
                    continue;
                }
                visited_count += 1;
                if let Some(parent) = self
                    .instance_parent(id)
                    .map_err(|_| RuntimeError::InvalidExecutionState)?
                {
                    self.push_owned_instance(&mut pending, parent.0)?;
                }
                let template = self
                    .data
                    .construction
                    .instance_templates
                    .get(&id)
                    .copied()
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                for &declaration_id in self
                    .prepared
                    .declarations(StorageOwner::Instance, Some(template))
                {
                    let declaration = &self.prepared.layout.entries[declaration_id as usize];
                    if let Some(type_id) = declaration.type_id
                        && let Ok(view) =
                            self.read_slot(MemoryLocation::Instance(id), declaration.slot as usize)
                    {
                        self.owned_view_instances(type_id, view, &mut pending, 0)?;
                    }
                }
                // The members are the template's declarations: the record holds exactly these
                // (C1-9d1, R2). Each value is read through its slot.
                let count = self
                    .prepared
                    .declarations(StorageOwner::Instance, Some(template))
                    .len();
                let mut charge = count
                    .checked_mul(core::mem::size_of::<(smol_str::SmolStr, Value)>() * 4)
                    .ok_or(RuntimeError::Overflow)?;
                for slot in 0..count {
                    let view = self
                        .read_slot(MemoryLocation::Instance(id), slot)
                        .map_err(|_| RuntimeError::InvalidExecutionState)?;
                    charge = charge
                        .checked_add(self.view_clone_charge(&view)?)
                        .ok_or(RuntimeError::Overflow)?;
                }
                self.charge_allocation_bytes(charge)?;
                if backups.len() == backups_charged {
                    let capacity = backups_charged
                        .max(4)
                        .checked_mul(2)
                        .ok_or(RuntimeError::Overflow)?;
                    self.charge_allocation_bytes(
                        capacity
                            .checked_mul(core::mem::size_of::<(InstanceId, InstanceData)>())
                            .ok_or(RuntimeError::Overflow)?,
                    )?;
                    backups
                        .try_reserve_exact(capacity - backups.len())
                        .map_err(|_| {
                            super::super::super::VmTrap::BudgetExceeded.into_runtime_error()
                        })?;
                    backups_charged = capacity;
                }
                let snapshot = self
                    .snapshot_instance(id)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                backups.push((id, snapshot));
            }
            Ok(value)
        })();
        self.data.resources.bookkeeping.return_pending(pending.ids);
        match value {
            Ok(value) => Ok(InitializerSeed {
                value,
                backups,
                writable_instances: visited,
            }),
            Err(error) => {
                self.data.resources.bookkeeping.return_visited(visited);
                self.data.resources.bookkeeping.return_backups(backups);
                Err(error)
            }
        }
    }

    fn push_owned_instance(
        &self,
        pending: &mut PendingInstances,
        id: u32,
    ) -> Result<(), RuntimeError> {
        if pending.ids.len() == pending.charged_capacity {
            let capacity = pending
                .charged_capacity
                .max(4)
                .checked_mul(2)
                .ok_or(RuntimeError::Overflow)?;
            self.charge_allocation_bytes(
                capacity
                    .checked_mul(core::mem::size_of::<u32>())
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            pending
                .ids
                .try_reserve_exact(capacity - pending.ids.len())
                .map_err(|_| super::super::super::VmTrap::BudgetExceeded.into_runtime_error())?;
            pending.charged_capacity = capacity;
        }
        pending.ids.push(id);
        Ok(())
    }

    fn owned_value_instances(
        &self,
        type_id: u32,
        value: &Value,
        output: &mut PendingInstances,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let ty = self
            .prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        match (&ty.data, value) {
            (TypeData::Alias { target_type_id }, _) => {
                self.owned_value_instances(*target_type_id, value, output, depth + 1)?
            }
            (TypeData::Pou { .. }, Value::Instance(id)) => {
                self.push_owned_instance(output, id.0)?
            }
            (TypeData::Array { elem_type_id, .. }, Value::Array(array)) => {
                for value in array.elements() {
                    self.owned_value_instances(*elem_type_id, value, output, depth + 1)?;
                }
            }
            (TypeData::Struct { fields } | TypeData::Union { fields }, Value::Struct(value)) => {
                for field in fields {
                    let name = self
                        .prepared
                        .vm
                        .strings
                        .get(field.name_idx as usize)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    if let Some(value) = value.field(name) {
                        self.owned_value_instances(field.type_id, value, output, depth + 1)?;
                    }
                }
            }
            // References and interfaces do not own the objects they point at.
            _ => {}
        }
        Ok(())
    }

    /// The owned instances of one stored value of declared type `type_id` (C1-9d2a, item (e)): a `Value`
    /// is walked as its `Value`, and a cell value by its layout.
    fn owned_view_instances(
        &self,
        type_id: u32,
        view: ValueView<'_>,
        output: &mut PendingInstances,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        match view {
            ValueView::Value(value) => self.owned_value_instances(type_id, value, output, depth),
            ValueView::Cells { .. } => self.owned_cell_instances(view, output, depth),
        }
    }

    /// The owned instances of a cell value, walked by its layout as `owned_value_instances` walks a
    /// `Value` (C1-9d2a, item (e)): a function block cell is owned; an interface cell and a reference are
    /// not.
    fn owned_cell_instances(
        &self,
        view: ValueView<'_>,
        output: &mut PendingInstances,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        if let Some(instance) = view.owned_cell_instance()? {
            return self.push_owned_instance(output, instance.0);
        }
        let Some(count) = view.cell_child_count()? else {
            return Ok(());
        };
        for ordinal in 0..count {
            let child = view.cell_child(ordinal)?;
            self.owned_cell_instances(child, output, depth + 1)?;
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn restore_initializer_instances(
        &mut self,
        backups: &mut Vec<(InstanceId, InstanceData)>,
    ) {
        for (id, data) in backups.drain(..) {
            self.restore_instance(id, data);
        }
    }
}

/// The seed's pending instance identities. Growth is charged as a list growing on demand would
/// be, so taking the reserved list changes no charge (spec 34 TIME-08, census N3).
struct PendingInstances {
    ids: Vec<u32>,
    charged_capacity: usize,
}

impl PendingInstances {
    fn new(ids: Vec<u32>) -> Self {
        Self {
            ids,
            charged_capacity: 0,
        }
    }

    fn pop(&mut self) -> Option<u32> {
        self.ids.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;

    #[test]
    fn existing_instance_seed_reuses_exact_visited_identity_set() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::super::services::LOGICAL_ONLY,
        );
        let Some(Value::Instance(program)) = state.data.storage.get_global("Plant") else {
            panic!("program")
        };
        let program = *program;
        let pou = *state
            .data
            .construction
            .instance_templates
            .get(&program)
            .unwrap();
        let declaration = prepared
            .layout
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| {
                entry.owner == StorageOwner::Instance
                    && entry.owner_pou_id == Some(pou)
                    && prepared.vm.strings[entry.name_idx as usize].eq_ignore_ascii_case("counter")
            })
            .map(|(id, _)| id as u32)
            .unwrap();
        let mut entry = prepared.initializers.entries[0].clone();
        entry.stage = InitializationStage::Explicit;
        entry.declaration_idx = Some(declaration);
        let seed = state.initializer_seed(&entry, Some(program), None).unwrap();
        assert!(!seed.backups.is_empty());
        assert_eq!(
            seed.writable_instances.iter().collect::<Vec<_>>(),
            seed.backups
                .iter()
                .map(|(id, _)| *id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        for id in seed.writable_instances {
            assert!(state.data.storage.get_instance(id).is_some());
        }
    }
    #[test]
    fn aggregate_seed_keeps_nested_instance_and_restores_its_graph_on_failure() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::super::services::LOGICAL_ONLY)
                .unwrap();
        let Some(Value::Instance(program)) = data.storage.get_global("Plant") else {
            panic!("program")
        };
        let program = *program;
        let pou = *data.construction.instance_templates.get(&program).unwrap();
        let declaration = prepared
            .layout
            .entries
            .iter()
            .position(|entry| {
                entry.owner == StorageOwner::Instance
                    && entry.owner_pou_id == Some(pou)
                    && prepared.vm.strings[entry.name_idx as usize].eq_ignore_ascii_case("counter")
            })
            .unwrap();
        let name =
            prepared.vm.strings[prepared.layout.entries[declaration].name_idx as usize].clone();
        let Some(Value::Instance(counter)) = data.storage.get_instance_var(program, &name) else {
            panic!("counter")
        };
        let counter = *counter;
        let member_type = prepared.layout.entries[declaration].type_id.unwrap();
        let array_type = prepared.vm.types.entries.len() as u32;
        prepared.vm.types.entries.push(crate::bytecode::TypeEntry {
            kind: crate::bytecode::TypeKind::Array,
            name_idx: None,
            data: TypeData::Array {
                elem_type_id: member_type,
                dims: vec![(1, 1)],
            },
        });
        prepared.layout.entries[declaration].type_id = Some(array_type);
        data.storage.set_instance_var(
            program,
            name.as_str(),
            Value::Array(Box::new(crate::value::ArrayValue::from_canonical_parts(
                vec![Value::Instance(counter)],
                vec![(1, 1)],
            ))),
        );
        let original = data
            .storage
            .get_instance(counter)
            .unwrap()
            .variables
            .clone();
        let (changed_name, changed_value) = original
            .iter()
            .find_map(|(name, value)| match value {
                Value::Int(number) => Some((name.clone(), Value::Int(number.wrapping_add(1)))),
                Value::DInt(number) => Some((name.clone(), Value::DInt(number.wrapping_add(1)))),
                Value::Bool(value) => Some((name.clone(), Value::Bool(!value))),
                _ => None,
            })
            .expect("fixture mutable scalar");
        let reference = prepared.vm.refs.len() as u32;
        prepared
            .vm
            .refs
            .push(crate::vm::module::VmRef::InitializerResult {
                initializer_id: 0,
                path: vec![
                    crate::value::RefSegment::Index(crate::value::ref_indices_from_iter([1])),
                    crate::value::RefSegment::Field(changed_name),
                ],
            });
        prepared.vm.ref_types.push(None);
        let constant = prepared.vm.consts.len() as u32;
        prepared.vm.consts.push(changed_value);
        let code_offset = prepared.vm.code.len() as u32;
        prepared.vm.code.push(0x10); // LOAD_CONST
        prepared.vm.code.extend_from_slice(&constant.to_le_bytes());
        prepared.vm.code.push(0x21); // STORE_REF through the privately staged aggregate
        prepared.vm.code.extend_from_slice(&reference.to_le_bytes());
        prepared.vm.code.push(0xFF); // malformed instruction after the staged write
        prepared.initializers.entries[0].code_offset = code_offset;
        prepared.initializers.entries[0].code_length = 11;
        prepared.initializers.entries[0].stage = InitializationStage::Explicit;
        prepared.initializers.entries[0].declaration_idx = Some(declaration as u32);
        let result_ref_idx = prepared.initializers.entries[0].result_ref_idx as usize;
        *prepared
            .vm
            .ref_types
            .get_mut(result_ref_idx)
            .expect("initializer reference has a dense type slot") = Some(array_type);
        let mut state = EngineState::borrow(
            &mut data,
            &prepared,
            &super::super::super::services::LOGICAL_ONLY,
        );
        let mut entry = prepared.initializers.entries[0].clone();
        entry.stage = InitializationStage::Explicit;
        entry.declaration_idx = Some(declaration as u32);
        let mut seed = state.initializer_seed(&entry, Some(program), None).unwrap();
        let Value::Array(array) = &seed.value else {
            panic!("staged array")
        };
        assert_eq!(array.elements(), &[Value::Instance(counter)]);
        assert!(seed.writable_instances.contains(&counter));
        let member = original.get_index(0).unwrap().0.clone();
        state
            .data
            .storage
            .set_instance_var(counter, member.as_str(), Value::Null);
        state.restore_initializer_instances(&mut seed.backups);
        assert_eq!(
            state.data.storage.get_instance(counter).unwrap().variables,
            original
        );
        // The actual initializer cleanup path must restore an already-written
        // nested object after the following instruction faults.
        assert_eq!(
            state
                .evaluate_initializer(0, None, Some(program), 0)
                .unwrap_err()
                .stable_code(),
            crate::error::StableErrorCode::BytecodeInvalidOpcode
        );
        assert_eq!(
            state.data.storage.get_instance(counter).unwrap().variables,
            original
        );
        assert_eq!(state.data.storage.execution_frame_count(), 0);
        assert!(state.data.construction.initializers.is_empty());
        // An exhausted construction budget rejects before cloning or changing
        // either the aggregate destination or the owned instance graph.
        state
            .data
            .resources
            .construction_bytes
            .set(prepared.limits.max_construction_bytes);
        assert!(state.initializer_seed(&entry, Some(program), None).is_err());
        assert_eq!(
            state.data.storage.get_instance(counter).unwrap().variables,
            original
        );
        let Some(Value::Array(array)) = state.data.storage.get_instance_var(program, &name) else {
            panic!("original array")
        };
        assert_eq!(array.elements(), &[Value::Instance(counter)]);
    }
}
