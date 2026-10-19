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
    check_value, is_reference_type, leaf_in, read_value_in, structure_member_name, text_geometry,
    write_value_in, CellLeaf,
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
#[derive(Debug, Default)]
pub(crate) struct ReferenceTable {
    leaves: Vec<(u32, Option<ValueRef>)>,
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
        Ok(Self { leaves })
    }

    /// The reference the leaf at byte `offset` holds, or `None` for a null leaf and for an offset
    /// that is no reference leaf.
    pub(crate) fn get(&self, offset: u32) -> Option<&ValueRef> {
        self.leaves[self.index_of(offset)?].1.as_ref()
    }

    /// Store `reference` (`None` for a null reference) in the leaf at byte `offset`. Answers `false`
    /// when `offset` is no reference leaf of this region. Nothing is allocated.
    pub(crate) fn set(&mut self, offset: u32, reference: Option<ValueRef>) -> bool {
        match self.index_of(offset) {
            Some(index) => {
                self.leaves[index].1 = reference;
                true
            }
            None => false,
        }
    }

    /// The entry of the leaf at byte `offset`, found by binary search over the reserved offsets.
    fn index_of(&self, offset: u32) -> Option<usize> {
        self.leaves
            .binary_search_by_key(&offset, |(leaf, _)| *leaf)
            .ok()
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
    pub fn bytes(&self, range: ByteRange) -> Option<&[u8]> {
        if range.region != self.region {
            return None;
        }
        self.bytes.get(byte_span(range)?)
    }

    /// The bytes of `range`, or `None` when `range` leaves this block.
    #[must_use]
    pub fn bytes_mut(&mut self, range: ByteRange) -> Option<&mut [u8]> {
        if range.region != self.region {
            return None;
        }
        self.bytes.get_mut(byte_span(range)?)
    }
}

/// The half-open byte indexes of `range`, or `None` when the range cannot be expressed.
fn byte_span(range: ByteRange) -> Option<core::ops::Range<usize>> {
    let start = usize::try_from(range.start).ok()?;
    let end = start.checked_add(usize::try_from(range.bytes).ok()?)?;
    Some(start..end)
}
