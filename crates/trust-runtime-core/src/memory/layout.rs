//! Prepared layouts of the flat blocks (specification 34 MEM-12).
//!
//! A [`TypeLayout`](crate::memory::layout::TypeLayout) describes how one value of a TYPE_TABLE
//! type occupies a flat block: its declared size and alignment, its members' and elements' byte
//! offsets, and its text, reference and instance leaves. Every structure or array is a contiguous
//! byte range with its nested compounds inline. A text leaf records its byte capacity from D-C1-2,
//! and a structure with a physical layout (a `STRUCT OVERLAP` and every structure nested in one)
//! records its byte backing and the members that lie beside it (C1-15).
//!
//! The layout tables and their computation are charged once at preparation. A region's bytes are
//! only recorded here; construction charges them when it reserves the region. The traversal is
//! bounded by
//! [`BYTECODE_MAX_TYPE_NESTING`](crate::bytecode::BYTECODE_MAX_TYPE_NESTING): a type deeper than
//! the bound is refused with
//! [`RuntimeError::TypeNestingExceeds`](crate::error::RuntimeError::TypeNestingExceeds) and a
//! recursive type with `TypeMismatch`.

use alloc::vec::Vec;

use smol_str::SmolStr;

use crate::bytecode::{TypeData, TypeTable, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::value::{string_capacity, RefSegment};
use crate::vm::type_policy::is_wildcard_dim;
use crate::vm::Names;

/// Bytes of one decoded leaf value, a [`Cell`](crate::memory::cells::Cell): the unit the copy
/// charges of TIME-08 count a value's element in.
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
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] for an alignment of zero, and [`RuntimeError::Overflow`] when the
/// rounded offset leaves `u32`.
pub fn align_up(offset: u32, align: u32) -> Result<u32, RuntimeError> {
    let mask = align.checked_sub(1).ok_or(RuntimeError::TypeMismatch)?;
    offset
        .checked_add(mask)
        .map(|end| end & !mask)
        .ok_or(RuntimeError::Overflow)
}

/// The region a byte range is reserved in (design "Regions").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayoutRegion {
    /// Globals and persistent instance templates, reserved at construction.
    Persistent,
    /// Locals and initializer results, on the activation stack.
    Activation,
}

/// A contiguous run of bytes in one region's flat block (MEM-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    /// The region the bytes are reserved in.
    pub region: LayoutRegion,
    /// First byte of the range within its region.
    pub start: u32,
    /// Number of bytes in the range.
    pub bytes: u32,
}

/// The alignment of an instance's own variables in a flat block, and so of every instance range:
/// the largest alignment a variable can have (MEM-12).
pub const INSTANCE_ALIGN: u32 = 8;

/// A POU's prepared instance template: its own variables only, with a link to its parent's
/// template (specification 34 MEM-10, MEM-12; O6). A derived POU without variables of its own has
/// an empty template with its parent link (O7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceTemplateLayout {
    /// The template's own bytes, from byte 0, rounded up to [`INSTANCE_ALIGN`].
    pub range: ByteRange,
    /// Parent POU whose template holds the inherited variables.
    pub parent: Option<u32>,
}

/// One structure member in declaration (ordinal) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberLayout {
    /// Member name index in STRING_TABLE; a later `REF_FIELD` resolves through it.
    pub name_idx: u32,
    /// Byte offset of the member within its type's flat bytes (MEM-12).
    pub byte_offset: u32,
}

/// A text leaf and its byte capacity (D-C1-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLeaf {
    /// Bytes reserved for the text: `n` for `STRING[n]`, `2n` for `WSTRING[n]`, and 80 or 160
    /// bytes for a `STRING`/`WSTRING` declared without a length.
    pub capacity_bytes: u32,
    /// Byte offset of the leaf's length field within its type's flat bytes; the text follows it
    /// (MEM-12).
    pub byte_offset: u32,
}

/// A reference leaf: [`REFERENCE_BYTES`] holding a reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceLeaf {
    /// Byte offset of the leaf's [`REFERENCE_BYTES`] within its type's flat bytes (MEM-12).
    pub byte_offset: u32,
    /// Referenced or aliased type index.
    pub target_type_id: u32,
}

/// An instance leaf: [`INSTANCE_BYTES`] naming an FB, class or interface instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceLeaf {
    /// Byte offset of the leaf's [`INSTANCE_BYTES`] within its type's flat bytes (MEM-12).
    pub byte_offset: u32,
    /// The instance's FB, class or interface type index.
    pub type_id: u32,
}

/// An array's element stride and dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLayout {
    /// Bytes in one element, so element `k` starts at byte `k * byte_stride` (MEM-12).
    pub byte_stride: u32,
    /// Inclusive lower and upper bounds in dimension order.
    pub dims: Vec<(i64, i64)>,
}

/// One type's prepared layout in a flat block (MEM-12).
///
/// For an array, the text, reference and instance leaf offsets are within the first element and
/// [`ArrayLayout::byte_stride`] locates later elements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeLayout {
    /// The declared size and alignment of one value of this type in a flat block (MEM-12).
    pub shape: ByteShape,
    /// Structure members in ordinal order; empty for non-structures.
    pub members: Vec<MemberLayout>,
    /// Array element stride and dimensions, absent for non-arrays.
    pub array: Option<ArrayLayout>,
    /// Text leaves, each with its byte capacity (D-C1-2).
    pub text_leaves: Vec<TextLeaf>,
    /// Reference leaves.
    pub reference_leaves: Vec<ReferenceLeaf>,
    /// Instance leaves.
    pub instance_leaves: Vec<InstanceLeaf>,
    /// How a structure with a physical layout lies in the block: its backing, then the members
    /// that lie beside it (C1-15). `None` for every other type.
    pub overlay: Option<OverlayLayout>,
    /// Whether the type can hold a reference or an instance, so a later lifetime check visits it.
    pub holds_reference_or_instance: bool,
}

/// How a structure with a physical layout lies in a flat block (specification 34 MEM-12; C1-15): its
/// backing, which holds the bits of every member as specification 02 places them, gaps included,
/// then each member that has no bits there, which only it uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayout {
    /// Bytes of the backing, from byte 0 of the structure's flat bytes.
    pub backing_bytes: u32,
    /// The members that lie beside the backing, in declaration order through every nesting level.
    pub slots: Vec<OverlaySlot>,
}

/// A member of a structure with a physical layout that lies beside its backing (C1-15): a text, a
/// reference, a union, or an array of them, which specification 12 gives no bits that another member
/// shares. It lies at its own type's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlaySlot {
    /// The member's path from the structure: member names and element subscripts.
    pub path: Vec<RefSegment>,
    /// The member's type.
    pub type_id: u32,
    /// Byte offset of the member's bytes within the structure's flat bytes.
    pub byte_offset: u32,
}

/// Compute one [`TypeLayout`] per TYPE_TABLE entry, charging the layout tables it builds.
///
/// `physical_backing` returns the admitted byte backing length of a structure type that has a
/// physical layout, and `None` for one that has none (the shared value layer validates and sizes
/// it); `names` gives the member names of the paths of the members that lie beside a backing.
/// `charge` mirrors the preparation budget: it is called with the bytes of each layout table and the
/// work of computing it, so an oversized type table is refused here with its limit error. The
/// type's bytes are not charged; construction charges a region's bytes when it reserves the region.
pub(crate) fn compute_type_layouts(
    types: &TypeTable,
    names: &Names,
    physical_backing: &mut dyn FnMut(u32) -> Result<Option<u32>, RuntimeError>,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<TypeLayout>, RuntimeError> {
    let mut layouts: Vec<Option<TypeLayout>> = Vec::new();
    layouts
        .try_reserve_exact(types.entries.len())
        .map_err(|_| RuntimeError::PreparationLimit)?;
    layouts.resize_with(types.entries.len(), || None);
    let mut active = Vec::new();
    active.resize(types.entries.len(), false);
    let mut computation = Computation {
        types,
        names,
        layouts,
        active,
        physical_backing,
        charge,
    };
    for index in 0..types.entries.len() {
        let id = u32::try_from(index).map_err(|_| RuntimeError::Overflow)?;
        computation.compute_one(id, 0)?;
    }
    Ok(computation
        .layouts
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

/// The type layouts being computed, with what their computation reads and charges.
struct Computation<'a> {
    types: &'a TypeTable,
    names: &'a Names,
    layouts: Vec<Option<TypeLayout>>,
    /// The types whose layout is being computed, so a recursive type is refused.
    active: Vec<bool>,
    physical_backing: &'a mut dyn FnMut(u32) -> Result<Option<u32>, RuntimeError>,
    charge: &'a mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
}

impl Computation<'_> {
    /// Compute and cache the layout of one type.
    fn compute_one(&mut self, id: u32, depth: usize) -> Result<(), RuntimeError> {
        (self.charge)(0, 1)?;
        if self.layouts.get(id as usize).is_some_and(Option::is_some) {
            return Ok(());
        }
        if depth >= BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeNestingExceeds);
        }
        if *self
            .active
            .get(id as usize)
            .ok_or(RuntimeError::TypeMismatch)?
        {
            return Err(RuntimeError::TypeMismatch);
        }
        *self
            .active
            .get_mut(id as usize)
            .ok_or(RuntimeError::TypeMismatch)? = true;
        let layout = self.build_layout(id, depth)?;
        *self
            .active
            .get_mut(id as usize)
            .ok_or(RuntimeError::TypeMismatch)? = false;
        (self.charge)(layout_table_bytes(&layout)?, 1)?;
        self.layouts
            .get_mut(id as usize)
            .ok_or(RuntimeError::TypeMismatch)?
            .replace(layout);
        Ok(())
    }

    /// Compute a child layout and return a clone of it.
    fn compute_child(&mut self, id: u32, depth: usize) -> Result<TypeLayout, RuntimeError> {
        self.compute_one(id, depth + 1)?;
        self.layouts
            .get(id as usize)
            .and_then(Option::as_ref)
            .cloned()
            .ok_or(RuntimeError::TypeMismatch)
    }

    /// Build the layout of one already-entered type.
    fn build_layout(&mut self, id: u32, depth: usize) -> Result<TypeLayout, RuntimeError> {
        let types = self.types;
        let entry = types
            .entries
            .get(id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        let mut layout = TypeLayout::default();
        match &entry.data {
            TypeData::Primitive {
                prim_id,
                max_length,
            } => {
                let length = u32::from(string_capacity(*max_length));
                match *prim_id {
                    24 => layout.text_leaves.push(TextLeaf {
                        capacity_bytes: length,
                        byte_offset: 0,
                    }),
                    25 => layout.text_leaves.push(TextLeaf {
                        capacity_bytes: length * 2,
                        byte_offset: 0,
                    }),
                    _ => {}
                }
                layout.shape = primitive_shape(*prim_id, length)?;
            }
            // An enumeration or a subrange has its base type's declared size (MEM-12).
            TypeData::Enum { base_type_id, .. } | TypeData::Subrange { base_type_id, .. } => {
                let base = self.compute_child(*base_type_id, depth)?;
                layout.shape = base.shape;
            }
            TypeData::Alias { target_type_id } => {
                let child = self.compute_child(*target_type_id, depth)?;
                layout = child;
            }
            TypeData::Array { elem_type_id, dims } => {
                let child = self.compute_child(*elem_type_id, depth)?;
                array_from(&mut layout, child, dims)?;
            }
            TypeData::Struct { fields } => {
                if let Some(backing_bytes) = (self.physical_backing)(id)? {
                    for field in fields {
                        layout.members.push(MemberLayout {
                            name_idx: field.name_idx,
                            byte_offset: 0,
                        });
                    }
                    // The members lie over the backing bytes, laid out as specification 02 defines,
                    // and those with no bits there after it, each at its own layout.
                    let mut walk = SlotWalk {
                        slots: Vec::new(),
                        path: Vec::new(),
                        end: backing_bytes,
                        align: 1,
                    };
                    self.place_slots(&mut walk, &mut layout, id, depth)?;
                    // Aligned to its most aligned member beside the backing; the backing's bytes
                    // need no alignment.
                    layout.shape = ByteShape {
                        bytes: align_up(walk.end, walk.align)?,
                        align: walk.align,
                    };
                    layout.overlay = Some(OverlayLayout {
                        backing_bytes,
                        slots: walk.slots,
                    });
                } else {
                    let mut cursor = MemberCursor::default();
                    for field in fields {
                        let child = self.compute_child(field.type_id, depth)?;
                        cursor.place(&mut layout, &child, field.name_idx)?;
                    }
                    cursor.finish(&mut layout)?;
                }
            }
            // A union keeps every variant as a member of its own (specification 02 §3.7), so it lies
            // as a structure its members; `SIZEOF` keeps its largest variant (C1-15).
            TypeData::Union { fields } => {
                let mut cursor = MemberCursor::default();
                for field in fields {
                    let child = self.compute_child(field.type_id, depth)?;
                    cursor.place(&mut layout, &child, field.name_idx)?;
                }
                cursor.finish(&mut layout)?;
            }
            TypeData::Reference { target_type_id } => {
                layout.reference_leaves.push(ReferenceLeaf {
                    byte_offset: 0,
                    target_type_id: *target_type_id,
                });
                layout.shape = ByteShape {
                    bytes: REFERENCE_BYTES,
                    align: 8,
                };
            }
            TypeData::Pou { .. } | TypeData::Interface { .. } => {
                layout.instance_leaves.push(InstanceLeaf {
                    byte_offset: 0,
                    type_id: id,
                });
                layout.shape = ByteShape {
                    bytes: INSTANCE_BYTES,
                    align: 8,
                };
            }
        }
        layout.holds_reference_or_instance =
            !layout.reference_leaves.is_empty() || !layout.instance_leaves.is_empty();
        Ok(layout)
    }

    /// Place, after the backing of the structure with a physical layout whose layout is `layout`,
    /// each member of a value of `type_id` (the structure or one of its members) that lies beside
    /// the backing: in declaration order, elements in flat order, each at its own layout and
    /// alignment, its leaves added to `layout`'s at its offset (C1-15).
    fn place_slots(
        &mut self,
        walk: &mut SlotWalk,
        layout: &mut TypeLayout,
        type_id: u32,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        (self.charge)(0, 1)?;
        if depth >= BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeNestingExceeds);
        }
        let types = self.types;
        let entry = types
            .entries
            .get(type_id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        if lies_beside_backing(types, type_id) {
            let child = self.compute_child(type_id, depth)?;
            let offset = align_up(walk.end, child.shape.align)?;
            walk.align = walk.align.max(child.shape.align);
            extend_leaves(layout, &child, offset)?;
            let mut path = Vec::new();
            path.try_reserve_exact(walk.path.len())
                .map_err(|_| RuntimeError::PreparationLimit)?;
            path.extend(walk.path.iter().cloned());
            (self.charge)(path_table_bytes(&path)?, path.len())?;
            walk.slots.push(OverlaySlot {
                path,
                type_id,
                byte_offset: offset,
            });
            walk.end = offset
                .checked_add(child.shape.bytes)
                .ok_or(RuntimeError::Overflow)?;
            return Ok(());
        }
        match &entry.data {
            TypeData::Alias { target_type_id } => {
                self.place_slots(walk, layout, *target_type_id, depth + 1)
            }
            TypeData::Struct { fields } => {
                for field in fields {
                    if !holds_slot(types, field.type_id, depth + 1) {
                        continue;
                    }
                    let name = self
                        .names
                        .get(field.name_idx as usize)
                        .ok_or(RuntimeError::TypeMismatch)?;
                    walk.path.push(RefSegment::Field(SmolStr::new(name)));
                    let placed = self.place_slots(walk, layout, field.type_id, depth + 1);
                    walk.path.pop();
                    placed?;
                }
                Ok(())
            }
            TypeData::Array { elem_type_id, dims } => {
                if !holds_slot(types, *elem_type_id, depth + 1) {
                    return Ok(());
                }
                let count = array_count(dims)?;
                for ordinal in 0..count {
                    walk.path
                        .push(RefSegment::Index(subscripts(dims, ordinal)?));
                    let placed = self.place_slots(walk, layout, *elem_type_id, depth + 1);
                    walk.path.pop();
                    placed?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// The walk that places the members of a structure that lie beside its backing.
struct SlotWalk {
    slots: Vec<OverlaySlot>,
    /// The path of the member being walked.
    path: Vec<RefSegment>,
    /// The end of the bytes placed so far.
    end: u32,
    /// The largest alignment of a member placed so far.
    align: u32,
}

/// Whether a member of `type_id` in a structure with a physical layout lies beside its backing
/// (specification 12: a text has no overlapping bit representation, and the layout gives a reference
/// and a union none either): a text, a reference, a union, or an array of them.
pub(crate) fn lies_beside_backing(types: &TypeTable, type_id: u32) -> bool {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let Some(entry) = types.entries.get(current as usize) else {
            return false;
        };
        match &entry.data {
            TypeData::Alias { target_type_id } => current = *target_type_id,
            TypeData::Array { elem_type_id, .. } => current = *elem_type_id,
            TypeData::Primitive {
                prim_id: 24 | 25, ..
            }
            | TypeData::Reference { .. }
            | TypeData::Union { .. } => return true,
            _ => return false,
        }
    }
    false
}

/// Whether a value of `type_id` holds a member that lies beside a backing, at any nesting level.
fn holds_slot(types: &TypeTable, type_id: u32, depth: usize) -> bool {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    if lies_beside_backing(types, type_id) {
        return true;
    }
    match types.entries.get(type_id as usize).map(|entry| &entry.data) {
        Some(TypeData::Alias { target_type_id }) => holds_slot(types, *target_type_id, depth + 1),
        Some(TypeData::Array { elem_type_id, .. }) => holds_slot(types, *elem_type_id, depth + 1),
        Some(TypeData::Struct { fields }) => fields
            .iter()
            .any(|field| holds_slot(types, field.type_id, depth + 1)),
        _ => false,
    }
}

/// The subscripts of the element at flat position `ordinal` of an array of `dims`, the last
/// dimension varying fastest.
fn subscripts(dims: &[(i64, i64)], ordinal: u64) -> Result<Vec<i64>, RuntimeError> {
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(dims.len())
        .map_err(|_| RuntimeError::PreparationLimit)?;
    indices.resize(dims.len(), 0);
    let mut remaining = ordinal;
    for (index, (lower, upper)) in indices.iter_mut().zip(dims).rev() {
        let length = u64::try_from(i128::from(*upper) - i128::from(*lower) + 1)
            .map_err(|_| RuntimeError::TypeMismatch)?;
        let offset = i64::try_from(remaining % length).map_err(|_| RuntimeError::Overflow)?;
        *index = lower.checked_add(offset).ok_or(RuntimeError::Overflow)?;
        remaining /= length;
    }
    Ok(indices)
}

/// The bytes a slot's path occupies: its segments and each segment's subscripts or long name.
fn path_table_bytes(path: &[RefSegment]) -> Result<usize, RuntimeError> {
    let mut bytes = list_bytes::<RefSegment>(path.len())?;
    for segment in path {
        let own = match segment {
            RefSegment::Index(indices) => list_bytes::<i64>(indices.len())?,
            RefSegment::Field(name) if name.is_heap_allocated() => name.len(),
            RefSegment::Field(_) => 0,
        };
        bytes = bytes.checked_add(own).ok_or(RuntimeError::Overflow)?;
    }
    Ok(bytes)
}

/// The layout of an array of `dims` whose elements have the layout `child`, written into `layout`.
/// The elements follow each other, each at the element's size rounded to its alignment (MEM-12).
fn array_from(
    layout: &mut TypeLayout,
    child: TypeLayout,
    dims: &[(i64, i64)],
) -> Result<(), RuntimeError> {
    // An `ARRAY[*]` has no bytes of its own: its elements are the actual argument's (spec 12 §6.3),
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
    let byte_stride = align_up(child.shape.bytes, child.shape.align)?;
    layout.shape = ByteShape {
        bytes: times(byte_stride)?,
        align: child.shape.align,
    };
    layout.array = Some(ArrayLayout {
        byte_stride,
        dims: dims.to_vec(),
    });
    layout.text_leaves = child.text_leaves;
    layout.reference_leaves = child.reference_leaves;
    layout.instance_leaves = child.instance_leaves;
    Ok(())
}

/// Where the next member of a structure goes: its byte offset and the structure's alignment so far
/// (MEM-12).
#[derive(Default)]
struct MemberCursor {
    bytes: u32,
    align: u32,
}

impl MemberCursor {
    /// Place the member `name_idx` of layout `child` next, at the next byte its alignment allows.
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
            byte_offset: self.bytes,
        });
        extend_leaves(layout, child, self.bytes)?;
        self.bytes = self
            .bytes
            .checked_add(child.shape.bytes)
            .ok_or(RuntimeError::Overflow)?;
        Ok(())
    }

    /// The structure's shape, aligned to its largest member and padded to a multiple of it.
    fn finish(self, layout: &mut TypeLayout) -> Result<(), RuntimeError> {
        let align = self.align.max(1);
        layout.shape = ByteShape {
            bytes: align_up(self.bytes, align)?,
            align,
        };
        Ok(())
    }
}

/// The bytes one type's prepared layout table occupies: its [`TypeLayout`] entry plus the member,
/// leaf and dimension lists it owns (specification 34 §6.2).
fn layout_table_bytes(layout: &TypeLayout) -> Result<usize, RuntimeError> {
    let dims = layout.array.as_ref().map_or(0, |array| array.dims.len());
    let slots = layout
        .overlay
        .as_ref()
        .map_or(0, |overlay| overlay.slots.len());
    let mut bytes = core::mem::size_of::<TypeLayout>();
    for list in [
        list_bytes::<MemberLayout>(layout.members.len())?,
        list_bytes::<TextLeaf>(layout.text_leaves.len())?,
        list_bytes::<ReferenceLeaf>(layout.reference_leaves.len())?,
        list_bytes::<InstanceLeaf>(layout.instance_leaves.len())?,
        list_bytes::<(i64, i64)>(dims)?,
        list_bytes::<OverlaySlot>(slots)?,
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

/// Append a child's leaves to a compound layout, shifted by the child's byte offset.
fn extend_leaves(
    layout: &mut TypeLayout,
    child: &TypeLayout,
    byte_offset: u32,
) -> Result<(), RuntimeError> {
    let shift = |own: u32| own.checked_add(byte_offset).ok_or(RuntimeError::Overflow);
    for leaf in &child.text_leaves {
        layout.text_leaves.push(TextLeaf {
            capacity_bytes: leaf.capacity_bytes,
            byte_offset: shift(leaf.byte_offset)?,
        });
    }
    for leaf in &child.reference_leaves {
        layout.reference_leaves.push(ReferenceLeaf {
            byte_offset: shift(leaf.byte_offset)?,
            target_type_id: leaf.target_type_id,
        });
    }
    for leaf in &child.instance_leaves {
        layout.instance_leaves.push(InstanceLeaf {
            byte_offset: shift(leaf.byte_offset)?,
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
