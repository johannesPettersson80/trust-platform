//! Stage bound values/windows, never the unused storage or process image.
use super::super::cycle_port::{CycleAccess, CycleHooks, CyclePhase, OutputUndo};
use super::*;
use crate::io_image::PreparedIoBinding;

/// One staged change of the process image: the bytes or the bit of a flat window, or one
/// hierarchical key. Every check runs when a patch is built, so applying it cannot fail.
pub(in crate::vm::engine) enum OutputPatch {
    Flat {
        area: IoArea,
        start: usize,
        bytes: Vec<u8>,
        bit: Option<u8>,
    },
    Hierarchical(IoAddressKey, Value),
}

/// Write a staged patch into the images. The window was checked against the length of the image
/// it is applied to when the patch was built.
pub(in crate::vm::engine) fn apply_output_patch(images: &mut ProcessImages, patch: OutputPatch) {
    match patch {
        OutputPatch::Hierarchical(key, value) => {
            images.hierarchical.insert(key, value);
        }
        OutputPatch::Flat {
            area,
            start,
            bytes,
            bit,
        } => {
            let image = match area {
                IoArea::Output => &mut images.outputs,
                _ => &mut images.memory,
            };
            if let Some(bit) = bit {
                let mask = 1u8 << bit;
                image[start] = (image[start] & !mask) | (bytes[0] & mask);
            } else {
                image[start..start + bytes.len()].copy_from_slice(&bytes);
            }
        }
    }
}

/// Whether two bindings bind the same variable to the same address through the same codec. The
/// codec follows from the declared type, which a binding-only update never changes.
#[cfg(feature = "std")]
fn same_binding(a: &PreparedIoBinding, b: &PreparedIoBinding) -> bool {
    a.ref_idx == b.ref_idx && a.address == b.address && a.type_id == b.type_id
}

impl EngineState<'_> {
    #[cfg(test)]
    pub(in crate::vm::engine) fn publish_output_image(&mut self) -> Result<(), RuntimeError> {
        self.publish_output_image_with_hooks(&mut super::super::cycle_port::NoopCycleHooks)
    }

    pub(in crate::vm::engine) fn sample_input_image(&mut self) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let count = prepared
            .io_bindings
            .iter()
            .filter(|binding| matches!(binding.address.area, IoArea::Input | IoArea::Memory))
            .count();
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<(ValueRef, Value)>())
                .ok_or_else(|| RuntimeError::Overflow)?,
        )?;
        let mut writes = Vec::new();
        writes
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        for binding in &prepared.io_bindings {
            if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
                continue;
            }
            let reference = self.persistent_io_reference(binding.ref_idx)?;
            let value = binding
                .codec
                .decode(self.read_image_address(&binding.address)?)?;
            self.check_write(reference.as_view(), &value)?;
            let value = match binding.type_id {
                Some(ty) => self.normalize_assignment_value(ty, value)?,
                None => value,
            };
            writes.push((reference, value));
        }
        // Admitted I/O codecs produce scalars or byte strings, never instance or
        // reference roots, so these physical destinations cannot redirect each other.
        let saved = self
            .snapshot_input_destinations(writes.iter().map(|(reference, _)| reference.as_view()))?;
        for (reference, value) in writes {
            let result = self
                .charge_storage_path_write(
                    &self.data.storage,
                    reference.location,
                    reference.offset as usize,
                    &reference.path,
                    &value,
                )
                .and_then(|()| {
                    let prepared = self
                        .data
                        .storage
                        .prepare_by_ref_parts_write(
                            reference.location,
                            reference.offset as usize,
                            &reference.path,
                            &value,
                        )?
                        .ok_or_else(|| RuntimeError::NullReference)?;
                    self.check_entry_deadline()?;
                    self.data
                        .storage
                        .commit_prepared_write(&reference.path, value, prepared)
                        .then_some(())
                        .ok_or_else(|| RuntimeError::NullReference)
                });
            if let Err(error) = result {
                self.restore_destinations(None, saved);
                return Err(error);
            }
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn publish_output_image_with_hooks(
        &mut self,
        hooks: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        self.publish_images(hooks, false)
    }

    /// Publish the value of every variable located at `%M` into the marker image, as the
    /// end-of-cycle synchronization does, and touch nothing else. Construction and every restart
    /// or online change call this once the variables hold their initial or retained values, so
    /// the first scan, which loads the marker image into the bound variables, sees the value the
    /// variable already has (IEC 61131-3 Ed.3 6.5.6; specification 11 6.7). `%I` is left to
    /// the input sample and `%Q` to the first completed scan. The publication is atomic: a
    /// refusal changes no image.
    pub(in crate::vm::engine) fn publish_marker_image(&mut self) -> Result<(), RuntimeError> {
        if !self
            .prepared
            .io_bindings
            .iter()
            .any(|binding| binding.address.area == IoArea::Memory)
        {
            return Ok(());
        }
        self.publish_images(&mut super::super::cycle_port::NoopCycleHooks, true)
    }

    /// The staged, all-or-nothing image publication behind the end of a cycle. `markers_only`
    /// limits it to the `%M` bindings and runs no output hook.
    fn publish_images(
        &mut self,
        hooks: &mut dyn CycleHooks,
        markers_only: bool,
    ) -> Result<(), RuntimeError> {
        let published = |binding: &PreparedIoBinding| {
            let area = binding.address.area;
            area == IoArea::Memory || (area == IoArea::Output && !markers_only)
        };
        let windows = [
            self.data.images.outputs.len(),
            self.data.images.memory.len(),
        ];
        let patches = self.stage_patches(&published, windows)?;
        // The no-op composition retains its original publication checks/cost.
        self.check_entry_deadline()?;
        let output_hooks = hooks.has_output_hooks();
        let mut undo = Vec::new();
        if output_hooks {
            let count = patches
                .iter()
                .filter(|patch| {
                    matches!(
                        patch,
                        OutputPatch::Flat {
                            area: IoArea::Output,
                            ..
                        }
                    )
                })
                .count();
            self.charge_allocation_bytes(
                count
                    .checked_mul(size_of::<OutputUndo>())
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )?;
            undo.try_reserve_exact(count)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            for patch in &patches {
                self.save_output_patch(&mut undo, patch)?;
            }
        }
        // All fallible conversion/allocation/policy work precedes publication.
        for patch in patches {
            apply_output_patch(&mut self.data.images, patch);
        }
        if output_hooks {
            // Provisional output views are callback-scoped. Preserve the hosted
            // rollback domain: flat Q only, not M/hierarchical state effects.
            // Physical effects already completed by a driver cannot be rolled back.
            let result = hooks
                .on_phase(
                    CyclePhase::AfterOutputSample,
                    &mut CycleAccess {
                        engine: self,
                        phase: CyclePhase::AfterOutputSample,
                        output_undo: Some(&mut undo),
                    },
                )
                .and_then(|()| self.check_entry_deadline())
                .and_then(|()| {
                    hooks.publish_outputs(&CycleAccess {
                        engine: self,
                        phase: CyclePhase::AfterOutputSample,
                        output_undo: None,
                    })
                });
            if let Err(error) = result {
                self.restore_output_image(undo);
                return Err(error);
            }
            // Successful submission commits Q. A late deadline/observation
            // failure cannot undo the logical image already accepted by drivers.
            self.check_entry_deadline()?;
            self.cycle_hook(hooks, CyclePhase::CycleEnd)?;
        }
        Ok(())
    }

    /// Stage, in binding order, the patches that publish the variables of the bindings that
    /// `selected` accepts. Every fallible conversion, allocation and window check runs here and no
    /// image changes. `windows` are the lengths of the flat output and marker images that the
    /// patches will be applied to.
    fn stage_patches(
        &mut self,
        selected: &dyn Fn(&PreparedIoBinding) -> bool,
        windows: [usize; 2],
    ) -> Result<Vec<OutputPatch>, RuntimeError> {
        let prepared = self.prepared;
        let mut count = 0usize;
        for binding in &prepared.io_bindings {
            if selected(binding) {
                count += 1;
            }
        }
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<OutputPatch>())
                .ok_or_else(|| RuntimeError::Overflow)?,
        )?;
        let mut patches = Vec::new();
        patches
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        let mut hierarchical_count = 0usize;
        for binding in &prepared.io_bindings {
            if !selected(binding) {
                continue;
            }
            let window = if binding.address.area == IoArea::Output {
                windows[0]
            } else {
                windows[1]
            };
            let patch = self.binding_patch(binding, window)?;
            if matches!(patch, OutputPatch::Hierarchical(..)) {
                hierarchical_count += 1;
            }
            patches.push(patch);
        }
        self.data
            .images
            .hierarchical
            .try_reserve(hierarchical_count)
            .map_err(|_| RuntimeError::Overflow)?;
        Ok(patches)
    }

    /// The patch that publishes the current value of one binding's variable, encoded for the
    /// binding: it changes exactly the bytes, the bit or the hierarchical key that the binding
    /// covers. `window` is the length of the flat image of the binding's area that the patch will
    /// be applied to.
    fn binding_patch(
        &mut self,
        binding: &PreparedIoBinding,
        window: usize,
    ) -> Result<OutputPatch, RuntimeError> {
        let reference = self.persistent_io_reference(binding.ref_idx)?;
        self.check_read(reference.as_view())?;
        let source = self
            .data
            .storage
            .read_by_ref_ref(&reference)
            .ok_or_else(|| RuntimeError::NullReference)?;
        self.charge_allocation_bytes(self.value_clone_charge(source, 0)?)?;
        let value = self
            .data
            .storage
            .materialize_by_ref_ref(&reference)
            .ok_or_else(|| RuntimeError::NullReference)?;
        let value = binding.codec.encode(value, binding.address.size)?;
        if binding.address.path.len() > 1 {
            self.charge_hierarchical_entry(&binding.address)?;
            Ok(OutputPatch::Hierarchical(
                IoAddressKey::from(&binding.address),
                value,
            ))
        } else {
            self.encode_flat_patch(&binding.address, value, window)
        }
    }

    /// Stage what a binding-only update owes the variables it binds to `%M`: the current value of
    /// each marker binding of this module that `previous`, the bindings it replaces, does not
    /// already hold, as the patches that change exactly the bytes, bits or hierarchical keys that
    /// binding covers. A marker binding that the update leaves as it was, and every other bit and
    /// byte, is not touched, so what a driver wrote since the last scan survives. `memory_window`
    /// is the length that the marker image has once the update commits. Nothing is applied here
    /// (specification 11 6.7).
    #[cfg(feature = "std")]
    pub(in crate::vm::engine) fn stage_marker_patches(
        &mut self,
        previous: &[PreparedIoBinding],
        memory_window: usize,
    ) -> Result<Vec<OutputPatch>, RuntimeError> {
        let added_or_changed = |binding: &PreparedIoBinding| {
            binding.address.area == IoArea::Memory
                && !previous.iter().any(|before| same_binding(before, binding))
        };
        let patches = self.stage_patches(&added_or_changed, [0, memory_window])?;
        self.check_entry_deadline()?;
        Ok(patches)
    }

    fn save_output_patch(
        &self,
        undo: &mut Vec<OutputUndo>,
        patch: &OutputPatch,
    ) -> Result<(), RuntimeError> {
        let saved = match patch {
            OutputPatch::Flat {
                area: IoArea::Output,
                start,
                bytes,
                ..
            } => self.output_window_undo(*start, bytes.len())?,
            _ => return Ok(()),
        };
        self.push_output_undo(undo, saved)
    }

    fn output_window_undo(&self, start: usize, width: usize) -> Result<OutputUndo, RuntimeError> {
        let end = start
            .checked_add(width)
            .ok_or_else(|| RuntimeError::Overflow)?;
        let source = self
            .data
            .images
            .outputs
            .get(start..end)
            .ok_or_else(|| RuntimeError::TypeMismatch)?;
        self.charge_allocation_bytes(width)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(width)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        bytes.extend_from_slice(source);
        Ok(OutputUndo { start, bytes })
    }

    fn push_output_undo(
        &self,
        undo: &mut Vec<OutputUndo>,
        value: OutputUndo,
    ) -> Result<(), RuntimeError> {
        if undo.len() == undo.capacity() {
            let count = undo
                .len()
                .checked_add(1)
                .ok_or_else(|| RuntimeError::Overflow)?;
            self.charge_allocation_bytes(
                count
                    .checked_mul(size_of::<OutputUndo>())
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )?;
            undo.try_reserve_exact(1)
                .map_err(|_| RuntimeError::PreparationLimit)?;
        }
        undo.push(value);
        Ok(())
    }

    fn restore_output_image(&mut self, undo: Vec<OutputUndo>) {
        for saved in undo.into_iter().rev() {
            self.data.images.outputs[saved.start..saved.start + saved.bytes.len()]
                .copy_from_slice(&saved.bytes);
        }
    }

    pub(in crate::vm::engine) fn read_cycle_image(
        &mut self,
        address: &IoAddress,
    ) -> Result<Value, RuntimeError> {
        // Process-image reads are bookkeeping under TIME-08 and charge no work;
        // the entry deadline is still checked before and after the read.
        self.check_entry_deadline()?;
        let value = self.read_image_address(address)?;
        self.check_entry_deadline()?;
        Ok(value)
    }

    pub(in crate::vm::engine) fn validate_cycle_image_write(
        &mut self,
        address: &IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            let binding = self
                .prepared
                .io_bindings
                .iter()
                .find(|binding| binding.address == *address)
                .ok_or_else(|| {
                    RuntimeError::InvalidIoAddress("unbound hierarchical image".into())
                })?;
            binding
                .codec
                .encode(binding.codec.decode(value)?, address.size)?;
        } else {
            let len = match address.area {
                IoArea::Input => self.data.images.inputs.len(),
                IoArea::Output => self.data.images.outputs.len(),
                IoArea::Memory => self.data.images.memory.len(),
            };
            self.encode_flat_window(address, value, len)?;
        }
        self.check_entry_deadline()
    }

    pub(in crate::vm::engine) fn write_cycle_image(
        &mut self,
        address: &IoAddress,
        value: Value,
        undo: Option<&mut Vec<OutputUndo>>,
    ) -> Result<(), RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            let binding = self
                .prepared
                .io_bindings
                .iter()
                .find(|binding| binding.address == *address)
                .ok_or_else(|| {
                    RuntimeError::InvalidIoAddress("unbound hierarchical image".into())
                })?;
            // This port accepts wire values; the shared codec validates their
            // declared semantic representation before any image mutation.
            let value = binding
                .codec
                .encode(binding.codec.decode(value)?, address.size)?;
            self.charge_hierarchical_entry(address)?;
            self.data
                .images
                .hierarchical
                .try_reserve(1)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            let key = IoAddressKey::from(address);
            self.check_entry_deadline()?;
            self.data.images.hierarchical.insert(key, value);
        } else {
            let len = match address.area {
                IoArea::Input => self.data.images.inputs.len(),
                IoArea::Output => self.data.images.outputs.len(),
                IoArea::Memory => self.data.images.memory.len(),
            };
            self.check_image_window(address, len)?;
            let end = address
                .flat_byte_end()?
                .ok_or_else(|| RuntimeError::TypeMismatch)? as usize;
            let width = end
                .checked_sub(address.byte as usize)
                .ok_or_else(|| RuntimeError::Overflow)?;
            self.charge_allocation_bytes(width)?;
            // Encode into only the addressed window so rejection cannot change
            // neighboring bytes or require a whole-image rollback copy.
            let mut patch = Vec::new();
            patch
                .try_reserve_exact(width)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            let image = match address.area {
                IoArea::Input => &self.data.images.inputs,
                IoArea::Output => &self.data.images.outputs,
                IoArea::Memory => &self.data.images.memory,
            };
            patch.extend_from_slice(&image[address.byte as usize..end]);
            let local = IoAddress {
                area: address.area,
                size: address.size,
                bit: address.bit,
                byte: 0,
                path: Vec::new(),
                wildcard: false,
            };
            write_flat_image(&mut patch, &local, value)?;
            if address.area == IoArea::Output
                && let Some(undo) = undo
            {
                let saved = self.output_window_undo(address.byte as usize, width)?;
                self.push_output_undo(undo, saved)?;
            }
            self.check_entry_deadline()?;
            let image = match address.area {
                IoArea::Input => &mut self.data.images.inputs,
                IoArea::Output => &mut self.data.images.outputs,
                IoArea::Memory => &mut self.data.images.memory,
            };
            image[address.byte as usize..end].copy_from_slice(&patch);
        }
        Ok(())
    }
    /// The patch for a flat window of `len` bytes, the length of the image of the address's area.
    fn encode_flat_patch(
        &self,
        address: &IoAddress,
        value: Value,
        len: usize,
    ) -> Result<OutputPatch, RuntimeError> {
        if address.area == IoArea::Input {
            return Err(RuntimeError::TypeMismatch);
        }
        let bytes = self.encode_flat_window(address, value, len)?;
        Ok(OutputPatch::Flat {
            area: address.area,
            start: address.byte as usize,
            bytes,
            bit: matches!(address.size, crate::io_address::IoSize::Bit).then_some(address.bit),
        })
    }

    fn encode_flat_window(
        &self,
        address: &IoAddress,
        value: Value,
        len: usize,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.check_image_window(address, len)?;
        let end = address
            .flat_byte_end()?
            .ok_or_else(|| RuntimeError::TypeMismatch)? as usize;
        let width = end - address.byte as usize;
        self.charge_allocation_bytes(width)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(width)
            .map_err(|_| RuntimeError::Overflow)?;
        bytes.resize(width, 0);
        let local = IoAddress {
            area: address.area,
            size: address.size,
            bit: address.bit,
            byte: 0,
            path: Vec::new(),
            wildcard: false,
        };
        write_flat_image(&mut bytes, &local, value)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests;
