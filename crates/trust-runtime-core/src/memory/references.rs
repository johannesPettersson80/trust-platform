use super::*;

impl VariableStorage {
    pub fn read_direct_slot_by_location(
        &self,
        location: MemoryLocation,
        offset: usize,
    ) -> Option<&Value> {
        match location {
            MemoryLocation::Global => self
                .values
                .globals
                .get_index(offset)
                .map(|(_, value)| value),
            MemoryLocation::Local(frame_id) => self.local_slot(frame_id, offset),
            MemoryLocation::Instance(instance_id) => self
                .activations
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.variables.get_index(offset).map(|(_, value)| value)),
            MemoryLocation::Io(_) | MemoryLocation::Retain => None,
        }
    }

    pub fn read_global_slot_by_offset(&self, offset: usize) -> Option<&Value> {
        self.values
            .globals
            .get_index(offset)
            .map(|(_, value)| value)
    }

    pub fn write_direct_slot_by_location(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        value: Value,
    ) -> bool {
        match location {
            MemoryLocation::Global => self
                .values
                .globals
                .get_index_mut(offset)
                .map(|(_, slot)| {
                    *slot = crate::value::normalize_assignment_for_target(slot, value);
                })
                .is_some(),
            MemoryLocation::Local(frame_id) => self
                .local_slot_mut(frame_id, offset)
                .map(|slot| {
                    *slot = crate::value::normalize_assignment_for_target(slot, value);
                })
                .is_some(),
            MemoryLocation::Instance(instance_id) => self
                .activations
                .instances
                .get_mut(&instance_id)
                .and_then(|instance| {
                    instance.variables.get_index_mut(offset).map(|(_, slot)| {
                        *slot = crate::value::normalize_assignment_for_target(slot, value);
                    })
                })
                .is_some(),
            MemoryLocation::Io(_) | MemoryLocation::Retain => false,
        }
    }

    pub fn write_global_slot_by_offset(&mut self, offset: usize, value: Value) -> bool {
        self.values
            .globals
            .get_index_mut(offset)
            .map(|(_, slot)| {
                *slot = crate::value::normalize_assignment_for_target(slot, value);
            })
            .is_some()
    }

    pub fn read_by_ref(&self, value_ref: crate::value::ValueRef) -> Option<&Value> {
        self.read_by_ref_ref(&value_ref)
    }

    /// Read through a reference. A reference of another installation generation
    /// names no state here and reads nothing (specification 34 §11.3).
    pub fn read_by_ref_ref(&self, value_ref: &crate::value::ValueRef) -> Option<&Value> {
        if value_ref.generation != self.identities.generation {
            return None;
        }
        self.read_by_ref_parts(
            value_ref.location,
            value_ref.offset as usize,
            &value_ref.path,
        )
    }

    pub fn materialize_by_ref(&self, value_ref: crate::value::ValueRef) -> Option<Value> {
        self.materialize_by_ref_ref(&value_ref)
    }

    /// Copy the value a reference names. A reference of another generation names
    /// no state here.
    pub fn materialize_by_ref_ref(&self, value_ref: &crate::value::ValueRef) -> Option<Value> {
        if value_ref.generation != self.identities.generation {
            return None;
        }
        self.materialize_by_ref_parts(
            value_ref.location,
            value_ref.offset as usize,
            &value_ref.path,
        )
    }

    /// Read a slot of this storage's own generation by its parts. Callers that hold
    /// a `ValueRef` use `read_by_ref_ref`, which checks the reference's generation.
    pub fn read_by_ref_parts(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Option<&Value> {
        if path.is_empty() {
            return self.read_direct_slot_by_location(location, offset);
        }

        let resolved = self.resolve_reference_parts(location, offset, path)?;
        let root = self.read_direct_slot_by_location(resolved.location, resolved.offset)?;

        read_value_path_borrowed(root, resolved.path)
    }

    pub fn materialize_by_ref_parts(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Option<Value> {
        if path.is_empty() {
            return self.read_direct_slot_by_location(location, offset).cloned();
        }

        let resolved = self.resolve_reference_parts(location, offset, path)?;
        let root = self.read_direct_slot_by_location(resolved.location, resolved.offset)?;
        materialize_value_path(root, resolved.path)
    }

    pub fn write_by_ref(&mut self, value_ref: crate::value::ValueRef, value: Value) -> bool {
        self.write_by_ref_ref(&value_ref, value)
    }

    /// Write through a reference. A reference of another generation names no state
    /// here, so nothing is written.
    pub fn write_by_ref_ref(&mut self, value_ref: &crate::value::ValueRef, value: Value) -> bool {
        if value_ref.generation != self.identities.generation {
            return false;
        }
        self.write_by_ref_parts(
            value_ref.location,
            value_ref.offset as usize,
            &value_ref.path,
            value,
        )
    }

    pub fn write_by_ref_parts(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> bool {
        self.write_by_ref_parts_checked(location, offset, path, value)
            .unwrap_or(false)
    }

    /// Preserve storage rejection identity instead of translating it to a missing reference.
    /// A reference of another generation is refused with `ReferenceLifetime`.
    pub fn write_by_ref_ref_checked(
        &mut self,
        reference: &crate::value::ValueRef,
        value: Value,
    ) -> Result<bool, crate::error::RuntimeError> {
        if reference.generation != self.identities.generation {
            return Err(crate::error::RuntimeError::ReferenceLifetime);
        }
        self.write_by_ref_parts_checked(
            reference.location,
            reference.offset as usize,
            &reference.path,
            value,
        )
    }

    /// Checked shared writer used by the portable engine and engineering boundaries.
    pub fn write_by_ref_parts_checked(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, crate::error::RuntimeError> {
        let Some(resolved) = self.resolve_reference_parts(location, offset, path) else {
            return Ok(false);
        };
        let slot = match resolved.location {
            MemoryLocation::Global => self
                .values
                .globals
                .get_index_mut(resolved.offset)
                .map(|(_, slot)| slot),
            MemoryLocation::Local(id) => self.local_slot_mut(id, resolved.offset),
            MemoryLocation::Instance(id) => {
                self.activations
                    .instances
                    .get_mut(&id)
                    .and_then(|instance| {
                        instance
                            .variables
                            .get_index_mut(resolved.offset)
                            .map(|(_, slot)| slot)
                    })
            }
            MemoryLocation::Io(_) | MemoryLocation::Retain => None,
        };
        let Some(slot) = slot else { return Ok(false) };
        crate::value::write_value_path_checked(slot, resolved.path, value)
    }

    #[cfg(feature = "hir")]
    pub fn write_by_ref_parts_typed(
        &mut self,
        registry: &trust_hir::types::TypeRegistry,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
        capacity: usize,
    ) -> bool {
        if path.is_empty() {
            return self.write_direct_slot_by_location(location, offset, value);
        }
        let Some(resolved) = self.resolve_reference_parts(location, offset, path) else {
            return false;
        };
        match resolved.location {
            MemoryLocation::Global => self
                .values
                .globals
                .get_index_mut(resolved.offset)
                .map(|(_, slot)| {
                    crate::value::write_value_path_typed(
                        slot,
                        resolved.path,
                        value,
                        registry,
                        capacity,
                    )
                })
                .unwrap_or(false),
            MemoryLocation::Local(frame_id) => self
                .local_slot_mut(frame_id, resolved.offset)
                .map(|slot| {
                    crate::value::write_value_path_typed(
                        slot,
                        resolved.path,
                        value,
                        registry,
                        capacity,
                    )
                })
                .unwrap_or(false),
            MemoryLocation::Instance(instance_id) => self
                .activations
                .instances
                .get_mut(&instance_id)
                .and_then(|instance| instance.variables.get_index_mut(resolved.offset))
                .map(|(_, slot)| {
                    crate::value::write_value_path_typed(
                        slot,
                        resolved.path,
                        value,
                        registry,
                        capacity,
                    )
                })
                .unwrap_or(false),
            MemoryLocation::Io(_) | MemoryLocation::Retain => false,
        }
    }

    /// Resolve instance hops while borrowing the remaining aggregate path.
    pub(crate) fn resolve_reference_parts<'a>(
        &self,
        mut location: MemoryLocation,
        mut offset: usize,
        path: &'a [RefSegment],
    ) -> Option<crate::value::ValueRefView<'a>> {
        let mut start = 0;
        for (index, segment) in path.iter().enumerate() {
            if let RefSegment::Field(name) = segment {
                let root = self.read_direct_slot_by_location(location, offset)?;
                let current = read_value_path_borrowed(root, &path[start..index])?;
                if let Value::Instance(instance) = current {
                    let resolved = self.ref_for_instance_recursive(*instance, name)?;
                    location = resolved.location;
                    offset = resolved.offset as usize;
                    start = index + 1;
                }
            }
        }
        Some(crate::value::ValueRefView {
            location,
            offset,
            generation: self.identities.generation,
            path: &path[start..],
        })
    }
}
