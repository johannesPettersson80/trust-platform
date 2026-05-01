use super::*;

impl VariableStorage {
    pub(crate) fn read_direct_slot_by_location(
        &self,
        location: MemoryLocation,
        offset: usize,
    ) -> Option<&Value> {
        match location {
            MemoryLocation::Global => self.globals.get_index(offset).map(|(_, value)| value),
            MemoryLocation::Local(frame_id) => self
                .frames
                .iter()
                .find(|frame| frame.id == frame_id)
                .and_then(|frame| frame.variables.get_index(offset).map(|(_, value)| value)),
            MemoryLocation::Instance(instance_id) => self
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.variables.get_index(offset).map(|(_, value)| value)),
            MemoryLocation::Io(_) | MemoryLocation::Retain => None,
        }
    }

    pub(crate) fn read_global_slot_by_offset(&self, offset: usize) -> Option<&Value> {
        self.globals.get_index(offset).map(|(_, value)| value)
    }

    pub(crate) fn write_direct_slot_by_location(
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
                .frames
                .iter_mut()
                .find(|frame| frame.id == frame_id)
                .and_then(|frame| {
                    frame.variables.get_index_mut(offset).map(|(_, slot)| {
                        *slot = crate::value::normalize_assignment_for_target(slot, value);
                    })
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

    pub(crate) fn write_global_slot_by_offset(&mut self, offset: usize, value: Value) -> bool {
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

        read_value_path_borrowed(root, &resolved.path)
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
        materialize_value_path(root, &resolved.path)
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
                write_value_path(slot, &resolved.path, value)
            }
            MemoryLocation::Local(frame_id) => self
                .frames
                .iter_mut()
                .find(|frame| frame.id == frame_id)
                .and_then(|frame| {
                    frame
                        .variables
                        .get_index_mut(resolved.offset)
                        .map(|(_, v)| v)
                })
                .map(|slot| write_value_path(slot, &resolved.path, value))
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
                .map(|slot| write_value_path(slot, &resolved.path, value))
                .unwrap_or(false),
            MemoryLocation::Io(_) | MemoryLocation::Retain => false,
        }
    }

    fn resolve_reference_parts(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Option<crate::value::ValueRef> {
        let mut resolved = crate::value::ValueRef {
            location,
            offset,
            path: RefPath::new(),
        };

        for segment in path {
            match segment {
                RefSegment::Field(name) => {
                    let current =
                        self.read_by_ref_parts(resolved.location, resolved.offset, &resolved.path)?;
                    if let Value::Instance(instance_id) = current {
                        resolved = self.ref_for_instance_recursive(*instance_id, name.as_str())?;
                    } else {
                        resolved.path.push(RefSegment::Field(name.clone()));
                    }
                }
                RefSegment::Index(indices) => {
                    resolved.path.push(RefSegment::Index(indices.clone()));
                }
            }
        }

        Some(resolved)
    }
}
