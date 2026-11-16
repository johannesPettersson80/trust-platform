//! The bindings of the project's data types in layer 1 (specification 48 RU-39): a structure as a
//! Rust struct of its fields, an enumeration as a Rust enum of its values, and a type with named
//! values as a Rust struct of one value of its base with each named value as its constant; each
//! with its default, the value a variable of it starts at, and its place, the generated code's
//! cells, through which a call and a held block pass it. A type the crate declares has such a
//! binding too, which layer 1, compiled before the crate, holds beside the crate's own.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use smol_str::SmolStr;

use super::super::model::Model;
use super::super::types::{Kind, Ty};
use super::super::NativeError;
use super::st_bindings::{field_name, load, passed, rust_type, store, Scope};
use super::st_defaults;

/// The binding of the project's data type `ty`; empty for another type.
///
/// # Errors
///
/// [`NativeError`] for a default the build cannot work out.
pub(super) fn binding(
    model: &mut Model<'_>,
    ty: Ty,
    data_types: &mut BTreeSet<Ty>,
) -> Result<String, NativeError> {
    match &model.types.info(ty).kind {
        Kind::Struct { .. } => structure(model, ty, data_types),
        Kind::Enum { values, named, .. } if is_open(values, *named) => named_values(model, ty),
        Kind::Enum { .. } => enumeration(model, ty),
        _ => Ok(String::new()),
    }
}

/// Whether the binding of an enumeration of `values` is a Rust struct of one value with its names
/// as constants: a type with named values, which holds every value of its base, and an
/// enumeration that gives two of its names one value, which a Rust enum cannot hold (RU-39).
pub(super) fn is_open(values: &[(SmolStr, i64)], named: bool) -> bool {
    named
        || values
            .iter()
            .map(|(_, value)| value)
            .collect::<BTreeSet<_>>()
            .len()
            != values.len()
}

/// The binding of the Structured Text structure `ty`: a Rust struct of its fields, its default,
/// the value a variable of it starts at, and its place, the generated code's structure, through
/// which a call and a held block pass it (RU-39).
///
/// # Errors
///
/// [`NativeError`] for a default the build cannot work out.
fn structure(
    model: &mut Model<'_>,
    ty: Ty,
    data_types: &mut BTreeSet<Ty>,
) -> Result<String, NativeError> {
    let default = st_defaults::default(model, ty)?;
    let types = &model.types;
    let info = types.info(ty);
    let Kind::Struct { members } = &info.kind else {
        return Ok(String::new());
    };
    let name = &info.name;
    let mut fields = String::new();
    let mut loads = Vec::new();
    let mut stores = String::new();
    for member in members {
        let field = field_name(&member.name);
        let rust = rust_type(types, member.ty, data_types).unwrap_or_default();
        let _ = writeln!(fields, "    pub {field}: {rust},");
        let cells = format!("self.{}", member.field());
        loads.push(format!(
            "{field}: {}",
            load(types, member.ty, &cells, Scope::Bindings)
        ));
        let _ = writeln!(
            stores,
            "        {}",
            store(
                types,
                member.ty,
                &cells,
                &format!("value.{field}"),
                Scope::Bindings
            )
        );
    }
    Ok(format!(
        "/// The Structured Text structure `{name}` of the project (specification 48 RU-39).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct {name} {{
{fields}}}

impl trust_plc::Declared for {name} {{
    const DEFAULT: Self = {default};
}}

impl Default for {name} {{
    fn default() -> Self {{
        <Self as trust_plc::Declared>::DEFAULT
    }}
}}

impl trust_plc::Place<{name}> for __generated::{cells} {{
    fn load(&self) -> {name} {{
        {name} {{ {loads} }}
    }}

    fn store(&self, value: &{name}) {{
{stores}    }}
}}

",
        cells = info.rust,
        loads = loads.join(", "),
    ))
}

/// The binding of the Structured Text enumeration `ty`: a Rust enum of its values, each with its
/// value, which a variable of it holds; a value none of them has, which only a write through a
/// pointer can leave, reads as its default (RU-25, RU-39).
///
/// # Errors
///
/// [`NativeError`] for a default the build cannot work out.
fn enumeration(model: &mut Model<'_>, ty: Ty) -> Result<String, NativeError> {
    let default = st_defaults::default(model, ty)?;
    let info = model.types.info(ty);
    let Kind::Enum { base, values, .. } = &info.kind else {
        return Ok(String::new());
    };
    let name = &info.name;
    let raw = base.stored();
    let mut variants = String::new();
    let mut loads = String::new();
    for (value, number) in values {
        let variant = field_name(value);
        let _ = writeln!(variants, "    {variant} = {number},");
        let _ = writeln!(loads, "            {number} => {name}::{variant},");
    }
    Ok(format!(
        "/// The Structured Text enumeration `{name}` of the project (specification 48 RU-39).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr({raw})]
pub enum {name} {{
{variants}}}

impl trust_plc::Declared for {name} {{
    const DEFAULT: Self = {default};
}}

impl Default for {name} {{
    fn default() -> Self {{
        <Self as trust_plc::Declared>::DEFAULT
    }}
}}

impl trust_plc::Place<{name}> for core::cell::Cell<{raw}> {{
    fn load(&self) -> {name} {{
        match self.get() {{
{loads}            _ => <{name} as trust_plc::Declared>::DEFAULT,
        }}
    }}

    fn store(&self, value: &{name}) {{
        self.set(*value as {raw});
    }}
}}

"
    ))
}

/// The binding of the Structured Text type with named values `ty`: a Rust struct of one value of
/// its base, since a variable of it holds every value of its base (IEC §6.4.4.3.1, specification
/// 02 §3.2), with each named value as its constant (RU-39).
///
/// # Errors
///
/// [`NativeError`] for a default the build cannot work out.
fn named_values(model: &mut Model<'_>, ty: Ty) -> Result<String, NativeError> {
    let default = st_defaults::default(model, ty)?;
    let info = model.types.info(ty);
    let Kind::Enum { base, values, .. } = &info.kind else {
        return Ok(String::new());
    };
    let name = &info.name;
    let raw = base.stored();
    let (rust, own) = passed(*base);
    let mut constants = String::new();
    for (value, number) in values {
        let _ = writeln!(
            constants,
            "    pub const {}: Self = Self({});",
            field_name(value),
            st_defaults::elementary_constant(*base, &number.to_le_bytes())
        );
    }
    // A base of the generated code's own type is its cell's; another goes through the SDK's place.
    let (load, store) = if own {
        ("self.get()".to_string(), "self.set(value.0);".to_string())
    } else {
        (
            format!("<Self as trust_plc::Place<{rust}>>::load(self)"),
            format!("<Self as trust_plc::Place<{rust}>>::store(self, &value.0);"),
        )
    };
    Ok(format!(
        "/// The Structured Text type with named values `{name}` of the project, which holds every
/// value of its base (specification 48 RU-39).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct {name}(pub {rust});

impl {name} {{
{constants}}}

impl trust_plc::Declared for {name} {{
    const DEFAULT: Self = {default};
}}

impl Default for {name} {{
    fn default() -> Self {{
        <Self as trust_plc::Declared>::DEFAULT
    }}
}}

impl trust_plc::Place<{name}> for core::cell::Cell<{raw}> {{
    fn load(&self) -> {name} {{
        {name}({load})
    }}

    fn store(&self, value: &{name}) {{
        {store}
    }}
}}

"
    ))
}
