//! Shared variable storage and portable memory identities.

#![allow(missing_docs)]

/// Memory location identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IoArea {
    /// Input area (%I).
    Input,
    /// Output area (%Q).
    Output,
    /// Memory area (%M).
    Memory,
}

/// Frame identifier for call stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameId(pub u32);

/// Instance identifier for FB/Class instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId(pub u32);

#[cfg(test)]
mod identity_tests {
    use super::{FrameId, InstanceId, IoArea, MemoryLocation};
    use core::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    fn hash(value: MemoryLocation) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn memory_identity_values_preserve_equality_and_hash_shape() {
        assert_eq!(MemoryLocation::Global, MemoryLocation::Global);
        assert_eq!(
            MemoryLocation::Local(FrameId(7)),
            MemoryLocation::Local(FrameId(7))
        );
        assert_ne!(
            MemoryLocation::Instance(InstanceId(1)),
            MemoryLocation::Instance(InstanceId(2))
        );
        assert_eq!(
            MemoryLocation::Io(IoArea::Input),
            MemoryLocation::Io(IoArea::Input)
        );
        assert_ne!(
            MemoryLocation::Io(IoArea::Input),
            MemoryLocation::Io(IoArea::Output)
        );
        assert_ne!(MemoryLocation::Global, MemoryLocation::Retain);
        assert_eq!(
            hash(MemoryLocation::Local(FrameId(7))),
            hash(MemoryLocation::Local(FrameId(7)))
        );
    }
}

#[cfg(feature = "std")]
use crate::collections::LookupMap as FxHashMap;
use crate::collections::OrderedMap as IndexMap;
use crate::value::{
    materialize_value_path, read_value_path_borrowed, write_value_path, PartialAccess, RefPath,
    RefSegment, Value, ValueRef,
};
use alloc::vec::Vec;
#[cfg(feature = "std")]
use cache::StorageCache as RwLock;
use smol_str::SmolStr;

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
    execution_frames: live::LiveEntries<FrameId, Option<Vec<Value>>>,
    execution_frame_limit: Option<usize>,
    instances: InstanceMap,
    retain: IndexMap<SmolStr, Value>,
    #[cfg(feature = "std")]
    instance_field_offsets: RwLock<FxHashMap<(InstanceId, SmolStr), Option<usize>>>,
    #[cfg(feature = "std")]
    recursive_instance_field_resolutions:
        RwLock<FxHashMap<(InstanceId, SmolStr), RecursiveInstanceFieldResolution>>,
    #[cfg(feature = "std")]
    declared_instance_field_offsets: RwLock<FxHashMap<(SmolStr, SmolStr), usize>>,
    next_frame_id: u32,
    next_instance_id: u32,
}

impl Clone for VariableStorage {
    fn clone(&self) -> Self {
        Self {
            globals: self.globals.clone(),
            frames: self.frames.clone(),
            execution_frames: self.execution_frames.clone(),
            execution_frame_limit: self.execution_frame_limit,
            instances: self.instances.clone(),
            retain: self.retain.clone(),
            #[cfg(feature = "std")]
            instance_field_offsets: RwLock::new(
                recover_read_lock(self.instance_field_offsets.read())
                    .map(|cache| cache.clone())
                    .unwrap_or_default(),
            ),
            #[cfg(feature = "std")]
            recursive_instance_field_resolutions: RwLock::new(
                recover_read_lock(self.recursive_instance_field_resolutions.read())
                    .map(|cache| cache.clone())
                    .unwrap_or_default(),
            ),
            #[cfg(feature = "std")]
            declared_instance_field_offsets: RwLock::new(
                recover_read_lock(self.declared_instance_field_offsets.read())
                    .map(|cache| cache.clone())
                    .unwrap_or_default(),
            ),
            next_frame_id: self.next_frame_id,
            next_instance_id: self.next_instance_id,
        }
    }
}

#[cfg(feature = "std")]
use cache::recover_read_lock;
#[cfg(feature = "std")]
mod cache;
mod live;

#[cfg(feature = "std")]
pub type InstanceMap = FxHashMap<InstanceId, InstanceData>;
#[cfg(not(feature = "std"))]
pub type InstanceMap = live::LiveEntries<InstanceId, InstanceData>;

mod access;
mod execution_frames;
mod frames;
mod instances;
mod references;
mod storage;

#[cfg(all(test, feature = "std"))]
mod tests;

#[cfg(test)]
mod live_tests;
