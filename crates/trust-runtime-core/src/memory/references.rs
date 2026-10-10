use super::*;

impl VariableStorage {
    pub fn read_direct_slot_by_location(
        &self,
        location: MemoryLocation,
        offset: usize,
    ) -> Option<&Value> {
        match location {
            MemoryLocation::Global => self.globals.get_index(offset).map(|(_, value)| value),
            MemoryLocation::Local(frame_id) => self.local_slot(frame_id, offset),
            MemoryLocation::Instance(instance_id) => self
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.variables.get_index(offset).map(|(_, value)| value)),
            MemoryLocation::Io(_) | MemoryLocation::Retain => None,
        }
    }

    pub fn read_global_slot_by_offset(&self, offset: usize) -> Option<&Value> {
        self.globals.get_index(offset).map(|(_, value)| value)
    }

    pub fn write_direct_slot_by_location(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        value: Value,
    ) -> bool {
        match location {
            MemoryLocation::Global => self
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
        self.globals
            .get_index_mut(offset)
            .map(|(_, slot)| {
                *slot = crate::value::normalize_assignment_for_target(slot, value);
            })
            .is_some()
    }

    pub fn read_by_ref(&self, value_ref: crate::value::ValueRef) -> Option<&Value> {
        self.read_by_ref_ref(&value_ref)
    }

    pub fn read_by_ref_ref(&self, value_ref: &crate::value::ValueRef) -> Option<&Value> {
        self.read_by_ref_parts(value_ref.location, value_ref.offset, &value_ref.path)
    }

    pub fn materialize_by_ref(&self, value_ref: crate::value::ValueRef) -> Option<Value> {
        self.materialize_by_ref_ref(&value_ref)
    }

    pub fn materialize_by_ref_ref(&self, value_ref: &crate::value::ValueRef) -> Option<Value> {
        self.materialize_by_ref_parts(value_ref.location, value_ref.offset, &value_ref.path)
    }

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

    pub fn write_by_ref_ref(&mut self, value_ref: &crate::value::ValueRef, value: Value) -> bool {
        self.write_by_ref_parts(value_ref.location, value_ref.offset, &value_ref.path, value)
    }

    pub fn write_by_ref_parts(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> bool {
        if path.is_empty() {
            return self.write_direct_slot_by_location(location, offset, value);
        }

        let Some(resolved) = self.resolve_reference_parts(location, offset, path) else {
            return false;
        };

        match resolved.location {
            MemoryLocation::Global => {
                let Some((_, slot)) = self.globals.get_index_mut(resolved.offset) else {
                    return false;
                };
                write_value_path(slot, resolved.path, value)
            }
            MemoryLocation::Local(frame_id) => self
                .local_slot_mut(frame_id, resolved.offset)
                .map(|slot| write_value_path(slot, resolved.path, value))
                .unwrap_or(false),
            MemoryLocation::Instance(instance_id) => self
                .instances
                .get_mut(&instance_id)
                .and_then(|instance| {
                    instance
                        .variables
                        .get_index_mut(resolved.offset)
                        .map(|(_, v)| v)
                })
                .map(|slot| write_value_path(slot, resolved.path, value))
                .unwrap_or(false),
            MemoryLocation::Io(_) | MemoryLocation::Retain => false,
        }
    }

    #[cfg(feature = "hir")]
    pub fn write_by_ref_parts_typed(
        &mut self,
        registry: &trust_hir::types::TypeRegistry,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> bool {
        if path.is_empty() {
            return self.write_direct_slot_by_location(location, offset, value);
        }
        let Some(resolved) = self.resolve_reference_parts(location, offset, path) else {
            return false;
        };
        match resolved.location {
            MemoryLocation::Global => self
                .globals
                .get_index_mut(resolved.offset)
                .map(|(_, slot)| {
                    crate::value::write_value_path_typed(slot, resolved.path, value, registry)
                })
                .unwrap_or(false),
            MemoryLocation::Local(frame_id) => self
                .local_slot_mut(frame_id, resolved.offset)
                .map(|slot| {
                    crate::value::write_value_path_typed(slot, resolved.path, value, registry)
                })
                .unwrap_or(false),
            MemoryLocation::Instance(instance_id) => self
                .instances
                .get_mut(&instance_id)
                .and_then(|instance| instance.variables.get_index_mut(resolved.offset))
                .map(|(_, slot)| {
                    crate::value::write_value_path_typed(slot, resolved.path, value, registry)
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
                    offset = resolved.offset;
                    start = index + 1;
                }
            }
        }
        Some(crate::value::ValueRefView {
            location,
            offset,
            path: &path[start..],
        })
    }

    /// Restore an existing physical slot without normalization or allocation.
    /// Used only after a transaction has saved this exact slot.
    pub(crate) fn restore_direct_slot(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        value: Value,
    ) {
        let slot = match location {
            MemoryLocation::Global => self.globals.get_index_mut(offset).map(|(_, value)| value),
            MemoryLocation::Local(id) => self.local_slot_mut(id, offset),
            MemoryLocation::Instance(id) => self.instances.get_mut(&id).and_then(|instance| {
                instance
                    .variables
                    .get_index_mut(offset)
                    .map(|(_, value)| value)
            }),
            MemoryLocation::Io(_) | MemoryLocation::Retain => None,
        };
        if let Some(slot) = slot {
            *slot = value;
        }
    }
}
