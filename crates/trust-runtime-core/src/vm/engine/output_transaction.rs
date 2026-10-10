//! Output copy-back journals only physical destination slots.
use super::destinations::Key;
use super::*;
use crate::memory::MemoryLocation;
use crate::value::ValueRefView;
use crate::vm::VmTrap;

pub(super) use super::destinations::SavedDestinations;

fn lookup_work(count: usize) -> usize {
    // Conservative bound for the fixed-size key searches in each journal operation.
    12 * (count.max(1).ilog2() as usize + 1)
}

impl EngineState<'_> {
    fn snapshot_key(
        &self,
        frame: Option<&VmFrame>,
        reference: ValueRefView<'_>,
    ) -> Result<Key, RuntimeError> {
        self.charge_work_units(reference.path.len() + 1)?;
        let is_current = frame.is_some_and(|frame| {
            reference.location == MemoryLocation::Local(frame.reference_frame_id())
        });
        let resolved = if is_current {
            let root = frame
                .and_then(|frame| frame.locals.get(reference.offset))
                .ok_or(RuntimeError::NullReference)?;
            // A current local may itself hold an instance; resolve its first hop.
            if let (Value::Instance(instance), Some(crate::value::RefSegment::Field(name))) =
                (root, reference.path.first())
            {
                let field = self
                    .storage
                    .ref_for_instance_recursive(*instance, name)
                    .ok_or(RuntimeError::NullReference)?;
                self.storage
                    .resolve_reference_parts(field.location, field.offset, &reference.path[1..])
                    .ok_or(RuntimeError::NullReference)?
            } else {
                reference
            }
        } else {
            self.storage
                .resolve_reference_parts(reference.location, reference.offset, reference.path)
                .ok_or(RuntimeError::NullReference)?
        };
        Ok((resolved.location, resolved.offset))
    }

    fn snapshot_root<'s>(
        &'s self,
        frame: Option<&'s VmFrame>,
        key: Key,
    ) -> Result<&'s Value, RuntimeError> {
        if frame.is_some_and(|frame| key.0 == MemoryLocation::Local(frame.reference_frame_id())) {
            frame.and_then(|frame| frame.locals.get(key.1))
        } else {
            self.storage.read_direct_slot_by_location(key.0, key.1)
        }
        .ok_or(RuntimeError::NullReference)
    }

    /// Resolve input destinations in their original order, then copy each root once.
    /// Sorting identities before copying avoids quadratic insertion shifts regardless
    /// of binding order. This never reorders the staged writes themselves.
    pub(super) fn snapshot_input_destinations<'r>(
        &self,
        references: impl ExactSizeIterator<Item = ValueRefView<'r>>,
    ) -> Result<SavedDestinations, RuntimeError> {
        if references.len() <= 1 {
            return self.snapshot_destinations(None, references);
        }
        self.charge_allocation_bytes(
            references
                .len()
                .checked_mul(core::mem::size_of::<Key>())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(references.len())
            .map_err(|_| RuntimeError::Overflow)?;
        for reference in references {
            let key = self.snapshot_key(None, reference)?;
            // Detect the first invalid root before inspecting any later reference.
            self.snapshot_root(None, key)?;
            keys.push(key);
        }
        crate::sort::heap_sort(keys.len(), &mut |operation| -> Result<bool, RuntimeError> {
            match operation {
                crate::sort::Operation::Charge => {
                    self.charge_work_units(1)?;
                    Ok(false)
                }
                crate::sort::Operation::Less(a, b) => Ok(keys[a] < keys[b]),
                crate::sort::Operation::Swap(a, b) => {
                    keys.swap(a, b);
                    Ok(false)
                }
            }
        })?;
        self.charge_work_units(keys.len())?;
        keys.dedup();
        self.charge_allocation_bytes(SavedDestinations::allocation_bytes(keys.len())?)?;
        let mut saved = SavedDestinations::with_capacity(keys.len())?;
        for key in keys {
            self.charge_work_units(lookup_work(saved.len()))?;
            let root = self.snapshot_root(None, key)?;
            self.charge_allocation_bytes(self.value_clone_charge(root, 0)?)?;
            // Keys are unique and sorted; reserved capacity covers every append.
            saved.insert(key, root.clone())?;
        }
        Ok(saved)
    }

    pub(super) fn snapshot_destinations<'r>(
        &self,
        frame: Option<&VmFrame>,
        references: impl Iterator<Item = ValueRefView<'r>>,
    ) -> Result<SavedDestinations, RuntimeError> {
        let mut saved = SavedDestinations::new();
        for reference in references {
            let key = self.snapshot_key(frame, reference)?;
            let active = self.resources.output_journal.as_ref();
            self.charge_work_units(
                lookup_work(saved.len()) + active.map_or(0, |journal| lookup_work(journal.len())),
            )?;
            if saved.contains_key(&key) || active.is_some_and(|journal| journal.contains_key(&key))
            {
                continue;
            }
            let root = self.snapshot_root(frame, key)?;
            // Charge whole replacement allocations, relocation and insertion shifts
            // for both temporary and active journals before cloning or mutation.
            let (bytes, moved) = saved.insertion_demand(&key)?;
            let (active_bytes, active_moved) =
                active.map_or(Ok((0, 0)), |journal| journal.insertion_demand(&key))?;
            self.charge_work_units(
                lookup_work(saved.len())
                    + moved
                    + active.map_or(0, |journal| lookup_work(journal.len()))
                    + active_moved,
            )?;
            let clone_bytes = self.value_clone_charge(root, 0)?;
            self.charge_allocation_bytes(
                bytes
                    .checked_add(active_bytes)
                    .and_then(|bytes| bytes.checked_add(clone_bytes))
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            saved.insert(key, root.clone())?;
        }
        Ok(saved)
    }

    pub(super) fn restore_destinations(
        &mut self,
        mut frame: Option<&mut VmFrame>,
        saved: SavedDestinations,
    ) {
        for ((location, offset), value) in saved {
            if let Some(frame) = frame
                .as_deref_mut()
                .filter(|frame| location == MemoryLocation::Local(frame.reference_frame_id()))
            {
                frame.locals[offset] = value;
            } else {
                self.storage.restore_direct_slot(location, offset, value);
            }
        }
    }

    pub(super) fn commit_output_transaction<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self, &mut VmFrame) -> Result<T, VmTrap>,
    ) -> Result<T, VmTrap> {
        if self.resources.output_journal.is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        }
        self.resources.output_journal = Some(SavedDestinations::new());
        let result = action(self, frame);
        let saved = self.resources.output_journal.take().unwrap_or_default();
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.restore_destinations(Some(frame), saved);
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryLocation;
    use crate::vm::call::{bindings::VmWriteTarget, context::CallContext};

    #[test]
    fn bulk_input_snapshots_deduplicate_nested_roots_and_restore_all_values() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let original = Value::Array(alloc::boxed::Box::new(
            crate::value::ArrayValue::from_canonical_parts(
                vec![Value::DInt(7), Value::DInt(9)],
                vec![(0, 1)],
            ),
        ));
        state.storage.set_global("bulk_array", original.clone());
        state.storage.set_global("bulk_scalar", Value::DInt(11));
        let root = state.storage.ref_for_global("bulk_array").unwrap();
        let mut first = root.clone();
        first.path.push(crate::value::RefSegment::Index(vec![0]));
        let mut second = root.clone();
        second.path.push(crate::value::RefSegment::Index(vec![1]));
        let scalar = state.storage.ref_for_global("bulk_scalar").unwrap();
        let references = [&scalar, &second, &first, &second];
        let saved = state
            .snapshot_input_destinations(references.iter().map(|r| r.as_view()))
            .unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(
            saved.keys().copied().collect::<Vec<_>>(),
            vec![
                (root.location, root.offset),
                (scalar.location, scalar.offset)
            ]
        );
        assert!(state.storage.write_by_ref_ref(&second, Value::DInt(90)));
        assert!(state.storage.write_by_ref_ref(&first, Value::DInt(70)));
        assert!(state.storage.write_by_ref_ref(&scalar, Value::DInt(110)));
        state.restore_destinations(None, saved);
        assert_eq!(state.storage.get_global("bulk_array"), Some(&original));
        assert_eq!(
            state.storage.get_global("bulk_scalar"),
            Some(&Value::DInt(11))
        );
    }

    #[test]
    fn bulk_input_snapshots_stop_at_first_invalid_reference_without_mutation() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        state.storage.set_global("bulk_probe", Value::DInt(7));
        let valid = state.storage.ref_for_global("bulk_probe").unwrap();
        let mut invalid = valid.clone();
        invalid.offset = usize::MAX;
        let references = [&valid, &invalid, &valid];
        let visits = core::cell::Cell::new(0);
        let result = state.snapshot_input_destinations(references.iter().map(|reference| {
            visits.set(visits.get() + 1);
            reference.as_view()
        }));
        assert!(matches!(result, Err(RuntimeError::NullReference)));
        assert_eq!(
            visits.get(),
            2,
            "later references must not be inspected after failure"
        );
        assert_eq!(
            state.storage.get_global("bulk_probe"),
            Some(&Value::DInt(7))
        );
    }

    #[test]
    fn exhausted_second_output_rolls_back_global_and_current_frame() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        state.storage.set_global("output_probe", Value::DInt(7));
        let output = state.storage.ref_for_global("output_probe").unwrap();
        let id = state.storage.reserve_execution_frame().unwrap();
        state.lifetimes.live_activations.push(id);
        let suspended = state.storage.reserve_execution_frame().unwrap();
        state
            .storage
            .suspend_execution_frame(suspended, &mut vec![Value::DInt(11)])
            .unwrap();
        let mut frame = VmFrame {
            activation: Some(id),
            pou_id: Some(0),
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 1,
            locals: vec![Value::DInt(9)],
            parameter_values_present: Vec::new(),
            runtime_instance: None,
            instance_owner: None,
        };
        let error = state
            .with_output_transaction(&mut frame, |state, frame| {
                VmWriteTarget::from_reference(&output).write(state, frame, Value::DInt(70))?;
                VmWriteTarget::DirectStorage {
                    location: MemoryLocation::Local(id),
                    offset: 0,
                }
                .write(state, frame, Value::DInt(90))?;
                VmWriteTarget::DirectStorage {
                    location: MemoryLocation::Local(suspended),
                    offset: 0,
                }
                .write(state, frame, Value::DInt(110))?;
                // Exhaust the real policy budget after writes have begun, then exercise
                // the same checked target path used by the next native output.
                state.resources.work_budget.reset(0);
                VmWriteTarget::from_reference(&output).write(state, frame, Value::DInt(700))
            })
            .unwrap_err();
        assert!(matches!(
            error,
            VmTrap::Runtime(RuntimeError::ExecutionTimeout)
        ));
        assert_eq!(
            state.storage.get_global("output_probe"),
            Some(&Value::DInt(7))
        );
        assert_eq!(frame.locals, [Value::DInt(9)]);
        assert_eq!(
            state
                .storage
                .read_direct_slot_by_location(MemoryLocation::Local(suspended), 0),
            Some(&Value::DInt(11))
        );
        assert_eq!(state.resources.work_budget.remaining(), 0);
    }
    #[test]
    fn native_output_journal_ignores_unrelated_large_values_and_preserves_aliases() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        state.storage.set_global("output_probe", Value::DInt(7));
        let reference = state.storage.ref_for_global("output_probe").unwrap();
        let id = state.storage.reserve_execution_frame().unwrap();
        let mut frame = VmFrame {
            activation: Some(id),
            pou_id: None,
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
        let measure = |state: &mut EngineState<'_>, frame: &mut VmFrame| {
            let before = state.resources.construction_bytes.get();
            state
                .with_output_transaction(frame, |state, frame| {
                    VmWriteTarget::from_reference(&reference).write(
                        state,
                        frame,
                        Value::DInt(70),
                    )?;
                    VmWriteTarget::from_reference(&reference).write(
                        state,
                        frame,
                        Value::DInt(90),
                    )?;
                    Ok(())
                })
                .unwrap();
            state.resources.construction_bytes.get() - before
        };
        let small = measure(&mut state, &mut frame);
        state.storage.set_global(
            "unrelated",
            Value::Array(alloc::boxed::Box::new(
                crate::value::ArrayValue::from_canonical_parts(
                    vec![Value::DInt(0); 32768],
                    vec![(0, 32767)],
                ),
            )),
        );
        let large = measure(&mut state, &mut frame);
        assert_eq!(
            small, large,
            "snapshot cost follows destinations, not runtime size"
        );
        assert_eq!(
            state.storage.get_global("output_probe"),
            Some(&Value::DInt(90))
        );
    }

    #[test]
    fn redirected_instance_output_is_journaled_at_its_actual_write() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let first = state.storage.create_instance("Probe");
        let second = state.storage.create_instance("Probe");
        state
            .storage
            .set_instance_var(first, "number", Value::DInt(7));
        state
            .storage
            .set_instance_var(second, "number", Value::DInt(9));
        state
            .storage
            .set_global("alias_probe", Value::Instance(first));
        let root = state.storage.ref_for_global("alias_probe").unwrap();
        let mut field = root.clone();
        field
            .path
            .push(crate::value::RefSegment::Field("number".into()));
        let mut frame = VmFrame {
            activation: None,
            pou_id: None,
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
                VmWriteTarget::from_reference(&field).write(state, frame, Value::DInt(70))?;
                VmWriteTarget::from_reference(&root).write(
                    state,
                    frame,
                    Value::Instance(second),
                )?;
                VmWriteTarget::from_reference(&field).write(state, frame, Value::DInt(90))?;
                Err::<(), _>(VmTrap::Runtime(RuntimeError::Overflow))
            })
            .unwrap_err();
        assert!(matches!(error, VmTrap::Runtime(RuntimeError::Overflow)));
        assert_eq!(
            state.storage.get_global("alias_probe"),
            Some(&Value::Instance(first))
        );
        assert_eq!(
            state.storage.get_instance_var(first, "number"),
            Some(&Value::DInt(7))
        );
        assert_eq!(
            state.storage.get_instance_var(second, "number"),
            Some(&Value::DInt(9))
        );
        assert!(state.resources.output_journal.is_none());
    }
}
