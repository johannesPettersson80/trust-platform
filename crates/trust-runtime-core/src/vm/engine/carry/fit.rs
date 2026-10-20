//! The elementary value an online change keeps when a variable's declaration changed (owner,
//! 20 October 2026, "keep what fits"; specification 34 §11.3).
//!
//! The new type holds the running value when the value, converted to the new type and back to the
//! old one, comes back unchanged: an `INT` 5 becomes a `DINT` 5, a `DINT` 5 an `INT` 5, a `STRING[10]`
//! "abc" a `STRING[5]` "abc", while a `DINT` 70,000 does not fit an `INT`, an `LREAL` 0.1 does not fit
//! a `REAL` exactly and a text longer than the new length would be cut, so none of them is kept. A
//! subrange holds a value inside its bounds, and an enumeration keeps a value by its variant's name.
//! The conversion is the one construction gives an initializer of the new type
//! ([`scalar::coerce`](crate::vm::construction::values::scalar::coerce)), so a kept value is a value
//! the new declaration could have been initialized with.

use alloc::boxed::Box;

use smol_str::SmolStr;

use super::shape::{data, resolved};
use crate::bytecode::TypeData;
use crate::value::{EnumValue, Value};
use crate::vm::construction::values::scalar::{coerce, integer};
use crate::vm::PreparedModule;

/// The value of the candidate's elementary type `new` (a primitive, an enumeration or a subrange,
/// through aliases) that holds `value`, a value of the running program's type `old`, or `None` when
/// the new type does not hold it.
pub(super) fn elementary(
    before: &PreparedModule,
    old: u32,
    after: &PreparedModule,
    new: u32,
    value: &Value,
) -> Option<Value> {
    match data(after, new)? {
        TypeData::Subrange {
            base_type_id,
            lower,
            upper,
        } => {
            let kept = elementary(before, old, after, *base_type_id, value)?;
            let number = integer(&kept)?;
            (number >= i128::from(*lower) && number <= i128::from(*upper)).then_some(kept)
        }
        TypeData::Enum { variants, .. } => {
            let Value::Enum(running) = value else {
                return None;
            };
            let variant = variants.iter().find(|variant| {
                after
                    .vm
                    .strings
                    .get(variant.name_idx as usize)
                    .is_some_and(|name| name.eq_ignore_ascii_case(running.variant_name()))
            })?;
            let entry = after.vm.types.entries.get(resolved(after, new)? as usize)?;
            let type_name = SmolStr::new(after.vm.strings.get(entry.name_idx? as usize)?);
            let variant_name = SmolStr::new(after.vm.strings.get(variant.name_idx as usize)?);
            Some(Value::Enum(Box::new(EnumValue::from_canonical_parts(
                type_name,
                variant_name,
                variant.value,
            ))))
        }
        TypeData::Primitive {
            prim_id,
            max_length,
        } => {
            let (old_id, old_length) = primitive(before, old)?;
            let converted = coerce(*prim_id, *max_length, value.clone()).ok()?;
            let back = coerce(old_id, old_length, converted.clone()).ok()?;
            (back == *value).then_some(converted)
        }
        _ => None,
    }
}

/// Whether `type_id` of `prepared` is elementary: a primitive, an enumeration or a subrange.
pub(super) fn is_elementary(prepared: &PreparedModule, type_id: u32) -> bool {
    matches!(
        data(prepared, type_id),
        Some(TypeData::Primitive { .. } | TypeData::Enum { .. } | TypeData::Subrange { .. })
    )
}

/// The primitive encoding and declared length of `type_id`, a primitive or a subrange of one.
fn primitive(prepared: &PreparedModule, type_id: u32) -> Option<(u16, u16)> {
    match data(prepared, type_id)? {
        TypeData::Primitive {
            prim_id,
            max_length,
        } => Some((*prim_id, *max_length)),
        TypeData::Subrange { base_type_id, .. } => primitive(prepared, *base_type_id),
        _ => None,
    }
}
