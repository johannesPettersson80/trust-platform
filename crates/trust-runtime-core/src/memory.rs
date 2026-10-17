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

/// Installation generation of a storage and of every reference taken in it
/// (specification 34 §11.3, MEM-07, UPDATE-04).
///
/// Instantiating an application starts the first generation. Each committed
/// warm or cold restart and each committed replacement constructs the next one;
/// a resource-only reconfiguration keeps it. The counter never wraps: once it
/// is exhausted, a further restart or replacement is refused with `Overflow`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(pub u32);

impl Generation {
    /// The generation of a newly instantiated application.
    pub const FIRST: Self = Self(0);

    /// The generation a restart or replacement of this one constructs, or
    /// `None` once the 32-bit counter is exhausted.
    #[must_use]
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// VM owner metadata for a live execution activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExecutionFrameOwner {
    pub(crate) pou_id: u32,
    pub(crate) runtime_instance: Option<InstanceId>,
}

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
    materialize_value_path, read_value_path_borrowed, PartialAccess, RefPath, RefSegment, Value,
    ValueRef,
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
    execution_frames: live::LiveEntries<FrameId, ExecutionActivation>,
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
    /// The instance identity the current top-level execution entry started from.
    entry_instance_floor: u32,
    /// Live instances whose identity is `entry_instance_floor` or above.
    entry_instances_live: usize,
    /// The installation generation every reference into this storage carries.
    generation: Generation,
}

/// One reserved activation identity, with locals and engine metadata sharing
/// the same monotonically ordered registry entry.
#[derive(Debug, Clone, Default)]
struct ExecutionActivation {
    locals: Option<Vec<Value>>,
    active: bool,
    owner: Option<ExecutionFrameOwner>,
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
            entry_instance_floor: self.entry_instance_floor,
            entry_instances_live: self.entry_instances_live,
            generation: self.generation,
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
/// The cell, the unit of the bounded value store, and one region's cell store
/// (specification 34 §6.2).
pub mod cells;
mod execution_frames;
mod frames;
mod instances;
/// Prepared cell layouts for the bounded value store (specification 34 §6.2).
pub mod layout;
mod references;
pub(crate) use references::PreparedStorageWrite;
mod storage;

#[cfg(all(test, feature = "std"))]
mod tests;

#[cfg(test)]
mod live_tests;

#[cfg(test)]
mod bounded_identity_tests;
