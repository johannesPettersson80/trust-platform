//! The cell, the unit of the bounded value store, and the store of one region's cells and
//! reference records (specification 34 §6.2, MEM-10; design "Cells", "Regions" and "Operands and
//! references").
//!
//! One cell is [`CELL_BYTES`](crate::memory::layout::CELL_BYTES) bytes, `Copy` and without
//! `Drop`: every scalar of the value model fits inline, a compound is a contiguous
//! [`CellRange`](crate::memory::layout::CellRange) of its
//! [`TypeLayout`](crate::memory::layout::TypeLayout) and a text or reference leaf is a slot into
//! its region's text buffers (C1-7) or its reference records. A
//! [`CellStore`](crate::memory::cells::CellStore) owns one region: it reserves the region's cells
//! and its reference records once from the prepared layouts and never grows. Nothing reads the
//! store at run time yet: [`write_value`](crate::memory::cells::write_value) and
//! [`read_value`](crate::memory::cells::read_value) convert a `Value` of a prepared type into its
//! layout's cells and back (C1-6b), and C1-9 switches the engine onto them.

mod convert;

use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::memory::layout::{CellRange, LayoutRegion, CELL_BYTES};
use crate::memory::regions::PersistentExtent;
use crate::memory::{Generation, InstanceId};
use crate::vm::{PreparedModule, VmTrap};

pub use convert::{cell_offset, read_value, write_value};

/// One cell of the bounded value store (specification 34 §6.2, MEM-10; design "Cells").
///
/// Every scalar of [`Value`](crate::value::Value) is one cell, and a structure or array is a
/// contiguous [`CellRange`] of its type's prepared layout with its nested compounds inline.
/// [`Cell::Text`] and [`Cell::Ref`] address the region's text buffers (C1-7) and its reference
/// records; [`Cell::Null`] is an empty cell and [`Cell::NullRef`] a reference cell that holds no
/// reference.
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
    /// A text, addressed in its region's text buffers (C1-7).
    Text(TextSlot),
    /// A reference, addressed in its region's reference records.
    Ref(RefSlot),
    /// An empty cell.
    Null,
    /// A reference cell that holds no reference.
    NullRef,
}

/// One cell is exactly [`CELL_BYTES`] on every target (specification 34 §6.2, MEM-10): a cell that
/// grew would double a region's RAM and break the prepared layouts.
const _: () = assert!(core::mem::size_of::<Cell>() == CELL_BYTES);

/// Index of a text in its region's text buffers (design "Regions"; C1-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextSlot(pub u32);

/// Index of a reference record in its region's reference records (design "Regions").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefSlot(pub u32);

/// What a reference leaf refers to: the design's flat `RefAddr` (design "Operands and references").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefRecord {
    /// The region the target cell is reserved in.
    pub region: LayoutRegion,
    /// The cell index of the target in `region`, or [`RefRecord::NO_CELL`] when the record names
    /// no target.
    pub cell: u32,
    /// The character index of a character reference, zero otherwise.
    pub char_index: u32,
    /// The referenced or aliased TYPE_TABLE type.
    pub target_type: u32,
    /// The installation generation the reference was taken in (specification 34 §11.3, MEM-07).
    pub generation: Generation,
}

impl RefRecord {
    /// The cell index of a record that names no target.
    ///
    /// A region can never hold this many cells: its cells alone would need `u32::MAX × 16` bytes,
    /// far beyond any admitted capacity.
    pub const NO_CELL: u32 = u32::MAX;

    /// The record [`CellStore::reserve`] fills every reference slot with: it names no target
    /// (`Value::Reference(None)`).
    pub const NULL: Self = Self {
        region: LayoutRegion::Persistent,
        cell: Self::NO_CELL,
        char_index: 0,
        target_type: 0,
        generation: Generation::FIRST,
    };

    /// Whether this record names no target.
    #[must_use]
    pub fn is_null(self) -> bool {
        self.cell == Self::NO_CELL
    }
}

/// One region's cells and reference records (design "Regions").
///
/// The store reserves both once and never grows, and it belongs to one [`LayoutRegion`]: every
/// accessor answers `None` outside the reserved region and outside that region's cells, and
/// allocates nothing.
#[derive(Debug)]
pub struct CellStore {
    region: LayoutRegion,
    cells: Vec<Cell>,
    references: Vec<RefRecord>,
}

impl Default for CellStore {
    /// The empty persistent region: no cells and no reference records.
    fn default() -> Self {
        Self {
            region: LayoutRegion::Persistent,
            cells: Vec::new(),
            references: Vec::new(),
        }
    }
}

impl CellStore {
    /// The bytes reserving `cells` cells and `references` reference records takes, or `None` when
    /// the product leaves `usize` (specification 34 §6.2; construction charges them before it
    /// reserves).
    #[must_use]
    pub fn reservation_bytes(cells: u32, references: u32) -> Option<usize> {
        usize::try_from(cells)
            .ok()?
            .checked_mul(CELL_BYTES)?
            .checked_add(
                usize::try_from(references)
                    .ok()?
                    .checked_mul(core::mem::size_of::<RefRecord>())?,
            )
    }

    /// Reserve a persistent region of `cells` cells and `references` reference records, all
    /// [`Cell::Null`] and [`RefRecord::NULL`] (specification 34 §6.2, MEM-10).
    ///
    /// [`CellStore::reserve_in`] reserves a region of another [`LayoutRegion`]. Both vectors are
    /// reserved exactly once. A refused reservation is refused as the other construction
    /// reservations are.
    pub fn reserve(cells: u32, references: u32) -> Result<Self, RuntimeError> {
        Self::reserve_in(LayoutRegion::Persistent, cells, references)
    }

    /// Reserve a region of `region` of `cells` cells and `references` reference records, all
    /// [`Cell::Null`] and [`RefRecord::NULL`] (specification 34 §6.2, MEM-10; design "Regions").
    ///
    /// The store holds `region`: [`CellStore::cells`] and [`CellStore::cells_mut`] answer `None`
    /// for a range of another region, because a cell index means a cell of the region its store
    /// reserved.
    pub fn reserve_in(
        region: LayoutRegion,
        cells: u32,
        references: u32,
    ) -> Result<Self, RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        let cell_count = usize::try_from(cells).map_err(|_| RuntimeError::Overflow)?;
        let record_count = usize::try_from(references).map_err(|_| RuntimeError::Overflow)?;
        let mut store = Self {
            region,
            ..Self::default()
        };
        store.cells.try_reserve_exact(cell_count).map_err(refused)?;
        store
            .references
            .try_reserve_exact(record_count)
            .map_err(refused)?;
        store.cells.resize(cell_count, Cell::Null);
        store.references.resize(record_count, RefRecord::NULL);
        Ok(store)
    }

    /// Reserve the persistent region of `prepared`: its globals' cells, and each persistent root's
    /// own cells and reference leaves (specification 34 §6.2; O6, O8). The cell store reads the same
    /// extent (`memory/regions.rs`).
    ///
    /// # Errors
    ///
    /// Refused as [`CellStore::reserve`] is, or when the prepared layouts and declarations disagree.
    pub fn persistent(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        let extent = PersistentExtent::of(prepared)?;
        Self::reserve(extent.cells, extent.references)
    }

    /// The cells and the reference records this region has room for.
    ///
    /// [`CellStore::reserve`] fixes both and no later call changes them: the store never grows
    /// (specification 34 §6.2, the paragraph after MEM-10).
    #[must_use]
    pub fn reserved(&self) -> (usize, usize) {
        (self.cells.capacity(), self.references.capacity())
    }

    /// The region this store's cells are reserved in (design "Regions").
    #[must_use]
    pub fn region(&self) -> LayoutRegion {
        self.region
    }

    /// The cells of `range`, or `None` when `range` leaves this region.
    #[must_use]
    pub fn cells(&self, range: CellRange) -> Option<&[Cell]> {
        if range.region != self.region {
            return None;
        }
        self.cells.get(cell_span(range)?)
    }

    /// The cells of `range`, or `None` when `range` leaves this region.
    #[must_use]
    pub fn cells_mut(&mut self, range: CellRange) -> Option<&mut [Cell]> {
        if range.region != self.region {
            return None;
        }
        self.cells.get_mut(cell_span(range)?)
    }

    /// The reference record at `slot`, or `None` when the slot leaves this region.
    #[must_use]
    pub fn record(&self, slot: RefSlot) -> Option<&RefRecord> {
        self.references.get(usize::try_from(slot.0).ok()?)
    }

    /// The reference record at `slot`, or `None` when the slot leaves this region.
    #[must_use]
    pub fn record_mut(&mut self, slot: RefSlot) -> Option<&mut RefRecord> {
        self.references.get_mut(usize::try_from(slot.0).ok()?)
    }
}

/// The half-open cell indexes of `range`, or `None` when the range cannot be expressed.
fn cell_span(range: CellRange) -> Option<core::ops::Range<usize>> {
    let start = usize::try_from(range.start).ok()?;
    let end = start.checked_add(usize::try_from(range.cells).ok()?)?;
    Some(start..end)
}
