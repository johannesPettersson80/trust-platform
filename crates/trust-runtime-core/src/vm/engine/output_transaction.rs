//! Output copy-back journals only physical destination slots.
use super::destinations::Key;
use super::*;
use crate::memory::MemoryLocation;
use crate::value::ValueRefView;
use crate::vm::VmTrap;

pub(super) use super::destinations::SavedDestinations;

impl EngineState<'_> {
    /// Save a single write's original root directly in the active journal.
    /// Admission and cloning finish before insertion; a later failure still
    /// restores the first snapshot of each physical destination.
    pub(super) fn journal_output_destination(
        &mut self,
        frame: &VmFrame,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        let Some(journal) = self.data.resources.output_journal.as_ref() else {
            return Ok(());
        };
        let key = self.snapshot_key(Some(frame), reference)?;
        if journal.contains_key(&key) {
            return Ok(());
        }
        let root = self.snapshot_root(Some(frame), key)?;
        // The saved value and the journal's growth are call-transaction bookkeeping:
        // neither the work allowance nor the cumulative allocation charge admits them
        // (spec 34 TIME-08). The detached value needs no temporary heap collection.
        // Insertion can still refuse allocation, in which case the destination is untouched.
        let saved = root.clone();
        self.data
            .resources
            .output_journal
            .as_mut()
            .ok_or(RuntimeError::InvalidExecutionState)?
            .insert(key, saved)?;
        Ok(())
    }

    fn snapshot_key(
        &self,
        frame: Option<&VmFrame>,
        reference: ValueRefView<'_>,
    ) -> Result<Key, RuntimeError> {
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
                    .data
                    .storage
                    .ref_for_instance_recursive(*instance, name)
                    .ok_or(RuntimeError::NullReference)?;
                self.data
                    .storage
                    .resolve_reference_parts(field.location, field.offset, &reference.path[1..])
                    .ok_or(RuntimeError::NullReference)?
            } else {
                reference
            }
        } else {
            self.data
                .storage
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
            self.data.storage.read_direct_slot_by_location(key.0, key.1)
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
        // Destination sorting and deduplication are journal bookkeeping (TIME-08).
        crate::sort::heap_sort(keys.len(), &mut |operation| -> Result<bool, RuntimeError> {
            match operation {
                crate::sort::Operation::Charge => Ok(false),
                crate::sort::Operation::Less(a, b) => Ok(keys[a] < keys[b]),
                crate::sort::Operation::Swap(a, b) => {
                    keys.swap(a, b);
                    Ok(false)
                }
            }
        })?;
        keys.dedup();
        self.charge_allocation_bytes(SavedDestinations::allocation_bytes(keys.len())?)?;
        let mut saved = SavedDestinations::with_capacity(keys.len())?;
        for key in keys {
            let root = self.snapshot_root(None, key)?;
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
            let active = self.data.resources.output_journal.as_ref();
            if saved.contains_key(&key) || active.is_some_and(|journal| journal.contains_key(&key))
            {
                continue;
            }
            let root = self.snapshot_root(frame, key)?;
            // Admit both journals' growth before cloning or mutation. The saved values
            // are transient rollback snapshots; relocation and shifts are journal
            // bookkeeping (TIME-08).
            let (bytes, _) = saved.insertion_demand(&key)?;
            let (active_bytes, _) =
                active.map_or(Ok((0, 0)), |journal| journal.insertion_demand(&key))?;
            self.charge_allocation_bytes(
                bytes
                    .checked_add(active_bytes)
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
                self.data
                    .storage
                    .restore_direct_slot(location, offset, value);
            }
        }
    }

    pub(super) fn commit_output_transaction<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self, &mut VmFrame) -> Result<T, VmTrap>,
    ) -> Result<T, VmTrap> {
        if self.data.resources.output_journal.is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        }
        self.data.resources.output_journal = Some(SavedDestinations::new());
        let result = action(self, frame);
        let saved = self
            .data
            .resources
            .output_journal
            .take()
            .unwrap_or_default();
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
mod call_path_alias_tests;
#[cfg(test)]
mod single_destination_tests;
#[cfg(test)]
mod work_allowance_tests;

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
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let original = Value::Array(alloc::boxed::Box::new(
            crate::value::ArrayValue::from_canonical_parts(
                vec![Value::DInt(7), Value::DInt(9)],
                vec![(0, 1)],
            ),
        ));
        state
            .data
            .storage
            .set_global("bulk_array", original.clone());
        state
            .data
            .storage
            .set_global("bulk_scalar", Value::DInt(11));
        let root = state.data.storage.ref_for_global("bulk_array").unwrap();
        let mut first = root.clone();
        first.path.push(crate::value::RefSegment::Index(vec![0]));
        let mut second = root.clone();
        second.path.push(crate::value::RefSegment::Index(vec![1]));
        let scalar = state.data.storage.ref_for_global("bulk_scalar").unwrap();
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
        assert!(state
            .data
            .storage
            .write_by_ref_ref(&second, Value::DInt(90)));
        assert!(state.data.storage.write_by_ref_ref(&first, Value::DInt(70)));
        assert!(state
            .data
            .storage
            .write_by_ref_ref(&scalar, Value::DInt(110)));
        state.restore_destinations(None, saved);
        assert_eq!(state.data.storage.get_global("bulk_array"), Some(&original));
        assert_eq!(
            state.data.storage.get_global("bulk_scalar"),
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
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        state.data.storage.set_global("bulk_probe", Value::DInt(7));
        let valid = state.data.storage.ref_for_global("bulk_probe").unwrap();
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
            state.data.storage.get_global("bulk_probe"),
            Some(&Value::DInt(7))
        );
    }

    #[test]
    fn journal_growth_is_not_charged_and_a_later_failure_rolls_back_every_destination() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        state
            .data
            .storage
            .set_global("output_probe", Value::DInt(7));
        let output = state.data.storage.ref_for_global("output_probe").unwrap();
        state
            .data
            .storage
            .set_global("output_probe_extra", Value::DInt(8));
        let extra = state
            .data
            .storage
            .ref_for_global("output_probe_extra")
            .unwrap();
        let id = state.data.storage.reserve_execution_frame().unwrap();
        state.data.storage.activate_execution_frame(id).unwrap();
        let suspended = state.data.storage.reserve_execution_frame().unwrap();
        state
            .data
            .storage
            .suspend_execution_frame(suspended, &mut vec![Value::DInt(11)])
            .unwrap();
        let fresh = state.data.storage.reserve_execution_frame().unwrap();
        state
            .data
            .storage
            .suspend_execution_frame(fresh, &mut vec![Value::DInt(12)])
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
                VmWriteTarget::from_reference(&extra).write(state, frame, Value::DInt(80))?;
                // The journal's first allocation holds four entries, so the fifth
                // destination grows it. That growth is call-transaction bookkeeping
                // (spec 34 TIME-08): an exhausted allocation allowance admits it, and the
                // later failure must still restore every destination.
                state
                    .data
                    .resources
                    .construction_bytes
                    .set(state.prepared.limits.max_construction_bytes);
                VmWriteTarget::DirectStorage {
                    location: MemoryLocation::Local(fresh),
                    offset: 0,
                }
                .write(state, frame, Value::DInt(120))?;
                assert_eq!(
                    state.data.resources.construction_bytes.get(),
                    state.prepared.limits.max_construction_bytes
                );
                Err::<(), _>(VmTrap::Runtime(RuntimeError::DivisionByZero))
            })
            .unwrap_err();
        assert!(
            matches!(&error, VmTrap::Runtime(RuntimeError::DivisionByZero)),
            "unexpected later failure: {error:?}"
        );
        assert_eq!(
            state.data.storage.get_global("output_probe"),
            Some(&Value::DInt(7))
        );
        assert_eq!(
            state.data.storage.get_global("output_probe_extra"),
            Some(&Value::DInt(8))
        );
        assert_eq!(frame.locals, [Value::DInt(9)]);
        assert_eq!(
            state
                .data
                .storage
                .read_direct_slot_by_location(MemoryLocation::Local(suspended), 0),
            Some(&Value::DInt(11))
        );
        assert_eq!(
            state
                .data
                .storage
                .read_direct_slot_by_location(MemoryLocation::Local(fresh), 0),
            Some(&Value::DInt(12))
        );
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
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        state
            .data
            .storage
            .set_global("output_probe", Value::DInt(7));
        let reference = state.data.storage.ref_for_global("output_probe").unwrap();
        let id = state.data.storage.reserve_execution_frame().unwrap();
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
            let before = state.data.resources.construction_bytes.get();
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
            state.data.resources.construction_bytes.get() - before
        };
        let small = measure(&mut state, &mut frame);
        state.data.storage.set_global(
            "unrelated",
            Value::Array(alloc::boxed::Box::new(
                crate::value::ArrayValue::from_canonical_parts(
                    vec![Value::DInt(0); 32768],
                    vec![(0, 32767)],
                ),
            )),
        );
        let large = measure(&mut state, &mut frame);
        // TIME-08: journal growth is bookkeeping, so neither size draws allocation.
        assert_eq!(small, 0, "journal growth draws no allocation");
        assert_eq!(
            small, large,
            "snapshot cost follows destinations, not runtime size"
        );
        assert_eq!(
            state.data.storage.get_global("output_probe"),
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
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let first = state.data.storage.create_instance("Probe");
        let second = state.data.storage.create_instance("Probe");
        state
            .data
            .storage
            .set_instance_var(first, "number", Value::DInt(7));
        state
            .data
            .storage
            .set_instance_var(second, "number", Value::DInt(9));
        state
            .data
            .storage
            .set_global("alias_probe", Value::Instance(first));
        let root = state.data.storage.ref_for_global("alias_probe").unwrap();
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
            state.data.storage.get_global("alias_probe"),
            Some(&Value::Instance(first))
        );
        assert_eq!(
            state.data.storage.get_instance_var(first, "number"),
            Some(&Value::DInt(7))
        );
        assert_eq!(
            state.data.storage.get_instance_var(second, "number"),
            Some(&Value::DInt(9))
        );
        assert!(state.data.resources.output_journal.is_none());
    }
}
