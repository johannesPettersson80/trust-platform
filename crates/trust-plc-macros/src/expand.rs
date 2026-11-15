//! What the attributes expand to (specification 48 §6.1, RU-26, RU-28).

use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{Attribute, Fields, Item, ItemFn, ItemStruct};
use trust_plc_decl::{Elementary, Pou, Value, Variable};

/// `#[trust_fb]`: the struct without the attributes its reader reads, and its calls.
pub(crate) fn function_block(args: TokenStream, item: TokenStream) -> TokenStream {
    let Ok(mut parsed) = syn::parse2::<ItemStruct>(item.clone()) else {
        return refused(
            "`#[trust_fb]` declares a struct a Rust function block",
            item,
        );
    };
    let pou = trust_plc_decl::function_block(args, &parsed);
    remove_budget(&mut parsed.attrs);
    if let Fields::Named(fields) = &mut parsed.fields {
        for field in &mut fields.named {
            field
                .attrs
                .retain(|attribute| !trust_plc_decl::is_field_attribute(attribute));
        }
    }
    match pou {
        Ok(pou) => {
            let calls = calls(&parsed, &pou);
            quote! { #parsed #calls }
        }
        Err(error) => {
            let error = error.to_compile_error();
            quote! { #error #parsed }
        }
    }
}

/// `#[trust_function]`: the function as written, without its budget, which only the reader reads.
pub(crate) fn function(args: TokenStream, item: TokenStream) -> TokenStream {
    let Ok(mut parsed) = syn::parse2::<ItemFn>(item.clone()) else {
        return refused(
            "`#[trust_function]` declares a function a Rust function",
            item,
        );
    };
    let pou = trust_plc_decl::function(args, &parsed);
    remove_budget(&mut parsed.attrs);
    match pou {
        Ok(_) => quote! { #parsed },
        Err(error) => {
            let error = error.to_compile_error();
            quote! { #error #parsed }
        }
    }
}

/// `#[budget]` written above the POU's attribute expands first: it puts itself below that
/// attribute, which reads it. Without a POU's attribute it is refused.
pub(crate) fn budget(args: &TokenStream, item: TokenStream) -> TokenStream {
    const ALONE: &str = "`#[budget]` gives a Rust POU its budget: write it with `#[trust_fb]` or \
                         `#[trust_function]` (specification 48 RU-46)";
    let Ok(mut parsed) = syn::parse2::<Item>(item.clone()) else {
        return refused(ALONE, item);
    };
    let attributes = match &mut parsed {
        Item::Struct(item) => &mut item.attrs,
        Item::Fn(item) => &mut item.attrs,
        _ => return refused(ALONE, item),
    };
    if !attributes
        .iter()
        .any(|attribute| trust_plc_decl::declared_kind(attribute).is_some())
    {
        return refused(ALONE, item);
    }
    attributes.push(syn::parse_quote!(#[budget(#args)]));
    parsed.into_token_stream()
}

/// The error `message` at the attribute, with the item unchanged.
fn refused(message: &str, item: TokenStream) -> TokenStream {
    let mut expansion = syn::Error::new(Span::call_site(), message).to_compile_error();
    expansion.extend(item);
    expansion
}

fn remove_budget(attributes: &mut Vec<Attribute>) {
    attributes.retain(|attribute| !trust_plc_decl::is_budget(attribute));
}

/// A function block's `new`, `call` and the call of the generated code.
fn calls(item: &ItemStruct, pou: &Pou) -> TokenStream {
    let ident = &item.ident;
    let (fields, types): (Vec<_>, Vec<_>) = match &item.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| field.ident.as_ref().map(|ident| (ident, &field.ty)))
            .unzip(),
        Fields::Unit | Fields::Unnamed(_) => (Vec::new(), Vec::new()),
    };
    let starts: Vec<TokenStream> = pou.variables.iter().map(start).collect();
    let values: Vec<_> = (0..fields.len())
        .map(|index| format_ident!("v{index}"))
        .collect();
    quote! {
        impl #ident {
            /// The block's starting values: its declared initial values and its types' defaults
            /// (specification 48 RU-26).
            #[must_use]
            #[allow(clippy::new_without_default)]
            pub const fn new() -> Self {
                Self { #(#fields: #starts),* }
            }

            /// One call of the block as the kernel makes it (specification 48 RU-28): the body runs
            /// on a copy of the variables, which replaces them when it returns `Ok`, so that a fault
            /// leaves the block as it was.
            ///
            /// # Errors
            ///
            /// The body's fault, which ends the scan (specification 48 RU-14).
            pub fn call(
                &mut self,
                cx: &mut ::trust_plc::Cx,
            ) -> ::core::result::Result<(), ::trust_plc::Fault> {
                let mut next = Self { #(#fields: self.#fields),* };
                ::trust_plc::Body::body(&mut next, cx)?;
                *self = next;
                ::core::result::Result::Ok(())
            }

            /// The call the program's generated code makes, with the instance's variables in
            /// declaration order (specification 48 RU-28).
            ///
            /// # Errors
            ///
            /// The body's fault; the variables then keep their values.
            #[doc(hidden)]
            #[inline]
            #[allow(clippy::too_many_arguments)]
            pub fn __trust_call(
                cx: &mut ::trust_plc::Cx,
                #(#values: &mut #types),*
            ) -> ::core::result::Result<(), ::trust_plc::Fault> {
                let mut next = Self { #(#fields: *#values),* };
                ::trust_plc::Body::body(&mut next, cx)?;
                #(*#values = next.#fields;)*
                ::core::result::Result::Ok(())
            }
        }
    }
}

/// The starting value of `variable`: its initial value, else its type's default (IEC §6.4.2,
/// Table 10: FALSE, 0 and 0.0).
fn start(variable: &Variable) -> TokenStream {
    let signed = |negative: bool, literal: Literal| {
        if negative {
            quote!(-#literal)
        } else {
            literal.into_token_stream()
        }
    };
    match variable.init {
        Some(Value::Bool(value)) => quote!(#value),
        Some(Value::Integer(value)) => {
            signed(value < 0, Literal::u128_unsuffixed(value.unsigned_abs()))
        }
        Some(Value::Real(value)) => {
            signed(value.is_sign_negative(), Literal::f32_suffixed(value.abs()))
        }
        Some(Value::Lreal(value)) => {
            signed(value.is_sign_negative(), Literal::f64_suffixed(value.abs()))
        }
        None => match variable.ty {
            Elementary::Bool => quote!(false),
            Elementary::Real | Elementary::Lreal => quote!(0.0),
            _ => quote!(0),
        },
    }
}
