//! A Rust function block's methods as `#[trust_method]` declares them (specification 48 RU-32).

use proc_macro2::TokenStream;
use syn::{parse_quote, ImplItemFn, ItemStruct};

use super::{method, Access};
use crate::{function_block, DataType, Elementary, Section};

fn args(text: &str) -> TokenStream {
    text.parse().expect("the arguments are tokens")
}

/// The method JOG of an axis: an input, an in-out, an output and a result.
fn jog() -> ImplItemFn {
    parse_quote! {
        #[budget("2us")]
        pub fn jog(
            &mut self,
            cx: &mut Cx,
            distance: f64,
            limit: InOut<'_, f64>,
            reached: &mut bool,
        ) -> Result<f64, Fault> {
            Ok(0.0)
        }
    }
}

/// A method without a result or parameters.
fn stop() -> ImplItemFn {
    parse_quote! {
        #[budget("1us")]
        pub fn stop(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
            Ok(())
        }
    }
}

#[test]
fn a_method_is_read_with_its_parameters_result_and_access() {
    // RU-32: `&mut self` and the context first, then an input by value, an in-out as `InOut<T>`
    // and an output as `&mut T`, in their order; the result's type; PUBLIC without an argument,
    // PRIVATE with `private`; the budget. Catches a parameter in the wrong section, a lost result,
    // and an access specifier read wrong.
    let read = method(TokenStream::new(), &jog()).expect("the method is read");
    assert_eq!(
        (read.name.as_str(), read.access, read.budget_ns),
        ("jog", Access::Public, 2_000)
    );
    let sections: Vec<(&str, Section)> = read
        .variables
        .iter()
        .map(|variable| (variable.name.as_str(), variable.section))
        .collect();
    assert_eq!(
        sections,
        [
            ("distance", Section::Input),
            ("limit", Section::InOut),
            ("reached", Section::Output)
        ]
    );
    assert_eq!(read.result, Some(DataType::Elementary(Elementary::Lreal)));
    let read = method(args("private"), &stop()).expect("the method is read");
    assert_eq!((read.access, read.result), (Access::Private, None));
}

#[test]
fn a_blocks_declaration_carries_its_methods() {
    // RU-32: each method after the block's variables, its access specifier written out, since a
    // method without one is PROTECTED in Structured Text, its sections, and no body. Catches a
    // method left out, one written without PUBLIC, which no caller outside the block could call,
    // and parameters in the wrong sections.
    let item: ItemStruct = parse_quote! {
        #[budget("1us")]
        pub struct Axis { #[input] pub target: f64 }
    };
    let mut pou = function_block(args("name = \"AXIS\""), &item).expect("the block is read");
    pou.methods = vec![
        method(TokenStream::new(), &jog()).expect("jog"),
        method(args("private"), &stop()).expect("stop"),
    ];
    let declaration = pou.declaration("rust/src/axis.rs");
    assert!(
        declaration.ends_with(
            "VAR_OUTPUT\n    ENO : BOOL;\nEND_VAR\n\
             METHOD PUBLIC jog : LREAL\n\
             VAR_INPUT\n    distance : LREAL;\nEND_VAR\n\
             VAR_IN_OUT\n    limit : LREAL;\nEND_VAR\n\
             VAR_OUTPUT\n    reached : BOOL;\nEND_VAR\n\
             END_METHOD\n\
             METHOD PRIVATE stop\n\
             END_METHOD\n\
             END_FUNCTION_BLOCK\n"
        ),
        "{declaration}"
    );
}

#[test]
fn a_method_structured_text_could_not_call_is_refused() {
    // RU-32: a method is a `pub fn` of `&mut self` and the context first, without generic
    // parameters, named by an IEC identifier that is no reserved word, its parameters one name each,
    // and at most one access specifier among `private`, `protected` and `internal`. Catches a method
    // the generated code could not call or Structured Text could not declare.
    for (item, expected) in [
        (
            "pub fn m(&self, cx: &mut Cx) -> Result<(), Fault> { Ok(()) }",
            "`&mut self`",
        ),
        (
            "pub fn m(&mut self, a: i16) -> Result<(), Fault> { Ok(()) }",
            "its context",
        ),
        (
            "fn m(&mut self, cx: &mut Cx) -> Result<(), Fault> { Ok(()) }",
            "`pub`",
        ),
        (
            "pub fn m<T>(&mut self, cx: &mut Cx) -> Result<(), Fault> { Ok(()) }",
            "no generic parameters",
        ),
        (
            "pub fn step(&mut self, cx: &mut Cx) -> Result<(), Fault> { Ok(()) }",
            "reserved word",
        ),
        (
            "pub fn m(&mut self, cx: &mut Cx, a: i16, A: i16) -> Result<(), Fault> { Ok(()) }",
            "one name",
        ),
    ] {
        let parsed: ImplItemFn =
            syn::parse_str(&format!("#[budget(\"1us\")] {item}")).expect("a method");
        let message = method(TokenStream::new(), &parsed)
            .expect_err("the method is refused")
            .to_string();
        assert!(message.contains(expected), "{item}: {message}");
    }
    let message = method(args("public"), &stop())
        .expect_err("the specifier is refused")
        .to_string();
    assert!(message.contains("one access specifier"), "{message}");
}
