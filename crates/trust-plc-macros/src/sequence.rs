//! `#[trust_sequence]` (specification 48 §15, RU-71): on the `impl` block that holds a POU's
//! sequence, it declares the type of the sequence's state, `<Pou>Sequence`, which the POU's
//! attribute gives the POU as its last variable `_seq`, and lowers the `async fn` to a method of
//! the same name that runs one activation (see the `lower` module); each child sequence of the
//! block (RU-72) has a state of its own, `<Pou>Sequence<Child>`, in a field of `_seq`. An attribute
//! on a function inside the block could not declare the type, which is why the attribute stands on
//! the block.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt as _;
use syn::spanned::Spanned;
use syn::{Error, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, Pat, Type};
use trust_plc_decl::{DataType, Elementary};

use crate::expand::value;
use crate::lower::{self, Hoisted};

/// The type of the state of the POU `pou`'s sequence, `<Pou>Sequence`, which both attributes
/// name: `#[trust_sequence]` declares it, the POU's attribute gives `_seq` that type (RU-71).
pub(crate) fn state_type(pou: &Ident) -> Ident {
    format_ident!(
        "{}",
        trust_plc_decl::sequence::state_item(&pou.unraw().to_string())
    )
}

/// `#[trust_sequence]`: the state's type, and the block with its sequence lowered.
pub(crate) fn expand(args: &TokenStream, item: &TokenStream) -> TokenStream {
    let Ok(mut block) = syn::parse2::<ItemImpl>(item.clone()) else {
        let error = Error::new(
            item.span(),
            "`#[trust_sequence]` stands on the `impl` block of a POU's sequence: \
             `#[trust_sequence] impl Palletizer { async fn run(&mut self, s: &mut Seq) -> \
             Result<(), Fault> { .. } }` (specification 48 RU-71)",
        )
        .to_compile_error();
        return quote! { #error };
    };
    if !args.is_empty() {
        let error =
            Error::new(args.span(), "`#[trust_sequence]` takes no arguments").to_compile_error();
        return quote! { #error #block };
    }
    match lowered(&mut block) {
        Ok(state) => quote! { #state #block },
        Err((error, pou)) => {
            // The refusal, with the state's type and the sequence as plain methods, so that the
            // compiler names only the refusal.
            let error = error.to_compile_error();
            let state = pou.map(|pou| {
                let document =
                    format!("The state of the sequence of `{pou}` (specification 48 RU-71).");
                declaration(
                    &state_type(&pou),
                    &document,
                    false,
                    &format_ident!("shape_00000000"),
                    &[],
                    &[],
                    &[],
                )
            });
            stub_sequences(&mut block);
            quote! { #error #state #block }
        }
    }
}

/// Lowers the block's sequences in place, the POU's and its children, and gives the declarations of
/// their states' types: the POU's, with a field for each child's after its locals, then each
/// child's, in the block's order (RU-71).
fn lowered(block: &mut ItemImpl) -> Result<TokenStream, (Error, Option<Ident>)> {
    let pou = pou_of(block).map_err(|error| (error, None))?;
    let fail = |error: Error| (error, Some(pou.clone()));
    let read = trust_plc_decl::sequence::block(block).map_err(fail)?;
    let item = pou.unraw().to_string();
    let mut children: Vec<(Ident, Ident)> = Vec::new();
    let mut child_states = TokenStream::new();
    let mut own = None;
    for index in 0..block.items.len() {
        let Some(ImplItem::Fn(function)) = block.items.get(index) else {
            continue;
        };
        if function.sig.asyncness.is_none() {
            continue;
        }
        let name = function.sig.ident.unraw().to_string();
        let (sequence, role) = if name == read.sequence.name {
            (&read.sequence, lower::Role::Pou)
        } else if let Some(child) = read.children.iter().find(|child| child.name == name) {
            (child, lower::Role::Child(function.sig.ident.clone()))
        } else {
            continue;
        };
        let context = context_of(function).map_err(fail)?;
        let hoisted = lower::locals(&function.block);
        let starts: Vec<TokenStream> = sequence
            .locals
            .iter()
            .map(|local| value(&local.ty, None))
            .collect();
        if starts.len() != hoisted.len() {
            return Err(fail(Error::new(
                function.sig.ident.span(),
                "the sequence's locals could not be read alike by the reader and the lowering",
            )));
        }
        let shape = format_ident!("{}", trust_plc_decl::sequence::shape(sequence));
        let lowered = lower::lower(function, &context, &hoisted, &role, &shape).map_err(fail)?;
        let lowered: ImplItem = syn::parse2(lowered).map_err(fail)?;
        let all = sequence.holds_all();
        if let lower::Role::Child(field) = role {
            let ty = format_ident!(
                "{}",
                trust_plc_decl::sequence::child_state_item(&item, &name)
            );
            let document = format!(
                "The state of the child sequence `{name}` of `{pou}`, the field `_seq.{name}` \
                 (specification 48 RU-71): where it waits and its locals."
            );
            child_states.extend(declaration(
                &ty,
                &document,
                all,
                &shape,
                &hoisted,
                &starts,
                &[],
            ));
            children.push((field, ty));
        } else {
            own = Some((name, all, shape, hoisted, starts));
        }
        if let Some(slot) = block.items.get_mut(index) {
            *slot = lowered;
        }
    }
    let Some((name, all, shape, hoisted, starts)) = own else {
        return Err(fail(Error::new(
            block.impl_token.span(),
            "the POU's sequence went missing",
        )));
    };
    let document = format!(
        "The state of the sequence `{name}` of `{pou}`, its variable `_seq` (specification 48 \
         RU-71): where it waits, its locals and its child sequences' states."
    );
    let state = declaration(
        &state_type(&pou),
        &document,
        all,
        &shape,
        &hoisted,
        &starts,
        &children,
    );
    Ok(quote! { #state #child_states })
}

/// The struct the block's `impl` is for: the POU.
fn pou_of(block: &ItemImpl) -> Result<Ident, Error> {
    match &*block.self_ty {
        Type::Path(path) if path.qself.is_none() => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.clone())
            .ok_or_else(|| Error::new(path.span(), "the block is for no POU")),
        other => Err(Error::new(
            other.span(),
            "`#[trust_sequence]` stands on the `impl` block of a POU's struct",
        )),
    }
}

/// The name of the wait points' context, `s` in `s: &mut Seq`, which the reader has checked.
fn context_of(function: &ImplItemFn) -> Result<Ident, Error> {
    match function.sig.inputs.iter().nth(1) {
        Some(FnArg::Typed(typed)) => match &*typed.pat {
            Pat::Ident(ident) => Ok(ident.ident.clone()),
            other => Err(Error::new(
                other.span(),
                "the wait points' context is a name, `s: &mut Seq`",
            )),
        },
        _ => Err(Error::new(
            function.sig.inputs.span(),
            "a sequence takes its wait points' context after `&mut self`: `s: &mut Seq`",
        )),
    }
}

/// The declaration of the type `ty` of a sequence's state, documented by `document`: the wait
/// point, the time its wait began, the members of an `s.all` that have completed when the body holds
/// one (`all`), its shape, the member `shape` (RU-73), the locals, and for the POU's sequence the
/// state of each child, `children` as the field and its type (RU-71, RU-72), with the defaults a
/// cold or a warm restart starts it at, its top, its shape `FALSE` until its first activation.
fn declaration(
    ty: &Ident,
    document: &str,
    all: bool,
    shape: &Ident,
    locals: &[Hoisted],
    starts: &[TokenStream],
    children: &[(Ident, Ident)],
) -> TokenStream {
    let names: Vec<&Ident> = locals.iter().map(|local| &local.name).collect();
    let types: Vec<&Type> = locals.iter().map(|local| &local.ty).collect();
    let documents: Vec<String> = names
        .iter()
        .map(|name| format!("The sequence's local `{name}`."))
        .collect();
    let (fields, child_types): (Vec<&Ident>, Vec<&Ident>) =
        children.iter().map(|(field, ty)| (field, ty)).unzip();
    let child_documents: Vec<String> = fields
        .iter()
        .map(|field| format!("The state of the child sequence `{}`.", field.unraw()))
        .collect();
    let began = value(&DataType::Elementary(Elementary::Time), None);
    let (done, done_start) = if all {
        (
            quote! {
                /// The members of the `s.all` the sequence waits at that have completed, bit 0
                /// for the first; 0 at every other wait point (specification 48 RU-72).
                pub done: ::trust_plc::Word,
            },
            quote! { done: ::trust_plc::Word(0), },
        )
    } else {
        (TokenStream::new(), TokenStream::new())
    };
    quote! {
        #[doc = #document]
        #[derive(Clone, Copy, Debug)]
        pub struct #ty {
            /// The wait point the sequence waits at: 0 at its top, then the wait point's number
            /// among the body's wait points, from 1 (specification 48 RU-72).
            pub point: u16,
            /// The logical time of the activation that began the wait.
            pub began: ::trust_plc::Time,
            #done
            /// Whether the sequence has run with these wait points and locals, whose signature the
            /// member's name carries: an online change that changes them makes it new, `false`,
            /// and the next activation starts the sequence at its top (specification 48 RU-73).
            pub #shape: bool,
            #(
                #[doc = #documents]
                pub #names: #types,
            )*
            #(
                #[doc = #child_documents]
                pub #fields: #child_types,
            )*
        }

        impl ::trust_plc::Declared for #ty {
            const DEFAULT: Self = Self {
                point: 0,
                began: #began,
                #done_start
                #shape: false,
                #(#names: #starts,)*
                #(#fields: <#child_types as ::trust_plc::Declared>::DEFAULT,)*
            };
        }

        impl ::trust_plc::Value for #ty {}
    }
}

/// The block's `async fn`s as plain methods that do nothing, after a refusal.
fn stub_sequences(block: &mut ItemImpl) {
    for item in &mut block.items {
        if let ImplItem::Fn(function) = item
            && function.sig.asyncness.take().is_some()
        {
            function.block = syn::parse_quote!({ ::core::result::Result::Ok(()) });
        }
    }
}

#[cfg(test)]
#[path = "sequence_tests.rs"]
mod tests;
