//! Typed access between cooperative cycles, without exposing mutable storage.
use super::*;
use crate::bytecode::{AccessBindingEntry, StorageRole};
use crate::value::{
    read_partial_access, write_partial_access, PartialAccess, RefSegment, ValueRefView,
};
use crate::vm::construction::values::ValueConstructionContext;

impl EngineState<'_> {
    pub(super) fn read_checked_reference(
        &self,
        reference: &ValueRef,
    ) -> Result<Value, RuntimeError> {
        self.check_management_destination(reference.as_view())?;
        self.check_read(reference.as_view())?;
        let value = if let Some(value) = self.data.storage.read_by_ref_ref(reference) {
            self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
            self.check_entry_deadline()?;
            value.clone()
        } else {
            // STRING/WSTRING elements are scalar CHAR/WCHAR values, not borrowed
            // Values. Resolve only their parent, without cloning its backing text.
            let (last, prefix) = reference
                .path
                .split_last()
                .ok_or(RuntimeError::NullReference)?;
            let parent = self
                .data
                .storage
                .read_by_ref_parts(reference.location, reference.offset, prefix)
                .ok_or(RuntimeError::NullReference)?;
            // Reading one character scans the text up to its index: TEXT (TIME-08).
            let text_length = match (parent, last) {
                (Value::String(text), RefSegment::Index(_)) => text.len(),
                (Value::WString(text), RefSegment::Index(_)) => text.len(),
                _ => return Err(RuntimeError::NullReference),
            };
            self.charge_work_units(text_length)?;
            self.charge_allocation_bytes(core::mem::size_of::<Value>())?;
            self.check_entry_deadline()?;
            crate::value::materialize_value_path(parent, core::slice::from_ref(last))
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
            &self.data.storage,
            reference.location,
            reference.offset,
            reference.path,
            &value,
        )?;
        let prepared = self.prepare_engineering_storage_write(reference, &value)?;
        self.check_entry_deadline()?;
        if !self
            .data
            .storage
            .commit_prepared_write(reference.path, value, prepared)
        {
            return Err(RuntimeError::NullReference);
        }
        Ok(())
    }

    fn check_management_destination(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
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
                let target = self
                    .data
                    .storage
                    .materialize_by_ref_ref(&reference)
                    .ok_or(RuntimeError::NullReference)?;
                write_partial_access(target, access, value)
                    .map_err(crate::vm::dispatch::partial_access_error_to_runtime)?
            }
            None => value,
        };
        self.validate_engineering_value(&value, 0)?;
        self.check_write(reference.as_view(), &value)?;
        self.charge_storage_path_write(
            &self.data.storage,
            reference.location,
            reference.offset,
            &reference.path,
            &value,
        )?;
        let prepared = self.prepare_engineering_storage_write(reference.as_view(), &value)?;
        self.check_entry_deadline()?;
        if !self
            .data
            .storage
            .commit_prepared_write(&reference.path, value, prepared)
        {
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
    ) -> Result<(ValueRef, Value, crate::memory::PreparedStorageWrite), RuntimeError> {
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
        let length = reference
            .path
            .len()
            .checked_add(path.len())
            .ok_or(RuntimeError::Overflow)?;
        let bytes = if length > reference.path.capacity() {
            length
                .checked_mul(core::mem::size_of::<RefSegment>())
                .ok_or(RuntimeError::Overflow)?
        } else {
            0
        };
        if !path.is_empty() {
            self.charge_allocation(bytes)?;
            reference
                .path
                .try_reserve_exact(path.len())
                .map_err(|_| RuntimeError::Overflow)?;
        }
        for segment in path {
            let allocation = match segment {
                RefSegment::Field(_) => 0,
                RefSegment::Index(indices) => indices
                    .len()
                    .checked_mul(core::mem::size_of::<i64>())
                    .ok_or(RuntimeError::Overflow)?,
            };
            self.charge_allocation(allocation)?;
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
            &self.data.storage,
            reference.location,
            reference.offset,
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
    ) -> Result<crate::memory::PreparedStorageWrite, RuntimeError> {
        let prepared = self
            .data
            .storage
            .prepare_by_ref_parts_write(
                reference.location,
                reference.offset,
                reference.path,
                value,
            )?
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
        prepared: crate::memory::PreparedStorageWrite,
    ) -> Result<(), RuntimeError> {
        self.check_entry_deadline()?;
        if !self
            .data
            .storage
            .commit_prepared_write(&reference.path, value, prepared)
        {
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
        let prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let services = &super::super::services::LOGICAL_ONLY;
        let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
        // An explicit low-level storage fixture isolates the reader's accounting;
        // the hosted native test supplies real admitted STRING/WSTRING declarations.
        data.storage
            .set_global("probe_depth", Value::String("AB".into()));
        let mut reference = data.storage.ref_for_global("probe_depth").unwrap();
        reference.path.push(RefSegment::Index(alloc::vec![2]));
        // TIME-08 TEXT: each read scans the 2-byte text (2 units); the path walk is
        // bookkeeping. 3 units admit one read and refuse the second.
        data.resources.work_budget.reset(3);
        let mut engine = EngineState::borrow(&mut data, &prepared, services);
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
        assert_eq!(
            access.storage().get_global("probe_depth"),
            Some(&Value::String("AB".into()))
        );
    }
    #[test]
    fn engineering_bits_preflight_rejects_nonfinite_alias_before_storage_commit() {
        use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
        use crate::value::{StorageAbi, StructValue};
        use alloc::sync::Arc;
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let state =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        let names = vec!["real".into(), "bits".into()];
        let types = TypeTable {
            offsets: Vec::new(),
            entries: vec![
                TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: 14,
                        max_length: 0,
                    },
                },
                TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: 4,
                        max_length: 0,
                    },
                },
                TypeEntry {
                    kind: TypeKind::OverlappingStruct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: vec![
                            Field {
                                name_idx: 0,
                                type_id: 0,
                                bit_offset: Some(0),
                            },
                            Field {
                                name_idx: 1,
                                type_id: 1,
                                bit_offset: Some(0),
                            },
                        ],
                    },
                },
            ],
        };
        let layouts = crate::value::overlap::prepare_layouts(
            &types,
            &names,
            StorageAbi::UniformTemporal64,
            &mut |_, _| Ok(()),
        )
        .unwrap();
        let probe = StructValue::from_canonical_parts(
            "IEEE".into(),
            crate::collections::OrderedMap::from_iter([
                ("real".into(), Value::Real(0.0)),
                ("bits".into(), Value::DWord(0)),
            ]),
        )
        .initialize_physical_layout(layouts[2].clone())
        .unwrap();
        state
            .data
            .storage
            .set_global("probe", Value::Struct(Arc::new(probe)));
        let mut reference = state.data.storage.ref_for_global("probe").unwrap();
        reference.path = vec![RefSegment::Field("BiTs".into())];
        let input = Value::DWord(0x7FC0_0000);
        state.validate_engineering_value(&input, 0).unwrap();
        state
            .charge_storage_path_write(
                &state.data.storage,
                reference.location,
                reference.offset,
                &reference.path,
                &input,
            )
            .unwrap();
        let error = match state.prepare_engineering_storage_write(reference.as_view(), &input) {
            Ok(_) => panic!("non-finite alias admitted"),
            Err(error) => error,
        };
        assert_eq!(error, RuntimeError::NonFiniteValue);
        assert_eq!(
            state.data.storage.read_by_ref_ref(&reference),
            Some(&Value::DWord(0))
        );
        reference.path = vec![RefSegment::Field("ReAl".into())];
        assert_eq!(
            state.data.storage.read_by_ref_ref(&reference),
            Some(&Value::Real(0.0))
        );
    }
}

#[cfg(test)]
mod text_work_reads_tests;
