//! The bindings of layer 1 (specification 48 RU-39): for each Structured Text function that the
//! project's crate calls, a Rust function of the name the crate calls it by, which runs the
//! generated function on the kernel's context that the SDK's `Cx` keeps (RU-11, RU-38).

use std::collections::{BTreeSet, HashMap};
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
    named: &[Ty],
    crate_structures: &HashMap<String, String>,
) -> Result<String, NativeError> {
    // The cells a binding writes a value into, empty, whatever the generated code's type of them.
    let mut text = String::from(
        "/// Empty cells of the generated code's type `T`, its zero value (specification 44 §6.2),
/// which a binding writes a value into before the generated code reads them.
fn zeroed<T: trust_plc_abi::Plain>() -> T {
    T::ZEROED
}

",
    );
    let mut written: Vec<&str> = Vec::new();
    let mut blocks: Vec<String> = Vec::new();
    // The Structured Text structures the bindings pass, the crate's names among them.
    let mut structures: BTreeSet<Ty> = BTreeSet::new();
    for ty in named {
        let _ = by_cells(&model.types, *ty, crate_structures, &mut structures);
    }
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
                text.push_str(&block(model, used, def, crate_structures, &mut structures)?);
            }
            if used.name != def.name.as_str() {
                let _ = writeln!(text, "pub type {} = {};\n", used.name, def.name);
            }
            continue;
        }
        text.push_str(&function(model, used, crate_structures, &mut structures)?);
    }
    let all: Vec<Ty> = structures.iter().copied().collect();
    for ty in all {
        text.push_str(&structure(
            &model.types,
            ty,
            crate_structures,
            &mut structures,
        ));
    }
    Ok(text)
}

/// The binding of the Structured Text function the crate calls by `used` (RU-39): a Rust function
/// of its name that runs the generated function on the kernel's context the SDK keeps, passing its
/// inputs, in-outs, outputs and result as the Rust types of RU-25's table.
fn function(
    model: &mut Model<'_>,
    used: &StructuredTextUse,
    crate_structures: &HashMap<String, String>,
    structures: &mut BTreeSet<Ty>,
) -> Result<String, NativeError> {
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
    // What `call` does before the generated function's call, and what it passes it: a string
    // goes into a cell of the generated code's, which the function takes by reference.
    let mut setup = String::new();
    let mut arguments = String::new();
    // What `call` writes back once the function returns: an in-out's value and an output.
    let mut writebacks = String::new();
    for param in &def.params {
        // `EN` is TRUE whenever the function runs (specification 10 §7.5).
        if param.name.eq_ignore_ascii_case("EN") {
            continue;
        }
        let ty = model.ty(param.type_id)?;
        let name = parameter_name(&param.name);
        let index = values.len();
        let kind = &model.types.info(ty).kind;
        let cells = by_cells(&model.types, ty, crate_structures, structures);
        if param.direction != ParamDirection::In {
            // An in-out or an output: cells the function reads and writes, whose value goes
            // back to the caller's variable once it returns. A string in-out is a view of the
            // caller's string, which the bindings do not pass yet.
            let rust = match kind {
                Kind::Str(_) | Kind::WStr(_) if param.direction == ParamDirection::InOut => None,
                Kind::Elem(elem) => Some(passed(*elem).0),
                _ => cells,
            };
            let Some(rust) = rust else {
                return Err(unsupported(format!(
                    "has the in-out {} of type {}",
                    param.name,
                    model.types.info(ty).name
                )));
            };
            if param.direction == ParamDirection::InOut {
                let _ = write!(parameters, ", {name}: trust_plc::InOut<'a, {rust}>");
                fields.push(format!("trust_plc::InOut<'a, {rust}>"));
                let _ = writeln!(
                    setup,
                    "                let t{index} = zeroed();\n                \
                     trust_plc::Place::store(&t{index}, &self.{index}.get());"
                );
                let _ = writeln!(
                    writebacks,
                    "                self.{index}.set(trust_plc::Place::load(&t{index}));"
                );
            } else {
                let _ = write!(parameters, ", {name}: &'a mut {rust}");
                fields.push(format!("&'a mut {rust}"));
                let _ = writeln!(setup, "                let t{index} = zeroed();");
                let _ = writeln!(
                    writebacks,
                    "                *self.{index} = trust_plc::Place::load(&t{index});"
                );
            }
            values.push(name);
            let _ = write!(arguments, ", &t{index}");
            continue;
        }
        match (param.direction == ParamDirection::In, kind, cells) {
            (true, Kind::Elem(elem), _) => {
                let (rust, own) = passed(*elem);
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
                fields.push(elem.rust().to_string());
                let _ = write!(arguments, ", self.{index}");
            }
            (true, _, Some(rust)) => {
                let _ = write!(parameters, ", {name}: {rust}");
                values.push(name);
                fields.push(rust);
                let _ = writeln!(
                    setup,
                    "                let t{index} = zeroed();\n                \
                     trust_plc::Place::store(&t{index}, &self.{index});"
                );
                let _ = write!(arguments, ", &t{index}");
            }
            _ => {
                return Err(unsupported(format!(
                    "has the parameter {} of type {}",
                    param.name,
                    model.types.info(ty).name
                )));
            }
        }
    }
    // The binding's result, the generated function's, how `call` gets it, and how the one
    // becomes the other once the call returns.
    let function = mangle(&def.name);
    let (returns, raw, run, load) = match super::return_type(model, Some(def.return_type))? {
        None => (
            "()".to_string(),
            "()".to_string(),
            format!(
                "__generated::f_{function}(cx, &w{arguments})?;\n{writebacks}                Ok(())"
            ),
            "result",
        ),
        Some(ty) => match (
            &model.types.info(ty).kind,
            by_cells(&model.types, ty, crate_structures, structures),
        ) {
            (Kind::Elem(elem), _) => {
                let (rust, own) = passed(*elem);
                let load = if own {
                    "result"
                } else {
                    "trust_plc::Place::load(&core::cell::Cell::new(result))"
                };
                let run = format!(
                    "let result = __generated::f_{function}(cx, &w{arguments})?;\n\
                     {writebacks}                Ok(result)"
                );
                (rust, elem.rust().to_string(), run, load)
            }
            (_, Some(rust)) => {
                // A string or an array result is written into the cells the function takes
                // first.
                let run = format!(
                    "let out = zeroed();\n                \
                     __generated::f_{function}(cx, &w, &out{arguments})?;\n\
                     {writebacks}                Ok(trust_plc::Place::load(&out))"
                );
                (rust.clone(), rust, run, "result")
            }
            _ => {
                return Err(unsupported(format!(
                    "returns a {}",
                    model.types.info(ty).name
                )));
            }
        },
    };
    let mut text = String::new();
    let _ = write!(
        text,
        "/// The Structured Text function `{declared}` of the project (specification 48 RU-39). It runs on
/// the program's memory, which a unit test's context does not hold: there the call ends with
/// `FaultCode::ModuleInternal`; test it in a machine test with `trust-sim` (RU-56, RU-58).
#[inline]
pub fn {name}{generic}(cx: &mut trust_plc::Cx{parameters}) -> Result<{returns}, trust_plc::Fault> {{
struct Call{generic}({fields});
impl{generic} trust_plc_abi::StructuredTextCall for Call{generic} {{
    type Output = {raw};
    fn call(mut self, cx: &mut trust_plc_abi::Cx) -> Result<{raw}, trust_plc_abi::Fault> {{
        trust_plc_abi::module::with_areas(cx, |cx, areas| {{
            let w = __generated::World::new(cx, &areas)?;
{setup}                {run}
        }})
    }}
}}
let result = cx.__call_structured_text(Call({values}))?;
Ok({load})
}}

",
        declared = def.name,
        name = used.name,
        generic = if writebacks.is_empty() { "" } else { "<'a>" },
        fields = fields.join(", "),
        values = values.join(", "),
    );
    Ok(text)
}

/// The Rust type of RU-25's table of the string type `kind` and the generated code's cell type of
/// it: `trust_plc::Str<N>` and `trust_plc_abi::Str::<N>` for `STRING[N]`, the wide ones for
/// `WSTRING[N]`.
fn text_types(kind: &Kind) -> (String, String) {
    match kind {
        Kind::WStr(capacity) => (
            format!("trust_plc::WStr<{capacity}>"),
            format!("trust_plc_abi::WStr::<{capacity}>"),
        ),
        Kind::Str(capacity) => (
            format!("trust_plc::Str<{capacity}>"),
            format!("trust_plc_abi::Str::<{capacity}>"),
        ),
        _ => (String::new(), String::new()),
    }
}

/// The Rust type of RU-25's table of a value of `ty` that the generated code holds in cells and
/// takes by reference: a string, `Str<N>` or `WStr<N>`; a one-dimensional array of a type a binding
/// passes, `Array<T, A, N>`; a structure of the project's Structured Text, its binding `st::NAME`,
/// which joins `structures`. `None` for another type, among them a structure of the project's crate
/// (`crate_structures`), which layer 1, compiled before the crate, cannot name (RU-40).
fn by_cells(
    types: &Types<'_>,
    ty: Ty,
    crate_structures: &HashMap<String, String>,
    structures: &mut BTreeSet<Ty>,
) -> Option<String> {
    let info = types.info(ty);
    match &info.kind {
        Kind::Str(_) | Kind::WStr(_) => Some(text_types(&info.kind).0),
        Kind::Array { elem, dims } if dims.len() == 1 => {
            let (lower, upper) = dims[0];
            let len = upper.checked_sub(lower)?.checked_add(1)?;
            let element = rust_type(types, *elem, crate_structures, structures)?;
            Some(format!("trust_plc::Array<{element}, {lower}, {len}>"))
        }
        Kind::Struct { members } => {
            let of_the_crate =
                super::rust_glue::structure_path(crate_structures, &info.name).is_ok();
            if of_the_crate || syn::parse_str::<syn::Ident>(&info.name).is_err() {
                return None;
            }
            if structures.insert(ty) {
                let passes = members.iter().all(|member| {
                    rust_type(types, member.ty, crate_structures, structures).is_some()
                });
                if !passes {
                    structures.remove(&ty);
                    return None;
                }
            }
            Some(info.name.to_string())
        }
        _ => None,
    }
}

/// The Rust type of RU-25's table a binding passes a value of `ty` as: an elementary type's, or one
/// the generated code holds in cells ([`by_cells`]).
fn rust_type(
    types: &Types<'_>,
    ty: Ty,
    crate_structures: &HashMap<String, String>,
    structures: &mut BTreeSet<Ty>,
) -> Option<String> {
    match &types.info(ty).kind {
        Kind::Elem(elem) => Some(passed(*elem).0),
        _ => by_cells(types, ty, crate_structures, structures),
    }
}

/// The binding of the Structured Text structure `ty`: a Rust struct of its fields, its default, and
/// its place, the generated code's structure, through which a call and a held block pass it (RU-39).
fn structure(
    types: &Types<'_>,
    ty: Ty,
    crate_structures: &HashMap<String, String>,
    structures: &mut BTreeSet<Ty>,
) -> String {
    let info = types.info(ty);
    let Kind::Struct { members } = &info.kind else {
        return String::new();
    };
    let name = &info.name;
    let mut fields = String::new();
    let mut defaults = Vec::new();
    let mut loads = Vec::new();
    let mut stores = String::new();
    for member in members {
        let field = field_name(&member.name);
        let rust = rust_type(types, member.ty, crate_structures, structures).unwrap_or_default();
        let _ = writeln!(fields, "    pub {field}: {rust},");
        defaults.push(format!("{field}: trust_plc::declared_default()"));
        loads.push(format!(
            "{field}: trust_plc::Place::load(&self.{})",
            member.field()
        ));
        let _ = writeln!(
            stores,
            "        trust_plc::Place::store(&self.{}, &value.{field});",
            member.field()
        );
    }
    format!(
        "/// The Structured Text structure `{name}` of the project (specification 48 RU-39).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct {name} {{
{fields}}}

impl trust_plc::Declared for {name} {{
    const DEFAULT: Self = Self {{ {defaults} }};
}}

impl trust_plc::Place<{name}> for __generated::{cells} {{
    fn load(&self) -> {name} {{
        {name} {{ {loads} }}
    }}

    fn store(&self, value: &{name}) {{
{stores}    }}
}}

",
        defaults = defaults.join(", "),
        cells = info.rust,
        loads = loads.join(", "),
    )
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

/// A held block's input or output as its binding holds it (RU-39).
pub(super) struct HeldMember {
    /// Its name in the binding.
    pub(super) field: String,
    /// Its field in the instance's cells.
    pub(super) cells: String,
    /// Its Rust type, RU-25's for its elementary type.
    rust: String,
}

/// The inputs and the outputs of the Structured Text function block `ty` as its binding holds
/// them (RU-39).
///
/// # Errors
///
/// What the binding does not pass yet, described: a name in a namespace, an in-out, a variable of
/// a type other than RU-25's elementary types, strings and one-dimensional arrays of them.
pub(super) fn held_members(
    types: &Types<'_>,
    ty: Ty,
    crate_structures: &HashMap<String, String>,
    structures: &mut BTreeSet<Ty>,
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
        let rust = match rust_type(types, member.ty, crate_structures, structures) {
            Some(rust) => rust,
            None => {
                return Err(format!(
                    "has the variable {} of type {}",
                    member.name, member_info.name
                ));
            }
        };
        list.push(HeldMember {
            field: field_name(&member.name),
            cells: member.field(),
            rust,
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
    crate_structures: &HashMap<String, String>,
    structures: &mut BTreeSet<Ty>,
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
    let (inputs, outputs) =
        held_members(&model.types, ty, crate_structures, structures).map_err(unsupported)?;
    let fields = |list: &[HeldMember]| -> String {
        list.iter()
            .map(|member| format!("    pub {}: {},\n", member.field, member.rust))
            .collect()
    };
    let zeros = |list: &[HeldMember]| -> String {
        list.iter()
            .map(|member| format!("{}: trust_plc::declared_default()", member.field))
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
    /// own variables, and takes its outputs into `output`. Its instance lies in the program's
    /// memory, which a unit test does not hold: there the call ends with
    /// `FaultCode::ModuleInternal` (specification 48 RU-56).
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
