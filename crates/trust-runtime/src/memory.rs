//! Variable storage, frames, and instance data.

#![allow(missing_docs)]

use crate::value::{PartialAccess, RefSegment, Value, ValueRef};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

/// Memory location identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryLocation {
    /// Global variable area.
    Global,
    /// Local variable area for a specific call frame.
    Local(FrameId),
    /// FB/Class instance storage.
    Instance(InstanceId),
    /// I/O area (direct addresses).
    Io(IoArea),
    /// Retain area (persistent across warm restart).
    Retain,
}

/// I/O area identifiers per IEC 61131-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoArea {
    /// Input area (%I).
    Input,
    /// Output area (%Q).
    Output,
    /// Memory area (%M).
    Memory,
}

/// Frame identifier for call stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameId(pub u32);

/// Instance identifier for FB/Class instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u32);

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

/// Storage for runtime variables.
#[derive(Debug, Default, Clone)]
pub struct VariableStorage {
    globals: IndexMap<SmolStr, Value>,
    frames: Vec<LocalFrame>,
    instances: FxHashMap<InstanceId, InstanceData>,
    retain: IndexMap<SmolStr, Value>,
    next_frame_id: u32,
    next_instance_id: u32,
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
        if let Some(instance) = self.instances.get_mut(&id) {
            instance.variables.insert(name.into(), value);
            true
        } else {
            false
        }
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
        let instance = self.instances.get(&id)?;
        ref_for_map(&instance.variables, MemoryLocation::Instance(id), name)
    }

    pub fn ref_for_instance_recursive(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        let mut current = Some(id);
        while let Some(instance_id) = current {
            if let Some(reference) = self.ref_for_instance(instance_id, name) {
                return Some(reference);
            }
            current = self
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.parent);
        }
        None
    }

    pub fn read_by_ref(&self, value_ref: crate::value::ValueRef) -> Option<&Value> {
        self.read_by_ref_parts(value_ref.location, value_ref.offset, &value_ref.path)
    }

    pub fn read_by_ref_parts(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Option<&Value> {
        let resolved = self.resolve_reference_parts(location, offset, path)?;
        let root = match resolved.location {
            MemoryLocation::Global => self.globals.get_index(resolved.offset).map(|(_, v)| v),
            MemoryLocation::Local(frame_id) => self
                .frames
                .iter()
                .find(|frame| frame.id == frame_id)
                .and_then(|frame| frame.variables.get_index(resolved.offset).map(|(_, v)| v)),
            MemoryLocation::Instance(instance_id) => {
                self.instances.get(&instance_id).and_then(|instance| {
                    instance
                        .variables
                        .get_index(resolved.offset)
                        .map(|(_, v)| v)
                })
            }
            MemoryLocation::Io(_) | MemoryLocation::Retain => None,
        }?;

        read_by_ref_path(root, &resolved.path)
    }

    pub fn write_by_ref(&mut self, value_ref: crate::value::ValueRef, value: Value) -> bool {
        self.write_by_ref_parts(value_ref.location, value_ref.offset, &value_ref.path, value)
    }

    pub fn write_by_ref_parts(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> bool {
        let Some(resolved) = self.resolve_reference_parts(location, offset, path) else {
            return false;
        };

        match resolved.location {
            MemoryLocation::Global => {
                let Some((_, slot)) = self.globals.get_index_mut(resolved.offset) else {
                    return false;
                };
                write_by_ref_path(slot, &resolved.path, value)
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
                .map(|slot| write_by_ref_path(slot, &resolved.path, value))
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
                .map(|slot| write_by_ref_path(slot, &resolved.path, value))
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
            path: Vec::new(),
        };

        for segment in path {
            match segment {
                RefSegment::Field(name) => {
                    let current = self.read_by_ref(resolved.clone())?;
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
        path: Vec::new(),
    })
}

fn read_by_ref_path<'a>(value: &'a Value, path: &[RefSegment]) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }
    match &path[0] {
        RefSegment::Field(name) => match value {
            Value::Struct(struct_value) => struct_value
                .fields
                .get(name)
                .and_then(|field| read_by_ref_path(field, &path[1..])),
            _ => None,
        },
        RefSegment::Index(indices) => match value {
            Value::Array(array) => {
                let offset = array_offset_i64(&array.dimensions, indices)?;
                array
                    .elements
                    .get(offset)
                    .and_then(|element| read_by_ref_path(element, &path[1..]))
            }
            _ => None,
        },
    }
}

fn write_by_ref_path(target: &mut Value, path: &[RefSegment], value: Value) -> bool {
    if path.is_empty() {
        *target = value;
        return true;
    }

    match &path[0] {
        RefSegment::Field(name) => match target {
            Value::Struct(struct_value) => struct_value
                .fields
                .get_mut(name)
                .map(|field| write_by_ref_path(field, &path[1..], value))
                .unwrap_or(false),
            _ => false,
        },
        RefSegment::Index(indices) => match target {
            Value::Array(array) => {
                let offset = match array_offset_i64(&array.dimensions, indices) {
                    Some(offset) => offset,
                    None => return false,
                };
                array
                    .elements
                    .get_mut(offset)
                    .map(|element| write_by_ref_path(element, &path[1..], value))
                    .unwrap_or(false)
            }
            _ => false,
        },
    }
}

fn array_offset_i64(dimensions: &[(i64, i64)], indices: &[i64]) -> Option<usize> {
    if dimensions.len() != indices.len() {
        return None;
    }
    let mut offset: i128 = 0;
    let mut stride: i128 = 1;
    for ((lower, upper), index) in dimensions.iter().zip(indices).rev() {
        if index < lower || index > upper {
            return None;
        }
        let len = (*upper - *lower + 1) as i128;
        offset += (index - *lower) as i128 * stride;
        stride *= len;
    }
    usize::try_from(offset).ok()
}
