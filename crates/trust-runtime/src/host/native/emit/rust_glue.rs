//! The bodies of Rust POUs (specification 48 RU-28): the generated function of a Rust function
//! block reads the instance's variables from their cells, calls its Rust item with them in
//! declaration order and writes them back when the item returns `Ok`; the generated function of a
//! Rust function calls the Rust function with its inputs and returns its result. A fault the item
//! returns ends the scan as a fault of Structured Text does (RU-14).

use super::super::lower::{Lowerer, Place};
use super::super::rust_pous::RustLink;
use super::super::types::Ty;
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
        if lowerer.info(member.ty).scalar().is_none() {
            return Err(NativeError::unsupported(format!(
                "the variable {name} of the Rust POU {}, whose type is not elementary",
                link.call
            )));
        }
        places.push(Place {
            expr: format!("me.{}", member.field()),
            ty: member.ty,
            is_ref: false,
        });
    }
    let values: Vec<String> = (0..places.len())
        .map(|index| format!("rv{index}"))
        .collect();
    // Each value through the SDK's form of its type (`trust_plc::Stored`): a `TIME` read as `Time`,
    // a `BYTE` as `Byte` (specification 48 RU-25).
    for (place, value) in places.iter().zip(&values) {
        let loaded = lowerer.load(place);
        lowerer.line(&format!(
            "let mut {value} = ::trust_plc::Stored::from_raw({loaded});"
        ));
    }
    let arguments: String = values
        .iter()
        .map(|value| format!(", &mut {value}"))
        .collect();
    lowerer.line(&format!("{}::__trust_call(cx{arguments})?;", link.call));
    for (place, value) in places.iter().zip(&values) {
        lowerer.store_scalar(place, &format!("::trust_plc::Stored::into_raw({value})"));
    }
    Ok(())
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
        arguments.push_str(&format!(
            ", ::trust_plc::Stored::from_raw({})",
            crate::native::lower::parameter(&param.name)
        ));
    }
    let call = format!("{}(cx{arguments})?", link.call);
    match lowerer.result.clone() {
        Some(result) => {
            let value = lowerer.bind(&format!("::trust_plc::Stored::into_raw({call})"));
            lowerer.store_scalar(&result, &value);
        }
        None => lowerer.line(&format!("{call};")),
    }
    Ok(())
}
