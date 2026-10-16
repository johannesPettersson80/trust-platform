//! Sparse POU identities index payloads without specializing tree nodes per value.
//!
//! This representation is private to portable metadata. It never indexes a vector
//! by an artifact ID; only locally assigned dense ordinals address payloads.
use crate::error::RuntimeError;
use alloc::{collections::BTreeMap, vec::Vec};

/// Read-only portable POU lookup, with ascending artifact-ID iteration.
#[derive(Debug)]
pub struct PouMap<V> {
    index: BTreeMap<u32, usize>,
    values: Vec<V>,
    limit: usize,
}

impl<V> Default for PouMap<V> {
    fn default() -> Self {
        Self {
            index: BTreeMap::new(),
            values: Vec::new(),
            limit: 0,
        }
    }
}

impl<V> PouMap<V> {
    pub(super) fn with_capacity(count: usize) -> Result<Self, RuntimeError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        Ok(Self {
            index: BTreeMap::new(),
            values,
            limit: count,
        })
    }

    pub(super) fn try_insert(&mut self, id: u32, value: V) -> Result<Option<V>, RuntimeError> {
        if let Some(&at) = self.index.get(&id) {
            return Ok(Some(core::mem::replace(&mut self.values[at], value)));
        }
        // Materialization reserves from the validated record count. Refuse a
        // broken capacity plan rather than silently growing an uncharged vector.
        if self.values.len() >= self.limit {
            return Err(RuntimeError::PreparationLimit);
        }
        let at = self.values.len();
        self.values.push(value);
        self.index.insert(id, at);
        Ok(None)
    }

    #[cfg(test)]
    pub(crate) fn insert(&mut self, id: u32, value: V) -> Option<V> {
        if !self.index.contains_key(&id) && self.values.len() >= self.limit {
            self.values.reserve(1);
            self.limit = self.values.len() + 1;
        }
        self.try_insert(id, value).unwrap()
    }

    /// Number of admitted POU identities.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Whether there are no identities.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Lookup by a sparse artifact identity.
    pub fn get(&self, id: &u32) -> Option<&V> {
        self.index.get(id).map(|&at| &self.values[at])
    }

    /// Whether an admitted identity is present.
    #[cfg(test)]
    pub fn contains_key(&self, id: &u32) -> bool {
        self.index.contains_key(id)
    }

    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.index.clear();
        self.values.clear();
    }

    /// Ascending artifact identities, independent of insertion order.
    #[cfg(any(test, feature = "register-ir"))]
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &u32> {
        self.index.keys()
    }

    /// Payloads in ascending artifact identity order.
    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.index.values().map(|&at| &self.values[at])
    }

    /// Identity and payload pairs in ascending artifact identity order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&u32, &V)> {
        self.index.iter().map(|(id, &at)| (id, &self.values[at]))
    }

    #[cfg(test)]
    pub(crate) fn get_mut(&mut self, id: &u32) -> Option<&mut V> {
        let at = *self.index.get(id)?;
        self.values.get_mut(at)
    }

    // Production modules are immutable after preparation. These operations are
    // only for existing malformed-state/unit fixtures, never part of RUN.
    #[cfg(test)]
    pub(crate) fn remove(&mut self, id: &u32) -> Option<V> {
        let at = self.index.remove(id)?;
        let value = self.values.remove(at);
        for slot in self.index.values_mut() {
            if *slot > at {
                *slot -= 1;
            }
        }
        Some(value)
    }
}

impl<V: Clone> Clone for PouMap<V> {
    fn clone(&self) -> Self {
        // Preserve the logical reservation, even for a partially built map.
        // Allocator slack is never promoted into admitted payload capacity.
        let mut values = Vec::with_capacity(self.limit);
        values.extend_from_slice(&self.values);
        Self {
            index: self.index.clone(),
            values,
            limit: self.limit,
        }
    }
}

impl<V: PartialEq> PartialEq for PouMap<V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<V: Eq> Eq for PouMap<V> {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn reserved_payload_refuses_growth_but_allows_replacement() {
        let mut map = PouMap::with_capacity(1).unwrap();
        map.values.reserve(8); // Simulate allocator overcapacity.
        assert_eq!(map.try_insert(u32::MAX, 3), Ok(None));
        assert_eq!(map.try_insert(u32::MAX, 4), Ok(Some(3)));
        assert_eq!(map.try_insert(0, 9), Err(RuntimeError::PreparationLimit));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&u32::MAX), Some(&4));
        assert_eq!(map.get(&0), None);
    }

    #[test]
    fn clone_preserves_logical_reservation_without_allocating_on_insert() {
        let mut source = PouMap::with_capacity(2).unwrap();
        source.try_insert(19, 3).unwrap();
        let mut cloned = source.clone();
        let capacity = cloned.values.capacity();
        assert_eq!(cloned.try_insert(1, 7), Ok(None));
        assert_eq!(cloned.values.capacity(), capacity);
        assert_eq!(cloned.try_insert(2, 9), Err(RuntimeError::PreparationLimit));
        assert_eq!(source.get(&1), None);
    }

    #[test]
    fn sparse_identity_order_replacement_clone_and_removal_match_tree_semantics() {
        let mut actual = PouMap::with_capacity(4).unwrap();
        let mut expected = BTreeMap::new();
        for (id, value) in [(u32::MAX, 7), (3, 9), (40, 11), (3, 13), (0, 17)] {
            assert_eq!(actual.insert(id, value), expected.insert(id, value));
        }
        assert_eq!(
            actual.iter().collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        assert_eq!(actual.get(&9), None);
        assert_eq!(
            actual.keys().copied().collect::<Vec<_>>(),
            vec![0, 3, 40, u32::MAX]
        );
        let mut reversed = PouMap::with_capacity(4).unwrap();
        for (&id, &value) in expected.iter().rev() {
            reversed.insert(id, value);
        }
        assert_eq!(
            actual, reversed,
            "equality ignores payload ordinal assignment"
        );
        let saved = actual.clone();
        assert_eq!(actual.remove(&3), expected.remove(&3));
        *actual.get_mut(&40).unwrap() = 19;
        *expected.get_mut(&40).unwrap() = 19;
        assert_eq!(
            actual.iter().collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        assert_eq!(saved.get(&3), Some(&13));
        assert_eq!(saved.get(&40), Some(&11));
        actual.clear();
        assert!(actual.is_empty());
        assert_eq!(actual.values().count(), 0);
    }
}
