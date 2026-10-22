//! The decoded value of one leaf, and the flat block that holds one region's variables
//! (specification 34 MEM-12; design "Regions" and "Operands and references").
//!
//! A [`FlatBlock`](crate::memory::cells::FlatBlock) owns one region: it reserves the region's bytes
//! once from the prepared layouts and never grows. Every variable lies in it at its declared size
//! and alignment, as its [`TypeLayout`](crate::memory::layout::TypeLayout) places it, read and
//! written through `memory::flat`. A [`Cell`](crate::memory::cells::Cell) is the
//! decoded value of one scalar, enumeration, instance or reference leaf, as an operand is: the block
//! never holds one. [`write_value`](crate::memory::cells::write_value) and
//! [`read_value`](crate::memory::cells::read_value) convert a `Value` of a prepared type into its
//! layout's bytes and back.

mod convert;

use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::memory::layout::{ByteRange, LayoutRegion, CELL_BYTES};
use crate::memory::regions::PersistentExtent;
use crate::memory::InstanceId;
use crate::value::ValueRef;
use crate::vm::{PreparedModule, VmTrap};

pub use convert::{byte_offset, read_value, write_value};
pub(crate) use convert::{
    cell_slot, check_value, enumeration_number, enumeration_value, flat_cell, flat_reference,
    is_reference_type, largest_text_edit, leaf_in, max_encoded_len, read_scalar, read_scalar_cell,
    read_scalar_leaf, read_value_in, scalar_slot, structure_member_name, text_geometry,
    write_enum_in, write_scalar_cell_in, write_scalar_in, write_value_in, CellLeaf,
};

/// The decoded value of one leaf of a flat block (specification 34 MEM-12): a scalar, an
/// enumeration with its type, an instance identity or a reference leaf's state. [`Cell::Null`] is a
/// leaf that names no instance and an unbound native counter slot, and [`Cell::NullRef`] a reference
/// leaf that holds no reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cell {
    /// `BOOL`.
    Bool(bool),
    /// `SINT`.
    SInt(i8),
    /// `INT`.
    Int(i16),
    /// `DINT`.
    DInt(i32),
    /// `LINT`.
    LInt(i64),
    /// `USINT`.
    USInt(u8),
    /// `UINT`.
    UInt(u16),
    /// `UDINT`.
    UDInt(u32),
    /// `ULINT`.
    ULInt(u64),
    /// `REAL`.
    Real(f32),
    /// `LREAL`.
    LReal(f64),
    /// `BYTE`.
    Byte(u8),
    /// `WORD`.
    Word(u16),
    /// `DWORD`.
    DWord(u32),
    /// `LWORD`.
    LWord(u64),
    /// `CHAR`.
    Char(u8),
    /// `WCHAR`.
    WChar(u16),
    /// `TIME`, in nanoseconds.
    Time(i64),
    /// `LTIME`, in nanoseconds.
    LTime(i64),
    /// `DATE`, in ticks since the epoch.
    Date(i64),
    /// `LDATE`, in nanoseconds since the epoch.
    LDate(i64),
    /// `TIME_OF_DAY`, in ticks since midnight.
    Tod(i64),
    /// `LTIME_OF_DAY`, in nanoseconds since midnight.
    LTod(i64),
    /// `DATE_AND_TIME`, in ticks since the epoch.
    Dt(i64),
    /// `LDATE_AND_TIME`, in nanoseconds since the epoch.
    Ldt(i64),
    /// An enumeration, with its TYPE_TABLE type and its value.
    Enum {
        /// TYPE_TABLE ordinal of the enumeration type.
        type_idx: u32,
        /// The enumeration's value.
        value: i64,
    },
    /// An FB, class or interface instance.
    Instance(InstanceId),
    /// A reference leaf that holds a reference; its region's reference table holds the reference
    /// under the leaf's byte offset.
    Ref(RefSlot),
    /// A reference leaf that holds a flat reference (specification 34 MEM-12; C1-11d): the block and
    /// the byte offset of its target, its installation generation, and the element count of the
    /// path it was taken through, which its copies are charged by (TIME-08). Its target type is the
    /// leaf's declared one.
    Flat {
        /// The block of the target.
        region: LayoutRegion,
        /// The element count of the path the reference was taken through.
        elements: u16,
        /// The target's byte offset in its block.
        offset: u32,
        /// The installation generation the reference belongs to.
        generation: crate::memory::Generation,
    },
    /// An empty cell.
    Null,
    /// A reference cell that holds no reference.
    NullRef,
}

/// A decoded leaf is exactly [`CELL_BYTES`] on every target: the copy charges of TIME-08 count a
/// value's elements in that unit.
const _: () = assert!(core::mem::size_of::<Cell>() == CELL_BYTES);

/// The byte offset, in its region's block, of a reference leaf that holds a reference: the key of
/// its `ValueRef` in the region's reference table until references are flat (C1-10, O9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefSlot(pub u32);

/// The reference leaves of one region and the `ValueRef` each non-null leaf holds (specification 34
/// §6.2; O9, R3, R5). Every reference leaf's byte offset is known at construction, so the table is
/// reserved once, in ascending offset order, holding no reference. A write replaces an entry and
/// never adds one, so a reference write in a scan allocates nothing.
///
/// The activation stack's table takes the reference leaves of a frame, or of an instance a frame
/// owns, when it takes its bytes, above every leaf the table holds, and drops them when it leaves
/// the stack, keeping the references they held until the engine takes their path buffers back
/// (C1-19c4). It is reserved once for the most leaves the stack holds.
#[derive(Debug, Default)]
pub(crate) struct ReferenceTable {
    leaves: Vec<(u32, Option<ValueRef>)>,
    /// The reference the last write replaced, until the engine takes its path buffers back
    /// (spec 34 ARCH-05). It is held in place, so keeping it allocates nothing.
    displaced: Option<ValueRef>,
    /// The references that leaves leaving the activation stack held, until the engine takes their
    /// path buffers back, with room for as many as the table has leaves.
    released: Vec<ValueRef>,
}

impl ReferenceTable {
    /// The bytes a table of `leaves` entries reserves; construction charges them with the block.
    pub(crate) fn reservation_bytes(leaves: usize) -> Option<usize> {
        leaves.checked_mul(core::mem::size_of::<(u32, Option<ValueRef>)>())
    }

    /// Reserve one entry, holding no reference, for each leaf offset of `offsets`. The offsets must
    /// be distinct: a repeated offset is refused, because a reference leaf owns its bytes.
    pub(crate) fn reserve(mut offsets: Vec<u32>) -> Result<Self, RuntimeError> {
        offsets.sort_unstable();
        if offsets.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(RuntimeError::TypeMismatch);
        }
        let mut leaves = Vec::new();
        leaves
            .try_reserve_exact(offsets.len())
            .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
        leaves.extend(offsets.into_iter().map(|offset| (offset, None)));
        Ok(Self {
            leaves,
            displaced: None,
            released: Vec::new(),
        })
    }

    /// The reference the leaf at byte `offset` holds, or `None` for a null leaf and for an offset
    /// that is no reference leaf.
    pub(crate) fn get(&self, offset: u32) -> Option<&ValueRef> {
        self.leaves[self.index_of(offset)?].1.as_ref()
    }

    /// Store `reference` (`None` for a null reference) in the leaf at byte `offset`. Answers `false`
    /// when `offset` is no reference leaf of this region. Nothing is allocated: the reference the
    /// leaf held is kept as the displaced one, whose buffers the engine takes back.
    pub(crate) fn set(&mut self, offset: u32, reference: Option<ValueRef>) -> bool {
        match self.index_of(offset) {
            Some(index) => {
                let previous = core::mem::replace(&mut self.leaves[index].1, reference);
                if previous.is_some() {
                    self.displaced = previous;
                }
                true
            }
            None => false,
        }
    }

    /// The reference the last write replaced, if the engine has not taken it yet.
    pub(crate) fn take_displaced(&mut self) -> Option<ValueRef> {
        self.displaced.take()
    }

    /// The entry of the leaf at byte `offset`, found by binary search over the reserved offsets.
    fn index_of(&self, offset: u32) -> Option<usize> {
        self.leaves
            .binary_search_by_key(&offset, |(leaf, _)| *leaf)
            .ok()
    }

    /// The reference leaves the table holds an entry for.
    pub(crate) fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// An empty table with room for `leaves` leaves, which frames and the instances they own add
    /// as they take their bytes on the activation stack, and for the references those leaves hold
    /// when they leave it (C1-19c4).
    pub(crate) fn with_room(leaves: usize) -> Result<Self, RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        let mut table = Self::default();
        table.leaves.try_reserve_exact(leaves).map_err(refused)?;
        table.released.try_reserve_exact(leaves).map_err(refused)?;
        Ok(table)
    }

    /// The bytes [`ReferenceTable::with_room`] takes.
    pub(crate) fn room_bytes(leaves: usize) -> Option<usize> {
        leaves.checked_mul(
            core::mem::size_of::<(u32, Option<ValueRef>)>() + core::mem::size_of::<ValueRef>(),
        )
    }

    /// Add an entry, holding no reference, for each leaf at the byte offsets `offsets`, in
    /// ascending order and above every leaf the table holds. `false`, and nothing added, when one
    /// is not above the leaves before it or the table has no room left for them.
    pub(crate) fn push_leaves(&mut self, offsets: impl Iterator<Item = u32> + Clone) -> bool {
        let mut last = self.leaves.last().map(|(offset, _)| *offset);
        let mut count = 0usize;
        for offset in offsets.clone() {
            if last.is_some_and(|last| offset <= last) {
                return false;
            }
            last = Some(offset);
            count += 1;
        }
        if self.leaves.capacity() - self.leaves.len() < count {
            return false;
        }
        self.leaves.extend(offsets.map(|offset| (offset, None)));
        true
    }

    /// Drop the entries of the leaves at byte `start` and above, keeping each reference they held
    /// for [`ReferenceTable::drain_released`].
    pub(crate) fn release_from(&mut self, start: u32) {
        let at = self.leaves.partition_point(|(offset, _)| *offset < start);
        for (_, reference) in self.leaves.drain(at..) {
            // The room reserved for the released references is that of every leaf.
            if let Some(reference) = reference
                && self.released.len() < self.released.capacity()
            {
                self.released.push(reference);
            }
        }
    }

    /// The references that released leaves held, for the engine to take their path buffers back.
    pub(crate) fn drain_released(&mut self) -> impl Iterator<Item = ValueRef> + '_ {
        self.released.drain(..)
    }

    /// The reserved capacity, which the store never grows (test support).
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.leaves.capacity()
    }
}

/// One region's flat block (specification 34 MEM-12; design "Regions").
///
/// The block reserves its bytes once, zeroed, and never grows, and it belongs to one
/// [`LayoutRegion`]: every accessor answers `None` outside the reserved region and outside its
/// bytes, and allocates nothing.
#[derive(Debug)]
pub struct FlatBlock {
    region: LayoutRegion,
    bytes: Vec<u8>,
}

impl Default for FlatBlock {
    /// The empty persistent block.
    fn default() -> Self {
        Self {
            region: LayoutRegion::Persistent,
            bytes: Vec::new(),
        }
    }
}

impl FlatBlock {
    /// Reserve a persistent block of `bytes` bytes, all zero (specification 34 MEM-12).
    ///
    /// [`FlatBlock::reserve_in`] reserves a block of another [`LayoutRegion`]. The bytes are
    /// reserved exactly once. A refused reservation is refused as the other construction
    /// reservations are.
    pub fn reserve(bytes: u32) -> Result<Self, RuntimeError> {
        Self::reserve_in(LayoutRegion::Persistent, bytes)
    }

    /// Reserve a block of `region` of `bytes` bytes, all zero (specification 34 MEM-12; design
    /// "Regions").
    ///
    /// The block holds `region`: [`FlatBlock::bytes`] and [`FlatBlock::bytes_mut`] answer `None`
    /// for a range of another region, because an offset means a byte of the region its block
    /// reserved.
    pub fn reserve_in(region: LayoutRegion, bytes: u32) -> Result<Self, RuntimeError> {
        let count = usize::try_from(bytes).map_err(|_| RuntimeError::Overflow)?;
        let mut block = Self {
            region,
            bytes: Vec::new(),
        };
        block
            .bytes
            .try_reserve_exact(count)
            .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
        block.bytes.resize(count, 0);
        Ok(block)
    }

    /// Reserve the persistent block of `prepared`: its globals, and each persistent root's own
    /// variables (specification 34 §6.2, MEM-12; O6, O8). The regions read the same extent
    /// (`memory/regions.rs`).
    ///
    /// # Errors
    ///
    /// Refused as [`FlatBlock::reserve`] is, or when the prepared layouts and declarations disagree.
    pub fn persistent(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        Self::reserve(PersistentExtent::of(prepared)?.bytes)
    }

    /// The bytes this block has room for. [`FlatBlock::reserve`] fixes them and no later call
    /// changes them: the block never grows (specification 34 §6.2, the paragraph after MEM-10).
    #[must_use]
    pub fn reserved(&self) -> usize {
        self.bytes.capacity()
    }

    /// The region this block is reserved in (design "Regions").
    #[must_use]
    pub fn region(&self) -> LayoutRegion {
        self.region
    }

    /// The bytes of `range`, or `None` when `range` leaves this block.
    #[must_use]
    #[inline]
    pub fn bytes(&self, range: ByteRange) -> Option<&[u8]> {
        if range.region != self.region {
            return None;
        }
        self.bytes.get(byte_span(range)?)
    }

    /// The bytes of `range`, or `None` when `range` leaves this block.
    #[must_use]
    #[inline]
    pub fn bytes_mut(&mut self, range: ByteRange) -> Option<&mut [u8]> {
        if range.region != self.region {
            return None;
        }
        self.bytes.get_mut(byte_span(range)?)
    }
}

impl FlatBlock {
    /// Copy the bytes of `source` over those of `destination`, which may overlap them, as one
    /// assignment of a compound copies its value (specification 34 MEM-12; C1-10c). `false`, and
    /// nothing copied, when either range leaves this block or their lengths differ.
    #[must_use]
    pub fn copy_within(&mut self, source: ByteRange, destination: ByteRange) -> bool {
        if source.region != self.region
            || destination.region != self.region
            || source.bytes != destination.bytes
        {
            return false;
        }
        let (Some(from), Some(to)) = (byte_span(source), byte_span(destination)) else {
            return false;
        };
        if from.end > self.bytes.len() || to.end > self.bytes.len() {
            return false;
        }
        self.bytes.copy_within(from, to.start);
        true
    }
}

/// The half-open byte indexes of `range`, or `None` when the range cannot be expressed.
#[inline]
fn byte_span(range: ByteRange) -> Option<core::ops::Range<usize>> {
    let start = usize::try_from(range.start).ok()?;
    let end = start.checked_add(usize::try_from(range.bytes).ok()?)?;
    Some(start..end)
}

#[cfg(test)]
mod reference_table_tests {
    use super::ReferenceTable;
    use crate::memory::{Generation, MemoryLocation};
    use crate::value::{RefSegment, ValueRef};

    fn reference(offset: u32) -> ValueRef {
        ValueRef {
            location: MemoryLocation::Global,
            offset,
            generation: Generation(1),
            path: alloc::vec![RefSegment::Index(alloc::vec![1])],
        }
    }

    /// Spec 34 §6.3 "Activation stack" (C1-19c4): the activation stack's table takes a frame's
    /// reference leaves above the leaves it holds, refuses leaves below them or beyond its room, and
    /// on a release drops the leaves at and above the released bytes, keeping the references they
    /// held for the engine to take back. A frame at byte 0 with leaves 8 and 24 and an instance
    /// above it at byte 64 with leaf 64 fill a table with room for 3; a leaf at 40, below 64, and a
    /// fourth leaf are refused; releasing from byte 64 drops the instance's leaf and gives back its
    /// reference, the frame's stay and keep theirs. Catches a table that grows, one that keeps
    /// leaves out of order, a release that drops a frame's leaves with its instance's and one that
    /// drops the references instead of keeping them.
    #[test]
    fn the_activation_table_takes_and_releases_leaves_from_the_top() {
        let mut table = ReferenceTable::with_room(3).unwrap();
        assert!(table.push_leaves([8, 24].into_iter()));
        assert!(table.push_leaves([64].into_iter()));
        assert!(!table.push_leaves([40].into_iter()));
        assert!(!table.push_leaves([72].into_iter()));
        assert_eq!(table.leaf_count(), 3);
        assert!(table.set(24, Some(reference(1))));
        assert!(table.set(64, Some(reference(2))));
        table.release_from(64);
        assert_eq!(table.leaf_count(), 2);
        assert_eq!(table.get(24).map(|held| held.offset), Some(1));
        assert!(!table.set(64, None));
        let released: alloc::vec::Vec<_> = table.drain_released().map(|held| held.offset).collect();
        assert_eq!(released, [2]);
        assert!(table.push_leaves([64].into_iter()));
    }
}
