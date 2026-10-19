//! Places on the flat store (specification 34 MEM-12; C1-10c).
//!
//! A load of a text, or of an array or a structure whose bytes hold no reference, instance or
//! overlap, leaves the value where it lies, as a [`Place`], and a store copies its bytes to a
//! destination of its type, checked and charged as a write of the value is. A text a standard text
//! function made is written into its destination's text leaf, cut to that leaf's capacity (C1-12).
//! So such an assignment builds no value and allocates nothing.

use super::store_access::Routed;
use super::*;
use crate::bytecode::{TypeData, TypeEntry, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::memory::cell_access::CellReader;
use crate::memory::cells::text_geometry;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::memory::view::ValueView;
use crate::value::{string_capacity, string_capacity_cut, ValueRefView};
use crate::vm::{Place, PlaceDestination};

/// The `TYPE_TABLE` primitive ids of `STRING` and `WSTRING`.
const TEXT_PRIMITIVES: [u16; 2] = [24, 25];

impl EngineState<'_> {
    /// The place of `view`: an array or a structure in the persistent block whose value a store may
    /// copy as bytes.
    pub(super) fn place_of(&self, view: &ValueView<'_>) -> Option<Place> {
        let ValueView::Cells { range, type_id, .. } = *view else {
            return None;
        };
        (range.region == LayoutRegion::Persistent && copies_as_bytes(self.prepared, type_id))
            .then_some(Place { range, type_id })
    }

    /// The view of the compound `place` stands for.
    pub(super) fn place_view(&self, place: Place) -> Option<ValueView<'_>> {
        match self.data.routed(self.prepared) {
            Routed::Cells(reader) => Some(reader.place_view(place.range, place.type_id)),
            Routed::Values(_) => None,
        }
    }

    /// Where a store through `reference` copies `place`: the destination's bytes, when they lie in
    /// the persistent block and hold the place's type, as `declared` must too when the store
    /// normalizes to a declared type. A destination of another type is given the value instead, so
    /// that its normalization decides, as it decides for any value.
    pub(super) fn place_destination(
        &self,
        reference: ValueRefView<'_>,
        place: Place,
        declared: Option<u32>,
    ) -> Option<PlaceDestination> {
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return None;
        };
        let (range, type_id, presence) =
            reader.cells_target(reference.location, reference.offset, reference.path)?;
        let same = |other: u32| same_type(self.prepared, other, place.type_id, 0);
        (range.region == LayoutRegion::Persistent
            && range.bytes == place.range.bytes
            && same(type_id)
            && declared.is_none_or(same))
        .then_some(PlaceDestination { range, presence })
    }

    /// The checks of a write of the value `place` stands for through `reference`: those of
    /// `check_write`, whose lifetime check of that value only visits it.
    pub(super) fn check_place_write(
        &self,
        reference: ValueRefView<'_>,
        place: Place,
    ) -> Result<(), RuntimeError> {
        self.check_write_target(reference)?;
        self.check_place_lifetime(place.type_id)
    }

    /// Charge what normalizing the value `place` stands for to its own type charges, so that a copy
    /// of its bytes charges what a store of that value does (TIME-08; C1-12): each text is checked
    /// against its capacity, one unit for each byte of a text in a structure and two for each byte
    /// of any other, in the order the value's normalization reaches them. A value without a text
    /// charges nothing.
    pub(super) fn charge_place_normalization(&self, place: Place) -> Result<(), RuntimeError> {
        if !holds_text(self.prepared, place.type_id) {
            return Ok(());
        }
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        self.charge_text_normalization(reader, place.range, place.type_id, false, 0)
    }

    /// The normalization charges of the texts in the value of `type_id` at `range`, as
    /// [`Self::charge_place_normalization`] gives them; `in_structure` for a value inside a
    /// structure, which normalization checks member by member.
    fn charge_text_normalization(
        &self,
        reader: CellReader<'_>,
        range: ByteRange,
        type_id: u32,
        in_structure: bool,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let entry = resolve(self.prepared, type_id).ok_or(RuntimeError::TypeMismatch)?;
        let layout = self
            .prepared
            .type_layout(type_id)
            .ok_or(RuntimeError::TypeMismatch)?;
        match &entry.data {
            TypeData::Primitive { prim_id, .. } if TEXT_PRIMITIVES.contains(prim_id) => {
                let length = reader
                    .place_view(range, type_id)
                    .text_utf8_len()?
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let units = if in_structure {
                    length
                } else {
                    length.checked_mul(2).ok_or(RuntimeError::Overflow)?
                };
                self.charge_work_units(units)
            }
            TypeData::Array { elem_type_id, dims } if holds_text(self.prepared, *elem_type_id) => {
                let stride = layout
                    .array
                    .as_ref()
                    .map(|array| array.byte_stride)
                    .ok_or(RuntimeError::TypeMismatch)?;
                let bytes = self
                    .prepared
                    .type_layout(*elem_type_id)
                    .ok_or(RuntimeError::TypeMismatch)?
                    .shape
                    .bytes;
                let count = element_count(dims).ok_or(RuntimeError::TypeMismatch)?;
                for element in 0..count {
                    let offset = u32::try_from(element)
                        .ok()
                        .and_then(|element| element.checked_mul(stride))
                        .and_then(|offset| range.start.checked_add(offset))
                        .ok_or(RuntimeError::Overflow)?;
                    let element = ByteRange {
                        start: offset,
                        bytes,
                        ..range
                    };
                    self.charge_text_normalization(
                        reader,
                        element,
                        *elem_type_id,
                        in_structure,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            TypeData::Struct { fields } => {
                for (field, member) in fields.iter().zip(&layout.members) {
                    if !holds_text(self.prepared, field.type_id) {
                        continue;
                    }
                    let member = ByteRange {
                        start: range
                            .start
                            .checked_add(member.byte_offset)
                            .ok_or(RuntimeError::Overflow)?,
                        bytes: self
                            .prepared
                            .type_layout(field.type_id)
                            .ok_or(RuntimeError::TypeMismatch)?
                            .shape
                            .bytes,
                        ..range
                    };
                    self.charge_text_normalization(reader, member, field.type_id, true, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Store `text`, a `WSTRING` when `wide`, through `reference` as a store of its value does
    /// (TIME-08; C1-12): normalized to `declared`, the type the store normalizes to when it has one,
    /// and checked as a write, the check first when `check_first`, as a dynamic store takes them,
    /// then written into the destination's text leaf cut to its capacity. `None` when the
    /// destination is no text leaf of the flat store with the text's width, so the store is given
    /// the value instead; `Some(Ok(false))` when the block refuses the write.
    pub(super) fn store_text(
        &mut self,
        reference: ValueRefView<'_>,
        text: &str,
        wide: bool,
        declared: Option<u32>,
        check_first: bool,
    ) -> Option<Result<bool, RuntimeError>> {
        let declared_units = match declared {
            Some(declared) => {
                let (prim_id, max_length) = text_primitive(self.prepared, declared)?;
                if (prim_id == TEXT_PRIMITIVES[1]) != wide {
                    return None;
                }
                Some(usize::from(string_capacity(max_length)))
            }
            None => None,
        };
        let (range, type_id, presence) = match self.data.routed(self.prepared) {
            Routed::Cells(reader) => {
                reader.cells_target(reference.location, reference.offset, reference.path)?
            }
            Routed::Values(_) => return None,
        };
        let geometry =
            text_geometry(self.prepared, type_id).filter(|(_, _, other)| *other == wide)?;
        Some((|| {
            // A text holds no reference, so the write's checks are its target's (TIME-08).
            if check_first {
                self.check_write_target(reference)?;
            }
            let mut kept = text.len();
            if let Some(units) = declared_units {
                self.charge_work_units(kept.checked_mul(2).ok_or(RuntimeError::Overflow)?)?;
                if let Some(end) = string_capacity_cut(text, wide, units) {
                    // The kept prefix is a new text, charged by the copy rule (TIME-08).
                    self.charge_allocation_bytes(super::allocation::text_copy_allocation(
                        &text[..end],
                        wide,
                    )?)?;
                    kept = end;
                }
            }
            if !check_first {
                self.check_write_target(reference)?;
            }
            Ok(match &mut self.data.store {
                EngineStore::Cells(arm) => arm.write_text(range, geometry, presence, &text[..kept]),
                EngineStore::Values => false,
            })
        })())
    }

    /// Copy the bytes of `place` to `destination`.
    pub(super) fn copy_place(&mut self, place: Place, destination: PlaceDestination) -> bool {
        match &mut self.data.store {
            EngineStore::Cells(arm) => {
                arm.copy_place(place.range, destination.range, destination.presence)
            }
            EngineStore::Values => false,
        }
    }

    /// The lifetime check of the value of `type_id` a place stands for, as `check_value_lifetime`
    /// checks that value (TIME-08 LIFETIME). The value holds no reference and no instance, so the
    /// check finds nothing and only visits it: an array of scalars is admitted without a charge, and
    /// any other value is visited node by node, one unit each, as the `Value` walk visits it.
    fn check_place_lifetime(&self, type_id: u32) -> Result<(), RuntimeError> {
        if !is_compound(self.prepared, type_id) {
            // A text, as a scalar, is admitted without a charge.
            return Ok(());
        }
        match resolve(self.prepared, type_id).map(|entry| &entry.data) {
            Some(TypeData::Array { elem_type_id, .. })
                if !is_compound(self.prepared, *elem_type_id) =>
            {
                Ok(())
            }
            _ => self.visit_place_node(type_id, 0),
        }
    }

    /// One node of the lifetime walk: the value itself, then the elements of an array of compounds or
    /// the members of a structure, in order.
    fn visit_place_node(&self, type_id: u32, depth: usize) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match resolve(self.prepared, type_id).map(|entry| &entry.data) {
            Some(TypeData::Array { elem_type_id, dims })
                if is_compound(self.prepared, *elem_type_id) =>
            {
                let count = element_count(dims).ok_or(RuntimeError::TypeMismatch)?;
                for _ in 0..count {
                    self.visit_place_node(*elem_type_id, depth + 1)?;
                }
            }
            Some(TypeData::Struct { fields }) => {
                for field in fields {
                    self.visit_place_node(field.type_id, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// The entry `type_id` names, with its aliases resolved.
fn resolve(prepared: &PreparedModule, type_id: u32) -> Option<&TypeEntry> {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = prepared.type_table().entries.get(current as usize)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => current = *target_type_id,
            _ => return Some(entry),
        }
    }
    None
}

/// Whether a value of `type_id` is an array or a structure: a `Value` that the lifetime walk enters.
fn is_compound(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve(prepared, type_id).is_some_and(|entry| {
        matches!(
            entry.data,
            TypeData::Array { .. } | TypeData::Struct { .. } | TypeData::Union { .. }
        )
    })
}

/// Whether a store may copy a value of `type_id` as its bytes: a text, or an array or a structure
/// whose leaves are numbers, bits, characters, texts, times, dates and enumerations. A text of the
/// destination's type fits its capacity, so its bytes, the tail zeroed, are the bytes the
/// destination would hold (C1-12). A reference and an instance are checked and placed by the value
/// they hold, and an overlapping or located structure is written through its physical layout
/// (C1-15), so a value that holds any of them is stored as a value.
fn copies_as_bytes(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve(prepared, type_id).is_some_and(|entry| {
        matches!(entry.data, TypeData::Array { .. } | TypeData::Struct { .. })
            && plain(prepared, type_id, 0)
            || text_primitive(prepared, type_id).is_some()
    })
}

/// Whether a value of `type_id` holds a text.
fn holds_text(prepared: &PreparedModule, type_id: u32) -> bool {
    prepared
        .type_layout(type_id)
        .is_some_and(|layout| !layout.text_leaves.is_empty())
}

/// The primitive id and declared length of the text type `type_id`, or `None` for another type.
fn text_primitive(prepared: &PreparedModule, type_id: u32) -> Option<(u16, u16)> {
    match resolve(prepared, type_id)?.data {
        TypeData::Primitive {
            prim_id,
            max_length,
        } if TEXT_PRIMITIVES.contains(&prim_id) => Some((prim_id, max_length)),
        _ => None,
    }
}

/// Whether every leaf of a value of `type_id` is a scalar or a text its bytes hold whole.
fn plain(prepared: &PreparedModule, type_id: u32, depth: usize) -> bool {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    let Some(entry) = resolve(prepared, type_id) else {
        return false;
    };
    match &entry.data {
        TypeData::Primitive { .. } => true,
        TypeData::Enum { .. } => true,
        TypeData::Subrange { base_type_id, .. } => plain(prepared, *base_type_id, depth + 1),
        TypeData::Array { elem_type_id, .. } => plain(prepared, *elem_type_id, depth + 1),
        TypeData::Struct { fields } => {
            !matches!(
                entry.kind,
                TypeKind::OverlappingStruct | TypeKind::LocatedStruct
            ) && fields
                .iter()
                .all(|field| plain(prepared, field.type_id, depth + 1))
        }
        _ => false,
    }
}

/// Whether values of `left` and `right` have the same bytes for the same value: the same type, or
/// arrays of the same bounds whose elements are.
fn same_type(prepared: &PreparedModule, left: u32, right: u32, depth: usize) -> bool {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    let (Some(left_entry), Some(right_entry)) = (resolve(prepared, left), resolve(prepared, right))
    else {
        return false;
    };
    if core::ptr::eq(left_entry, right_entry) {
        return true;
    }
    match (&left_entry.data, &right_entry.data) {
        (
            TypeData::Array {
                elem_type_id: left_element,
                dims: left_dims,
            },
            TypeData::Array {
                elem_type_id: right_element,
                dims: right_dims,
            },
        ) => {
            left_dims == right_dims && same_type(prepared, *left_element, *right_element, depth + 1)
        }
        _ => false,
    }
}

/// The number of elements of an array of `dims`.
fn element_count(dims: &[(i64, i64)]) -> Option<usize> {
    dims.iter().try_fold(1usize, |count, (lower, upper)| {
        let length = upper.checked_sub(*lower)?.checked_add(1)?;
        count.checked_mul(usize::try_from(length).ok()?)
    })
}
