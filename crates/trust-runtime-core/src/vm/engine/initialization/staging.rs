//! Private initializer transactions for existing FB member override destinations.
use super::*;
use crate::bytecode::{InitializationPhase, InitializationStage, InitializationTarget, TypeData};
use crate::memory::InstanceData;

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
        let value = if let Some(reference) = &configured {
            self.check_read(reference.as_view())?;
            self.data.storage.read_by_ref_ref(reference)
        } else {
            match declaration.owner {
                StorageOwner::Global => self.data.storage.get_global(name),
                StorageOwner::Instance => {
                    instance.and_then(|id| self.data.storage.get_instance_var(id, name))
                }
                StorageOwner::Frame => {
                    lexical.and_then(|frame| frame.locals.get(declaration.slot as usize))
                }
            }
        }
        .ok_or(RuntimeError::InvalidExecutionState)?;
        if matches!(
            self.prepared.vm.types.entries[type_id as usize].data,
            TypeData::Pou { .. }
        ) && !matches!(value, Value::Instance(_))
        {
            return Err(RuntimeError::TypeMismatch);
        }
        // Discover ownership from the declared type, never through references or
        // interface handles. Charge the staged value before cloning its payload.
        let mut pending = Vec::new();
        self.owned_value_instances(type_id, value, &mut pending, 0)?;
        self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
        let value = value.clone();
        let mut visited = InstanceSet::default();
        let mut backups = Vec::new();
        while let Some(raw_id) = pending.pop() {
            let id = InstanceId(raw_id);
            self.charge_sorted_growth(visited.insertion_demand(id))?;
            if !visited.insert(id)? {
                continue;
            }
            let data = self
                .data
                .storage
                .get_instance(id)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            if let Some(parent) = data.parent {
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
                if let (Some(type_id), Some((_, value))) = (
                    declaration.type_id,
                    data.variables.get_index(declaration.slot as usize),
                ) {
                    self.owned_value_instances(type_id, value, &mut pending, 0)?;
                }
            }
            let mut charge = data
                .variables
                .len()
                .checked_mul(core::mem::size_of::<(smol_str::SmolStr, Value)>() * 4)
                .ok_or(RuntimeError::Overflow)?;
            for value in data.variables.values() {
                charge = charge
                    .checked_add(self.value_clone_charge(value, 0)?)
                    .ok_or(RuntimeError::Overflow)?;
            }
            self.charge_allocation_bytes(charge)?;
            if backups.len() == backups.capacity() {
                let capacity = backups
                    .capacity()
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
            }
            backups.push((
                id,
                self.data
                    .storage
                    .get_instance(id)
                    .ok_or(RuntimeError::InvalidExecutionState)?
                    .clone(),
            ));
        }
        Ok(InitializerSeed {
            value,
            backups,
            writable_instances: visited,
        })
    }

    fn push_owned_instance(&self, output: &mut Vec<u32>, id: u32) -> Result<(), RuntimeError> {
        if output.len() == output.capacity() {
            let capacity = output
                .capacity()
                .max(4)
                .checked_mul(2)
                .ok_or(RuntimeError::Overflow)?;
            self.charge_allocation_bytes(
                capacity
                    .checked_mul(core::mem::size_of::<u32>())
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            output
                .try_reserve_exact(capacity - output.len())
                .map_err(|_| super::super::super::VmTrap::BudgetExceeded.into_runtime_error())?;
        }
        output.push(id);
        Ok(())
    }

    fn owned_value_instances(
        &self,
        type_id: u32,
        value: &Value,
        output: &mut Vec<u32>,
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

    pub(in crate::vm::engine) fn restore_initializer_instances(
        &mut self,
        backups: Vec<(InstanceId, InstanceData)>,
    ) {
        for (id, data) in backups {
            if let Some(instance) = self.data.storage.get_instance_mut(id) {
                *instance = data;
            }
        }
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
        let seed = state.initializer_seed(&entry, Some(program), None).unwrap();
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
        state.restore_initializer_instances(seed.backups);
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
