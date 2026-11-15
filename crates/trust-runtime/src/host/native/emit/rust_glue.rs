//! The bodies of Rust POUs (specification 48 RU-28): the generated function of a Rust function
//! block reads the instance's variables from their cells, calls its Rust item with them in
//! declaration order and writes them back when the item returns `Ok`; the generated function of a
//! Rust function calls the Rust function with its inputs and returns its result. A fault the item
//! returns ends the scan as a fault of Structured Text does (RU-14).

use super::super::lower::{Lowerer, Place};
use super::super::rust_pous::RustLink;
use super::super::types::{Kind, Ty};
use super::super::NativeError;
use crate::program_model::Param;
use trust_hir::symbols::ParamDirection;

/// The body of the Rust function block whose instance type is `ty`, called through `link`.
pub(super) fn block(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    link: &RustLink,
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
        let placed = is_placed(lowerer, member.ty);
        if lowerer.info(member.ty).scalar().is_none() && !placed {
            return Err(NativeError::unsupported(format!(
                "the variable {name} of the Rust POU {}, whose type is not elementary",
                link.call
            )));
        }
        places.push((
            Place {
                expr: format!("me.{}", member.field()),
                ty: member.ty,
                is_ref: false,
            },
            placed,
        ));
    }
    let values: Vec<String> = (0..places.len())
        .map(|index| format!("rv{index}"))
        .collect();
    // Each value through the SDK's form of its type: a scalar's (`trust_plc::Stored`), a `TIME`
    // read as `Time`, a `BYTE` as `Byte`; a string's or an array's (`trust_plc::Placed`), read from
    // the cells it lies in (specification 48 RU-25).
    for ((place, placed), value) in places.iter().zip(&values) {
        if *placed {
            lowerer.line(&format!(
                "let mut {value} = ::trust_plc::Placed::load(&{});",
                place.expr
            ));
        } else {
            let loaded = lowerer.load(place);
            lowerer.line(&format!(
                "let mut {value} = ::trust_plc::Stored::from_raw({loaded});"
            ));
        }
    }
    let arguments: String = values
        .iter()
        .map(|value| format!(", &mut {value}"))
        .collect();
    lowerer.line(&format!("{}::__trust_call(cx{arguments})?;", link.call));
    for ((place, placed), value) in places.iter().zip(&values) {
        if *placed {
            // The instance's own string or array, which no pointer names: a string's length and the
            // 0 after its text (DEV-039).
            lowerer.line(&format!(
                "::trust_plc::Placed::store(&{value}, &{});",
                place.expr
            ));
        } else {
            lowerer.store_scalar(place, &format!("::trust_plc::Stored::into_raw({value})"));
        }
    }
    Ok(())
}

/// Whether `ty` lies in more than one cell, a `STRING[n]`, a `WSTRING[n]` or an array, which the
/// SDK reads and writes as it lies in its cells (`trust_plc::Placed`).
fn is_placed(lowerer: &Lowerer<'_, '_>, ty: Ty) -> bool {
    matches!(
        lowerer.info(ty).kind,
        Kind::Str(_) | Kind::WStr(_) | Kind::Array { .. }
    )
}

/// The body of the Rust function with the parameters `params`, called through `link`: its inputs
/// in declaration order, its result into the function's result.
pub(super) fn function(
    lowerer: &mut Lowerer<'_, '_>,
    params: &[Param],
    link: &RustLink,
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
        // A string or an array input is a reference to the caller's (specification 44 §6.2).
        let form = if is_placed(lowerer, ty) {
            "::trust_plc::Placed::load"
        } else {
            "::trust_plc::Stored::from_raw"
        };
        arguments.push_str(&format!(
            ", {form}({})",
            crate::native::lower::parameter(&param.name)
        ));
    }
    let call = format!("{}(cx{arguments})?", link.call);
    match lowerer.result.clone() {
        // A string or an array result goes into the function's result variable, which its exit
        // copies out.
        Some(result) if is_placed(lowerer, result.ty) => {
            let value = lowerer.bind(&call);
            lowerer.line(&format!(
                "::trust_plc::Placed::store(&{value}, &{});",
                result.expr
            ));
        }
        Some(result) => {
            let value = lowerer.bind(&format!("::trust_plc::Stored::into_raw({call})"));
            lowerer.store_scalar(&result, &value);
        }
        None => lowerer.line(&format!("{call};")),
    }
    Ok(())
}
