//! Compact live identities: binary search over sorted pairs, never ID-sized holes.
use crate::error::RuntimeError;
use alloc::vec::Vec;

#[derive(Debug)]
pub struct LiveEntries<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> Default for LiveEntries<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
impl<K: Clone, V: Clone> Clone for LiveEntries<K, V> {
    fn clone(&self) -> Self {
        // A snapshot must not silently discard a pre-reserved activation capacity.
        let mut entries = Vec::with_capacity(self.entries.capacity());
        entries.extend(self.entries.iter().cloned());
        Self { entries }
    }
}
impl<K: Ord, V> LiveEntries<K, V> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    #[cfg(any(test, not(feature = "std")))]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok()
            .map(|index| &self.entries[index].1)
    }
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let index = self
            .entries
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok()?;
        Some(&mut self.entries[index].1)
    }
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, value)| value)
    }
    #[cfg(any(test, not(feature = "std")))]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(pair)
    }
    pub(super) fn reserve_capacity(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        if capacity > self.entries.capacity() {
            self.entries
                .try_reserve_exact(capacity - self.entries.len())
                .map_err(|_| RuntimeError::Overflow)?;
        }
        Ok(())
    }
    pub(super) fn growth_capacity(&self) -> Option<usize> {
        if self.entries.len() < self.entries.capacity() {
            Some(self.entries.capacity())
        } else {
            self.entries.capacity().checked_mul(2).map(|n| n.max(4))
        }
    }
    pub(super) fn try_append(&mut self, key: K, value: V) -> Result<(), RuntimeError> {
        if self
            .entries
            .last()
            .is_some_and(|(previous, _)| previous >= &key)
        {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.reserve_capacity(self.growth_capacity().ok_or(RuntimeError::Overflow)?)?;
        self.entries.push((key, value));
        Ok(())
    }
    pub(super) fn remove(&mut self, key: &K) -> Option<V> {
        let index = self
            .entries
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok()?;
        Some(self.entries.remove(index).1)
    }
    #[cfg(any(test, not(feature = "std")))]
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) {
        self.entries.retain(|(key, value)| keep(key, value));
    }
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}
#[cfg(any(test, not(feature = "std")))]
fn pair<K, V>((key, value): &(K, V)) -> (&K, &V) {
    (key, value)
}
#[cfg(any(test, not(feature = "std")))]
impl<'a, K, V> IntoIterator for &'a LiveEntries<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter =
        core::iter::Map<core::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (&'a K, &'a V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(pair::<K, V>)
    }
}
