//! The bindings of layer 1 (specification 48 RU-39): for each Structured Text function that the
//! project's crate calls, a Rust function of the name the crate calls it by, which runs the
//! generated function on the kernel's context that the SDK's `Cx` keeps (RU-11, RU-38).

use std::fmt::Write as _;

use super::super::classes::Owner;
use super::super::model::Model;
use super::super::rust_pous::StructuredTextUse;
use super::super::types::{mangle, Elem, Kind, Ty, Types};
use super::super::NativeError;
use crate::program_model::FunctionBlockDef;
use trust_hir::symbols::ParamDirection;
use trust_plc_decl::Elementary;
use trust_runtime_core::native::image::{SYMBOL_INPUT, SYMBOL_IN_OUT, SYMBOL_OUTPUT};

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
    let mut blocks: Vec<String> = Vec::new();
    for (_, used) in roots {
        if written.contains(&used.name.as_str()) {
            continue;
        }
        written.push(&used.name);
        if model.function(&used.name).is_none()
            && let Some(Owner::Fb(def)) = model.types.owner(&used.name)
        {
            // One binding of the block, named as Structured Text declares it, which the generated
            // code names; another spelling the crate writes names it too.
            if !blocks.contains(&def.name.to_string()) {
                blocks.push(def.name.to_string());
                text.push_str(&block(model, used, def)?);
            }
            if used.name != def.name.as_str() {
                let _ = writeln!(text, "pub type {} = {};\n", used.name, def.name);
            }
            continue;
        }
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

/// The name of a held block's input or output in its binding: the Structured Text name as declared,
/// with `_` after one that Rust keeps as a keyword.
fn field_name(name: &str) -> String {
    if syn::parse_str::<syn::Ident>(name).is_err() {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

/// The zero of `elem` as a constant, which a held block's `DEFAULT` holds.
fn zero(elem: Elem) -> &'static str {
    match elem {
        Elem::Bool => "false",
        Elem::Real | Elem::LReal => "0.0",
        _ => "0",
    }
}

/// A held block's input or output as its binding holds it (RU-39).
pub(super) struct HeldMember {
    /// Its name in the binding.
    pub(super) field: String,
    /// Its field in the instance's cells.
    pub(super) cells: String,
    /// Its Rust type.
    rust: &'static str,
    /// Its type's zero.
    zero: &'static str,
}

/// The inputs and the outputs of the Structured Text function block `ty` as its binding holds
/// them (RU-39).
///
/// # Errors
///
/// What the binding does not pass yet, described: a name in a namespace, an in-out, a variable of
/// a type other than the numbers and BOOL.
pub(super) fn held_members(
    types: &Types<'_>,
    ty: Ty,
) -> Result<(Vec<HeldMember>, Vec<HeldMember>), String> {
    let info = types.info(ty);
    if syn::parse_str::<syn::Ident>(&info.name).is_err() {
        return Err("is named in a namespace".to_string());
    }
    let Kind::Fb { members, .. } = &info.kind else {
        return Err("is no function block".to_string());
    };
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for member in members {
        let list = if member.flags & SYMBOL_INPUT != 0 {
            &mut inputs
        } else if member.flags & SYMBOL_OUTPUT != 0 {
            &mut outputs
        } else if member.flags & SYMBOL_IN_OUT != 0 {
            return Err(format!("has the in-out {}", member.name));
        } else {
            continue;
        };
        let member_info = types.info(member.ty);
        let elem = match member_info.kind {
            Kind::Elem(elem) if passed(elem).1 => elem,
            _ => {
                return Err(format!(
                    "has the variable {} of type {}",
                    member.name, member_info.name
                ));
            }
        };
        list.push(HeldMember {
            field: field_name(&member.name),
            cells: member.field(),
            rust: elem.rust(),
            zero: zero(elem),
        });
    }
    Ok((inputs, outputs))
}

/// The binding of the Structured Text function block `def`, which a field of the crate holds
/// (RU-38, RU-39): its inputs, its outputs as its last call left them, and its call, which gives
/// the instance the inputs, runs its body on its own variables where they lie and takes its
/// outputs.
fn block(
    model: &Model<'_>,
    used: &StructuredTextUse,
    def: &FunctionBlockDef,
) -> Result<String, NativeError> {
    let unsupported = |what: String| {
        NativeError::Compile(format!(
            "st::{} at {}:{}: {} {what}, which a held block does not pass yet (specification 48 \
             RU-39, milestone R3)",
            used.name, used.file, used.line, def.name
        ))
    };
    let name = def.name.as_str();
    let Some(ty) = model.types.by_rust(&Owner::Fb(def).rust()) else {
        return Err(NativeError::Compile(format!(
            "function block {name} has no layout"
        )));
    };
    let info = model.types.info(ty);
    let (inputs, outputs) = held_members(&model.types, ty).map_err(unsupported)?;
    let fields = |list: &[HeldMember]| -> String {
        list.iter()
            .map(|member| format!("    pub {}: {},\n", member.field, member.rust))
            .collect()
    };
    let zeros = |list: &[HeldMember]| -> String {
        list.iter()
            .map(|member| format!("{}: {}", member.field, member.zero))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let stores: String = inputs
        .iter()
        .map(|member| {
            format!(
                "                    trust_plc::Place::store(&me.{}, &self.1.{});\n",
                member.cells, member.field
            )
        })
        .collect();
    let loads = outputs
        .iter()
        .map(|member| {
            format!(
                "{}: trust_plc::Place::load(&me.{})",
                member.field, member.cells
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "/// The Structured Text function block `{name}` of the project, as a Rust function block or
/// program holds it (specification 48 RU-38, RU-39).
#[derive(Clone, Copy)]
pub struct {name} {{
    /// Its inputs, which `call` gives it.
    pub input: {name}_Input,
    /// Its outputs, as its last call left them.
    pub output: {name}_Output,
    place: trust_plc_abi::RefValue,
}}

/// The inputs of [`{name}`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct {name}_Input {{
{input_fields}}}

/// The outputs of [`{name}`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct {name}_Output {{
{output_fields}}}

impl trust_plc::Declared for {name} {{
    const DEFAULT: Self = Self {{
        input: {name}_Input {{ {input_zeros} }},
        output: {name}_Output {{ {output_zeros} }},
        place: trust_plc_abi::RefValue::NULL_REF,
    }};
}}

impl {name} {{
    #[doc(hidden)]
    #[must_use]
    pub const fn __held(place: trust_plc_abi::RefValue, input: {name}_Input, output: {name}_Output) -> Self {{
        Self {{ input, output, place }}
    }}

    /// Calls the block as Structured Text calls an instance: gives it `input`, runs its body on its
    /// own variables, and takes its outputs into `output`.
    ///
    /// # Errors
    ///
    /// The fault of its Structured Text, which ends the scan.
    pub fn call(&mut self, cx: &mut trust_plc::Cx) -> Result<(), trust_plc::Fault> {{
        struct Call(trust_plc_abi::RefValue, {name}_Input);
        impl trust_plc_abi::StructuredTextCall for Call {{
            type Output = {name}_Output;
            fn call(self, cx: &mut trust_plc_abi::Cx) -> Result<{name}_Output, trust_plc_abi::Fault> {{
                trust_plc_abi::module::with_areas(cx, |cx, areas| {{
                    let w = __generated::World::new(cx, &areas)?;
                    let me: &__generated::{cells} = areas.deref(cx, &self.0)?;
{stores}                    __generated::fb_{body}(cx, &w, me)?;
                    Ok({name}_Output {{ {loads} }})
                }})
            }}
        }}
        self.output = cx.__call_structured_text(Call(self.place, self.input))?;
        Ok(())
    }}
}}

",
        input_fields = fields(&inputs),
        output_fields = fields(&outputs),
        input_zeros = zeros(&inputs),
        output_zeros = zeros(&outputs),
        cells = info.rust,
        body = mangle(&def.name),
    ))
}

#[cfg(test)]
#[path = "st_bindings_tests.rs"]
mod tests;
