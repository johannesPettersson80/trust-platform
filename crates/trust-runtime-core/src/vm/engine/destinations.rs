//! Ordered snapshots of distinct physical destination slots.
use super::{RuntimeError, Value, Vec};
use crate::memory::MemoryLocation;

pub(super) type Key = (MemoryLocation, usize);

/// Sorted destination snapshots. Growth is charged against `charged_capacity`, the capacity a
/// snapshot has when it grows only on demand, so storage kept from an earlier use or reserved
/// at construction changes no charge (spec 34 TIME-08).
#[derive(Default)]
pub(super) struct SavedDestinations {
    entries: Vec<(Key, Value)>,
    charged_capacity: usize,
}
impl SavedDestinations {
    pub(super) fn new() -> Self {
        Self::default()
    }
    #[cfg(test)]
    pub(super) fn with_capacity(capacity: usize) -> Result<Self, RuntimeError> {
        let mut saved = Self::new();
        saved.reuse(capacity)?;
        Ok(saved)
    }
    /// Empty the snapshot for another use, charged as a new snapshot with room for
    /// `capacity` entries; the storage it holds is kept.
    pub(super) fn reuse(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.entries.clear();
        self.entries
            .try_reserve_exact(capacity)
            .map_err(|_| RuntimeError::Overflow)?;
        self.charged_capacity = capacity;
        Ok(())
    }
    /// Empty the snapshot, charged as a new one from then on; the storage it holds is kept.
    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.charged_capacity = 0;
    }
    /// Reserve storage for `capacity` entries at construction without changing any charge.
    pub(super) fn reserve_storage(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.entries
            .try_reserve_exact(capacity.saturating_sub(self.entries.len()))
            .map_err(|_| RuntimeError::Overflow)
    }
    pub(super) fn allocation_bytes(capacity: usize) -> Result<usize, RuntimeError> {
        capacity
            .checked_mul(core::mem::size_of::<(Key, Value)>())
            .ok_or_else(|| RuntimeError::Overflow)
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn get(&self, key: &Key) -> Option<&Value> {
        self.entries
            .binary_search_by_key(key, |(key, _)| *key)
            .ok()
            .map(|at| &self.entries[at].1)
    }
    pub(super) fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }
    #[cfg(test)]
    pub(super) fn keys(&self) -> impl Iterator<Item = &Key> {
        self.entries.iter().map(|(key, _)| key)
    }
    /// Full replacement allocation and relocation work, before any mutation.
    pub(super) fn insertion_demand(&self, key: &Key) -> Result<(usize, usize), RuntimeError> {
        let at = match self.entries.binary_search_by_key(key, |(key, _)| *key) {
            Ok(_) => return Ok((0, 0)),
            Err(at) => at,
        };
        let shifts = self.entries.len() - at;
        if self.entries.len() < self.charged_capacity {
            return Ok((0, shifts));
        }
        let capacity = self.next_capacity()?;
        let bytes = Self::allocation_bytes(capacity)?;
        let work = self
            .entries
            .len()
            .checked_add(shifts)
            .ok_or_else(|| RuntimeError::Overflow)?;
        Ok((bytes, work))
    }
    fn next_capacity(&self) -> Result<usize, RuntimeError> {
        self.charged_capacity
            .checked_mul(2)
            .map(|n| n.max(4))
            .ok_or_else(|| RuntimeError::Overflow)
    }
    pub(super) fn insert(&mut self, key: Key, value: Value) -> Result<Option<Value>, RuntimeError> {
        match self.entries.binary_search_by_key(&key, |(key, _)| *key) {
            Ok(at) => Ok(Some(core::mem::replace(&mut self.entries[at].1, value))),
            Err(at) => {
                let len = self.entries.len();
                let charged = if len == self.charged_capacity {
                    self.next_capacity()?
                } else {
                    self.charged_capacity
                };
                // Kept or reserved storage already holds the entry; otherwise the snapshot
                // grows here, exactly to the charged capacity.
                if len == self.entries.capacity() {
                    self.entries
                        .try_reserve_exact(charged - len)
                        .map_err(|_| RuntimeError::Overflow)?;
                }
                self.charged_capacity = charged;
                self.entries.insert(at, (key, value));
                Ok(None)
            }
        }
    }
    /// Take every saved root in key order, keeping the storage.
    pub(super) fn drain(&mut self) -> alloc::vec::Drain<'_, (Key, Value)> {
        self.entries.drain(..)
    }
}
impl IntoIterator for SavedDestinations {
    type Item = (Key, Value);
    type IntoIter = alloc::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{FrameId, InstanceId, IoArea};
    #[test]
    fn insertion_demand_covers_growth_relocation_and_duplicate_replacement() {
        let mut saved = SavedDestinations::new();
        let bytes = core::mem::size_of::<(Key, Value)>();
        for offset in (0..33).rev() {
            let key = (MemoryLocation::Global, offset);
            let old_len = saved.len();
            let old_capacity = saved.entries.capacity();
            let (allocation, work) = saved.insertion_demand(&key).unwrap();
            saved.insert(key, Value::Int(1)).unwrap();
            if saved.entries.capacity() != old_capacity {
                assert_eq!(allocation, saved.entries.capacity() * bytes);
                assert_eq!(work, old_len * 2);
            } else {
                assert_eq!(allocation, 0);
                assert_eq!(work, old_len);
            }
            assert_eq!(saved.insertion_demand(&key).unwrap(), (0, 0));
            assert_eq!(
                saved.insert(key, Value::Int(2)).unwrap(),
                Some(Value::Int(1))
            );
        }
    }
    /// Spec 34 TIME-08: a snapshot kept from an earlier use, or reserved at construction, is
    /// charged its growth exactly as a new snapshot is, and holds its entries in the storage it
    /// kept.
    #[test]
    fn a_reused_snapshot_charges_its_growth_as_a_new_one_in_kept_storage() {
        let mut reused = SavedDestinations::new();
        reused.reserve_storage(16).unwrap();
        for offset in 0..12 {
            reused
                .insert((MemoryLocation::Global, offset), Value::Int(1))
                .unwrap();
        }
        let storage = reused.entries.as_ptr();
        for capacity in [0, 3] {
            reused.reuse(capacity).unwrap();
            let mut new = SavedDestinations::with_capacity(capacity).unwrap();
            for offset in (0..9).rev() {
                let key = (MemoryLocation::Global, offset);
                assert_eq!(
                    reused.insertion_demand(&key).unwrap(),
                    new.insertion_demand(&key).unwrap(),
                    "capacity {capacity}, offset {offset}"
                );
                reused.insert(key, Value::Int(2)).unwrap();
                new.insert(key, Value::Int(2)).unwrap();
            }
            assert_eq!(reused.entries, new.entries);
            assert_eq!(reused.entries.as_ptr(), storage);
        }
        reused.clear();
        assert_eq!(reused.len(), 0);
        assert_eq!(
            reused
                .insertion_demand(&(MemoryLocation::Global, 0))
                .unwrap(),
            SavedDestinations::new()
                .insertion_demand(&(MemoryLocation::Global, 0))
                .unwrap()
        );
    }

    #[test]
    fn insertion_demand_charges_only_actual_shifts_and_growth_relocation() {
        let mut saved = SavedDestinations::with_capacity(4).unwrap();
        for offset in [2, 4, 6, 8] {
            assert_eq!(
                saved
                    .insertion_demand(&(MemoryLocation::Global, offset))
                    .unwrap(),
                (0, 0)
            );
            saved
                .insert((MemoryLocation::Global, offset), Value::Int(1))
                .unwrap();
        }
        let bytes = SavedDestinations::allocation_bytes(8).unwrap();
        assert_eq!(
            saved
                .insertion_demand(&(MemoryLocation::Global, 10))
                .unwrap(),
            (bytes, 4)
        );
        assert_eq!(
            saved
                .insertion_demand(&(MemoryLocation::Global, 5))
                .unwrap(),
            (bytes, 6)
        );
        assert_eq!(
            saved
                .insertion_demand(&(MemoryLocation::Global, 0))
                .unwrap(),
            (bytes, 8)
        );
        saved
            .insert((MemoryLocation::Global, 10), Value::Int(1))
            .unwrap();
        assert_eq!(
            saved
                .insertion_demand(&(MemoryLocation::Global, 5))
                .unwrap(),
            (0, 3)
        );
        assert_eq!(
            saved
                .insertion_demand(&(MemoryLocation::Global, 12))
                .unwrap(),
            (0, 0)
        );
    }
    #[test]
    fn ordered_snapshots_preserve_full_domains_and_replace_exact_aliases() {
        let mut old = alloc::collections::BTreeMap::new();
        let mut new = SavedDestinations::new();
        for location in [
            MemoryLocation::Retain,
            MemoryLocation::Io(IoArea::Memory),
            MemoryLocation::Instance(InstanceId(u32::MAX)),
            MemoryLocation::Local(FrameId(u32::MAX)),
            MemoryLocation::Global,
        ] {
            for offset in [usize::MAX, 0, 3, usize::MAX] {
                let key = (location, offset);
                assert_eq!(
                    new.insert(key, Value::Int(8)).unwrap(),
                    old.insert(key, Value::Int(8))
                );
                assert_eq!(new.get(&key), old.get(&key));
            }
        }
        assert_eq!(
            new.into_iter().collect::<Vec<_>>(),
            old.into_iter().collect::<Vec<_>>()
        );
    }
}
