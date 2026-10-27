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
use crate::value::{materialize_value_path, read_value_path_borrowed, RefPath, RefSegment, Value};
use alloc::vec::Vec;
#[cfg(feature = "std")]
use cache::StorageCache as RwLock;
use smol_str::SmolStr;

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

/// Storage for the values the build evaluates initial values with (specification 44 §6.7): the
/// values half, the activation half and the identities.
#[derive(Debug, Default, Clone)]
pub struct VariableStorage {
    values: ValueStore,
    activations: ActivationStore,
    identities: Identities,
}

/// The values half of [`VariableStorage`]: the globals, the retain map and the instance lookup
/// caches. The cell arm holds none of them (specification 34 §6.2, MEM-10).
#[derive(Debug, Default)]
struct ValueStore {
    globals: IndexMap<SmolStr, Value>,
    retain: IndexMap<SmolStr, Value>,
    #[cfg(feature = "std")]
    instance_field_offsets: RwLock<FxHashMap<(InstanceId, SmolStr), Option<usize>>>,
    #[cfg(feature = "std")]
    recursive_instance_field_resolutions:
        RwLock<FxHashMap<(InstanceId, SmolStr), RecursiveInstanceFieldResolution>>,
    #[cfg(feature = "std")]
    declared_instance_field_offsets: RwLock<FxHashMap<(SmolStr, SmolStr), usize>>,
}

/// The activation half of [`VariableStorage`] and of the cell store: the local frames, the
/// execution registry and the per-call instances (specification 34 §6.2).
#[derive(Debug, Default, Clone)]
struct ActivationStore {
    frames: Vec<LocalFrame>,
    instances: InstanceMap,
    next_frame_id: u32,
}

/// The identities shared by both kinds of instance (specification 34 §6.2): the instance
/// counter, the top-level entry's floor and live count, and the generation (MEM-07).
#[derive(Debug, Default, Clone)]
struct Identities {
    next_instance_id: u32,
    /// The instance identity the current top-level execution entry started from.
    entry_instance_floor: u32,
    /// Live instances whose identity is `entry_instance_floor` or above.
    entry_instances_live: usize,
    /// The installation generation every reference into this storage carries.
    generation: Generation,
}

impl Identities {
    /// The identity the next instance takes and the counter value after it, or `Overflow` once the
    /// counter is exhausted: the allocator hands out `0` through `u32::MAX - 1` and never `u32::MAX`
    /// (specification 34 HOST-02). Nothing is issued until [`Identities::issue`] records it, so a
    /// caller that cannot place the instance leaves the counter where it was.
    fn next_instance(&self) -> Result<(InstanceId, u32), crate::error::RuntimeError> {
        let following = self
            .next_instance_id
            .checked_add(1)
            .ok_or(crate::error::RuntimeError::Overflow)?;
        Ok((InstanceId(self.next_instance_id), following))
    }

    /// Record that `id` was issued and the counter now holds `following`. An instance at or above
    /// the entry floor is live in the current top-level entry.
    fn issue(&mut self, id: InstanceId, following: u32) {
        self.next_instance_id = following;
        if id.0 >= self.entry_instance_floor {
            self.entry_instances_live = self.entry_instances_live.saturating_add(1);
        }
    }

    /// Record that `released` instances at or above the entry floor are gone.
    fn release(&mut self, released: usize) {
        self.entry_instances_live = self.entry_instances_live.saturating_sub(released);
    }

    /// Record that the instance `id` is gone, if it was live in the current entry.
    fn release_instance(&mut self, id: InstanceId) {
        if id.0 >= self.entry_instance_floor {
            self.release(1);
        }
    }

    /// Start the entry floor at the counter, with no instance live above it.
    fn start_entry_at_counter(&mut self) {
        self.entry_instance_floor = self.next_instance_id;
        self.entry_instances_live = 0;
    }
}

impl Clone for ValueStore {
    fn clone(&self) -> Self {
        Self {
            globals: self.globals.clone(),
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
        }
    }
}

#[cfg(feature = "std")]
use cache::recover_read_lock;
#[cfg(feature = "std")]
mod cache;
#[cfg(any(test, not(feature = "std")))]
mod live;

#[cfg(feature = "std")]
pub type InstanceMap = FxHashMap<InstanceId, InstanceData>;
#[cfg(not(feature = "std"))]
pub type InstanceMap = live::LiveEntries<InstanceId, InstanceData>;

mod frames;
mod instances;
mod references;
mod storage;

#[cfg(all(test, feature = "std"))]
mod tests;

#[cfg(test)]
mod live_tests;
