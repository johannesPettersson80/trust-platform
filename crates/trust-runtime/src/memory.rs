//! Variable storage, frames, and instance data.

#![allow(missing_docs)]

use crate::value::{
    materialize_value_path, read_value_path_borrowed, write_value_path, PartialAccess, RefPath,
    RefSegment, Value, ValueRef,
};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::sync::RwLock;
pub use trust_runtime_core::memory::{FrameId, InstanceId, IoArea, MemoryLocation};

pub use self::access::{AccessBinding, AccessMap};

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
                recover_read_lock(self.instance_field_offsets.read()).clone(),
            ),
            recursive_instance_field_resolutions: RwLock::new(
                recover_read_lock(self.recursive_instance_field_resolutions.read()).clone(),
            ),
            declared_instance_field_offsets: RwLock::new(
                recover_read_lock(self.declared_instance_field_offsets.read()).clone(),
            ),
            next_frame_id: self.next_frame_id,
            next_instance_id: self.next_instance_id,
        }
    }
}

fn recover_read_lock<T>(
    result: std::sync::LockResult<std::sync::RwLockReadGuard<'_, T>>,
) -> std::sync::RwLockReadGuard<'_, T> {
    result.unwrap_or_else(std::sync::PoisonError::into_inner)
}

mod access;
mod frames;
mod instances;
mod references;
mod storage;

#[cfg(test)]
mod tests;
