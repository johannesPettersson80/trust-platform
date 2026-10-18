//! The charges of cell values (specification 34 §6.2, TIME-08; C1-9c, R5). A copy of a value in cells
//! charges the work and the allocation that a copy of the same `Value` charges, computed from the
//! cells, so the cell arm and the `Value` arm agree for one program. The engine calls these from
//! C1-9d; until then only the tests compute them.

#![allow(
    dead_code,
    reason = "C1-9d charges the cell views in the engine; until then only the tests compute them"
)]

use super::{
    is_instance, member_name, primitive_of, resolve_entry, CellReader, CellsView, Target,
    STRING_PRIMITIVE, WSTRING_PRIMITIVE,
};
use crate::bytecode::{TypeData, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::memory::cells::Cell;
use crate::memory::layout::{CellRange, CELL_BYTES};
use crate::memory::regions::CellRegions;
use crate::memory::{InstanceId, MemoryLocation};
use crate::value::{RefSegment, Value};
use crate::vm::PreparedModule;

impl CellsView<'_> {
    /// The charge of one copy of the value of `type_id` at `range`, computed from its cells (TIME-08;
    /// C1-9c, R5). It follows the rules that charge a `Value` copy: a structure or an array charges one
    /// unit for itself and one for each member or element; a WSTRING and an enumeration one unit; a
    /// reference its path; a STRING its UTF-8 bytes and a WSTRING two bytes per UTF-16 unit. A structure
    /// or array member, and an array element, is a leaf of one cell unless it is a structure or an array
    /// that holds cells. A range of Null cells is the Null value and copies nothing.
    pub(crate) fn copy_charge(
        self,
        range: CellRange,
        type_id: u32,
        depth: usize,
    ) -> Result<CopyCharge, RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let cells = self
            .regions
            .persistent()
            .cells(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if cells.iter().all(|cell| *cell == Cell::Null) {
            return Ok(CopyCharge::default());
        }
        let (resolved, entry) =
            resolve_entry(self.prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
        match &entry.data {
            TypeData::Struct { fields } if entry.kind != TypeKind::OverlappingStruct => {
                let layout = self
                    .prepared
                    .type_layout(resolved)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let mut charge = CopyCharge {
                    work: 1 + fields.len(),
                    allocation: 0,
                };
                for (member, field) in layout.members.iter().zip(fields) {
                    let child = self.child_range(range, member.offset, field.type_id)?;
                    charge = charge.add(self.child_charge(child, field.type_id, depth)?)?;
                }
                Ok(charge)
            }
            TypeData::Array { elem_type_id, .. } => {
                let layout = self
                    .prepared
                    .type_layout(resolved)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let array = layout
                    .array
                    .as_ref()
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let count =
                    element_count(&array.dims).ok_or(RuntimeError::InvalidExecutionState)?;
                let mut charge = CopyCharge {
                    work: 1 + count,
                    allocation: 0,
                };
                for index in 0..count {
                    let offset = u32::try_from(index)
                        .ok()
                        .and_then(|index| index.checked_mul(array.stride))
                        .ok_or(RuntimeError::Overflow)?;
                    let child = self.child_range(range, offset, *elem_type_id)?;
                    charge = charge.add(self.child_charge(child, *elem_type_id, depth)?)?;
                }
                Ok(charge)
            }
            TypeData::Primitive { .. } | TypeData::Subrange { .. } => {
                match primitive_of(self.prepared, type_id) {
                    Some(prim_id @ (STRING_PRIMITIVE | WSTRING_PRIMITIVE)) => {
                        self.text_charge(cells, prim_id)
                    }
                    _ => Ok(CopyCharge::default()),
                }
            }
            TypeData::Enum { .. } => Ok(CopyCharge {
                work: 1,
                allocation: 0,
            }),
            TypeData::Reference { .. } => match self.reference_leaf(range, type_id)? {
                Some(reference) => {
                    let elements = reference_path_elements(&reference.path);
                    Ok(CopyCharge {
                        work: 1 + elements,
                        allocation: elements * CELL_BYTES,
                    })
                }
                None => Ok(CopyCharge::default()),
            },
            // An instance is one cell and copies nothing, as the `Value` instance does.
            TypeData::Pou { .. } | TypeData::Interface { .. } => Ok(CopyCharge::default()),
            // A `STRUCT OVERLAP` or a union shares its cells, which C1-15 owns.
            TypeData::Struct { .. } | TypeData::Union { .. } => {
                Err(RuntimeError::InvalidExecutionState)
            }
            TypeData::Alias { .. } => Err(RuntimeError::InvalidExecutionState),
        }
    }

    /// The charge a parent copy takes for one child: the child's own copy charge, and one cell of
    /// allocation when the child is a leaf of the parent (`vm::engine::allocation`).
    fn child_charge(
        self,
        range: CellRange,
        type_id: u32,
        depth: usize,
    ) -> Result<CopyCharge, RuntimeError> {
        let charge = self.copy_charge(range, type_id, depth + 1)?;
        let cells = self
            .regions
            .persistent()
            .cells(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let compound = is_compound(self.prepared, type_id);
        if compound && cells.iter().any(|cell| *cell != Cell::Null) {
            return Ok(charge);
        }
        charge.add(CopyCharge {
            work: 0,
            allocation: CELL_BYTES,
        })
    }

    /// The cell range of a member or element at `offset` of a value at `range`, of `type_id`.
    fn child_range(
        self,
        range: CellRange,
        offset: u32,
        type_id: u32,
    ) -> Result<CellRange, RuntimeError> {
        let cells = self
            .prepared
            .type_layout(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .cells;
        let start = range
            .start
            .checked_add(offset)
            .ok_or(RuntimeError::Overflow)?;
        Ok(CellRange {
            region: range.region,
            start,
            cells,
        })
    }

    /// The charge of a text leaf's copy: its bytes, and for a WSTRING one unit (TIME-08).
    fn text_charge(self, cells: &[Cell], prim_id: u16) -> Result<CopyCharge, RuntimeError> {
        let Some(Cell::Text(slot)) = cells.first() else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let allocation = self
            .regions
            .text()
            .length(*slot)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(CopyCharge {
            work: usize::from(prim_id == WSTRING_PRIMITIVE),
            allocation,
        })
    }
}

/// The work and the allocation of one copy, as `vm::engine::allocation` charges a copy of the `Value`
/// it materializes (TIME-08; C1-9c, R5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CopyCharge {
    /// The copy's work units.
    pub(crate) work: usize,
    /// The copy's allocation bytes.
    pub(crate) allocation: usize,
}

impl CopyCharge {
    /// The sum of two charges, refused on overflow.
    fn add(self, other: Self) -> Result<Self, RuntimeError> {
        Ok(Self {
            work: self
                .work
                .checked_add(other.work)
                .ok_or(RuntimeError::Overflow)?,
            allocation: self
                .allocation
                .checked_add(other.allocation)
                .ok_or(RuntimeError::Overflow)?,
        })
    }
}

/// Whether `type_id` is a structure or an array, which a copy takes as a compound (not as one leaf).
fn is_compound(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve_entry(prepared, type_id).is_some_and(|(_, entry)| match &entry.data {
        TypeData::Array { .. } => true,
        TypeData::Struct { .. } => entry.kind != TypeKind::OverlappingStruct,
        _ => false,
    })
}

/// The number of elements a reference path copies: its segments and its subscripts (TIME-08).
fn reference_path_elements(path: &[RefSegment]) -> usize {
    path.iter()
        .fold(path.len(), |count, segment| match segment {
            RefSegment::Index(indices) => count + indices.len(),
            RefSegment::Field(_) => count,
        })
}

/// The number of elements an array's bounds hold, or `None` when they hold none.
fn element_count(dims: &[(i64, i64)]) -> Option<usize> {
    dims.iter().try_fold(1usize, |count, (lower, upper)| {
        let length = upper.checked_sub(*lower)?.checked_add(1)?;
        count.checked_mul(usize::try_from(length).ok()?)
    })
}

/// The UTF-8 length of the text leaf at `range`: its bytes for a `STRING`, and the UTF-8 length of its
/// UTF-16 units for a `WSTRING` (TIME-08 counts UTF-8). `None` when the range holds no text.
fn text_utf8_length(regions: &CellRegions, range: CellRange, prim_id: u16) -> Option<usize> {
    let Some(Cell::Text(slot)) = regions.persistent().cells(range)?.first() else {
        return None;
    };
    let bytes = regions.text().bytes(*slot)?;
    Some(match prim_id {
        STRING_PRIMITIVE => bytes.len(),
        _ => utf8_length_of_utf16(bytes),
    })
}

/// The UTF-8 length of the text stored as little-endian UTF-16 code units, without allocating.
fn utf8_length_of_utf16(bytes: &[u8]) -> usize {
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_le_bytes(*unit));
    char::decode_utf16(units)
        .map(|decoded| decoded.map_or(3, char::len_utf8))
        .sum()
}

impl CellReader<'_> {
    /// The charge of a write to one character of a text leaf on `path` (TIME-08; C1-9c, R5). The first
    /// text leaf the path indexes charges what `charge_string_path_write` charges for its UTF-8 length.
    /// `None` when the path indexes no text leaf of the cells.
    pub(crate) fn text_element_charge(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<Option<CopyCharge>, RuntimeError> {
        for (depth, segment) in path.iter().enumerate() {
            if !matches!(segment, RefSegment::Index(_)) {
                continue;
            }
            let target = self
                .locate(location, offset, &path[..depth])
                .map_err(|_| RuntimeError::NullReference)?;
            let Target::Cells(range, type_id) = target else {
                continue;
            };
            let Some(prim_id @ (STRING_PRIMITIVE | WSTRING_PRIMITIVE)) =
                primitive_of(self.prepared, type_id)
            else {
                continue;
            };
            let length = text_utf8_length(self.arm.cells.regions(), range, prim_id)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            // `charge_string_path_write`: the text's length plus four units, three work units and one
            // cell of allocation each.
            let units = length.checked_add(4).ok_or(RuntimeError::Overflow)?;
            return Ok(Some(CopyCharge {
                work: units.checked_mul(3).ok_or(RuntimeError::Overflow)?,
                allocation: units
                    .checked_mul(CELL_BYTES)
                    .ok_or(RuntimeError::Overflow)?,
            }));
        }
        Ok(None)
    }
}

/// The facts of a cell range that the engine's walks read without a `Value` (specification 34 §6.2,
/// MEM-10; C1-9d1). Each answers what the `Value` form answers in [`crate::memory::view::ValueView`].
impl<'a> CellsView<'a> {
    /// The scalar of the cells at `range`, a value of `type_id`: a primitive other than a text, copied
    /// without allocation. `None` for any other type.
    pub(crate) fn scalar(
        self,
        range: CellRange,
        type_id: u32,
    ) -> Result<Option<Value>, RuntimeError> {
        match primitive_of(self.prepared, type_id) {
            None | Some(STRING_PRIMITIVE | WSTRING_PRIMITIVE) => Ok(None),
            Some(_) => self.materialize(range, type_id).map(Some),
        }
    }

    /// The instance the cells at `range` hold: the instance cell of a function block or an interface.
    pub(crate) fn instance(
        self,
        range: CellRange,
        type_id: u32,
    ) -> Result<Option<InstanceId>, RuntimeError> {
        if !is_instance(self.prepared, type_id) {
            return Ok(None);
        }
        let cells = self
            .regions
            .persistent()
            .cells(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(match cells.first() {
            Some(Cell::Instance(instance)) => Some(*instance),
            _ => None,
        })
    }

    /// The UTF-8 length of the text leaf at `range`: its bytes for a `STRING`, the UTF-8 length of its
    /// code units for a `WSTRING` (TIME-08). `None` for any other type.
    pub(crate) fn text_utf8_len(
        self,
        range: CellRange,
        type_id: u32,
    ) -> Result<Option<usize>, RuntimeError> {
        let Some(prim_id @ (STRING_PRIMITIVE | WSTRING_PRIMITIVE)) =
            primitive_of(self.prepared, type_id)
        else {
            return Ok(None);
        };
        text_utf8_length(self.regions, range, prim_id)
            .map(Some)
            .ok_or(RuntimeError::InvalidExecutionState)
    }

    /// Whether `type_id` is a text: `Some(false)` for a `STRING`, `Some(true)` for a `WSTRING` and `None`
    /// for any other type (C1-9d1).
    pub(crate) fn text_is_wide(self, type_id: u32) -> Option<bool> {
        match primitive_of(self.prepared, type_id)? {
            STRING_PRIMITIVE => Some(false),
            WSTRING_PRIMITIVE => Some(true),
            _ => None,
        }
    }

    /// Whether `type_id` is a structure, which owns one cell range per member (the layout of the
    /// `Value` structure). An overlapping structure is refused by the cell charge (C1-15).
    pub(crate) fn is_struct(self, type_id: u32) -> bool {
        resolve_entry(self.prepared, type_id)
            .is_some_and(|(_, entry)| matches!(entry.data, TypeData::Struct { .. }))
    }

    /// Whether `type_id` is a structure that declares a member named `name`, as the `Value` structure
    /// does by its field names.
    pub(crate) fn has_field(self, type_id: u32, name: &str) -> bool {
        let Some((resolved, entry)) = resolve_entry(self.prepared, type_id) else {
            return false;
        };
        if !matches!(entry.data, TypeData::Struct { .. }) {
            return false;
        }
        self.prepared.type_layout(resolved).is_some_and(|layout| {
            layout
                .members
                .iter()
                .any(|member| member_name(self.prepared, member.name_idx) == Some(name))
        })
    }

    /// The declared bounds of each dimension of the array type `type_id`, `None` for any other type.
    pub(crate) fn array_dimensions(self, type_id: u32) -> Option<&'a [(i64, i64)]> {
        let (_, entry) = resolve_entry(self.prepared, type_id)?;
        match &entry.data {
            TypeData::Array { dims, .. } => Some(dims.as_slice()),
            _ => None,
        }
    }

    /// Whether `type_id` is a function block, whose cell holds an instance that a walk owns (C1-9d2a,
    /// item (e)). An interface cell holds an instance that it does not own.
    pub(crate) fn is_pou(self, type_id: u32) -> bool {
        resolve_entry(self.prepared, type_id)
            .is_some_and(|(_, entry)| matches!(entry.data, TypeData::Pou { .. }))
    }

    /// The number of members of the structure, or of elements of the array, of `type_id` (C1-9d2a,
    /// item (e)): the children a layout walk visits, as a `Value` tree's fields and elements are
    /// visited. `None` for any other type. A `STRUCT OVERLAP` or a union has no walk until C1-15, so
    /// it is refused.
    pub(crate) fn child_count(self, type_id: u32) -> Result<Option<usize>, RuntimeError> {
        let (resolved, entry) =
            resolve_entry(self.prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
        match &entry.data {
            TypeData::Struct { fields } if entry.kind != TypeKind::OverlappingStruct => {
                Ok(Some(fields.len()))
            }
            TypeData::Array { .. } => {
                let layout = self
                    .prepared
                    .type_layout(resolved)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let array = layout
                    .array
                    .as_ref()
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let count =
                    element_count(&array.dims).ok_or(RuntimeError::InvalidExecutionState)?;
                Ok(Some(count))
            }
            TypeData::Struct { .. } | TypeData::Union { .. } => {
                Err(RuntimeError::InvalidExecutionState)
            }
            _ => Ok(None),
        }
    }

    /// The range and type of member `ordinal`, in declaration order, of the structure of `type_id` at
    /// `range`, or of element `ordinal`, in flat order, of the array (C1-9d2a, item (e)). The children
    /// of [`CellsView::child_count`].
    pub(crate) fn child(
        self,
        range: CellRange,
        type_id: u32,
        ordinal: usize,
    ) -> Result<(CellRange, u32), RuntimeError> {
        let (resolved, entry) =
            resolve_entry(self.prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
        let layout = self
            .prepared
            .type_layout(resolved)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let (offset, child_type) = match &entry.data {
            TypeData::Struct { fields } if entry.kind != TypeKind::OverlappingStruct => {
                let member = layout
                    .members
                    .get(ordinal)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let field = fields
                    .get(ordinal)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                (member.offset, field.type_id)
            }
            TypeData::Array { elem_type_id, .. } => {
                let array = layout
                    .array
                    .as_ref()
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let offset = u32::try_from(ordinal)
                    .ok()
                    .and_then(|index| index.checked_mul(array.stride))
                    .ok_or(RuntimeError::Overflow)?;
                (offset, *elem_type_id)
            }
            _ => return Err(RuntimeError::InvalidExecutionState),
        };
        Ok((self.child_range(range, offset, child_type)?, child_type))
    }
}
