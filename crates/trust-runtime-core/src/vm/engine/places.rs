//! Places on the flat store (specification 34 MEM-12; C1-10c).
//!
//! A load of an array or a structure whose bytes hold no text, reference, instance or overlap leaves
//! the value where it lies, as a [`Place`], and a store copies its bytes to a destination of its type,
//! checked and charged as a write of the value is. So an assignment of such a compound builds no
//! value and allocates nothing.

use super::store_access::Routed;
use super::*;
use crate::bytecode::{TypeData, TypeEntry, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::memory::layout::LayoutRegion;
use crate::memory::view::ValueView;
use crate::value::ValueRefView;
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

/// Whether a store may copy a value of `type_id` as its bytes: an array or a structure whose leaves
/// are numbers, bits, characters, times, dates and enumerations. A text is cut to its destination's
/// capacity, a reference and an instance are checked and placed by the value they hold, and an
/// overlapping or located structure is written through its physical layout (C1-15), so a value that
/// holds any of them is stored as a value.
fn copies_as_bytes(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve(prepared, type_id).is_some_and(|entry| {
        matches!(entry.data, TypeData::Array { .. } | TypeData::Struct { .. })
            && plain(prepared, type_id, 0)
    })
}

/// Whether every leaf of a value of `type_id` is a scalar its bytes hold whole.
fn plain(prepared: &PreparedModule, type_id: u32, depth: usize) -> bool {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    let Some(entry) = resolve(prepared, type_id) else {
        return false;
    };
    match &entry.data {
        TypeData::Primitive { prim_id, .. } => !TEXT_PRIMITIVES.contains(prim_id),
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
