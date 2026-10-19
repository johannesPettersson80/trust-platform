//! Prepared cell layouts for the bounded value store (specification 34 §6.2, MEM-10).
//!
//! A [`TypeLayout`](crate::memory::layout::TypeLayout) describes how one value of a TYPE_TABLE
//! type occupies cells. One cell is a
//! scalar (`Cell` from the design "Cells"), so a scalar is one cell and every structure or array
//! is a contiguous cell range with its nested compounds inline. A text leaf records its byte
//! capacity from D-C1-2, and a `STRUCT OVERLAP` records the length of its byte backing.
//!
//! Nothing reads these layouts at run time yet: the layout tables and their computation are
//! charged once at preparation, and later slices store values by them. A region's cells are only
//! recorded here; construction charges them when it reserves the region. The traversal is bounded
//! by
//! [`BYTECODE_MAX_TYPE_NESTING`](crate::bytecode::BYTECODE_MAX_TYPE_NESTING): a type deeper than
//! the bound is refused with
//! [`RuntimeError::TypeNestingExceeds`](crate::error::RuntimeError::TypeNestingExceeds) and a
//! recursive type with `TypeMismatch`.

use alloc::vec::Vec;

use crate::bytecode::{TypeData, TypeKind, TypeTable, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::value::string_capacity;
use crate::vm::type_policy::is_wildcard_dim;

/// Bytes in one cell (design "Cells").
pub const CELL_BYTES: usize = 16;

/// Bytes of a reference in a flat block on every target: its target's block and byte offset, the
/// target's installation generation and the character index of a character reference
/// (specification 34 MEM-12, MEM-07).
pub const REFERENCE_BYTES: u32 = 16;

/// Bytes of an instance leaf in a flat block: the identity of the function-block, class or interface
/// instance, whose own variables lie in a range of their own in the same block (MEM-12).
pub const INSTANCE_BYTES: u32 = 8;

/// Bytes of a native counter slot (primitive `0x0100`): an integer of the caller's width, with its
/// kind, since the slot keeps the caller's width and signedness.
const ANY_INT_SLOT_BYTES: u32 = 16;

/// The declared size and alignment of one value in a flat block (specification 34 MEM-12): the
/// bytes it occupies and the power of two its offset is a multiple of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteShape {
    /// Bytes in one value of the type, padding included.
    pub bytes: u32,
    /// The alignment of its offset: its size up to 8 bytes for a scalar, its largest member's for a
    /// compound.
    pub align: u32,
}

impl Default for ByteShape {
    /// An empty value: no bytes, aligned to one.
    fn default() -> Self {
        Self { bytes: 0, align: 1 }
    }
}

/// The length field of a `STRING[n]` or `WSTRING[n]` in a flat block: 2 bytes, or 4 when the
/// declared length exceeds 65,535 (MEM-12).
#[must_use]
pub const fn text_length_bytes(declared_length: u32) -> u32 {
    if declared_length > 65_535 {
        4
    } else {
        2
    }
}

/// The declared size and alignment of primitive `prim_id` (specification 12 §6.2) whose declared
/// text length is `length` (the `n` of `STRING[n]`; ignored for other primitives), as MEM-12 lays
/// it out.
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] for a primitive id the format does not define.
pub fn primitive_shape(prim_id: u16, length: u32) -> Result<ByteShape, RuntimeError> {
    let scalar = |bytes: u32| ByteShape {
        bytes,
        align: bytes,
    };
    Ok(match prim_id {
        // BOOL, BYTE, SINT, USINT, CHAR
        1 | 2 | 6 | 10 | 26 => scalar(1),
        // WORD, INT, UINT, WCHAR
        3 | 7 | 11 | 27 => scalar(2),
        // DWORD, DINT, UDINT, REAL
        4 | 8 | 12 | 14 => scalar(4),
        // LWORD, LINT, ULINT, LREAL and the eight temporal types (STBC 3.0's eight-byte storage)
        5 | 9 | 13 | 15..=23 => scalar(8),
        // STRING[n]: the length, then n bytes of UTF-8; WSTRING[n]: the length, then n UTF-16 units
        24 | 25 => {
            let field = text_length_bytes(length);
            let units = if prim_id == 25 { 2 } else { 1 };
            let text = length.checked_mul(units).ok_or(RuntimeError::Overflow)?;
            ByteShape {
                bytes: field.checked_add(text).ok_or(RuntimeError::Overflow)?,
                align: field,
            }
        }
        0x0100 => ByteShape {
            bytes: ANY_INT_SLOT_BYTES,
            align: 8,
        },
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

/// `offset` rounded up to a multiple of `align`, a power of two.
fn align_up(offset: u32, align: u32) -> Result<u32, RuntimeError> {
    let mask = align.checked_sub(1).ok_or(RuntimeError::TypeMismatch)?;
    offset
        .checked_add(mask)
        .map(|end| end & !mask)
        .ok_or(RuntimeError::Overflow)
}

/// The region a cell range is reserved in (design "Regions").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutRegion {
    /// Globals and persistent instance templates, reserved at construction.
    Persistent,
    /// Locals and initializer results, on the activation stack.
    Activation,
}

/// A contiguous run of cells in one region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRange {
    /// The region the cells are reserved in.
    pub region: LayoutRegion,
    /// First cell of the range within its region.
    pub start: u32,
    /// Number of cells in the range.
    pub cells: u32,
}

/// A POU's prepared instance template: its own cells only, with a link to its parent's template
/// (specification 34 MEM-10; O6). A derived POU without variables of its own has an empty
/// template with its parent link (O7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceTemplateLayout {
    /// The template's own cells, from cell 0.
    pub range: CellRange,
    /// Parent POU whose template holds the inherited cells.
    pub parent: Option<u32>,
}

/// One structure member in declaration (ordinal) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberLayout {
    /// Member name index in STRING_TABLE; a later `REF_FIELD` resolves through it.
    pub name_idx: u32,
    /// Cell offset of the member within its type's cell range.
    pub offset: u32,
    /// Byte offset of the member within its type's flat bytes (MEM-12).
    pub byte_offset: u32,
}

/// A text leaf and its byte capacity (D-C1-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLeaf {
    /// Cell offset of the leaf within its type's cell range.
    pub offset: u32,
    /// Bytes reserved for the text: `n` for `STRING[n]`, `2n` for `WSTRING[n]`, and 80 or 160
    /// bytes for a `STRING`/`WSTRING` declared without a length.
    pub capacity_bytes: u32,
    /// Byte offset of the leaf's length field within its type's flat bytes; the text follows it
    /// (MEM-12).
    pub byte_offset: u32,
}

/// A reference leaf: one cell holding a reference (`Cell::Ref`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceLeaf {
    /// Cell offset of the leaf within its type's cell range.
    pub offset: u32,
    /// Byte offset of the leaf's [`REFERENCE_BYTES`] within its type's flat bytes (MEM-12).
    pub byte_offset: u32,
    /// Referenced or aliased type index.
    pub target_type_id: u32,
}

/// An instance leaf: one cell holding an FB, class or interface instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceLeaf {
    /// Cell offset of the leaf within its type's cell range.
    pub offset: u32,
    /// Byte offset of the leaf's [`INSTANCE_BYTES`] within its type's flat bytes (MEM-12).
    pub byte_offset: u32,
    /// The instance's FB, class or interface type index.
    pub type_id: u32,
}

/// An array's element stride and dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLayout {
    /// Cells in one element, so element `k` starts at `k * stride`.
    pub stride: u32,
    /// Bytes in one element, so element `k` starts at byte `k * byte_stride` (MEM-12).
    pub byte_stride: u32,
    /// Inclusive lower and upper bounds in dimension order.
    pub dims: Vec<(i64, i64)>,
}

/// One type's prepared cell layout.
///
/// For an array, the text, reference and instance leaf offsets are within the first element and
/// [`ArrayLayout::stride`] locates later elements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeLayout {
    /// Cells in one value of this type. A scalar is one cell.
    pub cells: u32,
    /// The declared size and alignment of one value of this type in a flat block (MEM-12).
    pub shape: ByteShape,
    /// Structure members in ordinal order; empty for non-structures.
    pub members: Vec<MemberLayout>,
    /// Array element stride and dimensions, absent for non-arrays.
    pub array: Option<ArrayLayout>,
    /// Text leaves, each with its byte capacity (D-C1-2).
    pub text_leaves: Vec<TextLeaf>,
    /// Reference leaves (one cell each).
    pub reference_leaves: Vec<ReferenceLeaf>,
    /// Instance leaves (one cell each).
    pub instance_leaves: Vec<InstanceLeaf>,
    /// Length in bytes of a `STRUCT OVERLAP`'s byte backing, or zero.
    pub overlap_backing_bytes: u32,
    /// Whether the type can hold a reference or an instance, so a later lifetime check visits it.
    pub holds_reference_or_instance: bool,
}

/// Compute one [`TypeLayout`] per TYPE_TABLE entry, charging the layout tables it builds.
///
/// `overlap_backing` returns the admitted byte backing length of the overlapping structure at
/// `type_id` (the shared value layer already validates and sizes it). `charge` mirrors the
/// preparation budget: it is called with the bytes of each layout table and the work of computing
/// it, so an oversized type table is refused here with its limit error. The type's cells are not
/// charged; construction charges a region's cells when it reserves the region.
pub(crate) fn compute_type_layouts(
    types: &TypeTable,
    overlap_backing: &mut dyn FnMut(u32) -> Result<u32, RuntimeError>,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<TypeLayout>, RuntimeError> {
    let mut layouts: Vec<Option<TypeLayout>> = Vec::new();
    layouts
        .try_reserve_exact(types.entries.len())
        .map_err(|_| RuntimeError::PreparationLimit)?;
    layouts.resize_with(types.entries.len(), || None);
    let mut active = Vec::new();
    active.resize(types.entries.len(), false);
    for index in 0..types.entries.len() {
        let id = u32::try_from(index).map_err(|_| RuntimeError::Overflow)?;
        compute_one(
            types,
            id,
            &mut layouts,
            &mut active,
            0,
            overlap_backing,
            charge,
        )?;
    }
    Ok(layouts.into_iter().map(Option::unwrap_or_default).collect())
}

/// Compute and cache the layout of one type, returning its cell count.
fn compute_one(
    types: &TypeTable,
    id: u32,
    layouts: &mut [Option<TypeLayout>],
    active: &mut [bool],
    depth: usize,
    overlap_backing: &mut dyn FnMut(u32) -> Result<u32, RuntimeError>,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<u32, RuntimeError> {
    charge(0, 1)?;
    if let Some(cells) = layouts
        .get(id as usize)
        .and_then(Option::as_ref)
        .map(|layout| layout.cells)
    {
        return Ok(cells);
    }
    if depth >= BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeNestingExceeds);
    }
    if *active.get(id as usize).ok_or(RuntimeError::TypeMismatch)? {
        return Err(RuntimeError::TypeMismatch);
    }
    *active
        .get_mut(id as usize)
        .ok_or(RuntimeError::TypeMismatch)? = true;
    let (cells, layout) = build_layout(types, id, layouts, active, depth, overlap_backing, charge)?;
    *active
        .get_mut(id as usize)
        .ok_or(RuntimeError::TypeMismatch)? = false;
    charge(layout_table_bytes(&layout)?, 1)?;
    layouts
        .get_mut(id as usize)
        .ok_or(RuntimeError::TypeMismatch)?
        .replace(layout);
    Ok(cells)
}

/// Build the layout of one already-entered type.
fn build_layout(
    types: &TypeTable,
    id: u32,
    layouts: &mut [Option<TypeLayout>],
    active: &mut [bool],
    depth: usize,
    overlap_backing: &mut dyn FnMut(u32) -> Result<u32, RuntimeError>,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<(u32, TypeLayout), RuntimeError> {
    let entry = types
        .entries
        .get(id as usize)
        .ok_or(RuntimeError::TypeMismatch)?;
    let mut layout = TypeLayout::default();
    let cells = match &entry.data {
        TypeData::Primitive {
            prim_id,
            max_length,
        } => {
            let length = u32::from(string_capacity(*max_length));
            match *prim_id {
                24 => layout.text_leaves.push(TextLeaf {
                    offset: 0,
                    capacity_bytes: length,
                    byte_offset: 0,
                }),
                25 => layout.text_leaves.push(TextLeaf {
                    offset: 0,
                    capacity_bytes: length * 2,
                    byte_offset: 0,
                }),
                _ => {}
            }
            layout.shape = primitive_shape(*prim_id, length)?;
            1
        }
        // An enumeration or a subrange has its base type's declared size (MEM-12).
        TypeData::Enum { base_type_id, .. } | TypeData::Subrange { base_type_id, .. } => {
            let base = compute_child(
                types,
                *base_type_id,
                layouts,
                active,
                depth,
                overlap_backing,
                charge,
            )?;
            layout.shape = base.shape;
            1
        }
        TypeData::Alias { target_type_id } => {
            let child = compute_child(
                types,
                *target_type_id,
                layouts,
                active,
                depth,
                overlap_backing,
                charge,
            )?;
            layout = child;
            layout.cells
        }
        TypeData::Array { elem_type_id, dims } => {
            let child = compute_child(
                types,
                *elem_type_id,
                layouts,
                active,
                depth,
                overlap_backing,
                charge,
            )?;
            array_from(&mut layout, child, dims)?
        }
        TypeData::Struct { fields } => {
            if entry.kind == TypeKind::OverlappingStruct {
                layout.overlap_backing_bytes = overlap_backing(id)?;
                for field in fields {
                    layout.members.push(MemberLayout {
                        name_idx: field.name_idx,
                        offset: 0,
                        byte_offset: 0,
                    });
                }
                // The members lie over the backing bytes, laid out as specification 02 defines.
                layout.shape = ByteShape {
                    bytes: layout.overlap_backing_bytes,
                    align: 8,
                };
                1
            } else {
                let mut cursor = MemberCursor::default();
                for field in fields {
                    let child = compute_child(
                        types,
                        field.type_id,
                        layouts,
                        active,
                        depth,
                        overlap_backing,
                        charge,
                    )?;
                    cursor.place(&mut layout, &child, field.name_idx)?;
                }
                cursor.finish(&mut layout)?
            }
        }
        TypeData::Union { fields } => {
            let mut cells = 0u32;
            let mut shape = ByteShape::default();
            for field in fields {
                let child = compute_child(
                    types,
                    field.type_id,
                    layouts,
                    active,
                    depth,
                    overlap_backing,
                    charge,
                )?;
                layout.members.push(MemberLayout {
                    name_idx: field.name_idx,
                    offset: 0,
                    byte_offset: 0,
                });
                extend_leaves(&mut layout, &child, 0, 0)?;
                cells = cells.max(child.cells);
                shape.bytes = shape.bytes.max(child.shape.bytes);
                shape.align = shape.align.max(child.shape.align);
            }
            // A union reserves its largest variant, aligned to its most aligned one.
            layout.shape = ByteShape {
                bytes: align_up(shape.bytes, shape.align)?,
                align: shape.align,
            };
            cells
        }
        TypeData::Reference { target_type_id } => {
            layout.reference_leaves.push(ReferenceLeaf {
                offset: 0,
                byte_offset: 0,
                target_type_id: *target_type_id,
            });
            layout.shape = ByteShape {
                bytes: REFERENCE_BYTES,
                align: 8,
            };
            1
        }
        TypeData::Pou { .. } | TypeData::Interface { .. } => {
            layout.instance_leaves.push(InstanceLeaf {
                offset: 0,
                byte_offset: 0,
                type_id: id,
            });
            layout.shape = ByteShape {
                bytes: INSTANCE_BYTES,
                align: 8,
            };
            1
        }
    };
    layout.holds_reference_or_instance =
        !layout.reference_leaves.is_empty() || !layout.instance_leaves.is_empty();
    layout.cells = cells;
    Ok((cells, layout))
}

/// The layout of an array of `dims` whose elements have the layout `child`, written into `layout`;
/// returns its cells. The elements follow each other, a cell range or a byte run each, the bytes at
/// the element's size rounded to its alignment (MEM-12).
fn array_from(
    layout: &mut TypeLayout,
    child: TypeLayout,
    dims: &[(i64, i64)],
) -> Result<u32, RuntimeError> {
    let stride = child.cells;
    // An `ARRAY[*]` has no cells of its own: its elements are the actual argument's (spec 12 §6.3),
    // so its layout keeps the stride and the sentinel bounds and no count.
    let count = if dims.iter().any(|dim| is_wildcard_dim(*dim)) {
        0
    } else {
        array_count(dims)?
    };
    let times = |unit: u32| {
        u64::from(unit)
            .checked_mul(count)
            .and_then(|total| u32::try_from(total).ok())
            .ok_or(RuntimeError::Overflow)
    };
    let cells = times(stride)?;
    let byte_stride = align_up(child.shape.bytes, child.shape.align)?;
    layout.shape = ByteShape {
        bytes: times(byte_stride)?,
        align: child.shape.align,
    };
    layout.array = Some(ArrayLayout {
        stride,
        byte_stride,
        dims: dims.to_vec(),
    });
    layout.text_leaves = child.text_leaves;
    layout.reference_leaves = child.reference_leaves;
    layout.instance_leaves = child.instance_leaves;
    Ok(cells)
}

/// Where the next member of a structure goes: its cell offset, its byte offset and the structure's
/// alignment so far (MEM-12).
#[derive(Default)]
struct MemberCursor {
    cells: u32,
    bytes: u32,
    align: u32,
}

impl MemberCursor {
    /// Place the member `name_idx` of layout `child` next: at the next cell, and at the next byte its
    /// alignment allows.
    fn place(
        &mut self,
        layout: &mut TypeLayout,
        child: &TypeLayout,
        name_idx: u32,
    ) -> Result<(), RuntimeError> {
        self.bytes = align_up(self.bytes, child.shape.align)?;
        self.align = self.align.max(child.shape.align);
        layout.members.push(MemberLayout {
            name_idx,
            offset: self.cells,
            byte_offset: self.bytes,
        });
        extend_leaves(layout, child, self.cells, self.bytes)?;
        self.cells = self
            .cells
            .checked_add(child.cells)
            .ok_or(RuntimeError::Overflow)?;
        self.bytes = self
            .bytes
            .checked_add(child.shape.bytes)
            .ok_or(RuntimeError::Overflow)?;
        Ok(())
    }

    /// The structure's shape, aligned to its largest member and padded to a multiple of it; returns
    /// its cells.
    fn finish(self, layout: &mut TypeLayout) -> Result<u32, RuntimeError> {
        let align = self.align.max(1);
        layout.shape = ByteShape {
            bytes: align_up(self.bytes, align)?,
            align,
        };
        Ok(self.cells)
    }
}

/// The bytes one type's prepared layout table occupies: its [`TypeLayout`] entry plus the member,
/// leaf and dimension lists it owns (specification 34 §6.2).
fn layout_table_bytes(layout: &TypeLayout) -> Result<usize, RuntimeError> {
    let dims = layout.array.as_ref().map_or(0, |array| array.dims.len());
    let mut bytes = core::mem::size_of::<TypeLayout>();
    for list in [
        list_bytes::<MemberLayout>(layout.members.len())?,
        list_bytes::<TextLeaf>(layout.text_leaves.len())?,
        list_bytes::<ReferenceLeaf>(layout.reference_leaves.len())?,
        list_bytes::<InstanceLeaf>(layout.instance_leaves.len())?,
        list_bytes::<(i64, i64)>(dims)?,
    ] {
        bytes = bytes.checked_add(list).ok_or(RuntimeError::Overflow)?;
    }
    Ok(bytes)
}

/// The bytes a `count`-entry list of `T` occupies, or a checked refusal.
fn list_bytes<T>(count: usize) -> Result<usize, RuntimeError> {
    count
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(RuntimeError::Overflow)
}

/// Compute a child layout and return a clone of it.
fn compute_child(
    types: &TypeTable,
    id: u32,
    layouts: &mut [Option<TypeLayout>],
    active: &mut [bool],
    depth: usize,
    overlap_backing: &mut dyn FnMut(u32) -> Result<u32, RuntimeError>,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<TypeLayout, RuntimeError> {
    compute_one(
        types,
        id,
        layouts,
        active,
        depth + 1,
        overlap_backing,
        charge,
    )?;
    layouts
        .get(id as usize)
        .and_then(Option::as_ref)
        .cloned()
        .ok_or(RuntimeError::TypeMismatch)
}

/// Append a child's leaves to a compound layout, shifted by the child's cell offset and byte
/// offset.
fn extend_leaves(
    layout: &mut TypeLayout,
    child: &TypeLayout,
    offset: u32,
    byte_offset: u32,
) -> Result<(), RuntimeError> {
    let shift = |own: u32, by: u32| own.checked_add(by).ok_or(RuntimeError::Overflow);
    for leaf in &child.text_leaves {
        layout.text_leaves.push(TextLeaf {
            offset: shift(leaf.offset, offset)?,
            capacity_bytes: leaf.capacity_bytes,
            byte_offset: shift(leaf.byte_offset, byte_offset)?,
        });
    }
    for leaf in &child.reference_leaves {
        layout.reference_leaves.push(ReferenceLeaf {
            offset: shift(leaf.offset, offset)?,
            byte_offset: shift(leaf.byte_offset, byte_offset)?,
            target_type_id: leaf.target_type_id,
        });
    }
    for leaf in &child.instance_leaves {
        layout.instance_leaves.push(InstanceLeaf {
            offset: shift(leaf.offset, offset)?,
            byte_offset: shift(leaf.byte_offset, byte_offset)?,
            type_id: leaf.type_id,
        });
    }
    Ok(())
}

/// The element count of every array dimension, or a checked refusal.
fn array_count(dims: &[(i64, i64)]) -> Result<u64, RuntimeError> {
    let mut count: u64 = 1;
    for (lower, upper) in dims {
        let length = i128::from(*upper) - i128::from(*lower) + 1;
        let length = u64::try_from(length).map_err(|_| RuntimeError::TypeMismatch)?;
        if length == 0 {
            return Err(RuntimeError::TypeMismatch);
        }
        count = count.checked_mul(length).ok_or(RuntimeError::Overflow)?;
    }
    Ok(count)
}
