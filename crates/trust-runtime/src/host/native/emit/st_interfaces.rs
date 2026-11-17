//! The bindings of the Structured Text interfaces the project's crate reaches (specification 48
//! RU-39): each interface's value, a reference to an instance that implements it or `NULL`, with the
//! methods of the interface and of those it extends, which reach the method of the instance's class
//! through the interface's dispatcher (specification 44 §6.4).

use std::collections::BTreeSet;

use smol_str::SmolStr;

use super::super::dispatch::interface_function;
use super::super::model::Model;
use super::super::types::{Kind, Ty};
use super::super::NativeError;
use super::st_bindings::{field_name, parameters, returned, split_name, Parameters, Returned};
use crate::program_model::MethodDef;

/// The binding of the Structured Text interface `ty` (RU-39), with each `(interface, method)` whose
/// dispatcher it calls, which layer 1 holds with the methods the dispatcher reaches (RU-40).
///
/// # Errors
///
/// A method with a parameter or a result of a type the bindings do not pass yet, naming it.
pub(super) fn interface(
    model: &mut Model<'_>,
    ty: Ty,
    data_types: &mut BTreeSet<Ty>,
) -> Result<(String, Vec<(SmolStr, SmolStr)>), NativeError> {
    let info = model.types.info(ty);
    if !matches!(info.kind, Kind::Interface) {
        return Ok((String::new(), Vec::new()));
    }
    let declared = info.name.clone();
    let name = split_name(&declared).1.to_string();
    // The methods of the interface and of the interfaces it extends, each name once, the nearest
    // declaration of it (IEC §6.6.6.6).
    let defs = model.types.defs;
    let mut declarations: Vec<&MethodDef> = Vec::new();
    for def in defs.interface_closure(&declared) {
        for method in &def.methods {
            if !declarations
                .iter()
                .any(|known| known.name.eq_ignore_ascii_case(&method.name))
            {
                declarations.push(method);
            }
        }
    }
    let mut methods = String::new();
    let mut dispatched = Vec::new();
    for method in declarations {
        methods.push_str(&method_binding(model, &declared, method, data_types)?);
        dispatched.push((declared.clone(), method.name.clone()));
    }
    let text = format!(
        "/// The Structured Text interface `{name}` of the project (specification 48 RU-39): the value of
/// a variable of the interface, a reference to an instance of a function block or a class that
/// implements it, or `NULL` (IEC §6.6.6.5). Its methods are the interface's and those of the
/// interfaces it extends; each runs on the program's memory, which a unit test's context does not
/// hold: there the call ends with `FaultCode::ModuleInternal` (RU-56).
#[derive(Clone, Copy, Debug)]
pub struct {name} {{
    value: trust_plc_abi::RefValue,
}}

impl {name} {{
    /// `NULL`, which names no instance: the value a variable of the interface starts at
    /// (IEC §6.6.6.5.1).
    pub const NULL: Self = Self {{
        value: trust_plc_abi::RefValue::NULL_REF,
    }};

    /// Whether it is `NULL`.
    #[must_use]
    pub fn is_null(&self) -> bool {{
        self.value.kind == trust_plc_abi::RefValue::NULL
    }}

    /// The value that names the instance `place` names, a reference of kind instance
    /// (specification 44 §6.4); `NULL` for `NULL`.
    #[doc(hidden)]
    #[must_use]
    pub const fn __of(place: trust_plc_abi::RefValue) -> Self {{
        if place.kind == trust_plc_abi::RefValue::NULL {{
            return Self::NULL;
        }}
        Self {{
            value: trust_plc_abi::RefValue {{
                kind: trust_plc_abi::RefValue::INSTANCE,
                ..place
            }},
        }}
    }}
{methods}}}

/// Two values are equal when they name the same instance or are both `NULL` (IEC §6.6.6.5.1).
impl PartialEq for {name} {{
    fn eq(&self, other: &Self) -> bool {{
        __generated::same_target(self.value, other.value)
    }}
}}

impl Eq for {name} {{}}

impl trust_plc::Declared for {name} {{
    const DEFAULT: Self = Self::NULL;
}}

impl Default for {name} {{
    fn default() -> Self {{
        Self::NULL
    }}
}}

impl trust_plc::Value for {name} {{}}

/// A variable of the interface lies as a reference (specification 44 §6.4, §7.3).
impl trust_plc::Place<{name}> for core::cell::Cell<trust_plc_abi::RefValue> {{
    fn load(&self) -> {name} {{
        {name} {{ value: self.get() }}
    }}

    fn store(&self, value: &{name}) {{
        self.set(value.value);
    }}
}}

"
    );
    Ok((text, dispatched))
}

/// That an instance of the block or class `block` is a value of the interface `interface`, which it
/// implements (IEC §6.6.6.5.1; RU-39): `st::I_RUN::from(&self.motor)`, the reference to the
/// instance its binding names. Both are named by their paths from layer 1's root.
pub(super) fn conversion(block: &str, interface: &str) -> String {
    let block_path = format!("crate::{}", block.replace('.', "::"));
    let interface_path = format!("crate::{}", interface.replace('.', "::"));
    format!(
        "/// An instance of `{block}` is a value of the interface `{interface}`, which it implements: the
/// reference to it (IEC §6.6.6.5.1; specification 48 RU-39).
impl From<&{block_path}> for {interface_path} {{
    fn from(block: &{block_path}) -> Self {{
        Self::__of(block.__place())
    }}
}}

"
    )
}

/// The binding of the method `def` of the interface `interface` (RU-39): a method of the
/// interface's binding that runs the interface's dispatcher on the instance the value names, as
/// Structured Text's call through the interface does, its parameters and its result passed as a
/// function's.
///
/// # Errors
///
/// A parameter or a result of a type the bindings do not pass yet, naming the method.
fn method_binding(
    model: &mut Model<'_>,
    interface: &str,
    def: &MethodDef,
    data_types: &mut BTreeSet<Ty>,
) -> Result<String, NativeError> {
    let unsupported = |what: String| {
        NativeError::Compile(format!(
            "st::{interface}: the method {interface}.{} {what}, which a call from Rust does not \
             pass yet (specification 48 RU-39, milestone R3)",
            def.name
        ))
    };
    let params = parameters(model, &def.params, data_types, unsupported)?;
    let callee = interface_function(interface, &def.name);
    // The interface's value follows the parameters in `Call`, whose indices they keep.
    let receiver = format!(", &self.{}", params.values.len());
    let Returned {
        returns,
        raw,
        run,
        value,
        load,
    } = returned(
        model,
        def.return_type,
        &callee,
        &receiver,
        &params,
        data_types,
        unsupported,
    )?;
    let Parameters {
        signature,
        mut fields,
        mut values,
        setup,
        in_outs,
        outputs,
        ..
    } = params;
    let generic = if in_outs.is_empty() && outputs.is_empty() {
        ""
    } else {
        "<'a>"
    };
    fields.push("trust_plc_abi::RefValue".to_string());
    values.push("self.value".to_string());
    Ok(format!(
        "
    /// The method `{declared}` of the interface (IEC §6.6.6.3), called on the instance the value
    /// names: it reaches the method that the instance's class has, as Structured Text's call
    /// through the interface does (IEC §6.6.8; specification 48 RU-39).
    ///
    /// # Errors
    ///
    /// The fault of the call, which ends the scan: `NULL` or a value that names no instance
    /// (IEC §6.6.6.5.2), or the fault of the method's Structured Text.
    pub fn {method}{generic}(&self, cx: &mut trust_plc::Cx{signature}) -> Result<{returns}, trust_plc::Fault> {{
        struct Call{generic}({fields});
        impl{generic} trust_plc_abi::StructuredTextCall for Call{generic} {{
            type Output = {raw};
            fn call(mut self, cx: &mut trust_plc_abi::Cx) -> Result<{raw}, trust_plc_abi::Fault> {{
                trust_plc_abi::module::with_areas(cx, |cx, areas| {{
                    let w = __generated::World::new(cx, &areas)?;
{setup}                {run}                Ok({value})
                }})
            }}
        }}
        let result = cx.__call_structured_text(Call({values}))?;
        Ok({load})
    }}
",
        declared = def.name,
        method = field_name(&def.name),
        fields = fields.join(", "),
        values = values.join(", "),
    ))
}
