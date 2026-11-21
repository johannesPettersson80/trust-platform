//! `#[trust_sequence]` (specification 48 §15, RU-71): on the `impl` block that holds a POU's
//! sequence, it declares the type of the sequence's state, `<Pou>Sequence`, which the POU's
//! attribute gives the POU as its last variable `_seq`, and lowers the `async fn` to a method of
//! the same name that runs one activation (see the `lower` module). An attribute on a function
//! inside the block could not declare the type, which is why the attribute stands on the block.

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
            let state = pou.map(|pou| state(&pou, "", false, &[], &[]));
            stub_sequences(&mut block);
            quote! { #error #state #block }
        }
    }
}

/// Lowers the block's sequence in place and gives the declaration of its state's type.
fn lowered(block: &mut ItemImpl) -> Result<TokenStream, (Error, Option<Ident>)> {
    let pou = pou_of(block).map_err(|error| (error, None))?;
    let fail = |error: Error| (error, Some(pou.clone()));
    if block.trait_.is_some() {
        return Err(fail(Error::new(
            block.impl_token.span(),
            "`#[trust_sequence]` stands on the POU's own `impl` block, not on a trait's",
        )));
    }
    let sequences: Vec<usize> = block
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            ImplItem::Fn(function) if function.sig.asyncness.is_some() => Some(index),
            _ => None,
        })
        .collect();
    let index =
        match sequences.as_slice() {
            [index] => *index,
            [] => {
                return Err(fail(Error::new(
                    block.impl_token.span(),
                    "a `#[trust_sequence]` block holds the POU's sequence, an `async fn name(&mut \
                 self, s: &mut Seq) -> Result<(), Fault>` (specification 48 RU-70)",
                )))
            }
            [_, second, ..] => return Err(fail(Error::new(
                block
                    .items
                    .get(*second)
                    .map_or(block.impl_token.span(), Spanned::span),
                "a `#[trust_sequence]` block holds one sequence: a child sequence (specification \
                 48 RU-72) is not lowered yet",
            ))),
        };
    let Some(ImplItem::Fn(function)) = block.items.get(index) else {
        return Err(fail(Error::new(
            block.impl_token.span(),
            "the sequence went missing",
        )));
    };
    let sequence = trust_plc_decl::sequence::sequence(function).map_err(fail)?;
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
    let lowered = lower::lower(function, &context, &hoisted).map_err(fail)?;
    let lowered: ImplItem = syn::parse2(lowered).map_err(fail)?;
    let name = function.sig.ident.to_string();
    if let Some(item) = block.items.get_mut(index) {
        *item = lowered;
    }
    Ok(state(&pou, &name, sequence.holds_all(), &hoisted, &starts))
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

/// The declaration of the state's type: the wait point, the time its wait began, the members of an
/// `s.all` that have completed when the body holds one (`all`), and the locals (RU-71, RU-72), with
/// the defaults a cold or a warm restart starts it at, its top (RU-73).
fn state(
    pou: &Ident,
    sequence: &str,
    all: bool,
    locals: &[Hoisted],
    starts: &[TokenStream],
) -> TokenStream {
    let state = state_type(pou);
    let names: Vec<&Ident> = locals.iter().map(|local| &local.name).collect();
    let types: Vec<&Type> = locals.iter().map(|local| &local.ty).collect();
    let documents: Vec<String> = names
        .iter()
        .map(|name| format!("The sequence's local `{name}`."))
        .collect();
    let document = format!(
        "The state of the sequence `{sequence}` of `{pou}`, its variable `_seq` (specification 48 \
         RU-71): where it waits and its locals."
    );
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
        pub struct #state {
            /// The wait point the sequence waits at: 0 at its top, then the wait point's number
            /// among the body's wait points, from 1 (specification 48 RU-72).
            pub point: u16,
            /// The logical time of the activation that began the wait.
            pub began: ::trust_plc::Time,
            #done
            #(
                #[doc = #documents]
                pub #names: #types,
            )*
        }

        impl ::trust_plc::Declared for #state {
            const DEFAULT: Self = Self { point: 0, began: #began, #done_start #(#names: #starts),* };
        }

        impl ::trust_plc::Value for #state {}
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
