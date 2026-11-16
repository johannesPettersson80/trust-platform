//! The types of the project's crate (specification 48 RU-25): its enumerations and structures, and
//! the Rust POUs' variables and the structures' fields that name one. A POU's attribute sees only its own item, so its reader keeps such a
//! variable's type by its Rust name; this gives the variable the type's Structured Text name, which
//! the generated declarations write, and checks that a variant it starts at is one of the type's.

use trust_plc_decl::{DataType, Enumeration, Section, Structure, Value};

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

/// A structure of the project's crate, where it lies and the path the generated code builds it by.
#[derive(Debug, Clone, PartialEq)]
pub struct RustStructure {
    /// The structure as its attribute declares it.
    pub structure: Structure,
    /// The file its item lies in, relative to the project with `/`.
    pub file: String,
    /// The line of its item in that file, 1-based.
    pub line: usize,
    /// The path the program's generated code names the item by from the crate's root.
    pub path: Vec<String>,
}

/// The crate's types, by their Rust names and their Structured Text names.
pub(super) struct Types<'a> {
    pub(super) enumerations: &'a [RustEnumeration],
    pub(super) structures: &'a [RustStructure],
}

impl Types<'_> {
    /// The Structured Text name of the crate's type of the Rust name `rust`.
    fn name_of(&self, rust: &str) -> Option<&str> {
        self.enumerations
            .iter()
            .find(|found| found.enumeration.item == rust)
            .map(|found| found.enumeration.name.as_str())
            .or_else(|| {
                self.structures
                    .iter()
                    .find(|found| found.structure.item == rust)
                    .map(|found| found.structure.name.as_str())
            })
    }

    /// The enumeration of the Structured Text name `name`.
    fn enumeration(&self, name: &str) -> Option<&Enumeration> {
        self.enumerations
            .iter()
            .map(|found| &found.enumeration)
            .find(|enumeration| enumeration.name.eq_ignore_ascii_case(name))
    }
}

/// Gives each variable and result of `pous` and of their methods, and each field of `structures`,
/// of a type the crate declares that type's Structured Text name, and checks the values they start
/// at.
///
/// # Errors
///
/// A variable, a result or a field of a type that is none of the crate's, a variable that starts at
/// a variant its type does not have, and a variable of a structure given an initial value, each
/// where its POU or its structure lies.
pub(super) fn resolve(
    pous: &mut [RustPou],
    enumerations: &[RustEnumeration],
    structures: &mut [RustStructure],
) -> Vec<RustPouError> {
    let mut errors = Vec::new();
    // The structures' own fields first, against the types as read, which renaming does not change.
    let snapshot: Vec<RustStructure> = structures.to_vec();
    let types = Types {
        enumerations,
        structures: &snapshot,
    };
    for structure in structures.iter_mut() {
        let mut found = Vec::new();
        for field in &mut structure.structure.fields {
            let outcome = rename(&mut field.ty, &types)
                .and_then(|()| check(&field.ty, field.init.as_ref(), &types))
                .and_then(|()| named(&field.ty));
            if let Err(message) = outcome {
                found.push(format!("the field {}: {message}", field.name));
            }
        }
        errors.extend(found.into_iter().map(|message| RustPouError {
            file: structure.file.clone(),
            line: structure.line,
            column: 1,
            message: format!(
                "{}: {message} (specification 48 RU-25)",
                structure.structure.name
            ),
        }));
    }
    for pou in pous.iter_mut() {
        let mut found = Vec::new();
        for variable in &mut pou.pou.variables {
            let outcome = rename(&mut variable.ty, &types)
                .and_then(|()| check(&variable.ty, variable.init.as_ref(), &types))
                .and_then(|()| carried(&variable.ty, variable.section));
            if let Err(message) = outcome {
                found.push(format!("the variable {}: {message}", variable.name));
            }
        }
        if let Some(result) = &mut pou.pou.result
            && let Err(message) = rename(result, &types)
        {
            found.push(format!("the result: {message}"));
        }
        // A method's variables and result as the block's (RU-29, RU-32).
        for method in &mut pou.pou.methods {
            for variable in &mut method.variables {
                let outcome = rename(&mut variable.ty, &types)
                    .and_then(|()| carried(&variable.ty, variable.section));
                if let Err(message) = outcome {
                    found.push(format!(
                        "the variable {} of the method {}: {message}",
                        variable.name, method.name
                    ));
                }
            }
            if let Some(result) = &mut method.result
                && let Err(message) = rename(result, &types)
            {
                found.push(format!(
                    "the result of the method {}: {message}",
                    method.name
                ));
            }
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
fn rename(ty: &mut DataType, types: &Types<'_>) -> Result<(), String> {
    match ty {
        // A held Structured Text function block bears its Structured Text name (RU-39).
        DataType::Elementary(_) | DataType::Held(_) => Ok(()),
        DataType::Array { element, .. } | DataType::Ref(element) => rename(element, types),
        DataType::Named(rust) => {
            let name = types.name_of(rust).ok_or_else(|| {
                format!(
                    "its type `{rust}` is no `#[trust_enum]` or `#[trust_struct]` of the crate nor \
                     a type of RU-25's table"
                )
            })?;
            *rust = name.to_string();
            Ok(())
        }
    }
}

/// Checks that the SDK carries a variable of type `ty` in `section` (specification 48 RU-29): an
/// in-out is the place of a type its variable has, so its variable is no reference, and each of its
/// references names a type ([`named`]).
fn carried(ty: &DataType, section: Section) -> Result<(), String> {
    if section == Section::InOut && is_reference(ty) {
        return Err(
            "an in-out of a reference is not carried yet: the SDK reaches an in-out's variable as \
             the type it has, and a reference's place does not hold the type it names"
                .to_string(),
        );
    }
    named(ty)
}

/// Checks that each reference in a value of type `ty` names a type, not a reference or an array of
/// them (specification 48 RU-29): the SDK reads a reference's variable as the type it names, and a
/// reference's place does not hold the type it names.
fn named(ty: &DataType) -> Result<(), String> {
    let mut targets = Vec::new();
    reference_targets(ty, &mut targets);
    if targets.iter().any(|target| is_reference(target)) {
        return Err(
            "a reference to a reference is not carried yet: the SDK reads a reference's variable \
             as the type it names, and a reference's place does not hold the type it names"
                .to_string(),
        );
    }
    Ok(())
}

/// Whether `ty` is a reference or an array of them.
fn is_reference(ty: &DataType) -> bool {
    match ty {
        DataType::Ref(_) => true,
        DataType::Array { element, .. } => is_reference(element),
        DataType::Elementary(_) | DataType::Named(_) | DataType::Held(_) => false,
    }
}

/// Adds to `targets` the type each reference in `ty` names, through arrays and references.
fn reference_targets<'t>(ty: &'t DataType, targets: &mut Vec<&'t DataType>) {
    match ty {
        DataType::Ref(target) => {
            targets.push(target);
            reference_targets(target, targets);
        }
        DataType::Array { element, .. } => reference_targets(element, targets),
        DataType::Elementary(_) | DataType::Named(_) | DataType::Held(_) => {}
    }
}

/// Checks that the initial value `init` of a variable of type `ty`, whose types bear their
/// Structured Text names, starts each value of an enumeration at one of its variants.
fn check(ty: &DataType, init: Option<&Value>, types: &Types<'_>) -> Result<(), String> {
    match (ty, init) {
        (DataType::Array { element, .. }, Some(Value::List(items))) => items
            .iter()
            .try_for_each(|item| check(element, Some(item), types)),
        (DataType::Named(name), Some(Value::Variant(path))) => {
            let variant = path.last().map_or("", String::as_str);
            match types.enumeration(name) {
                Some(enumeration) if enumeration.has_variant(variant) => Ok(()),
                Some(_) => Err(format!("`{variant}` is no value of {name}")),
                None => Err(format!(
                    "a variable of the structure {name} starts at its fields' initial values and \
                     takes no `#[init]`"
                )),
            }
        }
        _ => Ok(()),
    }
}
