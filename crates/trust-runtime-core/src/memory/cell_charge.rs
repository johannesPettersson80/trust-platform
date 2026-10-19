//! The charges of values in the flat block (specification 34 §6.2, TIME-08; C1-9c, R5). A copy of
//! a value in the block charges the work and the allocation that a copy of the same `Value`
//! charges, computed from its bytes, so the cell arm and the `Value` arm agree for one program. The engine calls these from
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
use crate::memory::cells::{text_geometry, Cell};
use crate::memory::flat::{self, LeafKind, TextBytes};
use crate::memory::layout::{ByteRange, CELL_BYTES};
use crate::memory::regions::CellRegions;
use crate::memory::{InstanceId, MemoryLocation};
use crate::value::{string_element_position_in, RefSegment, Value};
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
        range: ByteRange,
        type_id: u32,
        depth: usize,
    ) -> Result<CopyCharge, RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
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
                    let child = self.child_range(range, member.byte_offset, field.type_id)?;
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
                        .and_then(|index| index.checked_mul(array.byte_stride))
                        .ok_or(RuntimeError::Overflow)?;
                    let child = self.child_range(range, offset, *elem_type_id)?;
                    charge = charge.add(self.child_charge(child, *elem_type_id, depth)?)?;
                }
                Ok(charge)
            }
            TypeData::Primitive { .. } | TypeData::Subrange { .. } => {
                match primitive_of(self.prepared, type_id) {
                    Some(prim_id @ (STRING_PRIMITIVE | WSTRING_PRIMITIVE)) => {
                        self.text_charge(range, type_id, prim_id)
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
                    // A flat reference is charged by the path it was taken through (C1-11d).
                    let elements = reference.location.flat_elements()
                        + reference_path_elements(reference.path);
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
        range: ByteRange,
        type_id: u32,
        depth: usize,
    ) -> Result<CopyCharge, RuntimeError> {
        let charge = self.copy_charge(range, type_id, depth + 1)?;
        if is_compound(self.prepared, type_id) {
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
        range: ByteRange,
        offset: u32,
        type_id: u32,
    ) -> Result<ByteRange, RuntimeError> {
        let bytes = self
            .prepared
            .type_layout(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .shape
            .bytes;
        let start = range
            .start
            .checked_add(offset)
            .ok_or(RuntimeError::Overflow)?;
        Ok(ByteRange {
            region: range.region,
            start,
            bytes,
        })
    }

    /// The charge of a text leaf's copy: its bytes, and for a WSTRING one unit (TIME-08).
    fn text_charge(
        self,
        range: ByteRange,
        type_id: u32,
        prim_id: u16,
    ) -> Result<CopyCharge, RuntimeError> {
        let allocation = text_bytes(self.prepared, self.regions, range, type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .len();
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
    crate::value::path_elements(path)
}

/// The number of elements an array's bounds hold, or `None` when they hold none.
fn element_count(dims: &[(i64, i64)]) -> Option<usize> {
    dims.iter().try_fold(1usize, |count, (lower, upper)| {
        let length = upper.checked_sub(*lower)?.checked_add(1)?;
        count.checked_mul(usize::try_from(length).ok()?)
    })
}

/// The bytes in use of the text leaf at `range`, or `None` when the range holds no text.
fn text_bytes<'a>(
    prepared: &PreparedModule,
    regions: &'a CellRegions,
    range: ByteRange,
    type_id: u32,
) -> Option<&'a [u8]> {
    let (length, _, wide) = text_geometry(prepared, type_id)?;
    match flat::read_text(regions.block(range.region).bytes(range)?, length, wide).ok()? {
        TextBytes::Narrow(text) => Some(text.as_bytes()),
        TextBytes::Wide(units) => Some(units),
    }
}

fn text_utf8_length(
    prepared: &PreparedModule,
    regions: &CellRegions,
    range: ByteRange,
    type_id: u32,
    prim_id: u16,
) -> Option<usize> {
    let bytes = text_bytes(prepared, regions, range, type_id)?;
    Some(match prim_id {
        STRING_PRIMITIVE => bytes.len(),
        _ => utf8_length_of_utf16(bytes),
    })
}

fn wide_chars(bytes: &[u8]) -> impl Iterator<Item = Result<char, core::char::DecodeUtf16Error>> {
    char::decode_utf16(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_le_bytes(*unit)),
    )
}

/// The UTF-8 length of the text stored as little-endian UTF-16 code units, without allocating.
fn utf8_length_of_utf16(bytes: &[u8]) -> usize {
    wide_chars(bytes)
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
            let Target::Cells(range, type_id, presence) = target else {
                continue;
            };
            if !self.present(range.region, presence) {
                continue;
            }
            let Some(prim_id @ (STRING_PRIMITIVE | WSTRING_PRIMITIVE)) =
                primitive_of(self.prepared, type_id)
            else {
                continue;
            };
            let length = text_utf8_length(
                self.prepared,
                self.arm.cells.regions(),
                range,
                type_id,
                prim_id,
            )
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
    /// The characters of the text leaf at `range`, a value of `type_id`, in place: a `STRING`'s
    /// UTF-8 or a `WSTRING`'s UTF-16 code units (C1-12). `None` for another type.
    pub(crate) fn text(self, range: ByteRange, type_id: u32) -> Option<TextBytes<'a>> {
        let (length, _, wide) = text_geometry(self.prepared, type_id)?;
        flat::read_text(self.regions.block(range.region).bytes(range)?, length, wide).ok()
    }

    /// The scalar of the cells at `range`, a value of `type_id`: a primitive other than a text, copied
    /// without allocation. `None` for any other type.
    pub(crate) fn scalar(
        self,
        range: ByteRange,
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
        range: ByteRange,
        type_id: u32,
    ) -> Result<Option<InstanceId>, RuntimeError> {
        if !is_instance(self.prepared, type_id) {
            return Ok(None);
        }
        let bytes = self
            .regions
            .block(range.region)
            .bytes(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(match flat::decode(LeafKind::Instance, bytes)? {
            Cell::Instance(instance) => Some(instance),
            _ => None,
        })
    }

    /// The UTF-8 length of the text leaf at `range`: its bytes for a `STRING`, the UTF-8 length of its
    /// code units for a `WSTRING` (TIME-08). `None` for any other type.
    pub(crate) fn text_utf8_len(
        self,
        range: ByteRange,
        type_id: u32,
    ) -> Result<Option<usize>, RuntimeError> {
        let Some(prim_id @ (STRING_PRIMITIVE | WSTRING_PRIMITIVE)) =
            primitive_of(self.prepared, type_id)
        else {
            return Ok(None);
        };
        text_utf8_length(self.prepared, self.regions, range, type_id, prim_id)
            .map(Some)
            .ok_or(RuntimeError::InvalidExecutionState)
    }

    /// The number of characters (Unicode scalar values) of the text leaf at `range` (C1-9d3a): the count
    /// that `LEN` and a subscript use (DEV-017). A `STRING` counts the characters of its UTF-8 bytes and
    /// a `WSTRING` those of its UTF-16 code units, decoded in place without building a `String`. `None`
    /// for any other type.
    pub(crate) fn text_char_count(
        self,
        range: ByteRange,
        type_id: u32,
    ) -> Result<Option<usize>, RuntimeError> {
        let Some(prim_id @ (STRING_PRIMITIVE | WSTRING_PRIMITIVE)) =
            primitive_of(self.prepared, type_id)
        else {
            return Ok(None);
        };
        let bytes = text_bytes(self.prepared, self.regions, range, type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let count = if prim_id != WSTRING_PRIMITIVE {
            core::str::from_utf8(bytes)
                .ok()
                .map(|text| text.chars().count())
        } else if bytes.len().is_multiple_of(2) {
            wide_chars(bytes)
                .try_fold(0usize, |count, decoded| decoded.map(|_| count + 1))
                .ok()
        } else {
            None
        };
        count.map(Some).ok_or(RuntimeError::InvalidExecutionState)
    }

    /// The character that the one-based `index` selects in the `WSTRING` at `range` (C1-9d3a), as a
    /// `WCHAR`, read from its UTF-16 code units in place. `None` when `index` selects no character or the
    /// character lies outside the Basic Multilingual Plane, as the same read of a `Value` answers, and
    /// for any type but a `WSTRING`.
    pub(crate) fn wide_text_element(
        self,
        range: ByteRange,
        type_id: u32,
        index: i64,
    ) -> Result<Option<Value>, RuntimeError> {
        if primitive_of(self.prepared, type_id) != Some(WSTRING_PRIMITIVE) {
            return Ok(None);
        }
        let count = self
            .text_char_count(range, type_id)?
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let Ok(position) = string_element_position_in(count, index) else {
            return Ok(None);
        };
        let bytes = text_bytes(self.prepared, self.regions, range, type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let Some(Ok(character)) = wide_chars(bytes).nth(position) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        Ok(u16::try_from(u32::from(character)).ok().map(Value::WChar))
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

    /// Whether `type_id` is an array whose elements are primitives, enumerations or subranges, which no
    /// reference or instance can be reached through (C1-9d2b-1): the lifetime check of a `Value` array
    /// skips such an array's elements, and the cell form skips them too.
    pub(crate) fn array_of_primitives(self, type_id: u32) -> bool {
        let Some((_, entry)) = resolve_entry(self.prepared, type_id) else {
            return false;
        };
        let TypeData::Array { elem_type_id, .. } = &entry.data else {
            return false;
        };
        resolve_entry(self.prepared, *elem_type_id).is_some_and(|(_, element)| {
            matches!(
                element.data,
                TypeData::Primitive { .. } | TypeData::Subrange { .. } | TypeData::Enum { .. }
            )
        })
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
        range: ByteRange,
        type_id: u32,
        ordinal: usize,
    ) -> Result<(ByteRange, u32), RuntimeError> {
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
                (member.byte_offset, field.type_id)
            }
            TypeData::Array { elem_type_id, .. } => {
                let array = layout
                    .array
                    .as_ref()
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let offset = u32::try_from(ordinal)
                    .ok()
                    .and_then(|index| index.checked_mul(array.byte_stride))
                    .ok_or(RuntimeError::Overflow)?;
                (offset, *elem_type_id)
            }
            _ => return Err(RuntimeError::InvalidExecutionState),
        };
        Ok((self.child_range(range, offset, child_type)?, child_type))
    }
}
