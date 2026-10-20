//! The bytes of heap that a prepared program holds, counted from its own tables (specification 34
//! MEM-12, MEM-14; checklist C1-21).
//!
//! A count adds, for each container, its capacity times its element's size, and then what its
//! elements hold in turn. An allocation that several values share (a long name's string, a shared
//! table) is counted once, where it is first met. The sizes are those of the device that counts, so
//! a 32-bit chip and a 64-bit computer each count their own bytes. A hash table is counted for the
//! widest control group that hashbrown uses, 16 bytes: exact on x86, and an upper bound on a target
//! with narrower groups.

use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::alloc::Layout;
use smol_str::SmolStr;

/// Counts the heap that values hold.
#[derive(Debug, Default)]
pub(crate) struct HeapCount {
    bytes: usize,
    /// The address of each shared allocation counted so far.
    shared: BTreeSet<usize>,
}

impl HeapCount {
    /// A count of nothing yet.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Take the bytes counted since the last take, keeping the shared allocations seen, so that a
    /// later part does not count them again.
    pub(crate) fn take(&mut self) -> usize {
        core::mem::take(&mut self.bytes)
    }

    /// Add `bytes`.
    pub(crate) fn add(&mut self, bytes: usize) {
        self.bytes = self.bytes.saturating_add(bytes);
    }

    /// Add what `value` holds.
    pub(crate) fn of<T: HeapBytes + ?Sized>(&mut self, value: &T) {
        value.add_heap(self);
    }

    /// Add the allocation of `capacity` elements of `T`.
    pub(crate) fn array<T>(&mut self, capacity: usize) {
        self.add(capacity.saturating_mul(core::mem::size_of::<T>()));
    }

    /// Whether the shared allocation at `address` is met for the first time.
    fn first(&mut self, address: usize) -> bool {
        self.shared.insert(address)
    }
}

/// A value whose heap a [`HeapCount`] counts.
pub(crate) trait HeapBytes {
    /// Add to `count` the heap that `self` holds beyond its own bytes.
    fn add_heap(&self, count: &mut HeapCount);
}

/// The bytes of an `Arc`'s allocation for a value of `value`'s layout: the two counts, then the
/// value, padded to the allocation's alignment.
fn arc_bytes(value: Layout) -> usize {
    Layout::new::<[usize; 2]>()
        .extend(value)
        .map_or(usize::MAX, |(layout, _)| layout.pad_to_align().size())
}

/// The bytes of an `Arc<T>`'s allocation (C1-21).
pub(crate) fn arc_allocation_bytes<T>() -> usize {
    arc_bytes(Layout::new::<T>())
}

/// The bytes of an insertion-ordered map with room for `capacity` entries of `K` and `V`: its entries,
/// each a bucket of the hash, the key and the value, and its hash table of indexes (C1-21).
pub(crate) fn index_map_bytes<K, V>(capacity: usize) -> usize {
    if capacity == 0 {
        return 0;
    }
    capacity
        .saturating_mul(core::mem::size_of::<(usize, K, V)>())
        .saturating_add(hash_table_bytes::<usize>(capacity))
}

/// The bytes of a hashbrown table of `T` that has room for `capacity` items: its buckets, padded
/// to the control alignment, then one control byte per bucket and one group of them more.
fn hash_table_bytes<T>(capacity: usize) -> usize {
    const GROUP: usize = 16;
    let buckets = if capacity < 4 {
        4
    } else if capacity < 8 {
        8
    } else if capacity < 15 {
        16
    } else {
        (capacity.saturating_mul(8) / 7).next_power_of_two()
    };
    buckets
        .saturating_mul(core::mem::size_of::<T>())
        .next_multiple_of(core::mem::align_of::<T>().max(GROUP))
        .saturating_add(buckets)
        .saturating_add(GROUP)
}

/// Values that hold no heap.
macro_rules! holds_no_heap {
    ($($ty:ty),* $(,)?) => {
        $(impl $crate::memory::figure::HeapBytes for $ty {
            fn add_heap(&self, _: &mut $crate::memory::figure::HeapCount) {}
        })*
    };
}

holds_no_heap!(
    bool,
    u8,
    u16,
    u32,
    u64,
    usize,
    i8,
    i16,
    i32,
    i64,
    f32,
    f64,
    char,
    str,
    (),
);
pub(crate) use holds_no_heap;

impl<T> HeapBytes for core::ops::Range<T> {
    fn add_heap(&self, _: &mut HeapCount) {}
}

impl<T: HeapBytes> HeapBytes for [T] {
    fn add_heap(&self, count: &mut HeapCount) {
        for item in self {
            item.add_heap(count);
        }
    }
}

impl<T: HeapBytes, const N: usize> HeapBytes for [T; N] {
    fn add_heap(&self, count: &mut HeapCount) {
        self.as_slice().add_heap(count);
    }
}

impl<T: HeapBytes> HeapBytes for Vec<T> {
    fn add_heap(&self, count: &mut HeapCount) {
        count.array::<T>(self.capacity());
        self.as_slice().add_heap(count);
    }
}

impl<T: HeapBytes> HeapBytes for Option<T> {
    fn add_heap(&self, count: &mut HeapCount) {
        if let Some(value) = self {
            value.add_heap(count);
        }
    }
}

impl<T: HeapBytes + ?Sized> HeapBytes for Box<T> {
    fn add_heap(&self, count: &mut HeapCount) {
        count.add(core::mem::size_of_val::<T>(self));
        (**self).add_heap(count);
    }
}

impl<T: HeapBytes + ?Sized> HeapBytes for Arc<T> {
    fn add_heap(&self, count: &mut HeapCount) {
        if count.first(Arc::as_ptr(self).cast::<()>() as usize) {
            count.add(arc_bytes(Layout::for_value::<T>(self)));
            (**self).add_heap(count);
        }
    }
}

impl<T: HeapBytes + ?Sized + alloc::borrow::ToOwned> HeapBytes for alloc::borrow::Cow<'_, T>
where
    T::Owned: HeapBytes,
{
    fn add_heap(&self, count: &mut HeapCount) {
        if let alloc::borrow::Cow::Owned(value) = self {
            value.add_heap(count);
        }
    }
}

impl HeapBytes for String {
    fn add_heap(&self, count: &mut HeapCount) {
        count.add(self.capacity());
    }
}

impl HeapBytes for SmolStr {
    fn add_heap(&self, count: &mut HeapCount) {
        // A long string is one `Arc<str>`, which every copy of it shares.
        if self.is_heap_allocated() && count.first(self.as_ptr() as usize) {
            count.add(arc_bytes(
                Layout::array::<u8>(self.len()).unwrap_or(Layout::new::<()>()),
            ));
        }
    }
}

impl<A: HeapBytes, B: HeapBytes> HeapBytes for (A, B) {
    fn add_heap(&self, count: &mut HeapCount) {
        self.0.add_heap(count);
        self.1.add_heap(count);
    }
}

impl<A: HeapBytes, B: HeapBytes, C: HeapBytes> HeapBytes for (A, B, C) {
    fn add_heap(&self, count: &mut HeapCount) {
        self.0.add_heap(count);
        self.1.add_heap(count);
        self.2.add_heap(count);
    }
}

impl<A: HeapBytes, B: HeapBytes, C: HeapBytes, D: HeapBytes> HeapBytes for (A, B, C, D) {
    fn add_heap(&self, count: &mut HeapCount) {
        self.0.add_heap(count);
        self.1.add_heap(count);
        self.2.add_heap(count);
        self.3.add_heap(count);
    }
}

impl<K: HeapBytes, V: HeapBytes, S> HeapBytes for indexmap::IndexMap<K, V, S> {
    fn add_heap(&self, count: &mut HeapCount) {
        count.add(index_map_bytes::<K, V>(self.capacity()));
        for (key, value) in self {
            key.add_heap(count);
            value.add_heap(count);
        }
    }
}

#[cfg(feature = "std")]
impl<K: HeapBytes, V: HeapBytes, S> HeapBytes for std::collections::HashMap<K, V, S> {
    fn add_heap(&self, count: &mut HeapCount) {
        if self.capacity() == 0 {
            return;
        }
        count.add(hash_table_bytes::<(K, V)>(self.capacity()));
        for (key, value) in self {
            key.add_heap(count);
            value.add_heap(count);
        }
    }
}

impl<K: HeapBytes, V: HeapBytes> HeapBytes for crate::collections::SortedMap<K, V> {
    fn add_heap(&self, count: &mut HeapCount) {
        count.array::<(K, V)>(self.capacity());
        self.entries().add_heap(count);
    }
}

impl<K: HeapBytes> HeapBytes for crate::collections::SortedSet<K> {
    fn add_heap(&self, count: &mut HeapCount) {
        count.array::<K>(self.capacity());
        self.members().add_heap(count);
    }
}
