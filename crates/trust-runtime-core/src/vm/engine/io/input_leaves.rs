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
use super::*;
use crate::io_address::IoSize;
use crate::io_image::PreparedIoBinding;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::vm::prepared::{InputLeaves, IoLeaf};

impl<'a> EngineState<'a> {
    /// Sample every input and marker binding into its leaf, as the general synchronization samples
    /// them (`staging::sample_inputs`), keeping the leaves' bytes in `saved`, the reserved snapshot
    /// of the destinations.
    pub(super) fn sample_input_leaves(
        &mut self,
        saved: &mut SavedDestinations,
        leaves: InputLeaves,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let count = leaves.count as usize;
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<(ValueRef, Value)>())
                .ok_or_else(|| RuntimeError::Overflow)?,
        )?;
        for (index, binding) in prepared.io_bindings.iter().enumerate() {
            match self.input_leaf(index, binding) {
                // A copied binding decodes and normalizes to its own bits: only the checks of its
                // image read can refuse it.
                Some(leaf) if leaf.copy => {
                    self.copied_input(binding, leaf)?;
                }
                Some(_) => {
                    self.input_value(binding)?;
                }
                None => {}
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
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )?;
            SavedDestinations::allocation_bytes(count)?
        };
        self.charge_allocation_bytes(entries)?;
        saved.clear();
        let mut written = 0;
        let mut root = RootRange::default();
        for (index, binding) in prepared.io_bindings.iter().enumerate() {
            let Some(leaf) = self.input_leaf(index, binding) else {
                continue;
            };
            let result = if leaf.copy {
                self.copied_input(binding, leaf).and_then(|bytes| {
                    self.write_input_leaf(saved, leaf, &mut root, |engine, range| {
                        engine
                            .data
                            .store
                            .write_bytes(range, &bytes[..usize::from(leaf.bytes)])
                    })
                })
            } else {
                self.input_value(binding).and_then(|value| {
                    self.write_input_leaf(saved, leaf, &mut root, |engine, range| {
                        engine.write_scalar_leaf(*range, leaf.prim, &value)
                    })
                })
            };
            if let Err(error) = result {
                self.put_back_input_leaves(saved, written);
                return Err(error);
            }
            written += 1;
        }
        Ok(())
    }

    /// The leaf of binding `index`, `binding`, when it is an input or a marker, borrowed from the
    /// prepared module: a leaf is not copied, which at `opt-level = "z"` is a `memcpy` call.
    fn input_leaf(&self, index: usize, binding: &PreparedIoBinding) -> Option<&'a IoLeaf> {
        if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
            return None;
        }
        // The prepared module outlives this borrow of the engine (`EngineState::prepared`).
        let prepared = self.prepared;
        prepared.io_leaves.get(index)?.as_ref()
    }

    /// One input write: the deadline check, then the leaf's bytes kept in `saved`, so a refusal can
    /// put them back, then `write` of the leaf's bytes, refused when it writes nothing. The range is
    /// handed to `write`, not returned, and lent rather than passed by value: a range in a returned
    /// result or passed by value is a `memcpy` call at `opt-level = "z"`.
    fn write_input_leaf(
        &mut self,
        saved: &mut SavedDestinations,
        leaf: &IoLeaf,
        root: &mut RootRange,
        write: impl FnOnce(&mut Self, &ByteRange) -> bool,
    ) -> Result<(), RuntimeError> {
        self.check_entry_deadline()?;
        let range = self
            .leaf_range_in(leaf, root)
            .ok_or_else(|| RuntimeError::NullReference)?;
        let held = self
            .data
            .store
            .bytes(&range)
            .ok_or_else(|| RuntimeError::NullReference)?;
        saved.keep(held)?;
        if write(self, &range) {
            Ok(())
        } else {
            Err(RuntimeError::NullReference)
        }
    }

    /// The leaf bytes of copied binding `binding` (`IoLeaf::copy`), read from its image after the
    /// checks of a read of its address: the image's little-endian bytes, or for a bit the BOOL
    /// leaf's 1 or 0. A window that preparation checked (`IoLeaf::window`) needs only the image's
    /// length checked; the checks run when it is shorter and refuse the binding as before.
    fn copied_input(
        &self,
        binding: &PreparedIoBinding,
        leaf: &IoLeaf,
    ) -> Result<[u8; 8], RuntimeError> {
        let address = &binding.address;
        let image = match address.area {
            IoArea::Input => &self.data.images.inputs,
            IoArea::Output => &self.data.images.outputs,
            IoArea::Memory => &self.data.images.memory,
        };
        if leaf
            .window
            .is_none_or(|window| window.end as usize > image.len())
        {
            crate::io_image::validate_process_image_address(address)?;
            crate::io_image::check_image_window(address, image.len())?;
        }
        let start = address.byte as usize;
        let len = usize::from(leaf.bytes);
        let Some(window) = image.get(start..start + len) else {
            return Err(RuntimeError::NullReference);
        };
        let mut bytes = [0u8; 8];
        if address.size == IoSize::Bit {
            bytes[0] = (window[0] >> address.bit) & 1;
        } else {
            crate::memory::flat::copy_leaf_bytes(&mut bytes[..len], window);
        }
        Ok(bytes)
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

    /// Put back the bytes that `saved` kept for the first `written` input leaves. No two inputs
    /// write one variable, so each leaf gets the bytes it held before the scan.
    fn put_back_input_leaves(&mut self, saved: &SavedDestinations, written: usize) {
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
            if let (Some(range), Some(held)) = (self.leaf_range(leaf), saved.kept(at, len)) {
                self.data.store.write_bytes(&range, held);
            }
            at += len;
        }
    }

    /// The bytes of `leaf` in the persistent block, or in the instance of its construction root.
    pub(super) fn leaf_range(&self, leaf: &IoLeaf) -> Option<ByteRange> {
        self.leaf_range_in(leaf, &mut RootRange::default())
    }

    /// [`Self::leaf_range`], finding the instance of the leaf's construction root through `root`,
    /// which keeps where the instance of the root it found last starts.
    pub(super) fn leaf_range_in(&self, leaf: &IoLeaf, root: &mut RootRange) -> Option<ByteRange> {
        let (region, start) = match leaf.root {
            None => (LayoutRegion::Persistent, 0),
            Some(index) => {
                if root.root != Some(index) {
                    let instance = (*self.data.construction.roots.get(index as usize)?)?;
                    let range = self.data.reader(self.prepared).instance_range(instance)?;
                    root.root = Some(index);
                    root.region = range.region;
                    root.start = range.start;
                }
                (root.region, root.start)
            }
        };
        Some(ByteRange {
            region,
            start: start.checked_add(leaf.offset)?,
            bytes: u32::from(leaf.bytes),
        })
    }
}

/// The construction root whose instance a pass over the bindings found last, and where that
/// instance's bytes start. The bindings of one program's variables name one root, so the pass finds
/// its instance once (checklist P1-6); a root's instance is persistent and keeps its bytes while the
/// pass runs. The fields are kept apart, so that no whole range is copied (a `memcpy` call at
/// `opt-level = "z"`).
#[derive(Debug)]
pub(super) struct RootRange {
    root: Option<u32>,
    region: LayoutRegion,
    start: u32,
}

impl Default for RootRange {
    fn default() -> Self {
        Self {
            root: None,
            region: LayoutRegion::Persistent,
            start: 0,
        }
    }
}
