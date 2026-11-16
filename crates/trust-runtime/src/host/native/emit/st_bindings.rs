//! The bindings of layer 1 (specification 48 RU-39): for each Structured Text function that the
//! project's crate calls, a Rust function of the name the crate calls it by, which runs the
//! generated function on the kernel's context that the SDK's `Cx` keeps (RU-11, RU-38).

use std::fmt::Write as _;

use super::super::model::Model;
use super::super::rust_pous::StructuredTextUse;
use super::super::types::{mangle, Elem, Kind};
use super::super::NativeError;
use trust_hir::symbols::ParamDirection;

/// The Rust type a binding passes a value of the elementary type `elem` as: the generated code's
/// own, for the types whose type in RU-25's table it is.
fn passed(elem: Elem) -> Option<&'static str> {
    matches!(
        elem,
        Elem::Bool
            | Elem::SInt
            | Elem::Int
            | Elem::DInt
            | Elem::LInt
            | Elem::USInt
            | Elem::UInt
            | Elem::UDInt
            | Elem::ULInt
            | Elem::Real
            | Elem::LReal
    )
    .then(|| elem.rust())
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
/// [`NativeError::Compile`] for a function whose parameters or result the bindings do not pass
/// yet, naming the call.
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
            let rust = match model.types.info(ty).kind {
                Kind::Elem(elem) if param.direction == ParamDirection::In => passed(elem),
                _ => None,
            };
            let Some(rust) = rust else {
                return Err(unsupported(format!(
                    "has the parameter {} of type {}",
                    param.name,
                    model.types.info(ty).name
                )));
            };
            let name = parameter_name(&param.name);
            let _ = write!(parameters, ", {name}: {rust}");
            values.push(name);
            fields.push(rust);
        }
        let returns = match super::return_type(model, Some(def.return_type))? {
            None => "()",
            Some(ty) => match model.types.info(ty).kind {
                Kind::Elem(elem) => passed(elem),
                _ => None,
            }
            .ok_or_else(|| unsupported(format!("returns a {}", model.types.info(ty).name)))?,
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
        type Output = {returns};
        fn call(self, cx: &mut trust_plc_abi::Cx) -> Result<{returns}, trust_plc_abi::Fault> {{
            trust_plc_abi::module::with_areas(cx, |cx, areas| {{
                let w = __generated::World::new(cx, &areas)?;
                __generated::f_{function}(cx, &w{arguments})
            }})
        }}
    }}
    cx.__call_structured_text(Call({values}))
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
