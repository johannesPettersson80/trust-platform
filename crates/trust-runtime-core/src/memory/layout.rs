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
}

/// A text leaf and its byte capacity (D-C1-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLeaf {
    /// Cell offset of the leaf within its type's cell range.
    pub offset: u32,
    /// Bytes reserved for the text: `n` for `STRING[n]`, `2n` for `WSTRING[n]`, and 80 or 160
    /// bytes for a `STRING`/`WSTRING` declared without a length.
    pub capacity_bytes: u32,
}

/// A reference leaf: one cell holding a reference (`Cell::Ref`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceLeaf {
    /// Cell offset of the leaf within its type's cell range.
    pub offset: u32,
    /// Referenced or aliased type index.
    pub target_type_id: u32,
}

/// An instance leaf: one cell holding an FB, class or interface instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceLeaf {
    /// Cell offset of the leaf within its type's cell range.
    pub offset: u32,
    /// The instance's FB, class or interface type index.
    pub type_id: u32,
}

/// An array's element stride and dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLayout {
    /// Cells in one element, so element `k` starts at `k * stride`.
    pub stride: u32,
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
            match *prim_id {
                24 => layout.text_leaves.push(TextLeaf {
                    offset: 0,
                    capacity_bytes: u32::from(string_capacity(*max_length)),
                }),
                25 => layout.text_leaves.push(TextLeaf {
                    offset: 0,
                    capacity_bytes: u32::from(string_capacity(*max_length)) * 2,
                }),
                _ => {}
            }
            1
        }
        TypeData::Enum { .. } | TypeData::Subrange { .. } => 1,
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
            let stride = child.cells;
            // An `ARRAY[*]` has no cells of its own: its elements are the actual argument's
            // (spec 12 §6.3), so its layout keeps the stride and the sentinel bounds and no count.
            let count = if dims.iter().any(|dim| is_wildcard_dim(*dim)) {
                0
            } else {
                array_count(dims)?
            };
            let cells = u64::from(stride)
                .checked_mul(count)
                .and_then(|cells| u32::try_from(cells).ok())
                .ok_or(RuntimeError::Overflow)?;
            layout.array = Some(ArrayLayout {
                stride,
                dims: dims.clone(),
            });
            layout.text_leaves = child.text_leaves;
            layout.reference_leaves = child.reference_leaves;
            layout.instance_leaves = child.instance_leaves;
            cells
        }
        TypeData::Struct { fields } => {
            if entry.kind == TypeKind::OverlappingStruct {
                layout.overlap_backing_bytes = overlap_backing(id)?;
                for field in fields {
                    layout.members.push(MemberLayout {
                        name_idx: field.name_idx,
                        offset: 0,
                    });
                }
                1
            } else {
                let mut offset = 0u32;
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
                        offset,
                    });
                    extend_leaves(&mut layout, &child, offset)?;
                    offset = offset
                        .checked_add(child.cells)
                        .ok_or(RuntimeError::Overflow)?;
                }
                offset
            }
        }
        TypeData::Union { fields } => {
            let mut cells = 0u32;
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
                });
                extend_leaves(&mut layout, &child, 0)?;
                cells = cells.max(child.cells);
            }
            cells
        }
        TypeData::Reference { target_type_id } => {
            layout.reference_leaves.push(ReferenceLeaf {
                offset: 0,
                target_type_id: *target_type_id,
            });
            1
        }
        TypeData::Pou { .. } | TypeData::Interface { .. } => {
            layout.instance_leaves.push(InstanceLeaf {
                offset: 0,
                type_id: id,
            });
            1
        }
    };
    layout.holds_reference_or_instance =
        !layout.reference_leaves.is_empty() || !layout.instance_leaves.is_empty();
    layout.cells = cells;
    Ok((cells, layout))
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

/// Append a child's leaves to a compound layout, shifted by the child's cell offset.
fn extend_leaves(
    layout: &mut TypeLayout,
    child: &TypeLayout,
    offset: u32,
) -> Result<(), RuntimeError> {
    for leaf in &child.text_leaves {
        layout.text_leaves.push(TextLeaf {
            offset: leaf
                .offset
                .checked_add(offset)
                .ok_or(RuntimeError::Overflow)?,
            capacity_bytes: leaf.capacity_bytes,
        });
    }
    for leaf in &child.reference_leaves {
        layout.reference_leaves.push(ReferenceLeaf {
            offset: leaf
                .offset
                .checked_add(offset)
                .ok_or(RuntimeError::Overflow)?,
            target_type_id: leaf.target_type_id,
        });
    }
    for leaf in &child.instance_leaves {
        layout.instance_leaves.push(InstanceLeaf {
            offset: leaf
                .offset
                .checked_add(offset)
                .ok_or(RuntimeError::Overflow)?,
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
