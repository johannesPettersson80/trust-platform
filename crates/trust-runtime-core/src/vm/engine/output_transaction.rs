//! Output copy-back journals only physical destination slots.
use super::destinations::Key;
use super::*;
use crate::memory::cell_access::CellsView;
use crate::memory::layout::ByteRange;
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::{IoArea, MemoryLocation};
use crate::value::{RefSegment, ValueRefView};
use crate::vm::VmTrap;

use super::destinations::Saved;
pub(super) use super::destinations::SavedDestinations;

/// The cells of `root` when a snapshot saves it as its bytes: a root that lies in cells whose bytes
/// hold its whole value (C1-16). A reference or an instance in cells is kept partly beside its bytes,
/// so a root that holds one is saved as its value.
fn cells_of_root<'v>(
    prepared: &PreparedModule,
    root: &ValueView<'v>,
) -> Option<(ByteRange, CellsView<'v>)> {
    match *root {
        ValueView::Cells {
            cells,
            range,
            type_id,
        } if prepared.type_layout(type_id).is_some_and(|layout| {
            layout.reference_leaves.is_empty() && layout.instance_leaves.is_empty()
        }) =>
        {
            Some((range, cells))
        }
        _ => None,
    }
}

/// Save the root `key`, whose view is `root`: as its bytes when [`cells_of_root`] gives its cells,
/// so that the snapshot builds no value, and as its value otherwise.
fn save_root(
    prepared: &PreparedModule,
    saved: &mut SavedDestinations,
    key: Key,
    root: ValueView<'_>,
) -> Result<(), RuntimeError> {
    match cells_of_root(prepared, &root) {
        Some((range, cells)) => {
            let held = cells
                .bytes(range)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            saved.insert_cells(key, range, held)
        }
        None => saved.insert(key, root.materialize()?).map(drop),
    }
}

impl EngineState<'_> {
    /// Save a single write's original root directly in the active journal.
    /// Admission and cloning finish before insertion; a later failure still
    /// restores the first snapshot of each physical destination.
    pub(super) fn journal_output_destination(
        &mut self,
        frame: &VmFrame,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        // The journal is set aside while the root is read, so that the root's view and the
        // journal it is saved in are borrowed apart; nothing it calls reads the journal.
        let Some(mut journal) = self.data.resources.output_journal.take() else {
            return Ok(());
        };
        let saved = self.journal_root(frame, reference, &mut journal);
        self.data.resources.output_journal = Some(journal);
        saved
    }

    /// Save in `journal` the root of `reference` that it does not hold yet: as its bytes when it
    /// lies in cells and holds no reference and no instance, so that the save builds no value
    /// (C1-16, C1-19c4), and as its value otherwise.
    fn journal_root(
        &self,
        frame: &VmFrame,
        reference: ValueRefView<'_>,
        journal: &mut SavedDestinations,
    ) -> Result<(), RuntimeError> {
        let key = self.snapshot_key(Some(frame), reference)?;
        if journal.contains_key(&key) {
            return Ok(());
        }
        let root = self.snapshot_root(Some(frame), key)?;
        // The saved root and the journal's growth are call-transaction bookkeeping:
        // neither the work allowance nor the cumulative allocation charge admits them
        // (spec 34 TIME-08). Insertion can still refuse allocation, in which case the
        // destination is untouched.
        save_root(self.prepared, journal, key, root)
    }

    fn snapshot_key(
        &self,
        frame: Option<&VmFrame>,
        reference: ValueRefView<'_>,
    ) -> Result<Key, RuntimeError> {
        // A slot on the activation stack is resolved through the store, as any other (C1-11).
        let is_current = frame.is_some_and(|frame| {
            reference.location == MemoryLocation::Local(frame.reference_frame_id())
                && self
                    .frame_slot_activation(frame, reference.offset)
                    .is_none()
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
                    .ref_for_instance_recursive(*instance, name)
                    .ok_or(RuntimeError::NullReference)?;
                self.resolve_root(field.location, field.offset as usize, &reference.path[1..])
            } else {
                Some((reference.location, reference.offset))
            }
        } else {
            self.resolve_root(reference.location, reference.offset, reference.path)
        };
        resolved.ok_or(RuntimeError::NullReference)
    }

    /// The slot that holds the value `path` names inside the slot at `location`/`offset`, once the
    /// instance hops of the path are followed, as its location and offset: the key of a journaled
    /// destination (TIME-08). A field of a value that holds an instance is a member of that instance, or
    /// of its nearest ancestor that declares it, so the walk goes on from that member's slot. `None` when
    /// a step of the path names no value. It reads through the store the engine holds, so the cell arm
    /// follows the hops through its cells and its instance table (C1-9d3a, R1).
    fn resolve_root(
        &self,
        mut location: MemoryLocation,
        mut offset: usize,
        path: &[RefSegment],
    ) -> Option<Key> {
        let mut start = 0;
        for (index, segment) in path.iter().enumerate() {
            let RefSegment::Field(name) = segment else {
                continue;
            };
            let held = self
                .read_path(location, offset, &path[start..index])
                .ok()?
                .instance()
                .ok()?;
            if let Some(instance) = held {
                let hop = self.ref_for_instance_recursive(instance, name)?;
                location = hop.location;
                offset = hop.offset as usize;
                start = index + 1;
            }
        }
        Some((location, offset))
    }

    fn snapshot_root<'s>(
        &'s self,
        frame: Option<&'s VmFrame>,
        key: Key,
    ) -> Result<ValueView<'s>, RuntimeError> {
        if frame.is_some_and(|frame| {
            key.0 == MemoryLocation::Local(frame.reference_frame_id())
                && self.frame_slot_activation(frame, key.1).is_none()
        }) {
            return frame
                .and_then(|frame| frame.locals.get(key.1))
                .map(ValueView::Value)
                .ok_or(RuntimeError::NullReference);
        }
        self.read_slot(key.0, key.1)
            .map_err(|_| RuntimeError::NullReference)
    }

    /// Reserve the bytes that the snapshot of the input destinations saves at a scan start: the cells
    /// of every distinct root an input writes that it saves as its bytes, charged as the cell store's
    /// own bytes are (specification 34 MEM-09; C1-16). A destination that does not resolve yet is
    /// left to the scan, which refuses it as before.
    pub(super) fn reserve_input_snapshot(&mut self) -> Result<(), RuntimeError> {
        if !matches!(self.data.store, EngineStore::Cells(_)) {
            return Ok(());
        }
        let inputs = self
            .prepared
            .io_bindings
            .iter()
            .filter(|binding| matches!(binding.address.area, IoArea::Input | IoArea::Memory));
        let mut roots: Vec<(Key, u32)> = Vec::new();
        roots
            .try_reserve_exact(inputs.clone().count())
            .map_err(|_| RuntimeError::Overflow)?;
        for binding in inputs {
            let Ok(reference) = self.persistent_io_target(binding.ref_idx) else {
                continue;
            };
            let Ok(key) = self.snapshot_key(None, reference) else {
                continue;
            };
            let Ok(root) = self.snapshot_root(None, key) else {
                continue;
            };
            if let Some((range, _)) = cells_of_root(self.prepared, &root) {
                roots.push((key, range.bytes));
            }
        }
        roots.sort_unstable_by_key(|(key, _)| *key);
        roots.dedup_by_key(|(key, _)| *key);
        let bytes = roots
            .iter()
            .try_fold(0usize, |sum, (_, bytes)| sum.checked_add(*bytes as usize))
            .ok_or(RuntimeError::Overflow)?;
        self.charge_allocation_bytes(bytes)?;
        self.data.resources.staging.reserve_snapshot_bytes(bytes)
    }

    /// Resolve input destinations in their original order, then copy each root once into
    /// `saved`, with `keys` as sorting scratch; both keep their storage. Sorting identities
    /// before copying avoids quadratic insertion shifts regardless of binding order. This
    /// never reorders the staged writes themselves. The charges are those of a new key list
    /// and snapshot (spec 34 TIME-08).
    pub(super) fn snapshot_input_destinations<'r>(
        &self,
        references: impl ExactSizeIterator<Item = ValueRefView<'r>>,
        keys: &mut Vec<Key>,
        saved: &mut SavedDestinations,
    ) -> Result<(), RuntimeError> {
        if references.len() <= 1 {
            saved.clear();
            return self.snapshot_destinations(None, references, saved);
        }
        self.charge_allocation_bytes(
            references
                .len()
                .checked_mul(core::mem::size_of::<Key>())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        keys.clear();
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
        saved.reuse(keys.len())?;
        for &key in keys.iter() {
            let root = self.snapshot_root(None, key)?;
            // Keys are unique and sorted; reserved capacity covers every append.
            save_root(self.prepared, saved, key, root)?;
        }
        Ok(())
    }

    /// Add the roots of `references` that neither `saved` nor the active output journal
    /// holds yet.
    pub(super) fn snapshot_destinations<'r>(
        &self,
        frame: Option<&VmFrame>,
        references: impl Iterator<Item = ValueRefView<'r>>,
        saved: &mut SavedDestinations,
    ) -> Result<(), RuntimeError> {
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
            save_root(self.prepared, saved, key, root)?;
        }
        Ok(())
    }

    /// Write every saved root back, emptying `saved` but keeping its storage.
    pub(super) fn restore_destinations(
        &mut self,
        mut frame: Option<&mut VmFrame>,
        saved: &mut SavedDestinations,
    ) {
        let (roots, bytes) = saved.drain();
        for ((location, offset), root) in roots {
            let value = match root {
                Saved::Value(value) => value,
                // A root saved as its cells gets its bytes back (C1-16).
                Saved::Cells { range, at } => {
                    if let (EngineStore::Cells(arm), Some(held)) = (
                        &mut self.data.store,
                        bytes.get(at..at + range.bytes as usize),
                    ) {
                        arm.write_bytes(range, held);
                    }
                    continue;
                }
            };
            let current = frame.as_deref().is_some_and(|frame| {
                location == MemoryLocation::Local(frame.reference_frame_id())
                    && self.frame_slot_activation(frame, offset).is_none()
            });
            match frame.as_deref_mut().filter(|_| current) {
                Some(frame) => frame.locals[offset] = value,
                // A slot on the activation stack is restored through the store (C1-11).
                None => self.restore_slot(location, offset, value),
            }
        }
        saved.forget_bytes();
    }

    pub(super) fn commit_output_transaction<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self, &mut VmFrame) -> Result<T, VmTrap>,
    ) -> Result<T, VmTrap> {
        if self.data.resources.output_journal.is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        }
        // The journal kept from the last transaction starts out as a new one (census C11).
        let mut journal = core::mem::take(&mut self.data.resources.spare_journal);
        journal.clear();
        self.data.resources.output_journal = Some(journal);
        let result = action(self, frame);
        let mut saved = self
            .data
            .resources
            .output_journal
            .take()
            .unwrap_or_default();
        if result.is_err() {
            self.restore_destinations(Some(frame), &mut saved);
        }
        saved.clear();
        self.data.resources.spare_journal = saved;
        result
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
        let (mut keys, mut saved) = (Vec::new(), SavedDestinations::new());
        state
            .snapshot_input_destinations(
                references.iter().map(|r| r.as_view()),
                &mut keys,
                &mut saved,
            )
            .unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(
            saved.keys().copied().collect::<Vec<_>>(),
            vec![
                (root.location, root.offset as usize),
                (scalar.location, scalar.offset as usize)
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
        state.restore_destinations(None, &mut saved);
        assert_eq!(saved.len(), 0);
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
        invalid.offset = u32::MAX;
        let references = [&valid, &invalid, &valid];
        let visits = core::cell::Cell::new(0);
        let (mut keys, mut saved) = (Vec::new(), SavedDestinations::new());
        let result = state.snapshot_input_destinations(
            references.iter().map(|reference| {
                visits.set(visits.get() + 1);
                reference.as_view()
            }),
            &mut keys,
            &mut saved,
        );
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
