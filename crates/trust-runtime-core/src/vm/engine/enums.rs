//! Enumerations as their type and number on the flat store (specification 34 MEM-12, ARCH-05;
//! C1-19a).
//!
//! A value of an enumeration holds its type's name and its variant's name, so building one for each
//! load allocates. On the flat store an enumeration leaf holds only its number, so a load of it, and a
//! load of an enumeration constant, push the enumeration as its type and number; a store writes the
//! number into the destination's leaf, and `=` and `<>` compare type and number, as the values
//! compare. Each step is checked and charged as the value path checks and charges it (TIME-08), and an
//! enumeration that any other instruction pops is given its value first.

use super::*;
use crate::bytecode::{EnumVariant, TypeData, TypeEntry};
use crate::memory::cell_access::PlaceTarget;
use crate::memory::view::ValueView;
use crate::value::ValueRefView;
use crate::vm::EnumTemp;

impl EngineState<'_> {
    /// The operand a load of the enumeration leaf `view` of the flat store pushes. `None` for any
    /// other view, and for a leaf whose number no variant has, which a read of the value refuses.
    pub(super) fn enum_operand(&self, view: &ValueView<'_>) -> Option<EnumTemp> {
        let (type_id, value) = match *view {
            ValueView::Cells {
                cells,
                range,
                type_id,
            } => crate::memory::cells::enumeration_number(
                cells.bytes(range)?,
                type_id,
                self.prepared,
            )?,
            // An enumeration member of an overlapping structure is read from its bits (C1-15).
            ValueView::Bits {
                cells,
                range,
                type_id,
                leaf,
                depth,
            } => cells.overlay_enum_number(range, type_id, leaf, depth)?,
            ValueView::Value(_) | ValueView::Instance(_) => return None,
        };
        Some(EnumTemp { type_id, value })
    }

    /// The operand a load of the enumeration constant `value` pushes on the flat store: its type,
    /// found by the name the value holds, and its number.
    pub(super) fn enum_constant(&self, value: &Value) -> Option<EnumTemp> {
        let Value::Enum(enumeration) = value else {
            return None;
        };
        let types = &self.prepared.vm.types.entries;
        let type_id = types.iter().position(|entry| {
            matches!(entry.data, TypeData::Enum { .. })
                && self.type_name(entry) == Some(enumeration.type_name().as_str())
        })?;
        Some(EnumTemp {
            type_id: u32::try_from(type_id).ok()?,
            value: enumeration.numeric_value(),
        })
    }

    /// The value the enumeration operand `temp` stands for, as a read of its leaf gives it.
    pub(super) fn enum_value(&self, temp: EnumTemp) -> Result<Value, RuntimeError> {
        crate::memory::cells::enumeration_value(temp.type_id, temp.value, self.prepared)
    }

    /// Whether the enumeration operands `left` and `right` are equal, as their values are: the same
    /// type name and the same number.
    pub(super) fn enums_equal(&self, left: EnumTemp, right: EnumTemp) -> bool {
        left.value == right.value
            && (left.type_id == right.type_id
                || self.enum_type_name(left.type_id) == self.enum_type_name(right.type_id))
    }

    /// Charge a copy of an enumeration as `before_value_clone` charges a copy of its value: one unit
    /// and no allocation (TIME-08).
    pub(super) fn charge_enum_copy(&self) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        self.charge_allocation_bytes(0)
    }

    /// Store the enumeration `temp` through `reference` as a store of its value does: admitted by
    /// `declared`, the type the store normalizes to when it has one, and by the destination's leaf,
    /// as the value is (neither check charges), checked as a write, and written into the leaf as its
    /// number, a leaf beside a backing after a sample of the deadline (C1-15b). `None` when the
    /// destination is no enumeration leaf of the flat store that admits it, so the store is given
    /// the value instead; `Some(Ok(false))` when the block refuses the write.
    pub(super) fn store_enum(
        &mut self,
        reference: ValueRefView<'_>,
        temp: EnumTemp,
        declared: Option<u32>,
    ) -> Option<Result<bool, RuntimeError>> {
        if declared.is_some_and(|declared| !self.enum_admitted(declared, temp)) {
            return None;
        }
        let reader = self.data.reader(self.prepared);
        let (range, type_id, presence, beside) =
            match reader.place_target(reference.location, reference.offset, reference.path)? {
                PlaceTarget::Cells {
                    range,
                    type_id,
                    presence,
                    beside,
                } => (range, type_id, presence, beside),
                PlaceTarget::Node {
                    range,
                    root_type,
                    anchor,
                    depth,
                    ..
                } => {
                    return Some(self.store_enum_leaf(
                        reference,
                        temp,
                        range,
                        root_type,
                        (anchor, depth),
                    ))
                }
            };
        if !self.enum_admitted(type_id, temp) {
            return None;
        }
        Some((|| {
            // An enumeration holds no reference, so the write's checks are its target's (TIME-08).
            self.check_write_target(reference)?;
            if beside {
                self.check_entry_deadline()?;
            }
            self.data
                .store
                .write_enum(range, presence, type_id, temp.value, self.prepared)
        })())
    }

    /// Store the enumeration `temp` into the enumeration leaf at the node `node` of the overlapping
    /// structure `root_type` whose cells are `range`, as the value store writes its value there: the
    /// write's checks, the physical layout's write cost of its path, the leaf and each leaf that
    /// shares a bit with it checked, the deadline sampled, then the bits written (C1-15b).
    fn store_enum_leaf(
        &mut self,
        reference: ValueRefView<'_>,
        temp: EnumTemp,
        range: crate::memory::layout::ByteRange,
        root_type: u32,
        node: (u32, u8),
    ) -> Result<bool, RuntimeError> {
        self.check_write_target(reference)?;
        // The charge of a leaf's write is its path's, whatever value it writes.
        self.charge_storage_path_write(
            &*self,
            reference.location,
            reference.offset,
            reference.path,
            &Value::Null,
        )?;
        let variant = (
            self.enum_type_name(temp.type_id)
                .ok_or(RuntimeError::InvalidExecutionState)?,
            self.variant_name(temp)
                .ok_or(RuntimeError::InvalidExecutionState)?,
            temp.value,
        );
        let patch = {
            let reader = self.data.reader(self.prepared);
            reader
                .view_cells()
                .enum_leaf_patch(range, root_type, node, variant)?
                .ok_or(RuntimeError::TypeMismatch)?
        };
        self.check_entry_deadline()?;
        Ok(self.data.store.write_leaf_patch(&patch))
    }

    /// Whether a value of `type_id` takes the enumeration `temp`, as `validate_assignment_enum` and
    /// the flat store's check take its value: an enumeration of the same name, ignoring case, with a
    /// variant of the same number and name.
    fn enum_admitted(&self, type_id: u32, temp: EnumTemp) -> bool {
        let Some(enum_type) =
            crate::vm::type_policy::resolved_alias_type(&self.prepared.vm.types, type_id, 0)
        else {
            return false;
        };
        if enum_type == temp.type_id {
            return true;
        }
        let (Some(name), Some(own)) = (
            self.enum_type_name(enum_type),
            self.enum_type_name(temp.type_id),
        ) else {
            return false;
        };
        let Some(variant) = self.variant_name(temp) else {
            return false;
        };
        name.eq_ignore_ascii_case(own)
            && self.enum_variants(enum_type).is_some_and(|variants| {
                variants.iter().any(|candidate| {
                    candidate.value == temp.value
                        && self
                            .prepared
                            .vm
                            .strings
                            .get(candidate.name_idx as usize)
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(variant))
                })
            })
    }

    /// The name of the enumeration type `type_id`.
    fn enum_type_name(&self, type_id: u32) -> Option<&str> {
        let entry = self.prepared.vm.types.entries.get(type_id as usize)?;
        matches!(entry.data, TypeData::Enum { .. })
            .then(|| self.type_name(entry))
            .flatten()
    }

    /// The name of the variant of the operand `temp`.
    fn variant_name(&self, temp: EnumTemp) -> Option<&str> {
        let variant = self
            .enum_variants(temp.type_id)?
            .iter()
            .find(|variant| variant.value == temp.value)?;
        self.prepared.vm.strings.get(variant.name_idx as usize)
    }

    /// The variants of the enumeration type `type_id`.
    fn enum_variants(&self, type_id: u32) -> Option<&[EnumVariant]> {
        match &self.prepared.vm.types.entries.get(type_id as usize)?.data {
            TypeData::Enum { variants, .. } => Some(variants),
            _ => None,
        }
    }

    /// The name of the type `entry`.
    fn type_name(&self, entry: &TypeEntry) -> Option<&str> {
        entry
            .name_idx
            .and_then(|name| self.prepared.vm.strings.get(name as usize))
    }
}
