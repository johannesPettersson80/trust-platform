//! Places on the flat store (specification 34 MEM-12; C1-10c).
//!
//! A load of a text, or of an array or a structure whose bytes hold no reference, instance or
//! overlap, leaves the value where it lies, as a [`Place`], and a store copies its bytes to a
//! destination of its type, checked and charged as a write of the value is. A text a standard text
//! function made is written into its destination's text leaf, cut to that leaf's capacity (C1-12).
//! So such an assignment builds no value and allocates nothing. A structure with a physical layout
//! is a place too, copied as its cells, and so is a structure nested in its backing whose members all
//! lie in the backing, whose bytes there are the cells of a structure of its type (C1-15b,
//! `places/backing.rs`).

use super::store_access::Routed;
use super::*;
use crate::bytecode::{TypeData, BYTECODE_MAX_TYPE_NESTING};
use crate::memory::cell_access::CellReader;
use crate::memory::cell_access::PlaceTarget;
use crate::memory::cells::text_geometry;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::memory::view::ValueView;
use crate::value::{string_capacity, string_capacity_cut, ValueRefView};
use crate::vm::prepared::byte_types::{
    copies_as_bytes, element_count, holds_text, is_compound, resolve, same_type, text_primitive,
    TEXT_PRIMITIVES,
};
use crate::vm::{Place, PlaceDestination, PlaceNode};

/// Places of structures with a physical layout and of the structures nested in their backings
/// (C1-15b).
#[path = "places/backing.rs"]
mod backing;

/// Copies of the places a call leaves on the operand stack (C1-19b).
#[path = "places/copies.rs"]
mod copies;

impl EngineState<'_> {
    /// The place of `view`: an array or a structure in the persistent block whose value a store may
    /// copy as bytes, or a structure there that lies as its backing, or a structure nested in a
    /// backing whose bytes there are the cells of a structure of its type (C1-15b).
    pub(super) fn place_of(&self, view: &ValueView<'_>) -> Option<Place> {
        match *view {
            ValueView::Cells { range, type_id, .. } => (range.region == LayoutRegion::Persistent
                && (copies_as_bytes(self.prepared, type_id) || self.backing_place(type_id)))
            .then_some(Place { range, type_id }),
            ValueView::Bits {
                cells,
                range,
                type_id,
                leaf,
                depth,
            } => {
                let (range, node_type) = self.nested_cells(cells, range, type_id, leaf, depth)?;
                (range.region == LayoutRegion::Persistent).then_some(Place {
                    range,
                    type_id: node_type,
                })
            }
            ValueView::Value(_) => None,
        }
    }

    /// The view of the compound `place` stands for.
    pub(super) fn place_view(&self, place: Place) -> Option<ValueView<'_>> {
        match self.data.routed(self.prepared) {
            Routed::Cells(reader) => Some(reader.place_view(place.range, place.type_id)),
            Routed::Values(_) => None,
        }
    }

    /// Where a store through `reference` copies `place`: the destination's bytes, when they lie in
    /// the block that holds the place (the persistent block, or the activation stack for the staged
    /// result of an initializer evaluated in place, C1-13) and hold the place's type, as `declared`
    /// must too when the store normalizes to a declared type. A destination of another type is given
    /// the value instead, so that its normalization decides, as it decides for any value.
    pub(super) fn place_destination(
        &self,
        reference: ValueRefView<'_>,
        place: Place,
        declared: Option<u32>,
    ) -> Option<PlaceDestination> {
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return None;
        };
        let same = |other: u32| same_type(self.prepared, other, place.type_id, 0);
        let (range, type_id, presence, node, physical) =
            match reader.place_target(reference.location, reference.offset, reference.path)? {
                PlaceTarget::Cells {
                    range,
                    type_id,
                    presence,
                    beside,
                } => (range, type_id, presence, None, beside),
                // A structure nested in a backing takes a structure of its type where its bits lie,
                // written as the value store's physical patch writes it (C1-15b).
                PlaceTarget::Node {
                    range: root,
                    root_type,
                    anchor,
                    depth,
                    presence,
                } => {
                    let (range, node_type) =
                        self.nested_cells(reader.view_cells(), root, root_type, anchor, depth)?;
                    let node = PlaceNode {
                        root,
                        root_type,
                        anchor,
                        depth,
                    };
                    (range, node_type, presence, Some(node), true)
                }
            };
        (range.region == place.range.region
            && range.bytes == place.range.bytes
            && same(type_id)
            && declared.is_none_or(same))
        .then_some(PlaceDestination {
            range,
            presence,
            node,
            physical,
        })
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
        self.charge_texts(
            (reader, None),
            place.range,
            place.type_id,
            TextRule::Normalization {
                in_structure: false,
            },
            0,
        )
    }

    /// The checks `check_value` makes of the value of `type_id` that a construction in place wrote
    /// at `range`, charged as it charges them for that value (TIME-08; C1-13). A constructed value is
    /// canonical for its type, so the check charges each of its texts once, one unit per byte, in
    /// the order the check reaches them; then the lifetime check visits the value.
    pub(super) fn check_constructed_place(
        &self,
        range: ByteRange,
        type_id: u32,
    ) -> Result<(), RuntimeError> {
        if holds_text(self.prepared, type_id) {
            let Routed::Cells(reader) = self.data.routed(self.prepared) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            self.charge_texts((reader, None), range, type_id, TextRule::Canonical, 0)?;
        }
        self.check_place_lifetime(type_id)
    }

    /// The bytes of element `index` of the array of `type_id` whose bytes are `range`.
    pub(super) fn element_range(
        &self,
        range: ByteRange,
        type_id: u32,
        index: usize,
    ) -> Result<ByteRange, RuntimeError> {
        let Some(TypeData::Array { elem_type_id, .. }) =
            resolve(self.prepared, type_id).map(|entry| &entry.data)
        else {
            return Err(RuntimeError::TypeMismatch);
        };
        let stride = self
            .prepared
            .type_layout(type_id)
            .and_then(|layout| layout.array.as_ref())
            .map(|array| array.byte_stride)
            .ok_or(RuntimeError::TypeMismatch)?;
        let offset = u32::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(stride))
            .and_then(|offset| range.start.checked_add(offset))
            .ok_or(RuntimeError::Overflow)?;
        Ok(ByteRange {
            start: offset,
            bytes: self.type_bytes(*elem_type_id)?,
            ..range
        })
    }

    /// The bytes of member `ordinal` of the structure of `type_id` whose bytes are `range`.
    pub(super) fn member_range(
        &self,
        range: ByteRange,
        type_id: u32,
        ordinal: usize,
    ) -> Result<ByteRange, RuntimeError> {
        let Some(TypeData::Struct { fields } | TypeData::Union { fields }) =
            resolve(self.prepared, type_id).map(|entry| &entry.data)
        else {
            return Err(RuntimeError::TypeMismatch);
        };
        let field = fields.get(ordinal).ok_or(RuntimeError::TypeMismatch)?;
        let member = self
            .prepared
            .type_layout(type_id)
            .and_then(|layout| layout.members.get(ordinal))
            .ok_or(RuntimeError::TypeMismatch)?;
        Ok(ByteRange {
            start: range
                .start
                .checked_add(member.byte_offset)
                .ok_or(RuntimeError::Overflow)?,
            bytes: self.type_bytes(field.type_id)?,
            ..range
        })
    }

    /// The bytes one value of `type_id` takes in a flat block.
    fn type_bytes(&self, type_id: u32) -> Result<u32, RuntimeError> {
        self.prepared
            .type_layout(type_id)
            .map(|layout| layout.shape.bytes)
            .ok_or(RuntimeError::TypeMismatch)
    }

    /// The charges of the texts in the value of `type_id` at `range` under `rule`, each text in the
    /// order a walk of the value reaches it: its elements and its members in order. `walk` holds the
    /// store's reader and, when the walk reads a copy a call made of a place, the copy, whose first
    /// byte is the walk's offset 0 (C1-19b).
    fn charge_texts(
        &self,
        walk: (CellReader<'_>, Option<&[u8]>),
        range: ByteRange,
        type_id: u32,
        rule: TextRule,
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
                let length = match walk {
                    (reader, None) => reader.place_view(range, type_id).text_utf8_len()?,
                    (reader, Some(copy)) => reader
                        .view_cells()
                        .copied_text_utf8_len(copy, range, type_id)?,
                }
                .ok_or(RuntimeError::InvalidExecutionState)?;
                let units = match rule {
                    TextRule::Normalization {
                        in_structure: false,
                    } => length.checked_mul(2).ok_or(RuntimeError::Overflow)?,
                    TextRule::Normalization { in_structure: true } | TextRule::Canonical => length,
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
                    self.charge_texts(walk, element, *elem_type_id, rule, depth + 1)?;
                }
                Ok(())
            }
            // A structure with a physical layout holds its texts beside its backing (C1-15b).
            TypeData::Struct { .. } if layout.overlay.is_some() => {
                self.charge_slot_texts(walk, range, type_id, rule, depth)
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => {
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
                    let rule = match rule {
                        TextRule::Normalization { .. } => {
                            TextRule::Normalization { in_structure: true }
                        }
                        TextRule::Canonical => TextRule::Canonical,
                    };
                    self.charge_texts(walk, member, field.type_id, rule, depth + 1)?;
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
        let (range, type_id, presence, beside) = match self.data.routed(self.prepared) {
            Routed::Cells(reader) => {
                match reader.place_target(reference.location, reference.offset, reference.path)? {
                    PlaceTarget::Cells {
                        range,
                        type_id,
                        presence,
                        beside,
                    } => (range, type_id, presence, beside),
                    PlaceTarget::Node { .. } => return None,
                }
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
            // A text beside a backing is written as the value store's physical patch writes it,
            // after a sample of the deadline (C1-15b).
            if beside {
                self.check_entry_deadline()?;
            }
            Ok(match &mut self.data.store {
                EngineStore::Cells(arm) => arm.write_text(range, geometry, presence, &text[..kept]),
                EngineStore::Values => false,
            })
        })())
    }

    /// Copy the bytes of `place` to `destination`. A copy into a structure nested in a backing is
    /// charged and checked as the value store charges and checks the write of the place's value
    /// there, and a copy the value store makes as a physical patch samples the deadline first
    /// (C1-15b). `Ok(false)` when the block refuses the copy.
    pub(super) fn copy_place(
        &mut self,
        place: Place,
        destination: PlaceDestination,
    ) -> Result<bool, RuntimeError> {
        if let Some(node) = destination.node {
            self.charge_and_check_node_copy(place, destination.range, node)?;
        }
        if destination.physical {
            self.check_entry_deadline()?;
        }
        Ok(match &mut self.data.store {
            EngineStore::Cells(arm) => {
                arm.copy_place(place.range, destination.range, destination.presence)
            }
            EngineStore::Values => false,
        })
    }

    /// The lifetime check of the value of `type_id` a place stands for, as `check_value_lifetime`
    /// checks that value (TIME-08 LIFETIME). The value holds no reference and no instance, so the
    /// check finds nothing and only visits it: an array of scalars is admitted without a charge, and
    /// any other value is visited node by node, one unit each, as the `Value` walk visits it.
    pub(super) fn check_place_lifetime(&self, type_id: u32) -> Result<(), RuntimeError> {
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
            Some(TypeData::Struct { fields } | TypeData::Union { fields }) => {
                for field in fields {
                    self.visit_place_node(field.type_id, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// How a walk over a value's texts charges each text (TIME-08).
#[derive(Clone, Copy)]
enum TextRule {
    /// Normalizing the value to its type checks each text against its capacity: one unit for each
    /// byte of a text in a structure and two for each byte of any other (C1-12).
    Normalization { in_structure: bool },
    /// The canonical check of a constructed value checks each text once: one unit for each byte
    /// (C1-13).
    Canonical,
}
