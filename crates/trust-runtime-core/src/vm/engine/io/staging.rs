//! Stage bound values/windows, never the unused storage or process image.
use super::*;

enum OutputPatch {
    Flat {
        area: IoArea,
        start: usize,
        bytes: Vec<u8>,
        bit: Option<u8>,
    },
    Hierarchical(IoAddressKey, Value),
}

impl EngineState<'_> {
    pub(in crate::vm::engine) fn sample_input_image(&mut self) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        self.charge_work_units(prepared.io_bindings.len())?;
        let count = prepared
            .io_bindings
            .iter()
            .filter(|binding| matches!(binding.address.area, IoArea::Input | IoArea::Memory))
            .count();
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<(ValueRef, Value)>())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let mut writes = Vec::new();
        writes
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        for binding in &prepared.io_bindings {
            self.charge_work_units(1)?;
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
                    &self.storage,
                    reference.location,
                    reference.offset,
                    &reference.path,
                )
                .and_then(|()| {
                    self.storage
                        .write_by_ref_ref(&reference, value)
                        .then_some(())
                        .ok_or(RuntimeError::NullReference)
                });
            if let Err(error) = result {
                self.restore_destinations(None, saved);
                return Err(error);
            }
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn publish_output_image(&mut self) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        self.charge_work_units(prepared.io_bindings.len())?;
        let count = prepared
            .io_bindings
            .iter()
            .filter(|binding| matches!(binding.address.area, IoArea::Output | IoArea::Memory))
            .count();
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<OutputPatch>())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let mut patches = Vec::new();
        patches
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        let mut hierarchical_count = 0usize;
        for binding in &prepared.io_bindings {
            self.charge_work_units(1)?;
            if !matches!(binding.address.area, IoArea::Output | IoArea::Memory) {
                continue;
            }
            let reference = self.persistent_io_reference(binding.ref_idx)?;
            self.check_read(reference.as_view())?;
            let source = self
                .storage
                .read_by_ref_ref(&reference)
                .ok_or(RuntimeError::NullReference)?;
            self.charge_allocation_bytes(self.value_clone_charge(source, 0)?)?;
            let value = self
                .storage
                .materialize_by_ref_ref(&reference)
                .ok_or(RuntimeError::NullReference)?;
            let value = binding.codec.encode(value, binding.address.size)?;
            let patch = if binding.address.path.len() > 1 {
                self.charge_hierarchical_entry(&binding.address)?;
                hierarchical_count += 1;
                OutputPatch::Hierarchical(IoAddressKey::from(&binding.address), value)
            } else {
                self.encode_flat_patch(&binding.address, value)?
            };
            patches.push(patch);
        }
        self.images
            .hierarchical
            .try_reserve(hierarchical_count)
            .map_err(|_| RuntimeError::Overflow)?;
        // Completion is the final prepublication boundary: no failing clock
        // sample may report an error after new output bytes become observable.
        self.check_entry_deadline()?;
        // All fallible conversion/allocation/policy work precedes publication.
        for patch in patches {
            match patch {
                OutputPatch::Hierarchical(key, value) => {
                    self.images.hierarchical.insert(key, value);
                }
                OutputPatch::Flat {
                    area,
                    start,
                    bytes,
                    bit,
                } => {
                    let image = match area {
                        IoArea::Output => &mut self.images.outputs,
                        _ => &mut self.images.memory,
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
        Ok(())
    }
    fn encode_flat_patch(
        &self,
        address: &IoAddress,
        value: Value,
    ) -> Result<OutputPatch, RuntimeError> {
        let len = match address.area {
            IoArea::Output => self.images.outputs.len(),
            IoArea::Memory => self.images.memory.len(),
            IoArea::Input => return Err(RuntimeError::TypeMismatch),
        };
        self.check_image_window(address, len)?;
        let end = address.flat_byte_end()?.ok_or(RuntimeError::TypeMismatch)? as usize;
        let width = end - address.byte as usize;
        self.charge_work_units(width)?;
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
        Ok(OutputPatch::Flat {
            area: address.area,
            start: address.byte as usize,
            bytes,
            bit: matches!(address.size, crate::io_address::IoSize::Bit).then_some(address.bit),
        })
    }
}
