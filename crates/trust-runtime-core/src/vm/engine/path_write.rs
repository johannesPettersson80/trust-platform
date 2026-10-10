//! Charge copy-on-write backing storage before changing a field or element.
use super::*;
use crate::memory::MemoryLocation;
use crate::value::{array_offset_i64, RefSegment};
use alloc::sync::Arc;

impl EngineState<'_> {
    pub(super) fn charge_storage_path_write(
        &self,
        storage: &VariableStorage,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<(), RuntimeError> {
        if path.is_empty() {
            return Ok(());
        }
        let root = storage
            .read_direct_slot_by_location(location, offset)
            .ok_or(RuntimeError::NullReference)?;
        self.charge_path_write_inner(storage, root, path, false, 0)
    }

    pub(super) fn charge_path_write(
        &self,
        root: &Value,
        path: &[RefSegment],
    ) -> Result<(), RuntimeError> {
        self.charge_path_write_inner(&self.storage, root, path, false, 0)
    }

    fn charge_path_write_inner(
        &self,
        storage: &VariableStorage,
        root: &Value,
        path: &[RefSegment],
        ancestor_cloned: bool,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let Some((segment, rest)) = path.split_first() else {
            return Ok(());
        };
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth.min(128) {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        match (root, segment) {
            (Value::Struct(structure), RefSegment::Field(name)) => {
                // A cloned ancestor increments descendant Arc counts even when the
                // original descendant currently has one strong owner.
                let shared = ancestor_cloned || Arc::strong_count(structure) > 1;
                if shared {
                    let mut bytes = core::mem::size_of::<crate::value::StructValue>()
                        .checked_add(
                            structure
                                .fields()
                                .len()
                                .checked_mul(4 * core::mem::size_of::<(smol_str::SmolStr, Value)>())
                                .ok_or(RuntimeError::Overflow)?,
                        )
                        .ok_or(RuntimeError::Overflow)?;
                    for value in structure.fields().values() {
                        bytes = bytes
                            .checked_add(self.value_clone_charge(value, 0)?)
                            .ok_or(RuntimeError::Overflow)?;
                    }
                    self.charge_allocation_bytes(bytes)?;
                }
                let field = structure.field(name).ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, field, rest, shared, depth + 1)
            }
            (Value::Instance(instance), RefSegment::Field(name)) => {
                let reference = storage
                    .ref_for_instance_recursive(*instance, name)
                    .ok_or(RuntimeError::NullReference)?;
                let field = storage
                    .read_by_ref_ref(&reference)
                    .ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, field, rest, false, depth + 1)
            }
            (Value::Array(array), RefSegment::Index(indices)) => {
                self.charge_work_units(indices.len())?;
                let offset = array_offset_i64(array.dimensions(), indices)
                    .ok_or(RuntimeError::NullReference)?;
                let element = array
                    .elements()
                    .get(offset)
                    .ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, element, rest, ancestor_cloned, depth + 1)
            }
            (Value::String(text), RefSegment::Index(_)) => self.charge_string_path_write(text),
            (Value::WString(text), RefSegment::Index(_)) => self.charge_string_path_write(text),
            _ => Err(RuntimeError::NullReference),
        }
    }

    fn charge_string_path_write(&self, text: &str) -> Result<(), RuntimeError> {
        // write_string_element collects Unicode scalars, then builds new UTF-8;
        // STRING also copies that result into SmolStr. Include vector growth and
        // the replacement scalar's possible UTF-8 expansion before either allocation.
        let units = text.len().checked_add(4).ok_or(RuntimeError::Overflow)?;
        self.charge_work_units(units.checked_mul(3).ok_or(RuntimeError::Overflow)?)?;
        self.charge_allocation_bytes(units.checked_mul(16).ok_or(RuntimeError::Overflow)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collections::OrderedMap;
    use crate::value::{ArrayValue, StructValue};
    use crate::vm::{
        call::{bindings::VmWriteTarget, context::CallContext},
        VmTrap,
    };

    #[test]
    fn output_copyback_charges_shared_struct_backing_before_any_partial_commit() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let mut fields = OrderedMap::default();
        fields.insert("small".into(), Value::DInt(9));
        fields.insert(
            "large".into(),
            Value::Array(alloc::boxed::Box::new(ArrayValue::from_canonical_parts(
                vec![Value::DInt(0); 2048],
                vec![(0, 2047)],
            ))),
        );
        state.storage.set_global(
            "aggregate_probe",
            Value::Struct(Arc::new(StructValue::from_canonical_parts(
                "Probe".into(),
                fields,
            ))),
        );
        state.storage.set_global("first_probe", Value::DInt(7));
        let first = state.storage.ref_for_global("first_probe").unwrap();
        let mut second = state.storage.ref_for_global("aggregate_probe").unwrap();
        second.path.push(RefSegment::Field("small".into()));
        let mut frame = VmFrame {
            activation: None,
            pou_id: Some(0),
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            locals: Vec::new(),
            parameter_values_present: Vec::new(),
            runtime_instance: None,
            instance_owner: None,
        };
        let error = state
            .with_output_transaction(&mut frame, |state, frame| {
                // The snapshot shares the aggregate Arc; only the member write needs
                // its large owned array copied. Reserve too little for that operation.
                state
                    .resources
                    .construction_bytes
                    .set(prepared.limits.max_construction_bytes - 1024);
                VmWriteTarget::from_reference(&first).write(state, frame, Value::DInt(70))?;
                VmWriteTarget::from_reference(&second).write(state, frame, Value::DInt(90))
            })
            .unwrap_err();
        assert!(matches!(
            error,
            VmTrap::Runtime(RuntimeError::ExecutionTimeout)
        ));
        assert_eq!(
            state.storage.get_global("first_probe"),
            Some(&Value::DInt(7))
        );
        assert_eq!(
            state.storage.read_by_ref_ref(&second),
            Some(&Value::DInt(9))
        );
    }

    #[test]
    fn string_element_charge_fails_before_replacing_backing_text() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        state
            .storage
            .set_global("text_probe", Value::String("abc".into()));
        let mut reference = state.storage.ref_for_global("text_probe").unwrap();
        reference.path.push(RefSegment::Index(vec![1]));
        state
            .resources
            .construction_bytes
            .set(prepared.limits.max_construction_bytes);
        let error = crate::vm::context::ReferenceContext::write_typed_storage(
            &mut state,
            reference.location,
            reference.offset,
            &reference.path,
            Value::Char(b'Z'),
        )
        .unwrap_err();
        assert_eq!(error, VmTrap::BudgetExceeded.into_runtime_error());
        assert_eq!(
            state.storage.get_global("text_probe"),
            Some(&Value::String("abc".into()))
        );
    }
}
