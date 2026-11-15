//! Structured Text's reserved words name nothing a Rust POU declares, but where specification 01
//! §3 lets them (RU-22).

use proc_macro2::TokenStream;
use syn::{parse_quote, ItemFn, ItemStruct};

use crate::{enumeration, function, function_block, Pou};

fn args(text: &str) -> TokenStream {
    text.parse().expect("the arguments are tokens")
}

fn refusal(result: Result<Pou, syn::Error>) -> String {
    result.expect_err("the item is refused").to_string()
}

/// The refusal of a function block with the fields `fields`.
fn fields_refusal(fields: &str) -> String {
    let item: ItemStruct = syn::parse_str(&format!(
        "#[budget(\"1us\")] pub struct Fields {{ {fields} }}"
    ))
    .expect("a struct");
    refusal(function_block(args("name = \"FIELDS\""), &item))
}

#[test]
fn a_reserved_word_names_nothing_a_rust_pou_declares() {
    // RU-22 and specification 01 §3: no POU, type, namespace, variable, field, parameter or
    // enumeration value is named by a reserved word, in any case: a field `at` became the variable
    // `AT`, and the declaration did not parse (R1q's first check). `NULL`, `ON` and `SET` neither,
    // which only the CODESYS profile takes as names (DEV-034), the reason saying so. Catches a
    // reserved word let through, or a CODESYS-only name taken that an IEC project's check refuses.
    let item: ItemStruct = parse_quote! { #[budget("1us")] pub struct Named; };
    for name in ["STEP", "Motion.TASK", "on", "Null.AXIS"] {
        let message = refusal(function_block(args(&format!("name = \"{name}\"")), &item));
        assert!(
            message.contains("is a reserved word of Structured Text"),
            "{name}: {message}"
        );
    }
    for field in [
        "at: bool",
        "Time: bool",
        "en: bool",
        "r#type: i16",
        "step: i16",
        "on: bool",
        "#[input] pub set: bool",
    ] {
        let message = fields_refusal(field);
        assert!(
            message.contains("is a reserved word of Structured Text") && message.contains("rename"),
            "{field}: {message}"
        );
    }
    assert!(fields_refusal("null: bool").contains("only the CODESYS profile takes"));
    let parameter: ItemFn = parse_quote! {
        #[budget("1us")]
        pub fn f(_cx: &mut Cx, step: i32) -> Result<i32, Fault> { Ok(step) }
    };
    assert!(refusal(function(args("name = \"F\""), &parameter)).contains("`step` is a reserved"));
    let ordinary: ItemStruct = parse_quote! {
        #[budget("1us")]
        pub struct Switch { #[input] pub onward: bool, #[input] pub settle: bool, nullable: bool }
    };
    assert!(function_block(args("name = \"SWITCH\""), &ordinary).is_ok());
    for (item, accepted) in [
        (
            "#[derive(Clone, Copy)] pub enum E { Idle = 0, Running = 1 }",
            true,
        ),
        (
            "#[derive(Clone, Copy)] pub enum E { Null = 0, Set = 1 }",
            false,
        ),
        (
            "#[derive(Clone, Copy)] pub enum E { Idle = 0, Step = 1 }",
            false,
        ),
    ] {
        let parsed: syn::ItemEnum = syn::parse_str(item).expect("an enum");
        let result = enumeration(args("name = \"E\""), &parsed);
        assert_eq!(result.is_ok(), accepted, "{item}: {result:?}");
    }
}
