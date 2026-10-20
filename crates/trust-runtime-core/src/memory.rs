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
    /// A target in a flat block (specification 34 MEM-12; C1-11d): the reference's `offset` is the
    /// target's byte offset in the block of `region`, and `type_id` its type. `elements` is the
    /// element count of the path the reference was taken through, which a copy of the reference
    /// is charged by, as the value store charged the copy of the path (TIME-08; C1-19e).
    Flat {
        /// The block that holds the target.
        region: layout::LayoutRegion,
        /// The element count of the path the reference was taken through.
        elements: u16,
        /// The target's `TYPE_TABLE` type.
        type_id: u32,
    },
}

/// A location is eight bytes on every target, so a flat reference is no larger than a path one.
const _: () = assert!(core::mem::size_of::<MemoryLocation>() == 8);

impl MemoryLocation {
    /// The element count of the path a flat reference was taken through, which its copies are
    /// charged by (TIME-08; C1-11d), and 0 for any other location.
    #[must_use]
    pub fn flat_elements(&self) -> usize {
        match self {
            Self::Flat { elements, .. } => usize::from(*elements),
            _ => 0,
        }
    }
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

/// Storage for runtime variables (specification 34 §6.2): the values half, the activation half
/// and the identities. The cell store (C1-9a, [`crate::memory::regions::CellStorage`]) shares the
/// activation half and the identities; the values half is deleted at C1-19.
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
    execution_frames: live::LiveEntries<FrameId, ExecutionActivation>,
    execution_frame_limit: Option<usize>,
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
    /// the entry floor is live in the current top-level entry, so [`Identities::begin_entry`]
    /// reissues the identities of the entry only once none of them is live.
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

    /// Begin a top-level execution entry (specification 34 §4.3.1, HOST-02). The identities issued
    /// since the previous entry began are issued again when none of their instances is still live;
    /// otherwise the entry starts above them. A live instance's identity is never issued again.
    fn begin_entry(&mut self) {
        if self.entry_instances_live == 0 {
            self.next_instance_id = self.entry_instance_floor;
        } else {
            self.start_entry_at_counter();
        }
    }

    /// Start the entry floor at the counter, with no instance live above it.
    fn start_entry_at_counter(&mut self) {
        self.entry_instance_floor = self.next_instance_id;
        self.entry_instances_live = 0;
    }
}

/// One reserved activation identity, with locals and engine metadata sharing
/// the same monotonically ordered registry entry.
#[derive(Debug, Clone, Default)]
struct ExecutionActivation {
    locals: Option<Vec<Value>>,
    active: bool,
    owner: Option<ExecutionFrameOwner>,
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
mod live;

#[cfg(feature = "std")]
pub type InstanceMap = FxHashMap<InstanceId, InstanceData>;
#[cfg(not(feature = "std"))]
pub type InstanceMap = live::LiveEntries<InstanceId, InstanceData>;

mod access;
/// The cell arm of the store-access seam: the reads over the cell store (specification 34 §6.2,
/// MEM-10; C1-9b).
pub mod cell_access;
/// The decoded value of one leaf and one region's flat block (specification 34 §6.2, MEM-12).
pub mod cells;
mod execution_frames;
/// The bytes of heap that a prepared program holds, counted from its own tables (specification 34
/// MEM-12, MEM-14).
pub(crate) mod figure;
/// The bytes of one leaf of a flat block: every variable at its declared size, with no tag
/// (specification 34 MEM-12).
pub mod flat;
mod frames;
mod instance_table;
mod instances;
/// Prepared layouts of the flat blocks (specification 34 §6.2, MEM-12).
pub mod layout;
mod references;
/// The persistent flat block of the bounded value store and its reference table, reserved once
/// from the prepared layouts (specification 34 §6.2, MEM-12, C1-9a).
pub mod regions;
pub(crate) use references::PreparedStorageWrite;
mod storage;
/// The store-access seam the engine reaches variable storage through
/// (specification 34 §6.2, MEM-10).
pub mod view;

#[cfg(all(test, feature = "std"))]
mod tests;

#[cfg(test)]
mod live_tests;

#[cfg(test)]
mod bounded_identity_tests;
