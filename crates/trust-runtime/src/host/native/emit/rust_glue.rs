//! The bodies of Rust POUs (specification 48 RU-28): the generated function of a Rust function
//! block reads the instance's variables from their cells, calls its Rust item with them in
//! declaration order and writes them back when the item returns `Ok`; the generated function of a
//! Rust function calls the Rust function with its inputs and returns its result. A fault the item
//! returns ends the scan as a fault of Structured Text does (RU-14).
//!
//! The call gets the SDK's own context, made from a clone of the generated code's, which takes back
//! only the record of the fault the call returns: a Rust POU reaches neither the statement budget,
//! the call depth nor the kernel's pointers (RU-11).
//!
//! A value goes through the SDK's form of its type (RU-25): a scalar through `trust_plc::Stored`,
//! a `TIME` read as `Time`, a `BYTE` as `Byte`; a string, or an array of such values, through
//! `trust_plc::Placed`, from the cells it lies in; a structure of the project's crate, or an array
//! that holds one, field by field and element by element, built by the structure's path.

use std::collections::HashMap;

use super::super::lower::{Lowerer, Place};
use super::super::rust_pous::RustLink;
use super::super::types::{Kind, Ty};
use super::super::NativeError;
use crate::program_model::Param;
use trust_hir::symbols::ParamDirection;

/// The body of the Rust function block whose instance type is `ty`, called through `link`;
/// `structures` gives the path of each structure of the crate by its name in capitals.
pub(super) fn block(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    link: &RustLink,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    let members = lowerer.info(ty).members().to_vec();
    let mut places = Vec::with_capacity(link.variables.len());
    for name in &link.variables {
        let Some(member) = members
            .iter()
            .find(|member| member.via.is_empty() && member.name.eq_ignore_ascii_case(name))
        else {
            return Err(NativeError::Compile(format!(
                "the variable {name} of {} is not in its declaration",
                link.call
            )));
        };
        places.push(Place {
            expr: format!("me.{}", member.field()),
            ty: member.ty,
            is_ref: false,
        });
    }
    let values: Vec<String> = (0..places.len())
        .map(|index| format!("rv{index}"))
        .collect();
    for (place, value) in places.iter().zip(&values) {
        let loaded = load(lowerer, place.ty, &place.expr, structures, 0)?;
        lowerer.line(&format!("let mut {value} = {loaded};"));
    }
    let arguments: String = values
        .iter()
        .map(|value| format!(", &mut {value}"))
        .collect();
    let called = call_rust(lowerer, |cx| {
        format!("{}::__trust_call({cx}{arguments})", link.call)
    });
    lowerer.line(&format!("{called}?;"));
    // The instance's own variables, which no pointer names: a string's length and the 0 after its
    // text as Structured Text's writes leave them (DEV-039).
    for (place, value) in places.iter().zip(&values) {
        store(lowerer, place.ty, &place.expr, value, structures, 0)?;
    }
    Ok(())
}

/// The body of the Rust function with the parameters `params`, called through `link`: its inputs
/// in declaration order, its result into the function's result.
pub(super) fn function(
    lowerer: &mut Lowerer<'_, '_>,
    params: &[Param],
    link: &RustLink,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    let mut arguments = String::new();
    for name in &link.variables {
        let Some(param) = params.iter().find(|param| {
            param.direction == ParamDirection::In && param.name.eq_ignore_ascii_case(name)
        }) else {
            return Err(NativeError::Compile(format!(
                "the input {name} of {} is not in its declaration",
                link.call
            )));
        };
        let ty = lowerer.model.ty(param.type_id)?;
        let argument = crate::native::lower::parameter(&param.name);
        // A scalar input is its value; any other is a reference to the caller's (specification 44
        // §6.2).
        let value = if lowerer.info(ty).scalar().is_some() {
            format!("::trust_plc::Stored::from_raw({argument})")
        } else {
            load(lowerer, ty, &format!("(*{argument})"), structures, 0)?
        };
        arguments.push_str(&format!(", {value}"));
    }
    let called = call_rust(lowerer, |cx| format!("{}({cx}{arguments})", link.call));
    let call = format!("{called}?");
    match lowerer.result.clone() {
        // A result that is not a scalar goes into the function's result variable, which its exit
        // copies out.
        Some(result) if lowerer.info(result.ty).scalar().is_none() => {
            let value = lowerer.bind(&call);
            store(lowerer, result.ty, &result.expr, &value, structures, 0)?;
        }
        Some(result) => {
            let value = lowerer.bind(&format!("::trust_plc::Stored::into_raw({call})"));
            lowerer.store_scalar(&result, &value);
        }
        None => lowerer.line(&format!("{call};")),
    }
    Ok(())
}

/// Calls a Rust POU with `call`, which writes the call given the expression of its context: the
/// SDK's context, made from a clone of the generated code's `cx`, from which only the record of the
/// fault the call returns goes back (specification 48 RU-11). Returns the temporary that holds the
/// call's `Result`.
fn call_rust(lowerer: &mut Lowerer<'_, '_>, call: impl FnOnce(&str) -> String) -> String {
    let context = lowerer.temp();
    lowerer.line(&format!(
        "let mut {context} = ::trust_plc::__private::enter(cx);"
    ));
    let called = lowerer.bind(&call(&format!("&mut {context}")));
    lowerer.line(&format!("::trust_plc::__private::leave({context}, cx);"));
    called
}

/// Whether the SDK reads and writes a value of `ty` as it lies (`trust_plc::Placed`): a string, or
/// an array of scalars or of such values, no structure in it.
fn placed_by_sdk(lowerer: &Lowerer<'_, '_>, ty: Ty) -> bool {
    match &lowerer.info(ty).kind {
        Kind::Str(_) | Kind::WStr(_) => true,
        Kind::Array { elem, .. } => {
            let elem = *elem;
            lowerer.info(elem).scalar().is_some() || placed_by_sdk(lowerer, elem)
        }
        _ => false,
    }
}

/// The Rust expression of the value of type `ty` that the place `place` holds, an expression of
/// the generated code's type; `depth` names the closures' variables apart.
fn load(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    place: &str,
    structures: &HashMap<String, String>,
    depth: usize,
) -> Result<String, NativeError> {
    let info = lowerer.info(ty).clone();
    if info.scalar().is_some() {
        let loaded = lowerer.load(&Place {
            expr: place.to_string(),
            ty,
            is_ref: false,
        });
        return Ok(format!("::trust_plc::Stored::from_raw({loaded})"));
    }
    if placed_by_sdk(lowerer, ty) {
        return Ok(format!("::trust_plc::Placed::load(&{place})"));
    }
    match &info.kind {
        Kind::Array { elem, .. } => {
            let cell = format!("e{depth}");
            let item = load(lowerer, *elem, &format!("(*{cell})"), structures, depth + 1)?;
            Ok(format!(
                "::trust_plc::Array::from_items({place}.each_ref().map(|{cell}| {item}))"
            ))
        }
        Kind::Struct { members } => {
            let path = structure_path(structures, &info.name)?;
            let mut fields = Vec::with_capacity(members.len());
            for member in members {
                let value = load(
                    lowerer,
                    member.ty,
                    &format!("{place}.{}", member.field()),
                    structures,
                    depth,
                )?;
                fields.push(format!("{}: {value}", member.name));
            }
            Ok(format!("{path} {{ {} }}", fields.join(", ")))
        }
        _ => Err(not_carried(&info.name)),
    }
}

/// Writes the value `value` of type `ty`, a Rust expression, into the place `place`; `depth` names
/// the loops' variables apart.
fn store(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    place: &str,
    value: &str,
    structures: &HashMap<String, String>,
    depth: usize,
) -> Result<(), NativeError> {
    let info = lowerer.info(ty).clone();
    if info.scalar().is_some() {
        let target = Place {
            expr: place.to_string(),
            ty,
            is_ref: false,
        };
        lowerer.store_scalar(&target, &format!("::trust_plc::Stored::into_raw({value})"));
        return Ok(());
    }
    if placed_by_sdk(lowerer, ty) {
        lowerer.line(&format!("::trust_plc::Placed::store(&{value}, &{place});"));
        return Ok(());
    }
    match &info.kind {
        Kind::Array { elem, .. } => {
            let (item, cell) = (format!("v{depth}"), format!("e{depth}"));
            lowerer.line(&format!(
                "for ({item}, {cell}) in {value}.iter().zip({place}.iter()) {{"
            ));
            store(
                lowerer,
                *elem,
                &format!("(*{cell})"),
                &format!("(*{item})"),
                structures,
                depth + 1,
            )?;
            lowerer.line("}");
            Ok(())
        }
        Kind::Struct { members } => {
            structure_path(structures, &info.name)?;
            for member in members {
                store(
                    lowerer,
                    member.ty,
                    &format!("{place}.{}", member.field()),
                    &format!("{value}.{}", member.name),
                    structures,
                    depth,
                )?;
            }
            Ok(())
        }
        _ => Err(not_carried(&info.name)),
    }
}

/// The path the generated code builds the crate's structure `name` by: its full name's, else, for
/// a type the registry names without its namespaces, the one structure of that own name.
fn structure_path<'s>(
    structures: &'s HashMap<String, String>,
    name: &str,
) -> Result<&'s str, NativeError> {
    let upper = name.to_ascii_uppercase();
    if let Some(path) = structures.get(&upper) {
        return Ok(path);
    }
    let suffix = format!(".{upper}");
    let mut matching = structures
        .iter()
        .filter(|(full, _)| full.ends_with(&suffix))
        .map(|(_, path)| path.as_str());
    match (matching.next(), matching.next()) {
        (Some(path), None) => Ok(path),
        _ => Err(not_carried(name)),
    }
}

/// The refusal of a Rust POU's variable whose type the SDK does not carry.
fn not_carried(name: &str) -> NativeError {
    NativeError::unsupported(format!(
        "a value of the type {name} in a Rust POU's variable, which is no type of specification 48 \
         RU-25 nor a structure of the project's crate"
    ))
}
