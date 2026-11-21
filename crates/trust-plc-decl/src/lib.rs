//! The Structured Text declarations of truST's Rust POUs (specification 48 §6.1).
//!
//! A Rust function or function block of a truST project is an item with an attribute of
//! `trust-plc-macros`: `#[trust_function(name = "...")]` on a function, `#[trust_fb(name = "...")]`
//! on a struct, each with its `#[budget("...")]`. This crate reads such an item into the POU it
//! declares, with its Structured Text name, its budget and its variables with their sections, IEC
//! types and initial values, and writes the Structured Text declaration the rest of the project sees
//! (RU-23). The attributes check an item with it and the build's declarations writer reads the
//! crate's sources with it, so the compiled code and the declarations agree by construction (RU-24).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod budget;
mod dates;
mod enumeration;
mod literal;
mod method;
pub mod names;
mod render;
pub mod sequence;
mod structure;
mod types;

use proc_macro2::{Span, TokenStream};
use syn::ext::IdentExt as _;
use syn::spanned::Spanned;
use syn::{
    Attribute, Error, Fields, FnArg, Ident, ItemFn, ItemStruct, LitStr, Pat, ReturnType, Type,
    Visibility,
};

pub use enumeration::{enumeration, is_enumeration, Enumeration};
pub use literal::Value;
pub use method::{method, Access, Method};
pub use structure::{is_structure, structure, Field, Structure};
pub use types::{DataType, Elementary};

/// What a Rust POU is (specification 48 RU-22).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A function, `#[trust_function]`: called with its inputs, it returns its result and keeps
    /// nothing between calls.
    Function,
    /// A function block, `#[trust_fb]`: a struct whose instance lies in the program's areas
    /// (RU-27).
    FunctionBlock,
    /// A program, `#[trust_program]`: a struct, as a function block, that a configuration runs
    /// (RU-35).
    Program,
}

/// The section of a variable (IEC §6.5.2, Table 13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// `VAR_INPUT`: a field with `#[input]`, or a function's parameter.
    Input,
    /// `VAR_OUTPUT`: a field with `#[output]`.
    Output,
    /// `VAR`: a field with neither.
    Var,
    /// `VAR_IN_OUT`: a function's parameter `InOut<T>`, or a function block's field `#[in_out]` of
    /// type `Ref<T>` (specification 48 RU-29).
    InOut,
}

/// A variable of a Rust POU.
#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    /// Its name, the field's or the parameter's, which Structured Text uses too.
    pub name: String,
    /// Its section.
    pub section: Section,
    /// Whether it is retained (`#[retain]`): it then lies in the retained area (RU-27).
    pub retain: bool,
    /// Its IEC type (RU-25).
    pub ty: DataType,
    /// Its initial value (`#[init(...)]`, RU-26); `None` starts it at its type's default.
    pub init: Option<Value>,
}

/// A Rust POU, as its attributes declare it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pou {
    /// Function or function block.
    pub kind: Kind,
    /// The Structured Text name, the attribute's `name`, with its namespaces
    /// (`Motion.AXIS_CONTROLLER`, IEC §6.9).
    pub name: String,
    /// The Rust item's name: the struct's or the function's.
    pub item: String,
    /// The longest one call may take, in nanoseconds (`#[budget]`, RU-46).
    pub budget_ns: u64,
    /// The variables in declaration order: a function block's fields, a function's parameters
    /// after its context.
    pub variables: Vec<Variable>,
    /// A function's result; `None` for a function block and for a function that returns `()`.
    pub result: Option<DataType>,
    /// A function block's methods (RU-32), which its `impl` blocks declare: none as its own
    /// attribute reads it, the crate's as the declarations' writer, which reads the whole crate,
    /// gives them.
    pub methods: Vec<Method>,
    /// The Structured Text interfaces a function block implements, `implements = "..."`, in their
    /// order (RU-33); none for a function.
    pub implements: Vec<String>,
    /// The sequence a function block's or a program's body runs, `sequence = "run"` (RU-71): the
    /// Rust name of the `async fn` its `#[trust_sequence]` block holds, whose state is the POU's
    /// last variable, `_seq`; `None` for a POU with its own `impl Body` and for a function.
    pub sequence: Option<String>,
}

impl Pou {
    /// The Structured Text declaration of the POU for the project's generated declarations
    /// (RU-23): its kind, name, sections and initial values, marked `{attribute 'rust'}` and
    /// without a body, after a comment that names `source`, the file the item lies in.
    #[must_use]
    pub fn declaration(&self, source: &str) -> String {
        render::declaration(self, source)
    }
}

/// The nanoseconds of a duration as a budget writes it: `500ns`, `15us`, `1.5ms`, `2s`; `None` for
/// another text, zero and a fraction of a nanosecond (RU-46).
#[must_use]
pub fn duration_nanoseconds(text: &str) -> Option<u64> {
    budget::nanoseconds(text)
}

/// The attributes of a function block's fields that this crate reads and the attribute removes.
const FIELD_ATTRIBUTES: [&str; 5] = ["input", "output", "in_out", "retain", "init"];

/// Whether `attribute` is one this crate reads on a function block's field: `#[input]`,
/// `#[output]`, `#[in_out]`, `#[retain]` or `#[init(...)]`.
#[must_use]
pub fn is_field_attribute(attribute: &Attribute) -> bool {
    FIELD_ATTRIBUTES
        .iter()
        .any(|name| attribute.path().is_ident(name))
}

/// Whether `attribute` is `#[trust_method(...)]`, also written with its crate's path (RU-32).
#[must_use]
pub fn is_method(attribute: &Attribute) -> bool {
    last_segment_is(attribute, "trust_method")
}

/// Whether `attribute` is `#[budget(...)]`, also written `#[trust_plc::budget(...)]`.
#[must_use]
pub fn is_budget(attribute: &Attribute) -> bool {
    last_segment_is(attribute, "budget")
}

/// The kind of POU `attribute` declares: `#[trust_fb]` a function block, `#[trust_function]` a
/// function, also written with their crate's path.
#[must_use]
pub fn declared_kind(attribute: &Attribute) -> Option<Kind> {
    if last_segment_is(attribute, "trust_fb") {
        Some(Kind::FunctionBlock)
    } else if last_segment_is(attribute, "trust_function") {
        Some(Kind::Function)
    } else if last_segment_is(attribute, "trust_program") {
        Some(Kind::Program)
    } else {
        None
    }
}

fn last_segment_is(attribute: &Attribute, name: &str) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

/// Reads the function block `item`, whose `#[trust_fb]` has the arguments `args`.
///
/// # Errors
///
/// An error at the part of the item that breaks a rule of specification 48 §6, naming the rule.
pub fn function_block(args: TokenStream, item: &ItemStruct) -> Result<Pou, Error> {
    let (name, implements, sequence) = instance_arguments(args, item.ident.span(), true)?;
    public(&item.vis, &item.ident, "function block")?;
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(Error::new(
            item.generics.span(),
            "a Rust function block has no generic parameters: its instance lies in the program's \
             areas with one layout (specification 48 RU-25)",
        ));
    }
    let fields: Vec<&syn::Field> = match &item.fields {
        Fields::Named(named) => named.named.iter().collect(),
        Fields::Unit => Vec::new(),
        Fields::Unnamed(unnamed) => {
            return Err(Error::new(
                unnamed.span(),
                "a Rust function block's variables are named fields: `pub struct Name { ... }`",
            ))
        }
    };
    let mut variables = Vec::with_capacity(fields.len());
    for field in fields {
        let Some(ident) = &field.ident else { continue };
        not_the_state(ident, sequence.as_deref())?;
        let variable = field_variable(ident, &field.ty, &field.attrs)?;
        add(&mut variables, variable, ident.span())?;
    }
    Ok(Pou {
        kind: Kind::FunctionBlock,
        name,
        item: item.ident.unraw().to_string(),
        budget_ns: budget::of(&item.attrs, item.ident.span())?,
        variables,
        result: None,
        methods: Vec::new(),
        implements,
        sequence,
    })
}

/// Reads the program `item`, whose `#[trust_program]` has the arguments `args` (specification 48
/// RU-35): a struct as a function block's, whose variables are its inputs, outputs, internal and
/// retained variables, without in-outs.
///
/// # Errors
///
/// An error at the part of the item that breaks a rule of specification 48 §7, naming the rule.
pub fn program(args: TokenStream, item: &ItemStruct) -> Result<Pou, Error> {
    let (name, _, sequence) = instance_arguments(args, item.ident.span(), false)?;
    public(&item.vis, &item.ident, "program")?;
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(Error::new(
            item.generics.span(),
            "a Rust program has no generic parameters: its instance lies in the program's areas \
             with one layout (specification 48 RU-35)",
        ));
    }
    let fields: Vec<&syn::Field> = match &item.fields {
        Fields::Named(named) => named.named.iter().collect(),
        Fields::Unit => Vec::new(),
        Fields::Unnamed(unnamed) => {
            return Err(Error::new(
                unnamed.span(),
                "a Rust program's variables are named fields: `pub struct Name { ... }`",
            ))
        }
    };
    let mut variables = Vec::with_capacity(fields.len());
    for field in fields {
        let Some(ident) = &field.ident else { continue };
        not_the_state(ident, sequence.as_deref())?;
        let variable = field_variable(ident, &field.ty, &field.attrs)?;
        if variable.section == Section::InOut {
            return Err(Error::new(
                ident.span(),
                "a Rust program's variables are its inputs, outputs, internal and retained \
                 variables, which its program connections bind: it has no in-out (specification \
                 48 RU-35)",
            ));
        }
        add(&mut variables, variable, ident.span())?;
    }
    Ok(Pou {
        kind: Kind::Program,
        name,
        item: item.ident.unraw().to_string(),
        budget_ns: budget::of(&item.attrs, item.ident.span())?,
        variables,
        result: None,
        methods: Vec::new(),
        implements: Vec::new(),
        sequence,
    })
}

/// Reads the function `item`, whose `#[trust_function]` has the arguments `args`.
///
/// # Errors
///
/// An error at the part of the item that breaks a rule of specification 48 §6, naming the rule.
pub fn function(args: TokenStream, item: &ItemFn) -> Result<Pou, Error> {
    let signature = &item.sig;
    let name = pou_name(args, signature.ident.span())?;
    public(&item.vis, &signature.ident, "function")?;
    if signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(Error::new(
            signature.ident.span(),
            "a Rust function is a plain `pub fn`: not `const`, `async` or `extern`",
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(Error::new(
            signature.generics.span(),
            "a Rust function has no generic parameters: Structured Text calls it with one \
             signature (specification 48 RU-25)",
        ));
    }
    let mut parameters = signature.inputs.iter();
    let context = parameters.next().is_some_and(|first| match first {
        FnArg::Typed(typed) => is_context(&typed.ty),
        FnArg::Receiver(_) => false,
    });
    if !context {
        return Err(Error::new(
            signature
                .inputs
                .first()
                .map_or(signature.ident.span(), Spanned::span),
            "a Rust function's first parameter is its context, `cx: &mut Cx` (specification 48 \
             RU-22)",
        ));
    }
    let mut variables = Vec::new();
    for parameter in parameters {
        let FnArg::Typed(typed) = parameter else {
            return Err(Error::new(
                parameter.span(),
                "a Rust function has no `self`",
            ));
        };
        let ident = match &*typed.pat {
            Pat::Ident(pattern) if pattern.by_ref.is_none() && pattern.subpat.is_none() => {
                &pattern.ident
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "a Rust function's parameter is a name with its type, `level: f64`, which \
                     Structured Text names it by",
                ))
            }
        };
        // An in-out is the caller's variable itself, borrowed for the call (RU-29).
        let (section, ty) = match types::wrapped(&typed.ty, "InOut") {
            Some(target) => (Section::InOut, DataType::of_value(target)?),
            None => (Section::Input, DataType::of_value(&typed.ty)?),
        };
        let variable = Variable {
            name: variable_name(ident)?,
            section,
            retain: false,
            ty,
            init: None,
        };
        add(&mut variables, variable, ident.span())?;
    }
    Ok(Pou {
        kind: Kind::Function,
        name,
        item: signature.ident.unraw().to_string(),
        budget_ns: budget::of(&item.attrs, signature.ident.span())?,
        variables,
        result: result(&signature.output, signature.ident.span())?,
        methods: Vec::new(),
        implements: Vec::new(),
        sequence: None,
    })
}

/// What a POU's attribute without `name` is told.
const POU_NAME_MISSING: &str = "a Rust POU names itself for Structured Text: \
                                `#[trust_fb(name = \"FILTER_EMA\")]` (specification 48 RU-22)";

/// The Structured Text name of a POU's attribute's arguments `args`, `name = "..."`.
fn pou_name(args: TokenStream, item: Span) -> Result<String, Error> {
    declared_name(args, item, POU_NAME_MISSING)
}

/// The arguments `args` of `#[trust_fb(...)]` (`block`) or `#[trust_program(...)]`: the POU's
/// Structured Text name, `name = "..."`; a block's interfaces, `implements = "I_RUN, I_RESET"`
/// (RU-33), none without it; and the sequence its body runs, `sequence = "run"` (RU-71).
fn instance_arguments(
    args: TokenStream,
    item: Span,
    block: bool,
) -> Result<(String, Vec<String>, Option<String>), Error> {
    let mut name: Option<LitStr> = None;
    let mut implements: Option<LitStr> = None;
    let mut sequence: Option<LitStr> = None;
    let parser = syn::meta::parser(|meta| {
        let (slot, key) = if meta.path.is_ident("name") {
            (&mut name, "name")
        } else if meta.path.is_ident("implements") && block {
            (&mut implements, "implements")
        } else if meta.path.is_ident("sequence") {
            (&mut sequence, "sequence")
        } else if meta.path.is_ident("implements") {
            return Err(meta.error(
                "`implements` belongs to `#[trust_fb]`: a function block implements interfaces \
                 (specification 48 RU-33)",
            ));
        } else if block {
            return Err(meta.error(
                "`#[trust_fb]` takes `name = \"...\"`, the block's Structured Text name, \
                 `implements = \"...\"`, the interfaces it implements (specification 48 RU-33), \
                 and `sequence = \"...\"`, the sequence its body runs (RU-71)",
            ));
        } else {
            return Err(meta.error(
                "`#[trust_program]` takes `name = \"...\"`, the program's Structured Text name, \
                 and `sequence = \"...\"`, the sequence its body runs (specification 48 RU-71)",
            ));
        };
        if slot.is_some() {
            return Err(meta.error(format!("`{key}` is given twice")));
        }
        *slot = Some(meta.value()?.parse()?);
        Ok(())
    });
    syn::parse::Parser::parse2(parser, args)?;
    let Some(name) = name else {
        return Err(Error::new(item, POU_NAME_MISSING));
    };
    let implements = match implements {
        Some(list) => interfaces(&list)?,
        None => Vec::new(),
    };
    let sequence = match sequence {
        Some(function) => {
            if syn::parse_str::<syn::Ident>(&function.value()).is_err() {
                return Err(Error::new(
                    function.span(),
                    "`sequence` names the `async fn` of the POU's `#[trust_sequence]` block by its \
                     Rust name: `sequence = \"run\"` (specification 48 RU-71)",
                ));
            }
            Some(function.value())
        }
        None => None,
    };
    Ok((
        structured_text_name(&name.value(), name.span())?,
        implements,
        sequence,
    ))
}

/// Refuses a variable named as a sequence's state, `_seq` in any case, of a POU whose body runs a
/// sequence: the state is the POU's last variable of that name (specification 48 RU-71, RU-72).
fn not_the_state(ident: &syn::Ident, sequence: Option<&str>) -> Result<(), Error> {
    if sequence.is_some()
        && ident
            .unraw()
            .to_string()
            .eq_ignore_ascii_case(sequence::STATE)
    {
        return Err(Error::new(
            ident.span(),
            "a POU that runs a sequence keeps the sequence's state in its variable `_seq` \
             (specification 48 RU-71): name this variable otherwise",
        ));
    }
    Ok(())
}

/// The interfaces `implements = "..."` names: Structured Text names separated by commas, each
/// once (IEC §6.6.6).
fn interfaces(list: &LitStr) -> Result<Vec<String>, Error> {
    let mut interfaces: Vec<String> = Vec::new();
    for entry in list.value().split(',').map(str::trim) {
        if entry.is_empty() {
            return Err(Error::new(
                list.span(),
                "`implements` names the interfaces the block implements, separated by commas: \
                 `implements = \"I_RUN, I_RESET\"`",
            ));
        }
        let interface = structured_text_name(entry, list.span())?;
        if let Some(other) = interfaces
            .iter()
            .find(|other| other.eq_ignore_ascii_case(&interface))
        {
            return Err(Error::new(
                list.span(),
                format!(
                    "`{other}` and `{interface}` are one interface in Structured Text, which \
                     ignores case (IEC §6.1.2): name it once"
                ),
            ));
        }
        interfaces.push(interface);
    }
    Ok(interfaces)
}

/// The Structured Text name of an attribute's arguments `args`, `name = "..."`, which `missing`
/// asks for when they have none.
fn declared_name(args: TokenStream, item: Span, missing: &str) -> Result<String, Error> {
    let mut name: Option<LitStr> = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("implements") {
            return Err(meta.error(
                "`implements` belongs to `#[trust_fb]`: a function block implements interfaces \
                 (specification 48 RU-33)",
            ));
        }
        if !meta.path.is_ident("name") {
            return Err(meta.error(
                "the attribute takes `name = \"...\"`, the Structured Text name of what it declares",
            ));
        }
        if name.is_some() {
            return Err(meta.error("`name` is given twice"));
        }
        name = Some(meta.value()?.parse()?);
        Ok(())
    });
    syn::parse::Parser::parse2(parser, args)?;
    let Some(name) = name else {
        return Err(Error::new(item, missing));
    };
    structured_text_name(&name.value(), name.span())
}

/// `text` as the Structured Text name of a POU, a type or an interface, at `span`: identifiers (IEC
/// §6.1.2) joined by `.` for its namespaces (IEC §6.9), none of them a reserved word.
fn structured_text_name(text: &str, span: Span) -> Result<String, Error> {
    if !text.split('.').all(is_identifier) {
        return Err(Error::new(
            span,
            format!(
                "`{text}` is not a Structured Text name: identifiers (IEC §6.1.2) joined by `.` \
                 for its namespaces (IEC §6.9)"
            ),
        ));
    }
    if let Some(word) = text.split('.').find(|segment| names::is_reserved(segment)) {
        return Err(Error::new(
            span,
            format!(
                "{}: no POU, type or namespace is named by it",
                names::reserved_reason(word)
            ),
        ));
    }
    Ok(text.to_string())
}

fn public(visibility: &Visibility, ident: &Ident, what: &str) -> Result<(), Error> {
    if matches!(visibility, Visibility::Public(_)) {
        Ok(())
    } else {
        Err(Error::new(
            ident.span(),
            format!(
                "a Rust {what} is `pub`: the program's generated code calls it from another crate \
                 (specification 48 RU-22)"
            ),
        ))
    }
}

/// The variable a function block's field declares.
fn field_variable(ident: &Ident, ty: &Type, attributes: &[Attribute]) -> Result<Variable, Error> {
    let name = variable_name(ident)?;
    let mut section = None;
    let mut retain = false;
    let mut init = None;
    for attribute in attributes {
        let path = attribute.path();
        if path.is_ident("input") || path.is_ident("output") || path.is_ident("in_out") {
            attribute.meta.require_path_only()?;
            if section.is_some() {
                return Err(Error::new(
                    attribute.span(),
                    "a variable has one section, `#[input]`, `#[output]` or `#[in_out]`",
                ));
            }
            section = Some(if path.is_ident("input") {
                Section::Input
            } else if path.is_ident("output") {
                Section::Output
            } else {
                Section::InOut
            });
        } else if path.is_ident("retain") {
            attribute.meta.require_path_only()?;
            if retain {
                return Err(Error::new(attribute.span(), "`#[retain]` is given twice"));
            }
            retain = true;
        } else if path.is_ident("init") {
            if init.is_some() {
                return Err(Error::new(attribute.span(), "`#[init]` is given twice"));
            }
            init = Some(attribute.parse_args::<syn::Expr>()?);
        }
    }
    let ty = if section == Some(Section::InOut) {
        // The block keeps the reference of its last call, of the in-out's type (RU-29).
        let Some(target) = types::wrapped(ty, "Ref") else {
            return Err(Error::new(
                ty.span(),
                "a function block's in-out keeps the reference of its last call: a field \
                 `#[in_out]` of type `Ref<T>`, `T` its type (specification 48 RU-29)",
            ));
        };
        if retain || init.is_some() {
            return Err(Error::new(
                ident.span(),
                "an in-out names the caller's variable: it takes no `#[retain]` and no `#[init]`",
            ));
        }
        DataType::of_value(target)?
    } else {
        // A held block or class is the POU's own variable (RU-39): rustc refuses one elsewhere, as
        // it is no `trust_plc::Value`, which the macros require of each `st::` type there.
        DataType::of(ty)?
    };
    let init = init
        .map(|value| literal::initial_of(&value, &ty))
        .transpose()?;
    Ok(Variable {
        name,
        section: section.unwrap_or(Section::Var),
        retain,
        ty,
        init,
    })
}

fn variable_name(ident: &Ident) -> Result<String, Error> {
    let name = ident.unraw().to_string();
    if !is_identifier(&name) {
        return Err(Error::new(
            ident.span(),
            format!(
                "`{name}` is not an IEC identifier (IEC §6.1.2), which Structured Text names the \
                 variable by: letters, digits and single underscores, not ending in one"
            ),
        ));
    }
    if names::is_reserved(&name) {
        return Err(Error::new(
            ident.span(),
            format!("{}: rename the variable", names::reserved_reason(&name)),
        ));
    }
    Ok(name)
}

/// Adds `variable`, refusing a name another variable has: Structured Text ignores case.
fn add(variables: &mut Vec<Variable>, variable: Variable, span: Span) -> Result<(), Error> {
    if let Some(other) = variables
        .iter()
        .find(|other| other.name.eq_ignore_ascii_case(&variable.name))
    {
        return Err(Error::new(
            span,
            format!(
                "`{}` and `{}` are one name in Structured Text, which ignores case (IEC §6.1.2)",
                other.name, variable.name
            ),
        ));
    }
    variables.push(variable);
    Ok(())
}

/// Whether `text` is an IEC identifier (IEC §6.1.2) as truST's lexer reads one: an ASCII letter or
/// an underscore, then letters, digits and underscores, never two underscores together nor one at
/// the end, and a letter in a name that starts with an underscore.
fn is_identifier(text: &str) -> bool {
    let bytes = text.as_bytes();
    let Some(&first) = bytes.first() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && (first != b'_' || bytes.iter().any(u8::is_ascii_alphabetic))
        && !text.ends_with('_')
        && !text.contains("__")
}

/// Whether `ty` is `&mut Cx`, the context of an activation.
fn is_context(ty: &Type) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    reference.mutability.is_some() && last_path_segment(&reference.elem) == Some("Cx".into())
}

/// The name of the last segment of the path type `ty`, when it has no generic arguments.
fn last_path_segment(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) if path.qself.is_none() => path
            .path
            .segments
            .last()
            .filter(|segment| segment.arguments.is_none())
            .map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

/// A function's result: `Result<T, Fault>` with `T` of RU-25's types or `()`.
fn result(output: &ReturnType, item: Span) -> Result<Option<DataType>, Error> {
    let refused = |span| {
        Error::new(
            span,
            "a Rust function returns `Result<T, Fault>`: a fault ends the scan as Structured \
             Text's faults do (specification 48 RU-14)",
        )
    };
    let ReturnType::Type(_, ty) = output else {
        return Err(refused(item));
    };
    let Type::Path(path) = &**ty else {
        return Err(refused(ty.span()));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(refused(ty.span()));
    };
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(refused(ty.span()));
    };
    let types: Vec<&Type> = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect();
    let [value, fault] = types.as_slice() else {
        return Err(refused(ty.span()));
    };
    if segment.ident != "Result" || last_path_segment(fault) != Some("Fault".into()) {
        return Err(refused(ty.span()));
    }
    match value {
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(None),
        other => DataType::of_value(other).map(Some),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "references_tests.rs"]
mod references_tests;

#[cfg(test)]
#[path = "interface_tests.rs"]
mod interface_tests;

#[cfg(test)]
#[path = "sequence_tests.rs"]
mod sequence_tests;
