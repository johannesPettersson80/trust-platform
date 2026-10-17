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
//! store at run time yet; C1-6b converts `Value`s into cells and back, and C1-9 switches the engine
//! onto them.

use alloc::vec::Vec;

use crate::bytecode::{StorageOwner, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::memory::layout::{CellRange, LayoutRegion, CELL_BYTES};
use crate::memory::{Generation, InstanceId};
use crate::vm::{PreparedModule, VmTrap};

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
/// The store reserves both once and never grows: every accessor answers `None` outside the
/// reserved region and allocates nothing.
#[derive(Debug, Default)]
pub struct CellStore {
    cells: Vec<Cell>,
    references: Vec<RefRecord>,
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

    /// Reserve a region of `cells` cells and `references` reference records, all [`Cell::Null`] and
    /// [`RefRecord::NULL`] (specification 34 §6.2, MEM-10).
    ///
    /// Both vectors are reserved exactly once. A refused reservation is refused as the other
    /// construction reservations are.
    pub fn reserve(cells: u32, references: u32) -> Result<Self, RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        let cell_count = usize::try_from(cells).map_err(|_| RuntimeError::Overflow)?;
        let record_count = usize::try_from(references).map_err(|_| RuntimeError::Overflow)?;
        let mut store = Self::default();
        store.cells.try_reserve_exact(cell_count).map_err(refused)?;
        store
            .references
            .try_reserve_exact(record_count)
            .map_err(refused)?;
        store.cells.resize(cell_count, Cell::Null);
        store.references.resize(record_count, RefRecord::NULL);
        Ok(store)
    }

    /// Reserve the persistent region of `prepared`: its globals' and its instance templates'
    /// cells, and one reference record per reference leaf they hold (specification 34 §6.2, the
    /// paragraph after MEM-10).
    ///
    /// The region's cells and reference leaves are counted from the prepared layouts, never by
    /// hand: a template's references include its parent chain's, whose cells it places first.
    pub fn persistent(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        let mut cells = 0u32;
        let mut references = 0u32;
        for declaration in prepared.storage_declarations() {
            if declaration.owner != StorageOwner::Global || !declaration.owns_storage() {
                continue;
            }
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            let layout = prepared
                .type_layout(type_id)
                .ok_or(RuntimeError::TypeMismatch)?;
            cells = cells
                .checked_add(layout.cells)
                .ok_or(RuntimeError::Overflow)?;
            references = references
                .checked_add(leaf_count(layout.reference_leaves.len())?)
                .ok_or(RuntimeError::Overflow)?;
        }
        for (pou, _) in prepared.pous() {
            let Some(template) = prepared.instance_template_cells(pou) else {
                continue;
            };
            cells = cells
                .checked_add(template.range.cells)
                .ok_or(RuntimeError::Overflow)?;
            references = references
                .checked_add(template_reference_leaves(prepared, pou, 0)?)
                .ok_or(RuntimeError::Overflow)?;
        }
        Self::reserve(cells, references)
    }

    /// The cells and the reference records this region has room for.
    ///
    /// [`CellStore::reserve`] fixes both and no later call changes them: the store never grows
    /// (specification 34 §6.2, the paragraph after MEM-10).
    #[must_use]
    pub fn reserved(&self) -> (usize, usize) {
        (self.cells.capacity(), self.references.capacity())
    }

    /// The cells of `range`, or `None` when `range` leaves this region.
    #[must_use]
    pub fn cells(&self, range: CellRange) -> Option<&[Cell]> {
        self.cells.get(cell_span(range)?)
    }

    /// The cells of `range`, or `None` when `range` leaves this region.
    #[must_use]
    pub fn cells_mut(&mut self, range: CellRange) -> Option<&mut [Cell]> {
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

/// The reference leaves of one POU's instance template: its own declarations' leaves and then its
/// parent chain's, matching the parent cells a template places first (design "Prepared layouts").
///
/// The chain is bounded like the prepared template totals are, so a cyclic parent chain is refused
/// rather than followed.
fn template_reference_leaves(
    prepared: &PreparedModule,
    pou: u32,
    depth: usize,
) -> Result<u32, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let mut leaves = 0u32;
    for declaration in prepared.instance_declarations(pou) {
        if !declaration.owns_storage() {
            continue;
        }
        let Some(type_id) = declaration.type_id else {
            continue;
        };
        let layout = prepared
            .type_layout(type_id)
            .ok_or(RuntimeError::TypeMismatch)?;
        leaves = leaves
            .checked_add(leaf_count(layout.reference_leaves.len())?)
            .ok_or(RuntimeError::Overflow)?;
    }
    if let Some(parent) = prepared.parent_pou(pou) {
        leaves = leaves
            .checked_add(template_reference_leaves(prepared, parent, depth + 1)?)
            .ok_or(RuntimeError::Overflow)?;
    }
    Ok(leaves)
}

/// One leaf count as a `u32`.
fn leaf_count(leaves: usize) -> Result<u32, RuntimeError> {
    u32::try_from(leaves).map_err(|_| RuntimeError::Overflow)
}
