//! What the attributes expand to (specification 48 §6.1, RU-26, RU-28).

use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{Attribute, Fields, ImplItemFn, Item, ItemEnum, ItemFn, ItemStruct};
use trust_plc_decl::{DataType, Elementary, Pou, Section, Value, Variable};

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

/// `#[trust_method]`: the method as written, without its budget, which only the reader reads.
pub(crate) fn method(args: TokenStream, item: TokenStream) -> TokenStream {
    let Ok(mut parsed) = syn::parse2::<ImplItemFn>(item.clone()) else {
        return refused(
            "`#[trust_method]` declares a function of a Rust function block's `impl` block a \
             method of the block",
            item,
        );
    };
    let method = trust_plc_decl::method(args, &parsed);
    remove_budget(&mut parsed.attrs);
    match method {
        Ok(_) => quote! { #parsed },
        Err(error) => {
            let error = error.to_compile_error();
            quote! { #error #parsed }
        }
    }
}

/// `#[trust_enum]`: the enumeration as written, and what makes it a type of a Rust POU's
/// variables: its value as `INT` (`trust_plc::Stored`), its cell (`trust_plc::Placed`) and its
/// default, its first variant (`trust_plc::Declared`), a value none of its variants has reading as
/// that variant (specification 48 RU-25).
pub(crate) fn enumeration(args: TokenStream, item: TokenStream) -> TokenStream {
    let Ok(parsed) = syn::parse2::<ItemEnum>(item.clone()) else {
        return refused("`#[trust_enum]` declares an enum a Rust enumeration", item);
    };
    let enumeration = match trust_plc_decl::enumeration(args, &parsed) {
        Ok(enumeration) => enumeration,
        Err(error) => {
            let error = error.to_compile_error();
            return quote! { #error #parsed };
        }
    };
    let ident = &parsed.ident;
    let variants: Vec<proc_macro2::Ident> = enumeration
        .variants
        .iter()
        .map(|(variant, _)| format_ident!("{variant}"))
        .collect();
    let values: Vec<TokenStream> = enumeration
        .variants
        .iter()
        .map(|(_, value)| {
            let magnitude = Literal::i64_unsuffixed(value.abs());
            if *value < 0 {
                quote!(-#magnitude)
            } else {
                quote!(#magnitude)
            }
        })
        .collect();
    let first = &variants[0];
    quote! {
        #parsed

        impl ::trust_plc::Stored for #ident {
            type Raw = i16;
            #[inline]
            fn from_raw(raw: i16) -> Self {
                match raw {
                    #(#values => Self::#variants,)*
                    _ => Self::#first,
                }
            }
            #[inline]
            fn into_raw(self) -> i16 {
                match self {
                    #(Self::#variants => #values,)*
                }
            }
        }

        impl ::trust_plc::Placed for #ident {
            type Area = ::core::cell::Cell<i16>;
            #[inline]
            fn load(area: &Self::Area) -> Self {
                <Self as ::trust_plc::Stored>::from_raw(area.get())
            }
            #[inline]
            fn store(&self, area: &Self::Area) {
                area.set(::trust_plc::Stored::into_raw(*self));
            }
        }

        impl ::trust_plc::Declared for #ident {
            const DEFAULT: Self = Self::#first;
        }
    }
}

/// `#[trust_struct]`: the structure without the attributes its reader reads, and its default, its
/// fields' initial values and their types' defaults (`trust_plc::Declared`, specification 48
/// RU-25, RU-26).
pub(crate) fn structure(args: TokenStream, item: TokenStream) -> TokenStream {
    let Ok(mut parsed) = syn::parse2::<ItemStruct>(item.clone()) else {
        return refused("`#[trust_struct]` declares a struct a Rust structure", item);
    };
    let structure = trust_plc_decl::structure(args, &parsed);
    if let Fields::Named(fields) = &mut parsed.fields {
        for field in &mut fields.named {
            field
                .attrs
                .retain(|attribute| !trust_plc_decl::is_field_attribute(attribute));
        }
    }
    let structure = match structure {
        Ok(structure) => structure,
        Err(error) => {
            let error = error.to_compile_error();
            return quote! { #error #parsed };
        }
    };
    let ident = &parsed.ident;
    let fields: Vec<proc_macro2::Ident> = structure
        .fields
        .iter()
        .map(|field| format_ident!("{}", field.name))
        .collect();
    let starts: Vec<TokenStream> = structure
        .fields
        .iter()
        .map(|field| value(&field.ty, field.init.as_ref()))
        .collect();
    quote! {
        #parsed

        impl ::trust_plc::Declared for #ident {
            const DEFAULT: Self = Self { #(#fields: #starts),* };
        }
    }
}

/// `#[budget]` written above the POU's attribute expands first: it puts itself below that
/// attribute, which reads it. Without a POU's attribute it is refused.
pub(crate) fn budget(args: &TokenStream, item: TokenStream) -> TokenStream {
    const ALONE: &str = "`#[budget]` gives a Rust POU or method its budget: write it with \
                         `#[trust_fb]`, `#[trust_function]` or `#[trust_method]` (specification 48 \
                         RU-46)";
    let Ok(mut parsed) = syn::parse2::<Item>(item.clone()) else {
        return refused(ALONE, item);
    };
    let attributes = match &mut parsed {
        Item::Struct(item) => &mut item.attrs,
        Item::Fn(item) => &mut item.attrs,
        _ => return refused(ALONE, item),
    };
    if !attributes.iter().any(|attribute| {
        trust_plc_decl::declared_kind(attribute).is_some() || trust_plc_decl::is_method(attribute)
    }) {
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

            /// A call of a method of the block as the program's generated code makes it, with the
            /// instance's variables in declaration order: `run` runs on a copy of them, which
            /// replaces them when it returns `Ok` (specification 48 RU-32).
            ///
            /// # Errors
            ///
            /// `run`'s fault; the variables then keep their values.
            #[doc(hidden)]
            #[inline]
            #[allow(clippy::too_many_arguments)]
            pub fn __trust_with<R>(
                cx: &mut ::trust_plc::Cx,
                #(#values: &mut #types,)*
                run: impl ::core::ops::FnOnce(
                    &mut Self,
                    &mut ::trust_plc::Cx,
                ) -> ::core::result::Result<R, ::trust_plc::Fault>,
            ) -> ::core::result::Result<R, ::trust_plc::Fault> {
                let mut next = Self { #(#fields: *#values),* };
                let result = run(&mut next, cx)?;
                #(*#values = next.#fields;)*
                ::core::result::Result::Ok(result)
            }
        }
    }
}

/// The SDK's constant constructor of a date type's stored value (specification 44 §6.2): its
/// milliseconds, or a long form's nanoseconds, so that `new()` stays a constant function.
fn date_constructor(ty: Elementary) -> proc_macro2::Ident {
    if matches!(ty, Elementary::LDate | Elementary::LTod | Elementary::Ldt) {
        format_ident!("from_nanos")
    } else {
        format_ident!("from_millis")
    }
}

/// The capacity of a string type; 0 for another type.
fn capacity(ty: Elementary) -> u16 {
    match ty {
        Elementary::Str(capacity) | Elementary::WStr(capacity) => capacity,
        _ => 0,
    }
}

/// The starting value of `variable`: its initial value, else its type's default (IEC §6.4.2,
/// Table 10: FALSE, 0 and 0.0).
fn start(variable: &Variable) -> TokenStream {
    // An in-out's field holds the reference of the block's last call, `NULL` before the first
    // (specification 48 RU-29).
    if variable.section == Section::InOut {
        return quote!(::trust_plc::Ref::NULL);
    }
    value(&variable.ty, variable.init.as_ref())
}

/// The value of type `ty` that `init` gives, else the type's default; an array's elements each so,
/// those its list leaves out at their default.
fn value(ty: &DataType, init: Option<&Value>) -> TokenStream {
    match ty {
        DataType::Elementary(elementary) => elementary_value(*elementary, init),
        // A type the crate declares: the variant written, else the type's default.
        DataType::Named(_) => {
            if let Some(Value::Variant(path)) = init {
                let segments = path.iter().map(|segment| format_ident!("{segment}"));
                quote!(#(#segments)::*)
            } else {
                quote!(::trust_plc::declared_default())
            }
        }
        // A reference starts `NULL` (specification 48 RU-29).
        DataType::Ref(_) => quote!(::trust_plc::Ref::NULL),
        DataType::Array { element, len, .. } => {
            if let Some(Value::List(items)) = init {
                let len = usize::try_from(*len).unwrap_or(0);
                let default = value(element, None);
                let items = (0..len).map(|index| match items.get(index) {
                    Some(item) => value(element, Some(item)),
                    None => default.clone(),
                });
                quote!(::trust_plc::Array::from_items([#(#items),*]))
            } else {
                let item = value(element, None);
                quote!(::trust_plc::Array::filled(#item))
            }
        }
    }
}

/// The value of the elementary type `ty` that `init` gives, else the type's default.
fn elementary_value(ty: Elementary, init: Option<&Value>) -> TokenStream {
    let signed = |negative: bool, literal: Literal| {
        if negative {
            quote!(-#literal)
        } else {
            literal.into_token_stream()
        }
    };
    // The SDK's type of a bit string, a character or a duration, by the name RU-25 gives it.
    let sdk_type = format_ident!("{}", ty.rust());
    match init {
        Some(Value::Bool(value)) => quote!(#value),
        Some(Value::Integer(value)) => {
            signed(*value < 0, Literal::u128_unsuffixed(value.unsigned_abs()))
        }
        Some(Value::Real(value)) => {
            signed(value.is_sign_negative(), Literal::f32_suffixed(value.abs()))
        }
        Some(Value::Lreal(value)) => {
            signed(value.is_sign_negative(), Literal::f64_suffixed(value.abs()))
        }
        Some(Value::Bits { value, .. }) => {
            let value = Literal::u64_unsuffixed(*value);
            quote!(::trust_plc::#sdk_type(#value))
        }
        Some(Value::Char { code, .. }) => {
            let code = Literal::u32_unsuffixed(*code);
            quote!(::trust_plc::#sdk_type(#code))
        }
        Some(Value::Duration { nanos, .. }) => {
            let nanos = signed(*nanos < 0, Literal::u64_unsuffixed(nanos.unsigned_abs()));
            quote!(::trust_plc::#sdk_type::from_nanos(#nanos))
        }
        // A date's value as the areas store it, which the reader computed from the literal.
        Some(Value::Instant { ty, value }) => {
            let value = signed(*value < 0, Literal::u64_unsuffixed(value.unsigned_abs()));
            let from = date_constructor(*ty);
            quote!(::trust_plc::#sdk_type::#from(#value))
        }
        // A text's bytes or code units, which the reader checked its capacity holds.
        Some(Value::Text { wide, text }) => {
            let capacity = Literal::u16_unsuffixed(capacity(ty));
            if *wide {
                let units = text.encode_utf16().map(Literal::u16_unsuffixed);
                quote!(::trust_plc::#sdk_type::<#capacity>::__from_units(&[#(#units),*]))
            } else {
                let bytes = Literal::byte_string(text.as_bytes());
                quote!(::trust_plc::#sdk_type::<#capacity>::__from_bytes(#bytes))
            }
        }
        _ => match ty {
            Elementary::Bool => quote!(false),
            Elementary::Real | Elementary::Lreal => quote!(0.0),
            Elementary::Byte
            | Elementary::Word
            | Elementary::Dword
            | Elementary::Lword
            | Elementary::Char
            | Elementary::WChar => quote!(::trust_plc::#sdk_type(0)),
            Elementary::Time | Elementary::LTime => quote!(::trust_plc::#sdk_type::ZERO),
            // 1970-01-01 and midnight (specification 02).
            Elementary::Date
            | Elementary::LDate
            | Elementary::Tod
            | Elementary::LTod
            | Elementary::Dt
            | Elementary::Ldt => {
                let from = date_constructor(ty);
                quote!(::trust_plc::#sdk_type::#from(0))
            }
            Elementary::Str(_) | Elementary::WStr(_) => {
                let capacity = Literal::u16_unsuffixed(capacity(ty));
                quote!(::trust_plc::#sdk_type::<#capacity>::new())
            }
            _ => quote!(0),
        },
    }
}
