//! Variable storage, frames, and instance data.

#![allow(missing_docs)]

#[cfg(test)]
use crate::value::ref_indices_from_iter;
use crate::value::{
    materialize_value_path, read_value_path_borrowed, write_value_path, PartialAccess, RefPath,
    RefSegment, Value, ValueRef,
};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::sync::RwLock;
pub use trust_runtime_core::memory::{FrameId, InstanceId, IoArea, MemoryLocation};

/// A local variable frame for function/method calls.
#[derive(Debug, Clone)]
pub struct LocalFrame {
    pub id: FrameId,
    pub owner: SmolStr,
    pub variables: IndexMap<SmolStr, Value>,
    pub return_value: Option<Value>,
    pub instance_id: Option<InstanceId>,
}

/// Data for a single FB/Class instance.
#[derive(Debug, Clone)]
pub struct InstanceData {
    pub type_name: SmolStr,
    pub variables: IndexMap<SmolStr, Value>,
    pub parent: Option<InstanceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecursiveInstanceFieldResolution {
    owner_depth: usize,
    offset: usize,
}

/// Storage for runtime variables.
#[derive(Debug, Default)]
pub struct VariableStorage {
    globals: IndexMap<SmolStr, Value>,
    frames: Vec<LocalFrame>,
    instances: FxHashMap<InstanceId, InstanceData>,
    retain: IndexMap<SmolStr, Value>,
    instance_field_offsets: RwLock<FxHashMap<(InstanceId, SmolStr), Option<usize>>>,
    recursive_instance_field_resolutions:
        RwLock<FxHashMap<(InstanceId, SmolStr), RecursiveInstanceFieldResolution>>,
    declared_instance_field_offsets: RwLock<FxHashMap<(SmolStr, SmolStr), usize>>,
    next_frame_id: u32,
    next_instance_id: u32,
}

impl Clone for VariableStorage {
    fn clone(&self) -> Self {
        Self {
            globals: self.globals.clone(),
            frames: self.frames.clone(),
            instances: self.instances.clone(),
            retain: self.retain.clone(),
            instance_field_offsets: RwLock::new(
                self.instance_field_offsets
                    .read()
                    .expect("instance_field_offsets poisoned")
                    .clone(),
            ),
            recursive_instance_field_resolutions: RwLock::new(
                self.recursive_instance_field_resolutions
                    .read()
                    .expect("recursive_instance_field_resolutions poisoned")
                    .clone(),
            ),
            declared_instance_field_offsets: RwLock::new(
                self.declared_instance_field_offsets
                    .read()
                    .expect("declared_instance_field_offsets poisoned")
                    .clone(),
            ),
            next_frame_id: self.next_frame_id,
            next_instance_id: self.next_instance_id,
        }
    }
}

impl VariableStorage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.globals.insert(name.into(), value);
    }

    #[must_use]
    pub fn globals(&self) -> &IndexMap<SmolStr, Value> {
        &self.globals
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<&Value> {
        self.globals.get(name)
    }

    pub fn set_retain(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.retain.insert(name.into(), value);
    }

    #[must_use]
    pub fn retain(&self) -> &IndexMap<SmolStr, Value> {
        &self.retain
    }

    #[must_use]
    pub fn get_retain(&self, name: &str) -> Option<&Value> {
        self.retain.get(name)
    }

    pub fn push_frame(&mut self, owner: impl Into<SmolStr>) -> FrameId {
        let id = FrameId(self.next_frame_id);
        self.next_frame_id += 1;
        self.frames.push(LocalFrame {
            id,
            owner: owner.into(),
            variables: IndexMap::new(),
            return_value: None,
            instance_id: None,
        });
        id
    }

    pub fn push_frame_with_instance(
        &mut self,
        owner: impl Into<SmolStr>,
        instance_id: InstanceId,
    ) -> FrameId {
        let id = FrameId(self.next_frame_id);
        self.next_frame_id += 1;
        self.frames.push(LocalFrame {
            id,
            owner: owner.into(),
            variables: IndexMap::new(),
            return_value: None,
            instance_id: Some(instance_id),
        });
        id
    }

    pub fn pop_frame(&mut self) -> Option<LocalFrame> {
        self.frames.pop()
    }

    pub fn remove_frame(&mut self, frame_id: FrameId) -> Option<LocalFrame> {
        let idx = self.frames.iter().position(|frame| frame.id == frame_id)?;
        Some(self.frames.remove(idx))
    }

    #[must_use]
    pub fn frames(&self) -> &[LocalFrame] {
        &self.frames
    }

    #[must_use]
    pub fn current_frame(&self) -> Option<&LocalFrame> {
        self.frames.last()
    }

    pub fn current_frame_mut(&mut self) -> Option<&mut LocalFrame> {
        self.frames.last_mut()
    }

    pub fn set_local(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        if let Some(frame) = self.current_frame_mut() {
            frame.variables.insert(name.into(), value);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn get_local(&self, name: &str) -> Option<&Value> {
        self.current_frame()
            .and_then(|frame| frame.variables.get(name))
    }

    pub fn clear_locals(&mut self) {
        if let Some(frame) = self.current_frame_mut() {
            frame.variables.clear();
        }
    }

    pub fn clear_frames(&mut self) {
        self.frames.clear();
        self.next_frame_id = 0;
    }

    pub fn reset_runtime_values(&mut self, reset_instance_sequence: bool) {
        self.globals.clear();
        self.frames.clear();
        self.instances.clear();
        self.next_frame_id = 0;
        if reset_instance_sequence {
            self.next_instance_id = 0;
        }
        self.instance_field_offsets
            .write()
            .expect("instance_field_offsets poisoned")
            .clear();
        self.recursive_instance_field_resolutions
            .write()
            .expect("recursive_instance_field_resolutions poisoned")
            .clear();
    }

    /// Temporarily treat the provided frame as the current frame.
    pub fn with_frame<T>(
        &mut self,
        frame_id: FrameId,
        f: impl FnOnce(&mut Self) -> T,
    ) -> Option<T> {
        let idx = self.frames.iter().position(|frame| frame.id == frame_id)?;
        if idx + 1 == self.frames.len() {
            return Some(f(self));
        }

        let frame = self.frames.remove(idx);
        self.frames.push(frame);
        let result = f(self);
        let frame = self.frames.pop().expect("frame stack empty after eval");
        self.frames.insert(idx, frame);
        Some(result)
    }

    pub fn create_instance(&mut self, type_name: impl Into<SmolStr>) -> InstanceId {
        let id = InstanceId(self.next_instance_id);
        self.next_instance_id += 1;
        self.instances.insert(
            id,
            InstanceData {
                type_name: type_name.into(),
                variables: IndexMap::new(),
                parent: None,
            },
        );
        id
    }

    #[must_use]
    pub fn get_instance(&self, id: InstanceId) -> Option<&InstanceData> {
        self.instances.get(&id)
    }

    #[must_use]
    pub fn instances(&self) -> &FxHashMap<InstanceId, InstanceData> {
        &self.instances
    }

    pub fn get_instance_mut(&mut self, id: InstanceId) -> Option<&mut InstanceData> {
        self.instances.get_mut(&id)
    }

    pub fn set_instance_var(
        &mut self,
        id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        let name = name.into();
        let is_new = if let Some(instance) = self.instances.get_mut(&id) {
            let is_new = !instance.variables.contains_key(&name);
            instance.variables.insert(name, value);
            is_new
        } else {
            return false;
        };

        if is_new {
            self.invalidate_instance_field_caches(id);
        }
        true
    }

    #[must_use]
    pub fn get_instance_var(&self, id: InstanceId, name: &str) -> Option<&Value> {
        self.instances
            .get(&id)
            .and_then(|instance| instance.variables.get(name))
    }

    #[must_use]
    pub fn get_instance_var_recursive(&self, id: InstanceId, name: &str) -> Option<&Value> {
        let mut current = Some(id);
        while let Some(instance_id) = current {
            if let Some(value) = self.get_instance_var(instance_id, name) {
                return Some(value);
            }
            current = self
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.parent);
        }
        None
    }

    pub fn ref_for_global(&self, name: &str) -> Option<crate::value::ValueRef> {
        ref_for_map(&self.globals, MemoryLocation::Global, name)
    }

    pub fn ref_for_local(&self, name: &str) -> Option<crate::value::ValueRef> {
        let frame = self.current_frame()?;
        ref_for_map(&frame.variables, MemoryLocation::Local(frame.id), name)
    }

    pub fn ref_for_instance(&self, id: InstanceId, name: &str) -> Option<crate::value::ValueRef> {
        let field_name = SmolStr::new(name);
        let offset = self.cached_instance_field_offset(id, &field_name)?;
        Some(crate::value::ValueRef {
            location: MemoryLocation::Instance(id),
            offset,
            path: RefPath::new(),
        })
    }

    pub fn ref_for_instance_recursive(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        let field_name = SmolStr::new(name);
        if let Some(resolution) = self.cached_recursive_instance_field_resolution(id, &field_name) {
            let owner = self.resolve_ancestor_instance(id, resolution.owner_depth)?;
            return Some(crate::value::ValueRef {
                location: MemoryLocation::Instance(owner),
                offset: resolution.offset,
                path: RefPath::new(),
            });
        }

        let mut current = Some(id);
        let mut owner_depth = 0usize;
        while let Some(instance_id) = current {
            if let Some(offset) = self.cached_instance_field_offset(instance_id, &field_name) {
                let resolution = RecursiveInstanceFieldResolution {
                    owner_depth,
                    offset,
                };
                self.cache_recursive_instance_field_resolution(id, &field_name, resolution);
                return Some(crate::value::ValueRef {
                    location: MemoryLocation::Instance(instance_id),
                    offset,
                    path: RefPath::new(),
                });
            }
            current = self
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.parent);
            owner_depth += 1;
        }
        None
    }

    fn invalidate_instance_field_caches(&self, id: InstanceId) {
        self.instance_field_offsets
            .write()
            .expect("cache poisoned")
            .retain(|(instance_id, _), _| *instance_id != id);
        self.recursive_instance_field_resolutions
            .write()
            .expect("cache poisoned")
            .retain(|(instance_id, _), _| *instance_id != id);
    }

    fn cached_instance_field_offset(&self, id: InstanceId, field_name: &SmolStr) -> Option<usize> {
        let key = (id, field_name.clone());
        if let Some(cached) = self
            .instance_field_offsets
            .read()
            .expect("cache poisoned")
            .get(&key)
            .copied()
        {
            return cached;
        }

        let offset = self
            .instances
            .get(&id)
            .and_then(|instance| instance.variables.get_index_of(field_name.as_str()));
        self.instance_field_offsets
            .write()
            .expect("cache poisoned")
            .insert(key, offset);
        offset
    }

    fn cached_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &SmolStr,
    ) -> Option<RecursiveInstanceFieldResolution> {
        self.recursive_instance_field_resolutions
            .read()
            .expect("cache poisoned")
            .get(&(id, field_name.clone()))
            .copied()
    }

    fn cache_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &SmolStr,
        resolution: RecursiveInstanceFieldResolution,
    ) {
        self.recursive_instance_field_resolutions
            .write()
            .expect("cache poisoned")
            .insert((id, field_name.clone()), resolution);
    }

    fn resolve_ancestor_instance(&self, id: InstanceId, depth: usize) -> Option<InstanceId> {
        let mut current = id;
        for _ in 0..depth {
            current = self.instances.get(&current)?.parent?;
        }
        Some(current)
    }

    pub fn declared_instance_field_offset(&self, id: InstanceId, name: &str) -> Option<usize> {
        let instance = self.instances.get(&id)?;
        let field_name = SmolStr::new(name);
        let key = (instance.type_name.clone(), field_name.clone());
        if let Some(offset) = self
            .declared_instance_field_offsets
            .read()
            .expect("cache poisoned")
            .get(&key)
            .copied()
        {
            return Some(offset);
        }

        let offset = instance.variables.get_index_of(field_name.as_str())?;
        self.declared_instance_field_offsets
            .write()
            .expect("cache poisoned")
            .insert(key, offset);
        Some(offset)
    }

    pub fn declared_instance_field_ref(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        let offset = self.declared_instance_field_offset(id, name)?;
        Some(crate::value::ValueRef {
            location: MemoryLocation::Instance(id),
            offset,
            path: RefPath::new(),
        })
    }

    pub fn resolved_instance_field_ref(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        self.declared_instance_field_ref(id, name)
            .or_else(|| self.ref_for_instance_recursive(id, name))
    }

    pub fn read_instance_field_by_offset(&self, id: InstanceId, offset: usize) -> Option<&Value> {
        self.instances
            .get(&id)
            .and_then(|instance| instance.variables.get_index(offset).map(|(_, value)| value))
    }

    pub fn write_instance_field_by_offset(
        &mut self,
        id: InstanceId,
        offset: usize,
        value: Value,
    ) -> bool {
        self.instances
            .get_mut(&id)
            .and_then(|instance| {
                instance.variables.get_index_mut(offset).map(|(_, slot)| {
                    *slot = value;
                })
            })
            .is_some()
    }

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

#[derive(Debug, Clone)]
pub struct AccessBinding {
    pub name: SmolStr,
    pub reference: ValueRef,
    pub partial: Option<PartialAccess>,
}

#[derive(Debug, Default, Clone)]
pub struct AccessMap {
    bindings: IndexMap<SmolStr, AccessBinding>,
}

impl AccessMap {
    pub fn bind(&mut self, name: SmolStr, reference: ValueRef, partial: Option<PartialAccess>) {
        let binding = AccessBinding {
            name: name.clone(),
            reference,
            partial,
        };
        self.bindings.insert(name, binding);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AccessBinding> {
        self.bindings.get(name)
    }
}

fn ref_for_map(
    map: &IndexMap<SmolStr, Value>,
    location: MemoryLocation,
    name: &str,
) -> Option<crate::value::ValueRef> {
    map.get_index_of(name).map(|offset| crate::value::ValueRef {
        location,
        offset,
        path: RefPath::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{ArrayValue, StructValue};

    #[test]
    fn instance_field_cache_is_scoped_per_instance() {
        let mut storage = VariableStorage::new();
        let first = storage.create_instance("FB");
        let second = storage.create_instance("FB");

        assert!(storage.set_instance_var(first, "ACC", Value::DInt(7)));

        let first_ref = storage
            .ref_for_instance(first, "ACC")
            .expect("missing first ACC ref");
        assert_eq!(first_ref.location, MemoryLocation::Instance(first));
        assert_eq!(first_ref.offset, 0);

        let second_ref = storage.ref_for_instance(second, "ACC");
        assert!(second_ref.is_none());

        let cache = storage
            .instance_field_offsets
            .read()
            .expect("cache poisoned");
        assert_eq!(cache.get(&(first, SmolStr::new("ACC"))), Some(&Some(0)));
        assert_eq!(cache.get(&(second, SmolStr::new("ACC"))), Some(&None));
    }

    #[test]
    fn direct_instance_field_miss_cache_invalidates_on_new_insert() {
        let mut storage = VariableStorage::new();
        let instance = storage.create_instance("FB");

        assert!(storage.ref_for_instance(instance, "LATE").is_none());
        assert_eq!(
            storage
                .instance_field_offsets
                .read()
                .expect("cache poisoned")
                .get(&(instance, SmolStr::new("LATE"))),
            Some(&None)
        );

        assert!(storage.set_instance_var(instance, "LATE", Value::Bool(true)));
        assert!(storage
            .instance_field_offsets
            .read()
            .expect("cache poisoned")
            .get(&(instance, SmolStr::new("LATE")))
            .is_none());

        let resolved = storage
            .ref_for_instance(instance, "LATE")
            .expect("late field should resolve after insert");
        assert_eq!(resolved.location, MemoryLocation::Instance(instance));
        assert_eq!(resolved.offset, 0);
    }

    #[test]
    fn recursive_instance_field_cache_invalidates_when_child_adds_shadowing_field() {
        let mut storage = VariableStorage::new();
        let base = storage.create_instance("BASE");
        let derived = storage.create_instance("DERIVED");
        storage
            .get_instance_mut(derived)
            .expect("derived instance")
            .parent = Some(base);

        assert!(storage.set_instance_var(base, "ACC", Value::DInt(11)));

        let inherited = storage
            .ref_for_instance_recursive(derived, "ACC")
            .expect("inherited field should resolve");
        assert_eq!(inherited.location, MemoryLocation::Instance(base));
        assert_eq!(
            storage
                .recursive_instance_field_resolutions
                .read()
                .expect("cache poisoned")
                .get(&(derived, SmolStr::new("ACC")))
                .copied(),
            Some(RecursiveInstanceFieldResolution {
                owner_depth: 1,
                offset: inherited.offset,
            })
        );

        assert!(storage.set_instance_var(derived, "ACC", Value::DInt(22)));
        assert!(storage
            .recursive_instance_field_resolutions
            .read()
            .expect("cache poisoned")
            .get(&(derived, SmolStr::new("ACC")))
            .is_none());

        let shadowed = storage
            .ref_for_instance_recursive(derived, "ACC")
            .expect("shadowed field should resolve");
        assert_eq!(shadowed.location, MemoryLocation::Instance(derived));
        assert_eq!(shadowed.offset, 0);
        assert!(matches!(
            storage.read_by_ref(shadowed).expect("shadowed field value"),
            Value::DInt(22)
        ));
    }

    #[test]
    fn declared_instance_field_offset_reuses_type_layout_for_declared_fields() {
        let mut storage = VariableStorage::new();
        let first = storage.create_instance("FB");
        let second = storage.create_instance("FB");

        assert!(storage.set_instance_var(first, "IN", Value::Bool(false)));
        assert!(storage.set_instance_var(first, "OUT", Value::Bool(false)));
        assert!(storage.set_instance_var(second, "IN", Value::Bool(true)));
        assert!(storage.set_instance_var(second, "OUT", Value::Bool(true)));
        assert!(storage.set_instance_var(first, "__hidden", Value::Bool(true)));

        let first_in = storage
            .declared_instance_field_offset(first, "IN")
            .expect("first IN offset");
        let second_in = storage
            .declared_instance_field_offset(second, "IN")
            .expect("second IN offset");
        assert_eq!(first_in, 0);
        assert_eq!(second_in, first_in);

        let second_out = storage
            .declared_instance_field_offset(second, "OUT")
            .expect("second OUT offset");
        assert_eq!(second_out, 1);
        assert!(matches!(
            storage
                .read_instance_field_by_offset(second, second_out)
                .expect("second OUT value"),
            Value::Bool(true)
        ));
    }

    #[test]
    fn declared_instance_field_offset_skips_inherited_fields() {
        let mut storage = VariableStorage::new();
        let base = storage.create_instance("BASE");
        let derived = storage.create_instance("DERIVED");
        storage
            .get_instance_mut(derived)
            .expect("derived instance")
            .parent = Some(base);

        assert!(storage.set_instance_var(base, "PARENT_PARAM", Value::DInt(5)));
        assert!(storage
            .declared_instance_field_offset(derived, "PARENT_PARAM")
            .is_none());

        let inherited = storage
            .ref_for_instance_recursive(derived, "PARENT_PARAM")
            .expect("recursive inherited field");
        assert_eq!(inherited.location, MemoryLocation::Instance(base));
        assert_eq!(inherited.offset, 0);
    }

    #[test]
    fn resolved_instance_field_ref_prefers_direct_field_before_parent_fallback() {
        let mut storage = VariableStorage::new();
        let base = storage.create_instance("BASE");
        let derived = storage.create_instance("DERIVED");
        storage
            .get_instance_mut(derived)
            .expect("derived instance")
            .parent = Some(base);

        assert!(storage.set_instance_var(base, "ACC", Value::DInt(5)));
        assert!(storage.set_instance_var(derived, "ACC", Value::DInt(9)));
        let direct = storage
            .resolved_instance_field_ref(derived, "ACC")
            .expect("direct field ref");
        assert_eq!(direct.location, MemoryLocation::Instance(derived));
        assert_eq!(direct.offset, 0);

        let parent_only = storage
            .resolved_instance_field_ref(derived, "BASE_ONLY")
            .is_none();
        assert!(parent_only);

        assert!(storage.set_instance_var(base, "BASE_ONLY", Value::DInt(12)));
        let inherited = storage
            .resolved_instance_field_ref(derived, "BASE_ONLY")
            .expect("inherited field ref");
        assert_eq!(inherited.location, MemoryLocation::Instance(base));
        assert_eq!(inherited.offset, 1);
    }

    #[test]
    fn direct_instance_field_offset_reads_and_writes_without_value_ref() {
        let mut storage = VariableStorage::new();
        let instance = storage.create_instance("FB");
        assert!(storage.set_instance_var(instance, "ACC", Value::DInt(11)));

        let offset = storage
            .declared_instance_field_offset(instance, "ACC")
            .expect("ACC offset");
        assert!(matches!(
            storage
                .read_instance_field_by_offset(instance, offset)
                .expect("read by offset"),
            Value::DInt(11)
        ));

        assert!(storage.write_instance_field_by_offset(instance, offset, Value::DInt(22)));
        assert!(matches!(
            storage
                .read_instance_field_by_offset(instance, offset)
                .expect("updated read by offset"),
            Value::DInt(22)
        ));
    }

    #[test]
    fn direct_slot_helpers_cover_global_local_and_instance_locations() {
        let mut storage = VariableStorage::new();
        storage.set_global("G", Value::DInt(1));
        let frame_id = storage.push_frame("MAIN");
        assert!(storage.set_local("L", Value::DInt(2)));
        let instance = storage.create_instance("FB");
        assert!(storage.set_instance_var(instance, "I", Value::DInt(3)));

        let global_ref = storage.ref_for_global("G").expect("global ref");
        let local_ref = storage.ref_for_local("L").expect("local ref");
        let instance_ref = storage
            .ref_for_instance(instance, "I")
            .expect("instance ref");

        assert_eq!(local_ref.location, MemoryLocation::Local(frame_id));
        assert!(matches!(
            storage.read_direct_slot_by_location(MemoryLocation::Global, global_ref.offset),
            Some(&Value::DInt(1))
        ));
        assert!(matches!(
            storage.read_direct_slot_by_location(local_ref.location, local_ref.offset),
            Some(&Value::DInt(2))
        ));
        assert!(matches!(
            storage.read_direct_slot_by_location(instance_ref.location, instance_ref.offset),
            Some(&Value::DInt(3))
        ));

        assert!(storage.write_direct_slot_by_location(
            MemoryLocation::Global,
            global_ref.offset,
            Value::DInt(11)
        ));
        assert!(storage.write_direct_slot_by_location(
            local_ref.location,
            local_ref.offset,
            Value::DInt(12)
        ));
        assert!(storage.write_direct_slot_by_location(
            instance_ref.location,
            instance_ref.offset,
            Value::DInt(13)
        ));

        assert!(matches!(storage.get_global("G"), Some(&Value::DInt(11))));
        assert!(matches!(storage.get_local("L"), Some(&Value::DInt(12))));
        assert!(matches!(
            storage.get_instance_var(instance, "I"),
            Some(&Value::DInt(13))
        ));
    }

    #[test]
    fn direct_slot_helpers_match_empty_path_ref_helpers() {
        let mut storage = VariableStorage::new();
        storage.set_global("G", Value::DInt(5));
        let frame_id = storage.push_frame("MAIN");
        assert!(storage.set_local("L", Value::DInt(6)));
        let instance = storage.create_instance("FB");
        assert!(storage.set_instance_var(instance, "I", Value::DInt(7)));

        let refs = [
            storage.ref_for_global("G").expect("global ref"),
            storage.ref_for_local("L").expect("local ref"),
            storage
                .ref_for_instance(instance, "I")
                .expect("instance ref"),
        ];
        assert_eq!(refs[1].location, MemoryLocation::Local(frame_id));

        for reference in refs {
            let direct = storage
                .read_direct_slot_by_location(reference.location, reference.offset)
                .expect("direct slot read");
            let generic = storage
                .read_by_ref_parts(reference.location, reference.offset, &[])
                .expect("generic empty-path read");
            assert_eq!(direct, generic);
        }
    }

    #[test]
    fn borrowed_value_ref_helpers_match_owned_helpers() {
        let mut storage = VariableStorage::new();
        let instance = storage.create_instance("FB");
        assert!(storage.set_instance_var(instance, "ACC", Value::DInt(11)));

        let reference = storage
            .ref_for_instance(instance, "ACC")
            .expect("instance field reference");
        assert!(matches!(
            storage.read_by_ref_ref(&reference).expect("borrowed read"),
            Value::DInt(11)
        ));
        assert!(matches!(
            storage.read_by_ref(reference.clone()).expect("owned read"),
            Value::DInt(11)
        ));

        assert!(storage.write_by_ref_ref(&reference, Value::DInt(22)));
        assert!(matches!(
            storage
                .read_by_ref_ref(&reference)
                .expect("updated borrowed read"),
            Value::DInt(22)
        ));
    }

    #[test]
    fn recursive_lookup_does_not_cache_parent_chain_miss() {
        let mut storage = VariableStorage::new();
        let base = storage.create_instance("BASE");
        let derived = storage.create_instance("DERIVED");
        storage
            .get_instance_mut(derived)
            .expect("derived instance")
            .parent = Some(base);

        assert!(storage
            .ref_for_instance_recursive(derived, "LATE")
            .is_none());
        assert!(storage
            .recursive_instance_field_resolutions
            .read()
            .expect("cache poisoned")
            .get(&(derived, SmolStr::new("LATE")))
            .is_none());

        assert!(storage.set_instance_var(base, "LATE", Value::Bool(true)));

        let resolved = storage
            .ref_for_instance_recursive(derived, "LATE")
            .expect("parent field should resolve after insert");
        assert_eq!(resolved.location, MemoryLocation::Instance(base));
        assert_eq!(resolved.offset, 0);
        assert!(matches!(
            storage.read_by_ref(resolved).expect("parent field value"),
            Value::Bool(true)
        ));
    }

    #[test]
    fn write_by_ref_path_preserves_struct_copy_on_write_isolation() {
        let mut storage = VariableStorage::new();
        let shared = Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
            SmolStr::new("AXIS_REF"),
            IndexMap::from([(SmolStr::new("InternalIndex"), Value::UInt(1))]),
        )));
        storage.set_global("left", shared.clone());
        storage.set_global("right", shared);

        assert!(storage.write_by_ref_parts(
            MemoryLocation::Global,
            0,
            &[RefSegment::Field(SmolStr::new("InternalIndex"))],
            Value::UInt(7),
        ));

        let left = storage.get_global("left").expect("left global");
        let right = storage.get_global("right").expect("right global");
        let Value::Struct(left_struct) = left else {
            panic!("left should be struct");
        };
        let Value::Struct(right_struct) = right else {
            panic!("right should be struct");
        };
        assert_eq!(left_struct.field("InternalIndex"), Some(&Value::UInt(7)));
        assert_eq!(right_struct.field("InternalIndex"), Some(&Value::UInt(1)));
    }

    #[test]
    fn read_and_write_by_ref_handle_extreme_array_bounds_without_overflow() {
        let mut storage = VariableStorage::new();
        storage.set_global(
            "GRID",
            Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![Value::DInt(7)],
                vec![(i64::MIN, i64::MAX)],
            ))),
        );
        let mut reference = storage.ref_for_global("GRID").expect("grid ref");
        reference
            .path
            .push(RefSegment::Index(ref_indices_from_iter([i64::MIN])));

        let read = storage
            .read_by_ref(reference.clone())
            .expect("read extreme lower bound");
        assert_eq!(read, &Value::DInt(7));

        assert!(storage.write_by_ref(reference.clone(), Value::DInt(9)));
        let updated = storage
            .read_by_ref(reference)
            .expect("read updated lower bound");
        assert_eq!(updated, &Value::DInt(9));
    }

    #[test]
    fn read_and_write_by_ref_non_ascii_string_uses_character_elements() {
        let mut storage = VariableStorage::new();
        storage.set_global("TEXT", Value::String("ÄBC".into()));
        let mut reference = storage.ref_for_global("TEXT").expect("text ref");
        reference
            .path
            .push(RefSegment::Index(ref_indices_from_iter([1])));

        let read = storage
            .materialize_by_ref(reference.clone())
            .expect("read non-ascii string element");
        assert_eq!(read, Value::Char(0xC4));

        reference.path.clear();
        reference
            .path
            .push(RefSegment::Index(ref_indices_from_iter([2])));
        assert!(storage.write_by_ref(reference.clone(), Value::Char(b'X')));
        assert_eq!(
            storage.get_global("TEXT"),
            Some(&Value::String("ÄXC".into()))
        );
    }
}
