//! The types of the project's crate (specification 48 RU-25): its enumerations, and the Rust POUs'
//! variables that name one. A POU's attribute sees only its own item, so its reader keeps such a
//! variable's type by its Rust name; this gives the variable the type's Structured Text name, which
//! the generated declarations write, and checks that a variant it starts at is one of the type's.

use trust_plc_decl::{DataType, Enumeration, Value};

use super::{RustPou, RustPouError};

/// An enumeration of the project's crate, where it lies.
#[derive(Debug, Clone, PartialEq)]
pub struct RustEnumeration {
    /// The enumeration as its attribute declares it.
    pub enumeration: Enumeration,
    /// The file its item lies in, relative to the project with `/`.
    pub file: String,
    /// The line of its item in that file, 1-based.
    pub line: usize,
}

/// Gives each variable and result of `pous` of a type the crate declares that type's Structured
/// Text name, and checks the variants they start at.
///
/// # Errors
///
/// A variable or a result of a type that no enumeration of the crate is, and a variable that starts
/// at a variant its type does not have, each at its POU.
pub(super) fn resolve(pous: &mut [RustPou], enumerations: &[RustEnumeration]) -> Vec<RustPouError> {
    let mut errors = Vec::new();
    for pou in pous.iter_mut() {
        let mut found = Vec::new();
        for variable in &mut pou.pou.variables {
            let outcome = rename(&mut variable.ty, enumerations)
                .and_then(|()| check(&variable.ty, variable.init.as_ref(), enumerations));
            if let Err(message) = outcome {
                found.push(format!("the variable {}: {message}", variable.name));
            }
        }
        if let Some(result) = &mut pou.pou.result
            && let Err(message) = rename(result, enumerations)
        {
            found.push(format!("the result: {message}"));
        }
        errors.extend(found.into_iter().map(|message| RustPouError {
            file: pou.file.clone(),
            line: pou.line,
            column: 1,
            message: format!("{}: {message} (specification 48 RU-25)", pou.pou.name),
        }));
    }
    errors
}

/// Gives the types `ty` names by their Rust names their Structured Text names.
fn rename(ty: &mut DataType, enumerations: &[RustEnumeration]) -> Result<(), String> {
    match ty {
        DataType::Elementary(_) => Ok(()),
        DataType::Array { element, .. } => rename(element, enumerations),
        DataType::Named(rust) => {
            let enumeration = enumerations
                .iter()
                .find(|found| found.enumeration.item == *rust)
                .ok_or_else(|| {
                    format!(
                        "its type `{rust}` is no `#[trust_enum]` of the crate nor a type of RU-25's \
                         table"
                    )
                })?;
            rust.clone_from(&enumeration.enumeration.name);
            Ok(())
        }
    }
}

/// Checks that the initial value `init` of a variable of type `ty`, whose types bear their
/// Structured Text names, starts each value of an enumeration at one of its variants.
fn check(
    ty: &DataType,
    init: Option<&Value>,
    enumerations: &[RustEnumeration],
) -> Result<(), String> {
    match (ty, init) {
        (DataType::Array { element, .. }, Some(Value::List(items))) => items
            .iter()
            .try_for_each(|item| check(element, Some(item), enumerations)),
        (DataType::Named(name), Some(Value::Variant(path))) => {
            let variant = path.last().map_or("", String::as_str);
            let known = enumerations.iter().any(|found| {
                found.enumeration.name.eq_ignore_ascii_case(name)
                    && found.enumeration.has_variant(variant)
            });
            if known {
                Ok(())
            } else {
                Err(format!("`{variant}` is no value of {name}"))
            }
        }
        _ => Ok(()),
    }
}
