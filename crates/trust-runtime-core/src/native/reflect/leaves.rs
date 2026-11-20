//! The variables of a program that hold one elementary value, with their paths (specification 48
//! RU-62, a service's bindings): the leaves of the tree [`super::walk_instances`] walks the
//! instances of.

use alloc::format;

use super::{kind, roots, symbol_place, unaliased, Located, MAX_DEPTH};
use crate::error::RuntimeError;
use crate::native::image::{Image, MEMBER_HIDDEN};
use crate::native::values::Types;

/// Visits every variable of the program that holds one elementary value, with its path and place,
/// in declaration order: each program instance's own variables, then the globals, and in each the
/// members of structures, unions and function block, class and program instances, at any depth. An
/// array, a reference, a pointer and an interface hold no single value and are not visited, nor are
/// their elements. A variable or a member the program hides from tools (its `hide` mark) is left
/// out, and so is a member whose name begins with `__`, which is the compiler's own.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for tables that do not read.
pub fn walk_variables(
    image: &Image<'_>,
    visit: &mut dyn FnMut(&str, Located),
) -> Result<(), RuntimeError> {
    let types = Types::of(image)?;
    for (index, symbol) in roots(image)? {
        if symbol.hidden {
            continue;
        }
        let name = image
            .string(symbol.display)
            .ok_or_else(|| crate::native::values::damaged("string pool"))?;
        let (place, type_index) = symbol_place(image, index)?;
        leaf(&types, name, Located { place, type_index }, 0, visit)?;
    }
    Ok(())
}

fn leaf(
    types: &Types<'_>,
    path: &str,
    located: Located,
    depth: usize,
    visit: &mut dyn FnMut(&str, Located),
) -> Result<(), RuntimeError> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    let (_, record) = unaliased(types, located.type_index)?;
    match record.kind {
        kind::FUNCTION_BLOCK | kind::CLASS | kind::STRUCT | kind::UNION | kind::PROGRAM => {
            for detail in types.members(&record)? {
                let name = types.string(detail.name)?;
                if name.starts_with("__") || detail.flags & MEMBER_HIDDEN != 0 {
                    continue;
                }
                leaf(
                    types,
                    &format!("{path}.{name}"),
                    Located {
                        place: located.place.at(detail.offset),
                        type_index: detail.type_index,
                    },
                    depth + 1,
                    visit,
                )?;
            }
        }
        kind::ARRAY | kind::REFERENCE | kind::POINTER | kind::INTERFACE => {}
        _ => visit(path, located),
    }
    Ok(())
}
