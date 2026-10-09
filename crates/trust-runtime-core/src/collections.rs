//! Ordered storage shared by hosted and allocator-backed portable profiles.

/// Insertion-ordered storage retaining the existing hosted public map type.
#[cfg(feature = "std")]
pub type OrderedMap<K, V> = indexmap::IndexMap<K, V>;

/// Insertion-ordered storage with an explicit hasher available without `std`.
#[cfg(not(feature = "std"))]
pub type OrderedMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;
