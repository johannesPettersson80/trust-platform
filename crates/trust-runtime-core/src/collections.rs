//! Ordered storage shared by hosted and allocator-backed portable profiles.

/// Insertion-ordered storage retaining the existing hosted public map type.
#[cfg(feature = "std")]
pub type OrderedMap<K, V> = indexmap::IndexMap<K, V>;

/// Insertion-ordered storage with an explicit hasher available without `std`.
#[cfg(not(feature = "std"))]
pub type OrderedMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;

/// Keyed lookup storage preserving the hosted public hash-map type.
#[cfg(feature = "std")]
pub type LookupMap<K, V> = rustc_hash::FxHashMap<K, V>;

/// Allocator-backed deterministic lookup without a platform hash-map dependency.
#[cfg(not(feature = "std"))]
pub type LookupMap<K, V> = alloc::collections::BTreeMap<K, V>;

/// A map held as one vector of entries sorted by key and looked up by binary search
/// (specification 34 MEM-14): one array of fixed-size entries, the same on every target, so that
/// a prepared table is laid out as it is everywhere and the memory figure counts its bytes exactly
/// (MEM-12). It replaces the hash maps of hosted builds and the trees of portable ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortedMap<K, V> {
    entries: alloc::vec::Vec<(K, V)>,
}

impl<K, V> Default for SortedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> SortedMap<K, V> {
    /// The empty map, which holds no heap.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: alloc::vec::Vec::new(),
        }
    }

    /// An empty map with room for exactly `count` entries.
    ///
    /// # Errors
    ///
    /// The allocator's refusal of the room.
    pub fn try_with_capacity(count: usize) -> Result<Self, alloc::collections::TryReserveError> {
        let mut entries = alloc::vec::Vec::new();
        entries.try_reserve_exact(count)?;
        Ok(Self { entries })
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries the map has room for without growing.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// The entries in ascending key order.
    #[must_use]
    pub fn entries(&self) -> &[(K, V)] {
        &self.entries
    }

    /// The keys in ascending order.
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &K> + DoubleEndedIterator {
        self.entries.iter().map(|(key, _)| key)
    }

    /// The values in ascending key order.
    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> + DoubleEndedIterator {
        self.entries.iter().map(|(_, value)| value)
    }

    /// The values in ascending key order, to change in place.
    pub fn values_mut(&mut self) -> impl ExactSizeIterator<Item = &mut V> + DoubleEndedIterator {
        self.entries.iter_mut().map(|(_, value)| value)
    }

    /// The key and value pairs in ascending key order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&K, &V)> + DoubleEndedIterator {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    /// Remove every entry, keeping the room.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Give back the room beyond the entries.
    pub fn shrink_to_fit(&mut self) {
        self.entries.shrink_to_fit();
    }
}

impl<K: Ord, V> SortedMap<K, V> {
    fn position(&self, key: &K) -> Result<usize, usize> {
        self.entries.binary_search_by(|(entry, _)| entry.cmp(key))
    }

    /// The value of `key`.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.position(key).ok().map(|at| &self.entries[at].1)
    }

    /// The value of `key`, to change in place.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.position(key).ok().map(|at| &mut self.entries[at].1)
    }

    /// Whether the map holds `key`.
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.position(key).is_ok()
    }

    /// Set `key` to `value`, returning the value it replaced. The map grows when it has no room.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.position(&key) {
            Ok(at) => Some(core::mem::replace(&mut self.entries[at].1, value)),
            Err(at) => {
                self.entries.insert(at, (key, value));
                None
            }
        }
    }

    /// Set `key` to `value` within the room the map has, returning the value it replaced; a new
    /// key that finds no room is refused and changes nothing, so a table reserved from a validated
    /// count never grows (MEM-09).
    ///
    /// # Errors
    ///
    /// The key and value back, when the key is new and the map is full.
    pub fn insert_within_capacity(&mut self, key: K, value: V) -> Result<Option<V>, (K, V)> {
        match self.position(&key) {
            Ok(at) => Ok(Some(core::mem::replace(&mut self.entries[at].1, value))),
            Err(_) if self.entries.len() == self.entries.capacity() => Err((key, value)),
            Err(at) => {
                self.entries.insert(at, (key, value));
                Ok(None)
            }
        }
    }

    /// Remove `key`, returning its value.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.position(key).ok().map(|at| self.entries.remove(at).1)
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for SortedMap<K, V> {
    /// The map of `entries`; of two entries with one key, the later one stays, as a hash map's
    /// collection keeps it.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(entries: I) -> Self {
        let mut map = Self::new();
        for (key, value) in entries {
            map.insert(key, value);
        }
        map
    }
}

impl<'a, K, V> IntoIterator for &'a SortedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter =
        core::iter::Map<core::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (&'a K, &'a V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(key, value)| (key, value))
    }
}

impl<K: Ord, V> Extend<(K, V)> for SortedMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, entries: I) {
        for (key, value) in entries {
            self.insert(key, value);
        }
    }
}

/// A set held as one sorted vector of its members (specification 34 MEM-14), as [`SortedMap`]
/// holds its entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortedSet<K> {
    members: alloc::vec::Vec<K>,
}

impl<K> Default for SortedSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> SortedSet<K> {
    /// The empty set, which holds no heap.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            members: alloc::vec::Vec::new(),
        }
    }

    /// An empty set with room for exactly `count` members.
    ///
    /// # Errors
    ///
    /// The allocator's refusal of the room.
    pub fn try_with_capacity(count: usize) -> Result<Self, alloc::collections::TryReserveError> {
        let mut members = alloc::vec::Vec::new();
        members.try_reserve_exact(count)?;
        Ok(Self { members })
    }

    /// The number of members.
    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// The members the set has room for without growing.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.members.capacity()
    }

    /// Whether the set has no members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// The members in ascending order.
    #[must_use]
    pub fn members(&self) -> &[K] {
        &self.members
    }

    /// The members in ascending order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &K> + DoubleEndedIterator {
        self.members.iter()
    }

    /// Give back the room beyond the members.
    pub fn shrink_to_fit(&mut self) {
        self.members.shrink_to_fit();
    }
}

impl<K: Ord> SortedSet<K> {
    /// Whether the set holds `key`.
    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.members.binary_search(key).is_ok()
    }

    /// Add `key`; whether it was new.
    pub fn insert(&mut self, key: K) -> bool {
        match self.members.binary_search(&key) {
            Ok(_) => false,
            Err(at) => {
                self.members.insert(at, key);
                true
            }
        }
    }

    /// Remove `key`; whether the set held it.
    pub fn remove(&mut self, key: &K) -> bool {
        match self.members.binary_search(key) {
            Ok(at) => {
                self.members.remove(at);
                true
            }
            Err(_) => false,
        }
    }
}
