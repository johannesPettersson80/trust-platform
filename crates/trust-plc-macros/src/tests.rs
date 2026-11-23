//! The attributes expand as specification 48 §6.1 gives (RU-22, RU-26, RU-28, RU-46, RU-48). What
//! the expansion computes is proven by `trust-plc`'s tests, which compile and run it.

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{File, ImplItem, Item};

use crate::expand;

fn file(tokens: TokenStream) -> File {
    syn::parse2(tokens).expect("the expansion is Rust items")
}

fn attribute_names(attributes: &[syn::Attribute]) -> Vec<String> {
    attributes
        .iter()
        .filter_map(|attribute| attribute.path().segments.last())
        .map(|segment| segment.ident.to_string())
        .collect()
}

#[test]
fn a_function_block_keeps_its_struct_without_the_attributes_and_gains_its_calls() {
    // Catches an expansion that leaves `#[input]` or `#[budget]` on the struct, which the compiler
    // cannot read, or that lacks one of the calls RU-26, RU-28 and RU-56 give.
    let expanded = file(expand::function_block(
        quote!(name = "FILTER_EMA"),
        quote! {
            /// Exponential moving average of one input.
            #[budget("15us")]
            pub struct FilterEma {
                #[input] pub x: f64,
                #[input] #[init(0.1)] #[allow(dead_code)] pub alpha: f64,
                #[output] pub y: f64,
                primed: bool,
            }
        },
    ));
    let [Item::Struct(item), Item::Impl(calls)] = expanded.items.as_slice() else {
        panic!("a struct and its calls: {}", expanded.to_token_stream());
    };
    assert_eq!(attribute_names(&item.attrs), ["doc"]);
    let syn::Fields::Named(fields) = &item.fields else {
        panic!("named fields")
    };
    let kept: Vec<Vec<String>> = fields
        .named
        .iter()
        .map(|field| attribute_names(&field.attrs))
        .collect();
    assert_eq!(kept, [vec![], vec!["allow".to_string()], vec![], vec![]]);
    let functions: Vec<(String, usize)> = calls
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(function) => {
                Some((function.sig.ident.to_string(), function.sig.inputs.len()))
            }
            _ => None,
        })
        .collect();
    // `new()`, `call(&mut self, cx)`, the generated code's call with the context and the four
    // variables in declaration order, and its call of a method with them and the method to run
    // (specification 48 RU-32).
    assert_eq!(
        functions,
        [
            ("new".to_string(), 0),
            ("call".to_string(), 2),
            ("__trust_call".to_string(), 5),
            ("__trust_with".to_string(), 6)
        ]
    );
}

#[test]
fn a_refused_item_reports_one_error_and_keeps_its_item() {
    // Catches an expansion that drops the item, which would add an error at every use of it, or that
    // leaves attributes the compiler cannot read beside the one error.
    let expanded = expand::function_block(
        quote!(name = "TEXT"),
        quote! {
            #[budget("15us")]
            pub struct Text { #[input] pub name: String }
        },
    );
    let text = expanded.to_string();
    assert_eq!(text.matches("compile_error").count(), 1, "{text}");
    assert!(text.contains("RU-25"), "{text}");
    let expanded = file(expanded);
    let Some(Item::Struct(item)) = expanded.items.last() else {
        panic!("the struct is kept: {}", expanded.to_token_stream());
    };
    assert_eq!(item.ident, "Text");
    assert_eq!(attribute_names(&item.attrs), Vec::<String>::new());
}

#[test]
fn a_function_keeps_its_signature() {
    // Catches an expansion that changes what the generated code calls: the function stays as
    // written, without its budget.
    let expanded = file(expand::function(
        quote!(name = "MAX_LIMITED"),
        quote! {
            #[budget("2us")]
            pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
                Ok(a.max(b).min(limit))
            }
        },
    ));
    let [Item::Fn(function)] = expanded.items.as_slice() else {
        panic!("one function: {}", expanded.to_token_stream());
    };
    assert_eq!(attribute_names(&function.attrs), Vec::<String>::new());
    assert_eq!(function.sig.ident, "max_limited");
    assert_eq!(function.sig.inputs.len(), 4);
}

#[test]
fn a_budget_above_the_pous_attribute_moves_below_it() {
    // `#[budget]` written first expands first: it must reach the POU's attribute, which reads it,
    // so a budget written above the attribute counts as one written below it.
    let expanded = file(expand::budget(
        &quote!("2us"),
        quote! {
            #[trust_function(name = "F")]
            pub fn f(cx: &mut Cx) -> Result<(), Fault> { Ok(()) }
        },
    ));
    let [Item::Fn(function)] = expanded.items.as_slice() else {
        panic!("one function: {}", expanded.to_token_stream());
    };
    assert_eq!(
        attribute_names(&function.attrs),
        ["trust_function", "budget"]
    );
}

#[test]
fn a_budget_without_a_pou_is_refused() {
    // RU-46: a budget belongs to a Rust POU; on another item it would bound nothing.
    let text = expand::budget(&quote!("2us"), quote! { pub fn helper() {} }).to_string();
    assert!(
        text.contains("compile_error") && text.contains("RU-46"),
        "{text}"
    );
}

#[test]
fn bench_cases_above_the_pous_attribute_move_below_it_in_their_order() {
    // RU-48: `#[bench_case]` written above the POU's attribute expands first and must reach the
    // attribute, which reads it; two cases written above expand one after the other, the upper
    // first, so each must go after the cases already below, or their order would turn round.
    let first = expand::bench_case(
        &quote!(x = 1.0),
        quote! {
            #[bench_case(x = 2.0)]
            #[trust_function(name = "F")]
            #[budget("1us")]
            pub fn f(cx: &mut Cx, x: f64) -> Result<(), Fault> { Ok(()) }
        },
    );
    // The lower case expands next, on the item the upper one left.
    let after_first = file(first);
    let [Item::Fn(after_first)] = after_first.items.as_slice() else {
        panic!("one function");
    };
    let mut item = after_first.clone();
    let lower = item.attrs.remove(0);
    let args = match &lower.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        other => panic!("a case with values: {}", other.to_token_stream()),
    };
    let expanded = file(expand::bench_case(&args, item.to_token_stream()));
    let [Item::Fn(function)] = expanded.items.as_slice() else {
        panic!("one function: {}", expanded.to_token_stream());
    };
    assert_eq!(
        attribute_names(&function.attrs),
        ["trust_function", "budget", "bench_case", "bench_case"]
    );
    let cases: Vec<String> = function
        .attrs
        .iter()
        .filter(|attribute| trust_plc_decl::is_bench_case(attribute))
        .map(|attribute| attribute.to_token_stream().to_string())
        .collect();
    assert_eq!(
        cases,
        ["# [bench_case (x = 1.0)]", "# [bench_case (x = 2.0)]"]
    );
}

#[test]
fn a_pous_expansion_drops_its_bench_cases() {
    // RU-48: only the reader reads a case; an expansion that kept `#[bench_case]` on the function
    // would expand it again after the POU's attribute is gone, and refuse it.
    let expanded = file(expand::function(
        quote!(name = "F"),
        quote! {
            #[budget("1us")]
            #[bench_case(x = 1.0)]
            #[bench_case]
            pub fn f(cx: &mut Cx, x: f64) -> Result<(), Fault> { Ok(()) }
        },
    ));
    let function = expanded
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) => Some(function),
            _ => None,
        })
        .expect("the function");
    assert!(
        attribute_names(&function.attrs).is_empty(),
        "{:?}",
        attribute_names(&function.attrs)
    );
}

#[test]
fn a_bench_case_without_a_pou_is_refused() {
    // RU-48: a case belongs to a Rust POU or method; on another item it would measure nothing.
    let text =
        expand::bench_case(&quote!(x = 1.0), quote! { pub fn helper(x: f64) {} }).to_string();
    assert!(
        text.contains("compile_error") && text.contains("RU-48"),
        "{text}"
    );
}

#[test]
fn every_attribute_shows_its_use_in_an_example_that_runs() {
    // Specification 48 RU-89: each attribute's documentation shows it in a whole program, which CI
    // runs as a doc test. Catches an attribute added without an example, and an example fenced
    // `ignore`, `no_run` or `text`, which no test runs: until 22 November 2026 all eight were
    // `ignore`, and the method's could not compile (its parameter `step` is a reserved word of
    // Structured Text). The names pin what the reading found, so that a misread file fails too.
    let mut docs: Vec<&str> = Vec::new();
    let mut attributes = Vec::new();
    for line in include_str!("lib.rs").lines().map(str::trim_start) {
        if let Some(doc) = line.strip_prefix("///") {
            docs.push(doc.trim());
            continue;
        }
        if line == "#[proc_macro_attribute]" {
            continue;
        }
        if let Some(signature) = line.strip_prefix("pub fn ") {
            let name = signature.split('(').next().unwrap_or_default();
            let fences: Vec<&str> = docs
                .iter()
                .copied()
                .filter(|doc| doc.starts_with("```"))
                .collect();
            assert!(!fences.is_empty(), "#[{name}] has no example");
            for fence in fences {
                assert_eq!(
                    fence, "```",
                    "#[{name}] has an example fenced `{fence}`, which no test runs"
                );
            }
            attributes.push(name);
        }
        docs.clear();
    }
    assert_eq!(
        attributes,
        [
            "trust_fb",
            "trust_program",
            "trust_sequence",
            "trust_function",
            "trust_method",
            "trust_enum",
            "trust_struct",
            "budget",
            "bench_case",
        ]
    );
}
