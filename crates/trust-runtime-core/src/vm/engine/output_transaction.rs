//! Output copy-back journals only physical destination slots.
use super::*;
use crate::memory::MemoryLocation;
use crate::value::ValueRefView;
use crate::vm::VmTrap;

pub(super) type SavedDestinations = BTreeMap<(MemoryLocation, usize), Value>;

fn lookup_work(count: usize) -> usize {
    // B-tree nodes compare a bounded number of fixed-size location/slot keys.
    12 * (count.max(1).ilog2() as usize + 1)
}

impl EngineState<'_> {
    pub(super) fn snapshot_destinations<'r>(
        &self,
        frame: Option<&VmFrame>,
        references: impl Iterator<Item = ValueRefView<'r>>,
    ) -> Result<SavedDestinations, RuntimeError> {
        let mut saved = SavedDestinations::new();
        for reference in references {
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
            let key = (resolved.location, resolved.offset);
            let active = self.resources.output_journal.as_ref();
            self.charge_work_units(
                lookup_work(saved.len()) + active.map_or(0, |journal| lookup_work(journal.len())),
            )?;
            if saved.contains_key(&key) || active.is_some_and(|journal| journal.contains_key(&key))
            {
                continue;
            }
            let root = if frame.is_some_and(|frame| {
                resolved.location == MemoryLocation::Local(frame.reference_frame_id())
            }) {
                frame.and_then(|frame| frame.locals.get(resolved.offset))
            } else {
                self.storage
                    .read_direct_slot_by_location(resolved.location, resolved.offset)
            }
            .ok_or(RuntimeError::NullReference)?;
            // Native copy-back transfers the temporary entry into its active
            // journal; charge both map insertions before either can allocate.
            let copies = if active.is_some() { 2 } else { 1 };
            self.charge_work_units(
                lookup_work(saved.len()) + active.map_or(0, |journal| lookup_work(journal.len())),
            )?;
            self.charge_allocation_bytes(
                copies * 4 * core::mem::size_of::<((MemoryLocation, usize), Value)>()
                    + self.value_clone_charge(root, 0)?,
            )?;
            saved.insert(key, root.clone());
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
