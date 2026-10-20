//! Typed access between cooperative cycles, without exposing mutable storage.
use super::*;
use crate::bytecode::{AccessBindingEntry, StorageRole};
use crate::memory::layout::CELL_BYTES;
use crate::memory::view::{PreparedWrite, StoreAccess, StoreMutate, ValueView};
use crate::value::{
    read_partial_access, write_partial_access, PartialAccess, RefSegment, ValueRefView,
};
use crate::vm::construction::values::ValueConstructionContext;

impl EngineState<'_> {
    /// The owned value a reference names, in this storage's generation.
    ///
    /// The read goes through the seam. A STRING/WSTRING element has no borrowed
    /// `Value`, so it is resolved through its parent as its CHAR/WCHAR scalar, the
    /// way `VariableStorage::materialize_by_ref_ref` did before the seam; unlike
    /// `read_checked_reference`'s own inline fallback, neither caller charges that
    /// walk.
    pub(in crate::vm::engine) fn materialize_reference(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<Value, RuntimeError> {
        if let Ok(view) = self.read_reference(reference) {
            return self.materialize(&view);
        }
        let (last, prefix) = reference
            .path
            .split_last()
            .ok_or(RuntimeError::NullReference)?;
        let view = self
            .read_path(reference.location, reference.offset, prefix)
            .map_err(|_| RuntimeError::NullReference)?;
        crate::vm::dispatch_refs::materialize_segment(&view, last)?
            .ok_or(RuntimeError::NullReference)
    }

    /// The allocation of one copy of the stored `view`, after its work is charged (TIME-08). A `Value`
    /// view charges what `value_clone_charge` charges; a cell view charges the same work node by node,
    /// in the same order (C1-13), and answers the same allocation (C1-9d1).
    pub(in crate::vm::engine) fn view_clone_charge(
        &self,
        view: &ValueView<'_>,
    ) -> Result<usize, RuntimeError> {
        match view {
            ValueView::Value(value) => self.value_clone_charge(value, 0),
            ValueView::Instance(instance) => {
                self.value_clone_charge(&Value::Instance(*instance), 0)
            }
            ValueView::Cells {
                cells,
                range,
                type_id,
            } => {
                cells.copy_work(*range, *type_id, 0, &mut |units| {
                    self.charge_work_units(units)
                })?;
                Ok(cells.copy_charge(*range, *type_id, 0)?.allocation)
            }
            // A node of a backing charges what the copy of the value it decodes to charges (C1-15).
            ValueView::Bits {
                cells,
                range,
                type_id,
                leaf,
                depth,
            } => {
                cells.overlay_copy_work(*range, *type_id, *leaf, *depth, 0, &mut |units| {
                    self.charge_work_units(units)
                })?;
                Ok(cells
                    .overlay_copy_charge(*range, *type_id, *leaf, *depth, 0)?
                    .allocation)
            }
        }
    }

    pub(super) fn read_checked_reference(
        &self,
        reference: &ValueRef,
    ) -> Result<Value, RuntimeError> {
        self.check_management_destination(reference.as_view())?;
        self.check_read(reference.as_view())?;
        let value = if let Ok(view) = self.read_reference(reference.as_view()) {
            self.charge_allocation_bytes(self.view_clone_charge(&view)?)?;
            self.check_entry_deadline()?;
            view.materialize()?
        } else {
            // STRING/WSTRING elements are scalar CHAR/WCHAR values, not stored
            // Values. Resolve only their parent, without cloning its backing text.
            let (last, prefix) = reference
                .path
                .split_last()
                .ok_or(RuntimeError::NullReference)?;
            let view = self
                .read_path(reference.location, reference.offset as usize, prefix)
                .map_err(|_| RuntimeError::NullReference)?;
            let RefSegment::Index(indices) = last else {
                return Err(RuntimeError::NullReference);
            };
            // Reading one character scans the text up to its index: TEXT (TIME-08).
            let text_length = view.text_utf8_len()?.ok_or(RuntimeError::NullReference)?;
            // The character is a scalar copy: it allocates nothing (TIME-08).
            self.charge_work_units(text_length)?;
            self.check_entry_deadline()?;
            view.text_element(indices)?
                .ok_or(RuntimeError::NullReference)?
        };
        self.check_entry_deadline()?;
        Ok(value)
    }

    pub(super) fn write_checked_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.check_management_destination(reference)?;
        self.check_write(reference, &value)?;
        let value = self.normalize_reference_value(reference, value)?;
        self.validate_engineering_value(&value, 0)?;
        self.charge_storage_path_write(
            &*self,
            reference.location,
            reference.offset,
            reference.path,
            &value,
        )?;
        let capacity = self.declared_string_capacity(reference);
        let prepared = self.prepare_engineering_storage_write(reference, &value)?;
        self.check_entry_deadline()?;
        if !StoreMutate::commit_write(self, reference.path, value, prepared, capacity) {
            return Err(RuntimeError::NullReference);
        }
        Ok(())
    }

    fn check_management_destination(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        // A handle from before a restart or replacement is refused before any other
        // check or charge (spec 34 §11.3, MEM-07, UPDATE-04).
        self.check_generation(reference.generation)?;
        match reference.location {
            crate::memory::MemoryLocation::Local(id) => {
                if !self.data.storage.execution_frame_is_live(id) {
                    return Err(RuntimeError::ReferenceLifetime);
                }
            }
            crate::memory::MemoryLocation::Instance(id) => {
                self.check_instance_lifetime(id, None)?
            }
            _ => {}
        }
        Ok(())
    }

    fn access_binding(&self, name: &str) -> Option<AccessBindingEntry> {
        self.prepared
            .access
            .entries
            .iter()
            .find(|entry| {
                self.prepared.vm.strings[entry.name_idx as usize].eq_ignore_ascii_case(name)
            })
            .cloned()
    }

    pub(super) fn resolve_access_reference(
        &mut self,
        name: &str,
    ) -> Result<ValueRef, RuntimeError> {
        let binding = self
            .access_binding(name)
            .ok_or(RuntimeError::InvalidAlias)?;
        let reference = self.persistent_io_reference(binding.ref_idx)?;
        self.check_management_destination(reference.as_view())?;
        self.check_read(reference.as_view())?;
        self.check_entry_deadline()?;
        Ok(reference)
    }

    /// Read a declared VAR_ACCESS alias, including its partial selection.
    pub fn read_access(&mut self, name: &str) -> Result<Value, RuntimeError> {
        let binding = self
            .access_binding(name)
            .ok_or(RuntimeError::InvalidAlias)?;
        let reference = self.persistent_io_reference(binding.ref_idx)?;
        let value = self.read_checked_reference(&reference)?;
        match partial(&binding)? {
            Some(access) => read_partial_access(&value, access)
                .map_err(crate::vm::dispatch::partial_access_error_to_runtime),
            None => Ok(value),
        }
    }

    /// Write a writable alias through the same type and lifetime gates as execution.
    /// An unknown name is an undefined variable; a read-only alias stays `InvalidAlias`.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        let binding = self
            .access_binding(name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.into()))?;
        if !binding.is_writable() {
            return Err(RuntimeError::InvalidAlias);
        }
        let value = self.normalize_assignment_value(binding.type_id, value)?;
        let reference = self.persistent_io_reference(binding.ref_idx)?;
        let value = match partial(&binding)? {
            Some(access) => {
                self.check_read(reference.as_view())?;
                let target = self.materialize_reference(reference.as_view())?;
                write_partial_access(target, access, value)
                    .map_err(crate::vm::dispatch::partial_access_error_to_runtime)?
            }
            None => value,
        };
        self.validate_engineering_value(&value, 0)?;
        self.check_write(reference.as_view(), &value)?;
        self.charge_storage_path_write(
            &*self,
            reference.location,
            reference.offset as usize,
            &reference.path,
            &value,
        )?;
        let capacity = self.declared_string_capacity(reference.as_view());
        let prepared = self.prepare_engineering_storage_write(reference.as_view(), &value)?;
        self.check_entry_deadline()?;
        if !StoreMutate::commit_write(self, &reference.path, value, prepared, capacity) {
            return Err(RuntimeError::NullReference);
        }
        Ok(())
    }

    /// Write a mutable declared global between cycles.
    pub fn write_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.write_named_path(name, &[], value)
    }

    /// Admit the same named write without committing a value or changing readiness.
    pub(super) fn validate_global_write(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.prepare_named_write(name, &[], value)?;
        self.check_entry_deadline()
    }

    /// Resolve an engineering destination by its installed name and checked path.
    /// No instance or frame identity supplied by an external caller is trusted.
    pub fn write_named_path(
        &mut self,
        name: &str,
        path: &[RefSegment],
        value: Value,
    ) -> Result<(), RuntimeError> {
        let (reference, value, prepared) = self.prepare_named_write(name, path, value)?;
        self.commit_named_write(reference, value, prepared)
    }

    /// Return the normalized publication value for an external transaction.
    /// Result copying is charged and completed before the final commit check.
    #[cfg(feature = "std")]
    pub(super) fn write_global_canonical(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let (reference, value, prepared) = self.prepare_named_write(name, &[], value)?;
        self.charge_allocation_bytes(self.value_clone_charge(&value, 0)?)?;
        let canonical = value.clone();
        self.commit_named_write(reference, value, prepared)?;
        Ok(canonical)
    }

    fn prepare_named_write(
        &mut self,
        name: &str,
        path: &[RefSegment],
        value: Value,
    ) -> Result<(ValueRef, Value, PreparedWrite), RuntimeError> {
        let (_, declaration) = self
            .prepared
            .global(name)
            .ok_or(RuntimeError::NullReference)?;
        if declaration.is_constant() {
            return Err(RuntimeError::ConstantWrite);
        }
        if declaration.role != StorageRole::Variable
            && !(declaration.role == StorageRole::ProgramRoot && !path.is_empty())
        {
            return Err(RuntimeError::ProgramRootReplacement);
        }
        let declared_type = declaration.type_id;

        let mut reference =
            self.persistent_io_reference(declaration.ref_idx.ok_or(RuntimeError::TypeMismatch)?)?;
        if !path.is_empty() {
            reference
                .path
                .try_reserve_exact(path.len())
                .map_err(|_| RuntimeError::Overflow)?;
        }
        for segment in path {
            // TIME-08: an added segment and each of its subscripts cost one cell each, whatever
            // the capacity of the path's storage.
            let cells: usize = match segment {
                RefSegment::Field(_) => 1,
                RefSegment::Index(indices) => {
                    indices.len().checked_add(1).ok_or(RuntimeError::Overflow)?
                }
            };
            self.charge_allocation(
                cells
                    .checked_mul(CELL_BYTES)
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            let segment = match segment {
                RefSegment::Field(name) => RefSegment::Field(name.clone()),
                RefSegment::Index(indices) => {
                    let mut owned = Vec::new();
                    owned
                        .try_reserve_exact(indices.len())
                        .map_err(|_| RuntimeError::Overflow)?;
                    owned.extend_from_slice(indices);
                    RefSegment::Index(owned)
                }
            };
            reference.path.push(segment);
        }
        let ty = if path.is_empty() {
            declared_type.ok_or(RuntimeError::TypeMismatch)?
        } else {
            self.reference_type(reference.as_view())?
                .ok_or(RuntimeError::TypeMismatch)?
        };
        self.check_write(reference.as_view(), &value)?;
        let value = self.normalize_assignment_value(ty, value)?;
        self.validate_engineering_value(&value, 0)?;
        self.charge_storage_path_write(
            &*self,
            reference.location,
            reference.offset as usize,
            &reference.path,
            &value,
        )?;
        let prepared = self.prepare_engineering_storage_write(reference.as_view(), &value)?;
        Ok((reference, value, prepared))
    }

    fn prepare_engineering_storage_write(
        &self,
        reference: crate::value::ValueRefView<'_>,
        value: &Value,
    ) -> Result<PreparedWrite, RuntimeError> {
        let prepared = self
            .prepare_write(reference.location, reference.offset, reference.path, value)?
            .ok_or(RuntimeError::NullReference)?;
        // IEEE bit aliases remain valid internally. Engineering writes must also
        // validate the reconstructed aliases, not only the incoming integer bits.
        prepared.validate_aliases(&mut |value| self.validate_engineering_value(value, 0))?;
        Ok(prepared)
    }

    fn commit_named_write(
        &mut self,
        reference: ValueRef,
        value: Value,
        prepared: PreparedWrite,
    ) -> Result<(), RuntimeError> {
        self.check_entry_deadline()?;
        let capacity = self.declared_string_capacity(reference.as_view());
        if !StoreMutate::commit_write(self, &reference.path, value, prepared, capacity) {
            return Err(RuntimeError::NullReference);
        }
        Ok(())
    }

    /// Engineering ingress has a finite-value policy distinct from IEC arithmetic.
    /// Inspect owned compound contents, never the graph referenced by a PLC handle.
    fn validate_engineering_value(&self, value: &Value, depth: usize) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match value {
            Value::Real(value) if !value.is_finite() => Err(RuntimeError::NonFiniteValue),
            Value::LReal(value) if !value.is_finite() => Err(RuntimeError::NonFiniteValue),
            Value::Array(array) => {
                for element in array.elements() {
                    self.validate_engineering_value(element, depth + 1)?;
                }
                Ok(())
            }
            Value::Struct(structure) => {
                for field in structure.fields().values() {
                    self.validate_engineering_value(field, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn partial(binding: &AccessBindingEntry) -> Result<Option<PartialAccess>, RuntimeError> {
    PartialAccess::from_wire(binding.partial_kind, binding.partial_index)
        .map_err(crate::vm::dispatch::partial_access_error_to_runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::engine::{CycleAccess, CyclePhase};

    #[test]
    fn engineering_finite_policy_checks_nested_values_without_changing_ieee_arithmetic() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        for invalid in [
            Value::Real(f32::NAN),
            Value::Real(f32::INFINITY),
            Value::LReal(f64::NEG_INFINITY),
        ] {
            let nested = Value::Array(alloc::boxed::Box::new(
                crate::value::ArrayValue::from_canonical_parts(
                    vec![Value::Struct(alloc::sync::Arc::new(
                        crate::value::StructValue::from_canonical_parts(
                            "Record".into(),
                            crate::collections::OrderedMap::from_iter([("value".into(), invalid)]),
                        ),
                    ))],
                    vec![(1, 1)],
                ),
            ));
            assert_eq!(
                state.validate_engineering_value(&nested, 0),
                Err(RuntimeError::NonFiniteValue)
            );
        }
        for finite in [
            Value::Real(-0.0),
            Value::Real(f32::from_bits(1)),
            Value::LReal(f64::MAX),
        ] {
            state.validate_engineering_value(&finite, 0).unwrap();
        }
        // This policy is invoked by engineering ingress, not by the shared
        // primitive assignment rule used for internal IEC arithmetic/IS_VALID.
        assert!(crate::vm::type_policy::primitive_value_matches(
            14,
            &Value::Real(f32::NAN)
        ));
    }

    #[test]
    fn character_cycle_reads_consume_one_cumulative_allowance_before_materialization() {
        // The program `strings` of the bounded-C corpus declares `text : STRING[40]` in its
        // program instance `Plant`.
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/bounded-c/strings/program.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let services = &super::super::services::LOGICAL_ONLY;
        let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
        let mut engine = EngineState::borrow(&mut data, &prepared, services);
        let text_path = [RefSegment::Field("text".into())];
        engine
            .write_named_path("Plant", &text_path, Value::String("AB".into()))
            .unwrap();
        let mut text = crate::memory::view::StoreAccess::ref_for_global(&engine, "Plant").unwrap();
        text.path.push(RefSegment::Field("text".into()));
        let mut reference = text.clone();
        reference.path.push(RefSegment::Index(alloc::vec![2]));
        // TIME-08 TEXT: each read scans the 2-byte text (2 units); the path walk is
        // bookkeeping. 3 units admit one read and refuse the second.
        engine.data.resources.work_budget.reset(3);
        let mut access = CycleAccess {
            engine: &mut engine,
            phase: CyclePhase::BeforeInputSample,
            output_undo: None,
        };
        assert_eq!(
            access.read_reference(&reference).unwrap(),
            Value::Char(b'B')
        );
        assert!(matches!(
            access.read_reference(&reference),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::Work,
                used: 2,
                requested: 2,
                limit: 3,
            })
        ));
        let store = access.store();
        let probe = store.read_reference(text.as_view()).expect("text");
        assert_eq!(
            store.materialize(&probe).unwrap(),
            Value::String("AB".into())
        );
    }
}

#[cfg(test)]
mod text_work_reads_tests;
