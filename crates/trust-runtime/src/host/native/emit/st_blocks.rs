//! The bindings of the Structured Text function blocks a Rust POU holds (specification 48 RU-38,
//! RU-39): each block's inputs and outputs, its call, which gives the instance the inputs, runs its
//! body on its own variables where they lie and takes its outputs, and its `PUBLIC` methods.

use std::collections::BTreeSet;

use smol_str::SmolStr;

use super::super::classes::Owner;
use super::super::model::Model;
use super::super::rust_pous::StructuredTextUse;
use super::super::types::{mangle, Kind, Ty, Types};
use super::super::NativeError;
use super::st_bindings::{
    field_name, load, parameters, returned, rust_type, store, Parameters, Returned, Scope,
};
use trust_runtime_core::native::image::{SYMBOL_INPUT, SYMBOL_IN_OUT, SYMBOL_OUTPUT};

/// A `PUBLIC` method of a held block (RU-39): the block that declares it, its name, and the path
/// from the instance to that block's part, `.__BASE` once for each level the held block inherits
/// it by.
#[derive(Clone, Debug)]
pub(super) struct HeldMethod {
    pub(super) declaring: SmolStr,
    pub(super) method: SmolStr,
    pub(super) path: String,
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
    if super::st_bindings::binding_path(&info.name).is_none() {
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
pub(super) fn block(
    model: &mut Model<'_>,
    used: &StructuredTextUse,
    owner: Owner<'_>,
    methods: &[HeldMethod],
    data_types: &mut BTreeSet<Ty>,
) -> Result<String, NativeError> {
    let declared = owner.name();
    let unsupported = |what: String| {
        NativeError::Compile(format!(
            "st::{} at {}:{}: {} {what}, which a held block does not pass yet (specification 48 \
             RU-39, milestone R3)",
            used.name, used.file, used.line, declared
        ))
    };
    // Declared by its last name in its namespace's module (IEC §6.9).
    let name = super::st_bindings::split_name(declared.as_str()).1;
    let Some(ty) = model.types.by_rust(&owner.rust()) else {
        return Err(NativeError::Compile(format!(
            "function block {name} has no layout"
        )));
    };
    let (inputs, outputs) = held_members(&model.types, ty, data_types).map_err(unsupported)?;
    let cells = model.types.info(ty).rust.clone();
    let mut method_texts = String::new();
    for held in methods {
        let text = method(
            model,
            used,
            name,
            held,
            &cells,
            (&inputs, &outputs),
            data_types,
        )?;
        method_texts.push_str(&text);
    }
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
    // A class has no body, so its binding has no `call` (IEC §6.6.5).
    let what = if owner.is_class() {
        "class"
    } else {
        "function block"
    };
    let call = if owner.is_class() {
        String::new()
    } else {
        let body = mangle(declared);
        format!(
            "    /// Calls the block as Structured Text calls an instance: gives it `input`, runs its body on its\n\
    /// own variables, and takes its outputs into `output`. Its instance lies in the program's\n\
    /// memory, which a unit test does not hold: there the call ends with\n\
    /// `FaultCode::ModuleInternal` (specification 48 RU-56).\n\
    ///\n\
    /// # Errors\n\
    ///\n\
    /// The fault of its Structured Text, which ends the scan.\n\
    pub fn call(&mut self, cx: &mut trust_plc::Cx) -> Result<(), trust_plc::Fault> {{\n\
        struct Call(trust_plc_abi::RefValue, {name}_Input);\n\
        impl trust_plc_abi::StructuredTextCall for Call {{\n\
            type Output = {name}_Output;\n\
            fn call(self, cx: &mut trust_plc_abi::Cx) -> Result<{name}_Output, trust_plc_abi::Fault> {{\n\
                trust_plc_abi::module::with_areas(cx, |cx, areas| {{\n\
                    let w = __generated::World::new(cx, &areas)?;\n\
                    let me: &__generated::{cells} = areas.deref(cx, &self.0)?;\n\
{stores}                    __generated::fb_{body}(cx, &w, me)?;\n\
                    Ok({name}_Output {{ {loads} }})\n\
                }})\n\
            }}\n\
        }}\n\
        self.output = cx.__call_structured_text(Call(self.place, self.input))?;\n\
        Ok(())\n\
    }}\n\
"
        )
    };
    Ok(format!(
        "/// The Structured Text {what} `{name}` of the project, as a Rust function block or
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

{call}{method_texts}}}

",
        input_fields = fields(&inputs),
        output_fields = fields(&outputs),
        input_zeros = zeros(&inputs),
        output_zeros = zeros(&outputs),
    ))
}

/// The binding of the held block `block`'s method `held` (RU-39): a method of the block's binding
/// that gives the instance `input`, runs the method on it as Structured Text's call does (IEC
/// §6.6.5.4.4), and takes the instance's outputs into `output`; its parameters and its result pass
/// as a function's. `cells` is the instance's type in the generated code, `members` the block's
/// inputs and outputs.
///
/// # Errors
///
/// A parameter or a result of a type the bindings do not pass yet, naming the call.
fn method(
    model: &mut Model<'_>,
    used: &StructuredTextUse,
    block: &str,
    held: &HeldMethod,
    cells: &str,
    members: (&[HeldMember], &[HeldMember]),
    data_types: &mut BTreeSet<Ty>,
) -> Result<String, NativeError> {
    let unsupported = |what: String| {
        NativeError::Compile(format!(
            "st::{} at {}:{}: the method {}.{} {what}, which a call from Rust does not pass yet \
             (specification 48 RU-39, milestone R3)",
            used.name, used.file, used.line, held.declaring, held.method
        ))
    };
    // The block or the class that declares it (IEC §6.6.5).
    let Some(declaring) = model.types.owner(&held.declaring) else {
        return Err(NativeError::Compile(format!(
            "{} is not declared",
            held.declaring
        )));
    };
    let Some(def) = declaring
        .methods()
        .iter()
        .find(|method| method.name.eq_ignore_ascii_case(&held.method))
    else {
        return Err(NativeError::Compile(format!(
            "method {}.{} is not declared",
            held.declaring, held.method
        )));
    };
    let params = parameters(model, &def.params, data_types, unsupported)?;
    let callee = format!("m_{}__{}", mangle(&held.declaring), mangle(&def.name));
    let receiver = if held.path.is_empty() {
        ", me".to_string()
    } else {
        format!(", &me{}", held.path)
    };
    let Returned {
        returns,
        raw,
        run,
        value,
        load: taken,
    } = returned(
        model,
        def.return_type,
        &callee,
        &receiver,
        &params,
        data_types,
        unsupported,
    )?;
    // The place and the inputs follow the parameters in `Call`, whose indices they keep.
    let place = params.values.len();
    let input = place + 1;
    let (inputs, outputs) = members;
    let stores: String = inputs
        .iter()
        .map(|member| {
            format!(
                "                    {}\n",
                store(
                    &model.types,
                    member.ty,
                    &format!("me.{}", member.cells),
                    &format!("self.{input}.{}", member.field),
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
    let Parameters {
        signature,
        mut fields,
        mut values,
        setup,
        in_outs,
        outputs: written,
        ..
    } = params;
    let generic = if in_outs.is_empty() && written.is_empty() {
        ""
    } else {
        "<'a>"
    };
    fields.extend([
        "trust_plc_abi::RefValue".to_string(),
        format!("{block}_Input"),
    ]);
    values.extend(["self.place".to_string(), "self.input".to_string()]);
    Ok(format!(
        "
    /// The method `{method}` of the block (IEC §6.6.5.4), called as Structured Text calls it on
    /// the instance: gives the instance `input`, runs the method on it and takes its outputs
    /// into `output` (specification 48 RU-39).
    ///
    /// # Errors
    ///
    /// The fault of its Structured Text, which ends the scan.
    pub fn {method}{generic}(&mut self, cx: &mut trust_plc::Cx{signature}) -> Result<{returns}, trust_plc::Fault> {{
        struct Call{generic}({fields});
        impl{generic} trust_plc_abi::StructuredTextCall for Call{generic} {{
            type Output = ({block}_Output, {raw});
            fn call(mut self, cx: &mut trust_plc_abi::Cx) -> Result<({block}_Output, {raw}), trust_plc_abi::Fault> {{
                trust_plc_abi::module::with_areas(cx, |cx, areas| {{
                    let w = __generated::World::new(cx, &areas)?;
                    let me: &__generated::{cells} = areas.deref(cx, &self.{place})?;
{stores}{setup}                {run}                Ok(({block}_Output {{ {loads} }}, {value}))
                }})
            }}
        }}
        let (output, result) = cx.__call_structured_text(Call({values}))?;
        self.output = output;
        Ok({taken})
    }}
",
        method = def.name,
        fields = fields.join(", "),
        values = values.join(", "),
    ))
}
