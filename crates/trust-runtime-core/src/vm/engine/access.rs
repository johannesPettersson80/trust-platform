//! Typed access between cooperative cycles, without exposing mutable storage.
use super::*;
use crate::bytecode::{AccessBindingEntry, StorageRole};
use crate::value::{read_partial_access, write_partial_access, PartialAccess};
use crate::vm::construction::values::ValueConstructionContext;

impl EngineState<'_> {
    fn access_binding(&self, name: &str) -> Result<AccessBindingEntry, RuntimeError> {
        self.charge_work_units(self.prepared.access.entries.len())?;
        self.prepared
            .access
            .entries
            .iter()
            .find(|entry| {
                self.prepared.vm.strings[entry.name_idx as usize].eq_ignore_ascii_case(name)
            })
            .cloned()
            .ok_or(RuntimeError::InvalidAlias)
    }

    /// Read a declared VAR_ACCESS alias, including its partial selection.
    pub fn read_access(&mut self, name: &str) -> Result<Value, RuntimeError> {
        let binding = self.access_binding(name)?;
        let reference = self.persistent_io_reference(binding.ref_idx)?;
        self.check_read(reference.as_view())?;
        let value = self
            .storage
            .read_by_ref_ref(&reference)
            .ok_or(RuntimeError::NullReference)?;
        let bytes = self.value_clone_charge(value, 0)?;
        self.charge_allocation(bytes)?;
        let value = self
            .storage
            .materialize_by_ref_ref(&reference)
            .ok_or(RuntimeError::NullReference)?;
        match partial(&binding)? {
            Some(access) => read_partial_access(&value, access)
                .map_err(crate::vm::dispatch::partial_access_error_to_runtime),
            None => Ok(value),
        }
    }

    /// Write a writable alias through the same type and lifetime gates as execution.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        let binding = self.access_binding(name)?;
        if !binding.is_writable() {
            return Err(RuntimeError::InvalidAlias);
        }
        let value = self.normalize_assignment_value(binding.type_id, value)?;
        let reference = self.persistent_io_reference(binding.ref_idx)?;
        let value = match partial(&binding)? {
            Some(access) => {
                self.check_read(reference.as_view())?;
                let target = self
                    .storage
                    .materialize_by_ref_ref(&reference)
                    .ok_or(RuntimeError::NullReference)?;
                write_partial_access(target, access, value)
                    .map_err(crate::vm::dispatch::partial_access_error_to_runtime)?
            }
            None => value,
        };
        self.check_write(reference.as_view(), &value)?;
        self.charge_storage_path_write(
            &self.storage,
            reference.location,
            reference.offset,
            &reference.path,
        )?;
        self.check_entry_deadline()?;
        if !self.storage.write_by_ref_ref(&reference, value) {
            return Err(RuntimeError::NullReference);
        }
        Ok(())
    }

    /// Write a mutable declared global between cycles.
    pub fn write_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.charge_work_units(self.prepared.name_lookup_work(name))?;
        let (_, declaration) = self
            .prepared
            .global(name)
            .ok_or(RuntimeError::NullReference)?;
        if declaration.is_constant() {
            return Err(RuntimeError::ConstantWrite);
        }
        if declaration.role != StorageRole::Variable {
            return Err(RuntimeError::ProgramRootReplacement);
        }
        let ty = declaration.type_id.ok_or(RuntimeError::TypeMismatch)?;

        let reference =
            self.persistent_io_reference(declaration.ref_idx.ok_or(RuntimeError::TypeMismatch)?)?;
        self.check_write(reference.as_view(), &value)?;
        let value = self.normalize_assignment_value(ty, value)?;
        self.charge_storage_path_write(
            &self.storage,
            reference.location,
            reference.offset,
            &reference.path,
        )?;
        self.check_entry_deadline()?;
        if !self.storage.write_by_ref_ref(&reference, value) {
            return Err(RuntimeError::NullReference);
        }
        Ok(())
    }
}

fn partial(binding: &AccessBindingEntry) -> Result<Option<PartialAccess>, RuntimeError> {
    PartialAccess::from_wire(binding.partial_kind, binding.partial_index)
        .map_err(crate::vm::dispatch::partial_access_error_to_runtime)
}
