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
        let index = self.position(id)?;
        self.entries.get(index).map(|(_, instance)| instance)
    }

    /// The persistent instance `id`, mutably.
    pub(crate) fn get_mut(&mut self, id: InstanceId) -> Option<&mut PersistentInstance> {
        let index = self.position(id)?;
        self.entries.get_mut(index).map(|(_, instance)| instance)
    }

    /// The position of `id` in the table. Construction issues a module's persistent identities one
    /// after the other, so an identity is usually as far from the first entry's as its entry is from
    /// the first (checklist P1-1): that entry is compared first, and any other identity is found by
    /// its order.
    fn position(&self, id: InstanceId) -> Option<usize> {
        let (first, _) = self.entries.first()?;
        let distance = usize::try_from(id.0.checked_sub(first.0)?).ok()?;
        if self
            .entries
            .get(distance)
            .is_some_and(|(key, _)| *key == id)
        {
            return Some(distance);
        }
        self.entries.binary_search_by_key(&id, |(key, _)| *key).ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::layout::LayoutRegion;

    fn instance(pou: u32) -> PersistentInstance {
        PersistentInstance {
            pou,
            range: ByteRange {
                region: LayoutRegion::Persistent,
                start: pou * 8,
                bytes: 8,
            },
            parent: None,
        }
    }

    /// An identity is found whether it lies at its distance from the first entry's, as construction
    /// issues them, or past a gap, as when identities are issued between persistent ones; an
    /// identity below the first, past the last or in a gap is none. Catches the distance taken as
    /// the position without comparing the identity there, and a lookup that stops at the guess.
    #[test]
    fn an_identity_is_found_at_its_distance_or_by_its_order() {
        let mut table = InstanceTable::reserve(4).expect("reserves");
        for (id, pou) in [(5, 1), (6, 2), (9, 3), (10, 4)] {
            table.insert(InstanceId(id), instance(pou)).expect("fits");
        }
        assert_eq!(table.get(InstanceId(5)), Some(&instance(1)));
        assert_eq!(table.get(InstanceId(6)), Some(&instance(2)));
        assert_eq!(table.get(InstanceId(9)), Some(&instance(3)));
        assert_eq!(table.get(InstanceId(10)), Some(&instance(4)));
        for missing in [0, 4, 7, 8, 11, u32::MAX] {
            assert_eq!(table.get(InstanceId(missing)), None, "{missing}");
        }
        table.get_mut(InstanceId(9)).expect("present").parent = Some(InstanceId(5));
        assert_eq!(
            table.get(InstanceId(9)).and_then(|entry| entry.parent),
            Some(InstanceId(5))
        );
        assert_eq!(InstanceTable::default().get(InstanceId(0)), None);
    }
}
