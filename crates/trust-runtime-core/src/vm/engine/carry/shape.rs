//! Whether a variable's declared type is the same in the running program and in the candidate of an
//! online change (specification 34 §11.3), so that its value can be copied as bytes.
//!
//! Two programs have their own type tables, so a type is compared by what it is, not by its index:
//! a primitive by its encoding and declared length, an array by its bounds and element, a structure
//! or union by its fields' names and types in order (and the same physical layout), an enumeration by
//! its base and its variants' names and values, a subrange by its base and bounds, a reference by its
//! target. The names of structure and enumeration types do not matter: the bytes and what they mean
//! do. A function-block, class or interface value is a leaf that names an instance: its members
//! are carried one by one, so two such leaves compare as the same whatever their POUs.

use crate::bytecode::{TypeData, BYTECODE_MAX_TYPE_NESTING};
use crate::vm::PreparedModule;

/// Whether `old` of `before` and `new` of `after` have the same shape: the same bytes in a flat block,
/// leaf for leaf, and the same meaning for each byte.
pub(super) fn same(before: &PreparedModule, old: u32, after: &PreparedModule, new: u32) -> bool {
    same_at(before, old, after, new, 0)
}

fn same_at(
    before: &PreparedModule,
    old: u32,
    after: &PreparedModule,
    new: u32,
    depth: usize,
) -> bool {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    let (Some(old), Some(new)) = (resolved(before, old), resolved(after, new)) else {
        return false;
    };
    let (Some(old_entry), Some(new_entry)) = (
        before.vm.types.entries.get(old as usize),
        after.vm.types.entries.get(new as usize),
    ) else {
        return false;
    };
    match (&old_entry.data, &new_entry.data) {
        (
            TypeData::Primitive {
                prim_id: old_id,
                max_length: old_length,
            },
            TypeData::Primitive {
                prim_id: new_id,
                max_length: new_length,
            },
        ) => old_id == new_id && old_length == new_length,
        (
            TypeData::Array {
                elem_type_id: old_element,
                dims: old_dims,
            },
            TypeData::Array {
                elem_type_id: new_element,
                dims: new_dims,
            },
        ) => old_dims == new_dims && same_at(before, *old_element, after, *new_element, depth + 1),
        (TypeData::Struct { fields: old_fields }, TypeData::Struct { fields: new_fields })
        | (TypeData::Union { fields: old_fields }, TypeData::Union { fields: new_fields }) => {
            old_fields.len() == new_fields.len()
                && old_fields
                    .iter()
                    .zip(new_fields)
                    .all(|(old_field, new_field)| {
                        old_field.bit_offset == new_field.bit_offset
                            && same_name(before, old_field.name_idx, after, new_field.name_idx)
                            && same_at(
                                before,
                                old_field.type_id,
                                after,
                                new_field.type_id,
                                depth + 1,
                            )
                    })
                && same_physical_layout(before, old, after, new)
        }
        (
            TypeData::Enum {
                base_type_id: old_base,
                variants: old_variants,
            },
            TypeData::Enum {
                base_type_id: new_base,
                variants: new_variants,
            },
        ) => {
            same_at(before, *old_base, after, *new_base, depth + 1)
                && old_variants.len() == new_variants.len()
                && old_variants
                    .iter()
                    .zip(new_variants)
                    .all(|(old_variant, new_variant)| {
                        old_variant.value == new_variant.value
                            && same_name(before, old_variant.name_idx, after, new_variant.name_idx)
                    })
        }
        (
            TypeData::Subrange {
                base_type_id: old_base,
                lower: old_lower,
                upper: old_upper,
            },
            TypeData::Subrange {
                base_type_id: new_base,
                lower: new_lower,
                upper: new_upper,
            },
        ) => {
            old_lower == new_lower
                && old_upper == new_upper
                && same_at(before, *old_base, after, *new_base, depth + 1)
        }
        (
            TypeData::Reference {
                target_type_id: old_target,
            },
            TypeData::Reference {
                target_type_id: new_target,
            },
        ) => same_at(before, *old_target, after, *new_target, depth + 1),
        (TypeData::Pou { .. }, TypeData::Pou { .. })
        | (TypeData::Interface { .. }, TypeData::Interface { .. }) => true,
        _ => false,
    }
}

/// `type_id` of `prepared` with its aliases resolved.
pub(super) fn resolved(prepared: &PreparedModule, type_id: u32) -> Option<u32> {
    crate::vm::type_policy::resolved_alias_type(&prepared.vm.types, type_id, 0)
}

/// The data of `type_id` of `prepared`, with its aliases resolved.
pub(super) fn data(prepared: &PreparedModule, type_id: u32) -> Option<&TypeData> {
    let resolved = resolved(prepared, type_id)?;
    prepared
        .vm
        .types
        .entries
        .get(resolved as usize)
        .map(|entry| &entry.data)
}

/// Whether the strings `old` of `before` and `new` of `after` are one IEC name, which ignores ASCII
/// case.
pub(super) fn same_name(
    before: &PreparedModule,
    old: u32,
    after: &PreparedModule,
    new: u32,
) -> bool {
    match (
        before.vm.strings.get(old as usize),
        after.vm.strings.get(new as usize),
    ) {
        (Some(old), Some(new)) => old.eq_ignore_ascii_case(new),
        _ => false,
    }
}

/// Whether two structures lie in their blocks the same way: neither has a physical layout, or both
/// have layouts that hold the same storage.
fn same_physical_layout(
    before: &PreparedModule,
    old: u32,
    after: &PreparedModule,
    new: u32,
) -> bool {
    match (
        before.vm.physical_layout_ref(old),
        after.vm.physical_layout_ref(new),
    ) {
        (None, None) => true,
        (Some(old), Some(new)) => old.storage_compatible(new),
        _ => false,
    }
}
