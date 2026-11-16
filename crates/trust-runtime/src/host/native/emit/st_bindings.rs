//! The bindings of layer 1 (specification 48 RU-39): for each Structured Text function that the
//! project's crate calls, a Rust function of the name the crate calls it by, which runs the
//! generated function on the kernel's context that the SDK's `Cx` keeps (RU-11, RU-38).

use std::fmt::Write as _;

use super::super::model::Model;
use super::super::rust_pous::StructuredTextUse;
use super::super::types::{mangle, Elem, Kind};
use super::super::NativeError;
use trust_hir::symbols::ParamDirection;
use trust_plc_decl::Elementary;

/// RU-25's type of the elementary type `elem`.
fn elementary(elem: Elem) -> Elementary {
    match elem {
        Elem::Bool => Elementary::Bool,
        Elem::SInt => Elementary::Sint,
        Elem::Int => Elementary::Int,
        Elem::DInt => Elementary::Dint,
        Elem::LInt => Elementary::Lint,
        Elem::USInt => Elementary::Usint,
        Elem::UInt => Elementary::Uint,
        Elem::UDInt => Elementary::Udint,
        Elem::ULInt => Elementary::Ulint,
        Elem::Real => Elementary::Real,
        Elem::LReal => Elementary::Lreal,
        Elem::Byte => Elementary::Byte,
        Elem::Word => Elementary::Word,
        Elem::DWord => Elementary::Dword,
        Elem::LWord => Elementary::Lword,
        Elem::Time => Elementary::Time,
        Elem::LTime => Elementary::LTime,
        Elem::Date => Elementary::Date,
        Elem::LDate => Elementary::LDate,
        Elem::Tod => Elementary::Tod,
        Elem::LTod => Elementary::LTod,
        Elem::Dt => Elementary::Dt,
        Elem::Ldt => Elementary::Ldt,
        Elem::Char => Elementary::Char,
        Elem::WChar => Elementary::WChar,
    }
}

/// How a binding passes a value of the elementary type `elem` (RU-25): the Rust type a Rust POU
/// writes for it, and whether that is the generated code's own; one of the SDK's types becomes the
/// generated code's through `trust_plc::Place` on a cell, as the Rust POUs' glue does.
fn passed(elem: Elem) -> (String, bool) {
    let rust = elementary(elem).rust();
    if rust == elem.rust() {
        (rust.to_string(), true)
    } else {
        (format!("trust_plc::{rust}"), false)
    }
}

/// The name of a binding's parameter for the Structured Text parameter `name`: the name in lower
/// case, with `_` after one that Rust keeps as a keyword or that the context's parameter takes.
fn parameter_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower == "cx" || syn::parse_str::<syn::Ident>(&lower).is_err() {
        format!("{lower}_")
    } else {
        lower
    }
}

/// The bindings of the Structured Text functions that `roots` name, each root a generated function
/// with the crate's call of it, once for each name the crate writes.
///
/// # Errors
///
/// [`NativeError::Compile`] for a function with a parameter that is no elementary input, or with a
/// result of no elementary type, which the bindings do not pass yet, naming the call.
pub(super) fn bindings(
    model: &mut Model<'_>,
    roots: &[(String, StructuredTextUse)],
) -> Result<String, NativeError> {
    let mut text = String::new();
    let mut written: Vec<&str> = Vec::new();
    for (_, used) in roots {
        if written.contains(&used.name.as_str()) {
            continue;
        }
        written.push(&used.name);
        let Some(def) = model.function(&used.name) else {
            return Err(NativeError::Compile(format!(
                "function {} is not declared",
                used.name
            )));
        };
        let unsupported = |what: String| {
            NativeError::Compile(format!(
                "st::{} at {}:{}: {} {what}, which a call from Rust does not pass yet \
                 (specification 48 RU-38, milestone R3)",
                used.name, used.file, used.line, def.name
            ))
        };
        let mut parameters = String::new();
        let mut fields = Vec::new();
        let mut values = Vec::new();
        for param in &def.params {
            // `EN` is TRUE whenever the function runs (specification 10 §7.5).
            if param.name.eq_ignore_ascii_case("EN") {
                continue;
            }
            let ty = model.ty(param.type_id)?;
            let elem = match model.types.info(ty).kind {
                Kind::Elem(elem) if param.direction == ParamDirection::In => Some(elem),
                _ => None,
            };
            let Some(elem) = elem else {
                return Err(unsupported(format!(
                    "has the parameter {} of type {}",
                    param.name,
                    model.types.info(ty).name
                )));
            };
            let name = parameter_name(&param.name);
            let (rust, own) = passed(elem);
            let _ = write!(parameters, ", {name}: {rust}");
            values.push(if own {
                name
            } else {
                format!(
                    "{{ let cell = core::cell::Cell::<{}>::default(); \
                     trust_plc::Place::store(&cell, &{name}); cell.get() }}",
                    elem.rust()
                )
            });
            fields.push(elem.rust());
        }
        let result = match super::return_type(model, Some(def.return_type))? {
            None => None,
            Some(ty) => match model.types.info(ty).kind {
                Kind::Elem(elem) => Some(elem),
                _ => {
                    return Err(unsupported(format!(
                        "returns a {}",
                        model.types.info(ty).name
                    )));
                }
            },
        };
        // The binding's result, the generated function's, and how the one becomes the other.
        let (returns, raw, load) = match result {
            None => ("()".to_string(), "()", "result"),
            Some(elem) => {
                let (rust, own) = passed(elem);
                let load = if own {
                    "result"
                } else {
                    "trust_plc::Place::load(&core::cell::Cell::new(result))"
                };
                (rust, elem.rust(), load)
            }
        };
        let arguments: String = (0..values.len())
            .map(|index| format!(", self.{index}"))
            .collect();
        let _ = write!(
            text,
            "/// The Structured Text function `{declared}` of the project (specification 48 RU-39).
#[inline]
pub fn {name}(cx: &mut trust_plc::Cx{parameters}) -> Result<{returns}, trust_plc::Fault> {{
    struct Call({fields});
    impl trust_plc_abi::StructuredTextCall for Call {{
        type Output = {raw};
        fn call(self, cx: &mut trust_plc_abi::Cx) -> Result<{raw}, trust_plc_abi::Fault> {{
            trust_plc_abi::module::with_areas(cx, |cx, areas| {{
                let w = __generated::World::new(cx, &areas)?;
                __generated::f_{function}(cx, &w{arguments})
            }})
        }}
    }}
    let result = cx.__call_structured_text(Call({values}))?;
    Ok({load})
}}

",
            declared = def.name,
            name = used.name,
            fields = fields.join(", "),
            function = mangle(&def.name),
            values = values.join(", "),
        );
    }
    Ok(text)
}

#[cfg(test)]
#[path = "st_bindings_tests.rs"]
mod tests;
