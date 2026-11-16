//! The bindings of layer 1 (specification 48 RU-39): for each Structured Text function that the
//! project's crate calls, a Rust function of the name the crate calls it by, which runs the
//! generated function on the kernel's context that the SDK's `Cx` keeps (RU-11, RU-38).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::super::classes::Owner;
use super::super::model::Model;
use super::super::rust_pous::StructuredTextUse;
use super::super::types::{mangle, Elem, Kind, Ty, Types};
use super::super::NativeError;
use crate::program_model::{FunctionBlockDef, Param};
use trust_hir::symbols::ParamDirection;
use trust_plc_decl::Elementary;
use trust_runtime_core::native::image::{SYMBOL_INPUT, SYMBOL_IN_OUT, SYMBOL_OUTPUT};

/// RU-25's type of the elementary type `elem`.
pub(super) fn elementary(elem: Elem) -> Elementary {
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
pub(super) fn passed(elem: Elem) -> (String, bool) {
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
/// [`NativeError::Compile`] for a function or a held block with a variable of a type the bindings
/// have no Rust type for, naming the call.
pub(super) fn bindings(
    model: &mut Model<'_>,
    roots: &[(String, StructuredTextUse)],
    named: &[Ty],
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
    // The project's data types the bindings pass and the crate names.
    let mut data_types: BTreeSet<Ty> = BTreeSet::new();
    for ty in named {
        let _ = rust_type(&model.types, *ty, &mut data_types);
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
                text.push_str(&block(model, used, def, &mut data_types)?);
            }
            if used.name != def.name.as_str() {
                let _ = writeln!(text, "pub type {} = {};\n", used.name, def.name);
            }
            continue;
        }
        text.push_str(&function(model, used, &mut data_types)?);
    }
    // Each data type's binding, those its own reach among them.
    let mut written_types = BTreeSet::new();
    while let Some(ty) = data_types
        .iter()
        .copied()
        .find(|ty| !written_types.contains(ty))
    {
        written_types.insert(ty);
        text.push_str(&super::st_types::binding(model, ty, &mut data_types)?);
    }
    Ok(text)
}

/// The binding of the Structured Text function the crate calls by `used` (RU-39): a Rust function
/// of its name that runs the generated function on the kernel's context the SDK keeps, passing its
/// inputs, in-outs, outputs and result as the Rust types of RU-25's table.
fn function(
    model: &mut Model<'_>,
    used: &StructuredTextUse,
    data_types: &mut BTreeSet<Ty>,
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
    let mut params = Parameters::default();
    for param in &def.params {
        // `EN` is TRUE whenever the function runs (specification 10 §7.5).
        if param.name.eq_ignore_ascii_case("EN") {
            continue;
        }
        let ty = model.ty(param.type_id)?;
        params
            .push(&model.types, param, ty, data_types)
            .map_err(unsupported)?;
    }
    let Parameters {
        signature: parameters,
        fields,
        values,
        setup,
        arguments,
        in_outs,
        outputs,
    } = params;
    // The binding's result, the generated function's, how `call` gets it, and how the one
    // becomes the other once the call returns.
    let function = mangle(&def.name);
    let (returns, raw, run, load) = match super::return_type(model, Some(def.return_type))? {
        None => (
            "()".to_string(),
            "()".to_string(),
            format!(
                "let done = __generated::f_{function}(cx, &w{arguments});\n\
                 {in_outs}                done?;\n{outputs}                Ok(())"
            ),
            "result",
        ),
        Some(ty) => match (
            &model.types.info(ty).kind,
            by_cells(&model.types, ty, data_types),
        ) {
            (Kind::Elem(elem), _) => {
                let (rust, own) = passed(*elem);
                let load = if own {
                    "result"
                } else {
                    "trust_plc::Place::load(&core::cell::Cell::new(result))"
                };
                let run = format!(
                    "let result = __generated::f_{function}(cx, &w{arguments});\n\
                     {in_outs}                let result = result?;\n\
                     {outputs}                Ok(result)"
                );
                (rust, elem.rust().to_string(), run, load)
            }
            (Kind::Enum { base, .. }, _) => {
                let Some(rust) = rust_type(&model.types, ty, data_types) else {
                    return Err(unsupported(format!(
                        "returns a {}",
                        model.types.info(ty).name
                    )));
                };
                let run = format!(
                    "let result = __generated::f_{function}(cx, &w{arguments});\n\
                     {in_outs}                let result = result?;\n\
                     {outputs}                Ok(result)"
                );
                let load = "trust_plc::Place::load(&core::cell::Cell::new(result))";
                (rust, base.rust().to_string(), run, load)
            }
            (_, Some(rust)) => {
                // A string or an array result is written into the cells the function takes
                // first.
                let out = load(&model.types, ty, "out", Scope::Bindings);
                let run = format!(
                    "let out = zeroed();\n                \
                     let done = __generated::f_{function}(cx, &w, &out{arguments});\n\
                     {in_outs}                done?;\n\
                     {outputs}                Ok({out})"
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
        generic = if in_outs.is_empty() && outputs.is_empty() {
            ""
        } else {
            "<'a>"
        },
        fields = fields.join(", "),
        values = values.join(", "),
    );
    Ok(text)
}

/// What the binding of a called function writes for the function's parameters (RU-39): its own
/// parameters, the fields of its `Call` and their values, what `call` does before the generated
/// function runs and what it passes it, and what it writes back once the function ends.
#[derive(Default)]
struct Parameters {
    /// The binding's parameters after `cx`, each after `, `.
    signature: String,
    /// The fields of the binding's `Call`, as the generated code takes them.
    fields: Vec<String>,
    /// The values the binding gives the fields of its `Call`.
    values: Vec<String>,
    /// What `call` does before the generated function's call: a value the function takes by
    /// reference goes into cells of the generated code's.
    setup: String,
    /// What `call` passes the generated function after its context and its world.
    arguments: String,
    /// The in-outs' values, which `call` writes back also when the function faults, as a write
    /// through Structured Text's in-out reaches the caller's variable at once (RU-29).
    in_outs: String,
    /// The outputs, which `call` writes only when the function returns.
    outputs: String,
}

impl Parameters {
    /// Adds the parameter `param` of type `ty`.
    ///
    /// # Errors
    ///
    /// What the bindings do not pass yet, described: a parameter of a type they have no Rust type
    /// for, a string in-out.
    fn push(
        &mut self,
        types: &Types<'_>,
        param: &Param,
        ty: Ty,
        data_types: &mut BTreeSet<Ty>,
    ) -> Result<(), String> {
        let name = parameter_name(&param.name);
        let index = self.values.len();
        let info = types.info(ty);
        let cells = by_cells(types, ty, data_types);
        if param.direction != ParamDirection::In {
            // An in-out or an output: cells the function reads and writes, whose value goes back
            // to the caller's variable. A string in-out goes as a view of a string of the declared
            // capacity, which takes the caller's text and gives it back as an assignment to the
            // caller's string does (RU-29).
            let in_out = param.direction == ParamDirection::InOut;
            let text = matches!(info.kind, Kind::Str(_) | Kind::WStr(_));
            let rust = match &info.kind {
                Kind::Elem(elem) => Some(passed(*elem).0),
                Kind::Enum { .. } => rust_type(types, ty, data_types),
                _ => cells,
            };
            let Some(rust) = rust else {
                return Err(format!(
                    "has the {} {} of type {}",
                    if in_out { "in-out" } else { "output" },
                    param.name,
                    info.name
                ));
            };
            if in_out {
                let _ = write!(self.signature, ", {name}: trust_plc::InOut<'a, {rust}>");
                self.fields.push(format!("trust_plc::InOut<'a, {rust}>"));
                let cell = format!("t{index}");
                // A view's cells are named, as `view()` takes no type from its use.
                let annotation = if text {
                    format!(": {}", text_types(&info.kind).1)
                } else {
                    String::new()
                };
                let _ = writeln!(
                    self.setup,
                    "                let t{index}{annotation} = zeroed();\n                {}",
                    store(
                        types,
                        ty,
                        &cell,
                        &format!("self.{index}.get()"),
                        Scope::Bindings
                    )
                );
                let _ = writeln!(
                    self.in_outs,
                    "                self.{index}.set({});",
                    load(types, ty, &cell, Scope::Bindings)
                );
            } else {
                let _ = write!(self.signature, ", {name}: &'a mut {rust}");
                self.fields.push(format!("&'a mut {rust}"));
                let _ = writeln!(self.setup, "                let t{index} = zeroed();");
                let _ = writeln!(
                    self.outputs,
                    "                *self.{index} = {};",
                    load(types, ty, &format!("t{index}"), Scope::Bindings)
                );
            }
            self.values.push(name);
            if text && in_out {
                let _ = write!(self.arguments, ", t{index}.view()");
            } else {
                let _ = write!(self.arguments, ", &t{index}");
            }
            return Ok(());
        }
        match (&info.kind, cells) {
            (Kind::Elem(elem), _) => {
                let (rust, own) = passed(*elem);
                let _ = write!(self.signature, ", {name}: {rust}");
                self.values.push(if own {
                    name
                } else {
                    format!(
                        "{{ let cell = core::cell::Cell::<{}>::default(); \
                         trust_plc::Place::store(&cell, &{name}); cell.get() }}",
                        elem.rust()
                    )
                });
                self.fields.push(elem.rust().to_string());
                let _ = write!(self.arguments, ", self.{index}");
            }
            // An enumeration's value, which the generated function takes as its base's.
            (Kind::Enum { base, .. }, _) => {
                let Some(rust) = rust_type(types, ty, data_types) else {
                    return Err(format!(
                        "has the parameter {} of type {}",
                        param.name, info.name
                    ));
                };
                let _ = write!(self.signature, ", {name}: {rust}");
                self.values.push(format!(
                    "{{ let cell = core::cell::Cell::<{}>::default(); \
                     trust_plc::Place::store(&cell, &{name}); cell.get() }}",
                    base.stored()
                ));
                self.fields.push(base.rust().to_string());
                let _ = write!(self.arguments, ", self.{index}");
            }
            (_, Some(rust)) => {
                let _ = write!(self.signature, ", {name}: {rust}");
                self.values.push(name);
                self.fields.push(rust);
                let _ = writeln!(
                    self.setup,
                    "                let t{index} = zeroed();\n                {}",
                    store(
                        types,
                        ty,
                        &format!("t{index}"),
                        &format!("self.{index}"),
                        Scope::Bindings
                    )
                );
                let _ = write!(self.arguments, ", &t{index}");
            }
            _ => {
                return Err(format!(
                    "has the parameter {} of type {}",
                    param.name, info.name
                ));
            }
        }
        Ok(())
    }
}

/// Where generated text names the generated code's types: in layer 1's bindings, by their paths
/// from there, or inside the generated code, by their own names.
#[derive(Clone, Copy)]
pub(super) enum Scope {
    Bindings,
    Generated,
}

/// The generated code's cells of a value of `ty` as the bindings name them: a scalar's
/// `core::cell::Cell`, a string's, a structure's under `__generated`, an array of them, a
/// reference's; `None` for another type.
fn cell_type(types: &Types<'_>, ty: Ty) -> Option<String> {
    let info = types.info(ty);
    match &info.kind {
        Kind::Elem(elem) | Kind::Enum { base: elem, .. } => {
            Some(format!("core::cell::Cell<{}>", elem.stored()))
        }
        Kind::Str(_) | Kind::WStr(_) => Some(text_types(&info.kind).1),
        Kind::Struct { .. } => Some(format!("__generated::{}", info.rust)),
        Kind::Array { elem, dims } => {
            let mut rust = cell_type(types, *elem)?;
            for (lower, upper) in dims.iter().rev() {
                let len = upper.checked_sub(*lower)?.checked_add(1)?;
                rust = format!("[{rust}; {len}]");
            }
            Some(rust)
        }
        Kind::Ref { pointer: false, .. } => {
            Some("core::cell::Cell<trust_plc_abi::RefValue>".to_string())
        }
        _ => None,
    }
}

/// The cells, as `scope` names them, of the variable a reference of type `ty` names, when `ty` is a
/// reference the SDK holds as `trust_plc::Ref`.
fn referenced_cells(types: &Types<'_>, ty: Ty, scope: Scope) -> Option<String> {
    let Kind::Ref {
        target,
        pointer: false,
    } = types.info(ty).kind
    else {
        return None;
    };
    match scope {
        Scope::Bindings => cell_type(types, target),
        Scope::Generated => Some(types.info(target).rust.clone()),
    }
}

/// The binding's value of type `ty` that the generated code's cells `cell` hold, read through
/// `trust_plc::Place`; a reference's through the SDK's reference of the cells it names (RU-29).
pub(super) fn load(types: &Types<'_>, ty: Ty, cell: &str, scope: Scope) -> String {
    match referenced_cells(types, ty, scope) {
        Some(target) => {
            format!(
                "::trust_plc::__private::reference::<_, {target}>(::core::cell::Cell::get(&{cell}))"
            )
        }
        None => format!("::trust_plc::Place::load(&{cell})"),
    }
}

/// Writes the binding's value `value` of type `ty` into the generated code's cells `cell`.
pub(super) fn store(types: &Types<'_>, ty: Ty, cell: &str, value: &str, scope: Scope) -> String {
    if referenced_cells(types, ty, scope).is_some() {
        format!(
            "::core::cell::Cell::set(&{cell}, ::trust_plc::__private::reference_value({value}));"
        )
    } else {
        format!("::trust_plc::Place::store(&{cell}, &{value});")
    }
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
/// passes, `Array<T, A, N>`, one of several dimensions the array of arrays of them; a reference to
/// a variable of such a type, `Ref<T>`; a structure of the project, its binding `st::NAME`, which
/// joins `data_types`. `None` for another type.
fn by_cells(types: &Types<'_>, ty: Ty, data_types: &mut BTreeSet<Ty>) -> Option<String> {
    let info = types.info(ty);
    match &info.kind {
        Kind::Str(_) | Kind::WStr(_) => Some(text_types(&info.kind).0),
        // An array of several dimensions lies as the array of arrays of them (specification 44
        // §6.2), which RU-25 writes `Array<Array<T, A2, N2>, A1, N1>`.
        Kind::Array { elem, dims } if !dims.is_empty() => {
            let mut rust = rust_type(types, *elem, data_types)?;
            for (lower, upper) in dims.iter().rev() {
                let len = upper.checked_sub(*lower)?.checked_add(1)?;
                rust = format!("trust_plc::Array<{rust}, {lower}, {len}>");
            }
            Some(rust)
        }
        // A reference, `REF_TO T` (specification 44 §7.3), as the SDK's `Ref<T>`, to a variable of
        // a type a binding passes (RU-25, RU-29).
        Kind::Ref {
            target,
            pointer: false,
        } => {
            cell_type(types, *target)?;
            let target = rust_type(types, *target, data_types)?;
            Some(format!("trust_plc::Ref<{target}>"))
        }
        Kind::Struct { members } => {
            if syn::parse_str::<syn::Ident>(&info.name).is_err() {
                return None;
            }
            if data_types.insert(ty) {
                let passes = members
                    .iter()
                    .all(|member| rust_type(types, member.ty, data_types).is_some());
                if !passes {
                    data_types.remove(&ty);
                    return None;
                }
            }
            Some(info.name.to_string())
        }
        _ => None,
    }
}

/// Why the bindings have no Rust type for the project's data type `ty` that the crate names, `None`
/// when they have one.
pub(super) fn unpassed(types: &Types<'_>, ty: Ty) -> Option<String> {
    if rust_type(types, ty, &mut BTreeSet::new()).is_some() {
        return None;
    }
    let info = types.info(ty);
    if syn::parse_str::<syn::Ident>(&info.name).is_err() {
        return Some("is named in a namespace".to_string());
    }
    let Kind::Struct { members } = &info.kind else {
        return Some("is of a type the bindings do not pass yet".to_string());
    };
    members
        .iter()
        .find(|member| rust_type(types, member.ty, &mut BTreeSet::new()).is_none())
        .map(|member| {
            format!(
                "has the variable {}, of a type the bindings do not pass yet",
                member.name
            )
        })
}

/// The Rust type of RU-25's table a binding passes a value of `ty` as: an elementary type's; the
/// binding `st::NAME` of an enumeration or a type with named values of the project, which joins
/// `data_types`; or one the generated code holds in cells ([`by_cells`]).
pub(super) fn rust_type(
    types: &Types<'_>,
    ty: Ty,
    data_types: &mut BTreeSet<Ty>,
) -> Option<String> {
    let info = types.info(ty);
    match &info.kind {
        Kind::Elem(elem) => Some(passed(*elem).0),
        Kind::Enum { .. } => {
            if syn::parse_str::<syn::Ident>(&info.name).is_err() {
                return None;
            }
            data_types.insert(ty);
            Some(info.name.to_string())
        }
        _ => by_cells(types, ty, data_types),
    }
}

/// The name of a held block's input or output in its binding: the Structured Text name as declared,
/// with `_` after one that Rust keeps as a keyword.
pub(super) fn field_name(name: &str) -> String {
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
    /// Its type.
    pub(super) ty: Ty,
}

/// The inputs and the outputs of the Structured Text function block `ty` as its binding holds
/// them (RU-39).
///
/// # Errors
///
/// What the binding does not pass yet, described: a name in a namespace, a variable of a type the
/// bindings have no Rust type for.
pub(super) fn held_members(
    types: &Types<'_>,
    ty: Ty,
    data_types: &mut BTreeSet<Ty>,
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
        // An in-out, which the instance holds as the reference of its last call (specification 44
        // §7.3), is a field of `input` of type `Ref<T>` (RU-29).
        let list = if member.flags & (SYMBOL_INPUT | SYMBOL_IN_OUT) != 0 {
            &mut inputs
        } else if member.flags & SYMBOL_OUTPUT != 0 {
            &mut outputs
        } else {
            continue;
        };
        let member_info = types.info(member.ty);
        let rust = match rust_type(types, member.ty, data_types) {
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
            ty: member.ty,
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
    data_types: &mut BTreeSet<Ty>,
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
    let (inputs, outputs) = held_members(&model.types, ty, data_types).map_err(unsupported)?;
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
                "                    {}\n",
                store(
                    &model.types,
                    member.ty,
                    &format!("me.{}", member.cells),
                    &format!("self.1.{}", member.field),
                    Scope::Bindings
                )
            )
        })
        .collect();
    let loads = outputs
        .iter()
        .map(|member| {
            format!(
                "{}: {}",
                member.field,
                load(
                    &model.types,
                    member.ty,
                    &format!("me.{}", member.cells),
                    Scope::Bindings
                )
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
