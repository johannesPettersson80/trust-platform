//! The persistent instances of the cell store: a fixed table reserved once
//! (specification 34 §6.2, MEM-10; C1-9d2b-2).
//!
//! A persistent instance is one function block, class or program instance that construction creates
//! for a CONSTRUCTION_ROOTS record with a template POU. Its members live in a cell range of the
//! persistent region; this table says which range, which template, and which parent. The table is
//! sized from the roots of the prepared module, charged with the rest of the construction charge
//! before it is reserved, and never grown, so it allocates nothing after construction.

use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::memory::layout::ByteRange;
use crate::memory::InstanceId;
use crate::vm::VmTrap;

/// A persistent instance held in cells: its template POU, the range of its own members and its parent
/// (specification 34 §6.2; O6, O8). Its type name is the template POU's name, so the entry holds no text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PersistentInstance {
    /// The template POU the instance was created from.
    pub(crate) pou: u32,
    /// The cells of the instance's own members.
    pub(crate) range: ByteRange,
    /// The instance whose members it inherits, if any.
    pub(crate) parent: Option<InstanceId>,
}

/// The bytes of one table entry: the identity and the instance. No field holds a pointer, so an entry has
/// the same size on every target, which the assertion below states (specification 34 §6.2).
pub(crate) const INSTANCE_ENTRY_BYTES: usize = 28;

const _: () =
    assert!(core::mem::size_of::<(InstanceId, PersistentInstance)>() == INSTANCE_ENTRY_BYTES);

/// The persistent instances of one region, ordered by identity, in storage reserved once.
#[derive(Debug, Default)]
pub(crate) struct InstanceTable {
    entries: Vec<(InstanceId, PersistentInstance)>,
}

impl InstanceTable {
    /// The bytes a table of `instances` entries reserves; construction charges them before it reserves.
    pub(crate) fn reservation_bytes(instances: usize) -> Option<usize> {
        instances.checked_mul(INSTANCE_ENTRY_BYTES)
    }

    /// Reserve room for `instances` entries, once. The table never grows.
    ///
    /// # Errors
    ///
    /// Refused as the other construction reservations are when the room cannot be reserved.
    pub(crate) fn reserve(instances: usize) -> Result<Self, RuntimeError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(instances)
            .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
        Ok(Self { entries })
    }

    /// Record the persistent instance `id`, replacing the entry it already has.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when `id` is new and the reserved room is full: the table
    /// never grows, so the count the construction charged and the instances it creates disagree.
    pub(crate) fn insert(
        &mut self,
        id: InstanceId,
        instance: PersistentInstance,
    ) -> Result<(), RuntimeError> {
        match self.entries.binary_search_by_key(&id, |(key, _)| *key) {
            Ok(index) => self.entries[index].1 = instance,
            Err(index) => {
                if self.entries.len() == self.entries.capacity() {
                    return Err(RuntimeError::InvalidExecutionState);
                }
                self.entries.insert(index, (id, instance));
            }
        }
        Ok(())
    }

    /// The persistent instance `id`, or `None` for an identity that is not one.
    pub(crate) fn get(&self, id: InstanceId) -> Option<&PersistentInstance> {
        let index = self
            .entries
            .binary_search_by_key(&id, |(key, _)| *key)
            .ok()?;
        self.entries.get(index).map(|(_, instance)| instance)
    }

    /// The persistent instance `id`, mutably.
    pub(crate) fn get_mut(&mut self, id: InstanceId) -> Option<&mut PersistentInstance> {
        let index = self
            .entries
            .binary_search_by_key(&id, |(key, _)| *key)
            .ok()?;
        self.entries.get_mut(index).map(|(_, instance)| instance)
    }

    /// The persistent instance whose own members' bytes hold the byte at `offset`, read at the
    /// boundary, not in a scan (C1-11d).
    pub(crate) fn holding(&self, offset: u32) -> Option<(InstanceId, PersistentInstance)> {
        self.entries.iter().copied().find(|(_, instance)| {
            offset >= instance.range.start && offset - instance.range.start < instance.range.bytes
        })
    }

    /// The number of persistent instances recorded.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// The persistent instance at `position` in identity order.
    pub(crate) fn at(&self, position: usize) -> Option<(InstanceId, PersistentInstance)> {
        self.entries.get(position).copied()
    }

    /// The identities of the persistent instances, in identity order.
    pub(crate) fn ids(&self) -> impl Iterator<Item = InstanceId> + '_ {
        self.entries.iter().map(|(id, _)| *id)
    }

    /// The number of entries the reserved room holds, which the table never exceeds (test support).
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.entries.capacity()
    }
}
