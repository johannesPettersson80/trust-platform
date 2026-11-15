//! A Rust function block's method (specification 48 RU-32): a `#[trust_method]` function of an
//! `impl` block of the block's struct, read for its Structured Text declaration.

use proc_macro2::TokenStream;
use syn::ext::IdentExt as _;
use syn::spanned::Spanned;
use syn::{Error, FnArg, ImplItemFn, Pat, ReceiverKind, Type};

use crate::{names, types, DataType, Section, Variable};

/// The access specifier of a method (IEC §6.6.5.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// `PUBLIC`, which a method without an argument has.
    Public,
    /// `PRIVATE`: `#[trust_method(private)]`.
    Private,
    /// `PROTECTED`: `#[trust_method(protected)]`.
    Protected,
    /// `INTERNAL`: `#[trust_method(internal)]`.
    Internal,
}

impl Access {
    /// The specifier as the declaration writes it. A method without one is `PROTECTED` in
    /// Structured Text, so a public method's is written too.
    #[must_use]
    pub fn iec(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
            Self::Protected => "PROTECTED",
            Self::Internal => "INTERNAL",
        }
    }
}

/// A method of a Rust function block, as `#[trust_method]` declares it.
#[derive(Clone, Debug, PartialEq)]
pub struct Method {
    /// Its name, the Rust function's, which Structured Text uses too.
    pub name: String,
    /// The Rust function's name as Rust writes it, without `r#`.
    pub item: String,
    /// Its access specifier.
    pub access: Access,
    /// The longest one call may take, in nanoseconds (`#[budget]`, RU-46).
    pub budget_ns: u64,
    /// Its inputs, in-outs and outputs in the order of its parameters after the context.
    pub variables: Vec<Variable>,
    /// Its result; `None` for a method that returns `()`.
    pub result: Option<DataType>,
}

/// Reads the method `item`, whose `#[trust_method]` has the arguments `args`.
///
/// # Errors
///
/// An error at the part of the method that breaks a rule of specification 48 RU-32.
pub fn method(args: TokenStream, item: &ImplItemFn) -> Result<Method, Error> {
    let signature = &item.sig;
    let access = access(args)?;
    crate::public(&item.vis, &signature.ident, "method")?;
    if signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(Error::new(
            signature.ident.span(),
            "a Rust method is a plain `pub fn`: not `const`, `async` or `extern`",
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(Error::new(
            signature.generics.span(),
            "a Rust method has no generic parameters: Structured Text calls it with one signature \
             (specification 48 RU-32)",
        ));
    }
    let item_name = signature.ident.unraw().to_string();
    if names::is_reserved(&item_name) {
        return Err(Error::new(
            signature.ident.span(),
            format!("{}: rename the method", names::reserved_reason(&item_name)),
        ));
    }
    let mut parameters = signature.inputs.iter();
    let receiver = parameters.next().is_some_and(|first| match first {
        FnArg::Receiver(receiver) => {
            matches!(receiver.kind, ReceiverKind::Reference(_, _, Some(_)))
        }
        FnArg::Typed(_) => false,
    });
    if !receiver {
        return Err(Error::new(
            signature
                .inputs
                .first()
                .map_or(signature.ident.span(), Spanned::span),
            "a Rust method's first parameter is `&mut self`, the instance it runs on \
             (specification 48 RU-32)",
        ));
    }
    let context = parameters.next().is_some_and(|second| match second {
        FnArg::Typed(typed) => crate::is_context(&typed.ty),
        FnArg::Receiver(_) => false,
    });
    if !context {
        return Err(Error::new(
            signature.ident.span(),
            "a Rust method's second parameter is its context, `cx: &mut Cx` (specification 48 \
             RU-32)",
        ));
    }
    let mut variables = Vec::new();
    for parameter in parameters {
        let FnArg::Typed(typed) = parameter else {
            return Err(Error::new(parameter.span(), "a method has one `self`"));
        };
        let ident = match &*typed.pat {
            Pat::Ident(pattern) if pattern.by_ref.is_none() && pattern.subpat.is_none() => {
                &pattern.ident
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "a Rust method's parameter is a name with its type, `step: f64`, which \
                     Structured Text names it by",
                ))
            }
        };
        let (section, ty) = parameter_section(&typed.ty)?;
        let variable = Variable {
            name: crate::variable_name(ident)?,
            section,
            retain: false,
            ty,
            init: None,
        };
        crate::add(&mut variables, variable, ident.span())?;
    }
    Ok(Method {
        name: item_name.clone(),
        item: item_name,
        access,
        budget_ns: crate::budget::of(&item.attrs, signature.ident.span())?,
        variables,
        result: crate::result(&signature.output, signature.ident.span())?,
    })
}

/// The section and the type of a method's parameter of type `ty`: an in-out `InOut<T>`, an output
/// `&mut T`, else an input by value.
fn parameter_section(ty: &Type) -> Result<(Section, DataType), Error> {
    if let Some(target) = types::wrapped(ty, "InOut") {
        return Ok((Section::InOut, DataType::of(target)?));
    }
    if let Type::Reference(reference) = ty
        && reference.mutability.is_some()
    {
        return Ok((Section::Output, DataType::of(&reference.elem)?));
    }
    Ok((Section::Input, DataType::of(ty)?))
}

/// The access specifier `#[trust_method]`'s arguments `args` give: none, `private`, `protected`
/// or `internal`.
fn access(args: TokenStream) -> Result<Access, Error> {
    let mut access = None;
    let parser = syn::meta::parser(|meta| {
        let given = if meta.path.is_ident("private") {
            Access::Private
        } else if meta.path.is_ident("protected") {
            Access::Protected
        } else if meta.path.is_ident("internal") {
            Access::Internal
        } else {
            return Err(meta.error(
                "`#[trust_method]` takes one access specifier: `private`, `protected` or \
                 `internal`; a method without one is public",
            ));
        };
        if access.replace(given).is_some() {
            return Err(meta.error("a method has one access specifier"));
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, args)?;
    Ok(access.unwrap_or(Access::Public))
}

#[cfg(test)]
#[path = "method_tests.rs"]
mod tests;
