//! Ordered snapshots of distinct physical destination slots.
use super::{RuntimeError, Value, Vec};
use crate::memory::MemoryLocation;

pub(super) type Key = (MemoryLocation, usize);

#[derive(Default)]
pub(super) struct SavedDestinations(Vec<(Key, Value)>);
impl SavedDestinations {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn with_capacity(capacity: usize) -> Result<Self, RuntimeError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| RuntimeError::Overflow)?;
        Ok(Self(entries))
    }
    pub(super) fn allocation_bytes(capacity: usize) -> Result<usize, RuntimeError> {
        capacity
            .checked_mul(core::mem::size_of::<(Key, Value)>())
            .ok_or(RuntimeError::Overflow)
    }
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn get(&self, key: &Key) -> Option<&Value> {
        self.0
            .binary_search_by_key(key, |(key, _)| *key)
            .ok()
            .map(|at| &self.0[at].1)
    }
    pub(super) fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }
    #[cfg(test)]
    pub(super) fn keys(&self) -> impl Iterator<Item = &Key> {
        self.0.iter().map(|(key, _)| key)
    }
    /// Full replacement allocation and relocation work, before any mutation.
    pub(super) fn insertion_demand(&self, key: &Key) -> Result<(usize, usize), RuntimeError> {
        let at = match self.0.binary_search_by_key(key, |(key, _)| *key) {
            Ok(_) => return Ok((0, 0)),
            Err(at) => at,
        };
        let shifts = self.0.len() - at;
        if self.0.len() < self.0.capacity() {
            return Ok((0, shifts));
        }
        let capacity = self.next_capacity()?;
        let bytes = Self::allocation_bytes(capacity)?;
        let work = self
            .0
            .len()
            .checked_add(shifts)
            .ok_or(RuntimeError::Overflow)?;
        Ok((bytes, work))
    }
    fn next_capacity(&self) -> Result<usize, RuntimeError> {
        self.0
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(4))
            .ok_or(RuntimeError::Overflow)
    }
    pub(super) fn insert(&mut self, key: Key, value: Value) -> Result<Option<Value>, RuntimeError> {
        match self.0.binary_search_by_key(&key, |(key, _)| *key) {
            Ok(at) => Ok(Some(core::mem::replace(&mut self.0[at].1, value))),
            Err(at) => {
                if self.0.len() == self.0.capacity() {
                    let capacity = self.next_capacity()?;
                    self.0
                        .try_reserve_exact(capacity - self.0.len())
                        .map_err(|_| RuntimeError::Overflow)?;
                }
                self.0.insert(at, (key, value));
                Ok(None)
            }
        }
    }
}
impl IntoIterator for SavedDestinations {
    type Item = (Key, Value);
    type IntoIter = alloc::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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
            let old_capacity = saved.0.capacity();
            let (allocation, work) = saved.insertion_demand(&key).unwrap();
            saved.insert(key, Value::Int(1)).unwrap();
            if saved.0.capacity() != old_capacity {
                assert_eq!(allocation, saved.0.capacity() * bytes);
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
