//! Private initializer transactions for existing FB member override destinations.
use super::*;
use crate::bytecode::{InitializationStage, TypeData};
use crate::memory::InstanceData;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn initializer_seed(
        &mut self,
        entry: &InitializerEntry,
        instance: Option<InstanceId>,
        lexical: Option<&VmFrame>,
    ) -> Result<(Value, Vec<(InstanceId, InstanceData)>), RuntimeError> {
        if entry.stage != InitializationStage::Explicit {
            return Ok((Value::Null, Vec::new()));
        }
        let Some(declaration) = entry
            .declaration_idx
            .and_then(|id| self.prepared.layout.entries.get(id as usize))
        else {
            return Ok((Value::Null, Vec::new()));
        };
        let Some(type_id) = declaration.type_id.and_then(|id| {
            super::super::super::type_policy::resolved_alias_type(&self.prepared.vm.types, id, 0)
        }) else {
            return Ok((Value::Null, Vec::new()));
        };
        if !matches!(
            self.prepared.vm.types.entries[type_id as usize].data,
            TypeData::Pou { .. }
        ) {
            return Ok((Value::Null, Vec::new()));
        }
        let name = &self.prepared.vm.strings[declaration.name_idx as usize];
        let value = match declaration.owner {
            StorageOwner::Global => self.storage.get_global(name),
            StorageOwner::Instance => {
                instance.and_then(|id| self.storage.get_instance_var(id, name))
            }
            StorageOwner::Frame => {
                lexical.and_then(|frame| frame.locals.get(declaration.slot as usize))
            }
        }
        .cloned()
        .ok_or(RuntimeError::InvalidExecutionState)?;
        let Value::Instance(root) = value else {
            return Err(RuntimeError::TypeMismatch);
        };
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        let mut backups = Vec::new();
        while let Some(id) = pending.pop() {
            self.charge_work_units(1)?;
            if !visited.insert(id) {
                continue;
            }
            let data = self
                .storage
                .get_instance(id)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            if let Some(parent) = data.parent {
                pending.push(parent);
            }
            let template = self
                .construction
                .instance_templates
                .get(&id)
                .copied()
                .ok_or(RuntimeError::InvalidExecutionState)?;
            for &declaration_id in self
                .prepared
                .declarations(StorageOwner::Instance, Some(template))
            {
                self.charge_work_units(1)?;
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
            ValueConstructionContext::charge_allocation(self, charge)?;
            backups.push((
                id,
                self.storage
                    .get_instance(id)
                    .ok_or(RuntimeError::InvalidExecutionState)?
                    .clone(),
            ));
        }
        Ok((Value::Instance(root), backups))
    }

    fn owned_value_instances(
        &self,
        type_id: u32,
        value: &Value,
        output: &mut Vec<InstanceId>,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth {
            return Err(super::super::super::VmTrap::CallStackOverflow.into_runtime_error());
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
            (TypeData::Pou { .. }, Value::Instance(id)) => output.push(*id),
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
            if let Some(instance) = self.storage.get_instance_mut(id) {
                *instance = data;
            }
        }
    }
}
