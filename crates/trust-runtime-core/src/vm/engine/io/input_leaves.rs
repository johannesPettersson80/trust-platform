//! Inputs and markers sampled into their variables' leaves (checklist P1-6; specification 34
//! PERF-05 and MEM-12).
//!
//! When preparation found that every input and marker binding names, without a path, a writable
//! scalar variable of the primitive the binding declares, through a flat address of a scalar, and
//! that no two name one variable ([`InputLeaves`]), a scan decodes each binding from its image and
//! writes the value into the variable's leaf. It charges what the general synchronization charges,
//! in the same order: the staged pairs, the decoding and normalizing of every value, the snapshot of
//! the destinations, and before each write the deadline. A refusal while writing puts back the
//! bytes of the leaves written before it, as the snapshot does, so the variables are as they were.

use super::super::destinations::{Key, SavedDestinations};
use super::staging::ImageStaging;
use super::*;
use crate::io_image::PreparedIoBinding;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::vm::prepared::{InputLeaves, IoLeaf};

impl EngineState<'_> {
    /// Sample every input and marker binding into its leaf, as the general synchronization samples
    /// them (`staging::sample_inputs`).
    pub(super) fn sample_input_leaves(
        &mut self,
        staging: &mut ImageStaging,
        leaves: InputLeaves,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let count = leaves.count as usize;
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<(ValueRef, Value)>())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        for (index, binding) in prepared.io_bindings.iter().enumerate() {
            if self.input_leaf(index, binding).is_some() {
                self.input_value(binding)?;
            }
        }
        // The snapshot of the destinations charges a new snapshot's entries: room for four when one
        // input saves its variable, else each input's key and each variable's entry.
        let entries = if count <= 1 {
            SavedDestinations::allocation_bytes(4)?
        } else {
            self.charge_allocation_bytes(
                count
                    .checked_mul(size_of::<Key>())
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            SavedDestinations::allocation_bytes(count)?
        };
        self.charge_allocation_bytes(entries)?;
        staging.saved.clear();
        let mut written = 0;
        for (index, binding) in prepared.io_bindings.iter().enumerate() {
            let Some(leaf) = self.input_leaf(index, binding) else {
                continue;
            };
            let result = self.input_value(binding).and_then(|value| {
                self.check_entry_deadline()?;
                let range = self.leaf_range(leaf).ok_or(RuntimeError::NullReference)?;
                let held = self
                    .data
                    .store
                    .bytes(range)
                    .ok_or(RuntimeError::NullReference)?;
                staging.saved.keep(held)?;
                if self.write_scalar_leaf(range, leaf.prim, &value) {
                    Ok(())
                } else {
                    Err(RuntimeError::NullReference)
                }
            });
            if let Err(error) = result {
                self.put_back_input_leaves(staging, written);
                return Err(error);
            }
            written += 1;
        }
        Ok(())
    }

    /// The leaf of binding `index`, `binding`, when it is an input or a marker.
    fn input_leaf(&self, index: usize, binding: &PreparedIoBinding) -> Option<IoLeaf> {
        if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
            return None;
        }
        self.prepared.io_leaves.get(index).copied().flatten()
    }

    /// The value of `binding` in its image, decoded and normalized for its declared type, as the
    /// general synchronization stages it.
    fn input_value(&mut self, binding: &PreparedIoBinding) -> Result<Value, RuntimeError> {
        let value = binding
            .codec
            .decode(self.read_image_address(&binding.address)?)?;
        match binding.type_id {
            Some(ty) => self.normalize_assignment_value(ty, value),
            None => Ok(value),
        }
    }

    /// Put back the bytes that `staging` kept for the first `written` input leaves. No two inputs
    /// write one variable, so each leaf gets the bytes it held before the scan.
    fn put_back_input_leaves(&mut self, staging: &ImageStaging, written: usize) {
        let prepared = self.prepared;
        let mut at = 0usize;
        let mut left = written;
        for (index, binding) in prepared.io_bindings.iter().enumerate() {
            if left == 0 {
                break;
            }
            let Some(leaf) = self.input_leaf(index, binding) else {
                continue;
            };
            left -= 1;
            let len = usize::from(leaf.bytes);
            if let (Some(range), Some(held)) = (self.leaf_range(leaf), staging.saved.kept(at, len))
            {
                self.data.store.write_bytes(range, held);
            }
            at += len;
        }
    }

    /// The bytes of `leaf` in the persistent block, or in the instance of its construction root.
    pub(super) fn leaf_range(&self, leaf: IoLeaf) -> Option<ByteRange> {
        let (region, start) = match leaf.root {
            None => (LayoutRegion::Persistent, 0),
            Some(root) => {
                let instance = (*self.data.construction.roots.get(root as usize)?)?;
                let range = self.data.reader(self.prepared).instance_range(instance)?;
                (range.region, range.start)
            }
        };
        Some(ByteRange {
            region,
            start: start.checked_add(leaf.offset)?,
            bytes: u32::from(leaf.bytes),
        })
    }
}
