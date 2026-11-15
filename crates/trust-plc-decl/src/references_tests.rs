//! A Rust POU's in-outs and references in its declaration (specification 48 RU-25, RU-29).

use proc_macro2::TokenStream;
use syn::{parse_quote, ItemFn, ItemStruct};

use crate::{function, function_block};

fn args(text: &str) -> TokenStream {
    text.parse().expect("the arguments are tokens")
}

#[test]
fn in_outs_and_references_are_declared_as_structured_text_declares_them() {
    // RU-25 and RU-29: a function's `InOut<T>` parameter is a VAR_IN_OUT of T, a `Ref<T>`
    // parameter an input REF_TO T; a block's `#[in_out]` field of type `Ref<T>` is a VAR_IN_OUT of
    // T, the reference of its last call, and a `Ref<T>` field a VAR REF_TO T. Catches a section or
    // a type written wrong, or a reference taken for its target.
    let bump: ItemFn = parse_quote! {
        #[budget("1us")]
        pub fn bump(_cx: &mut Cx, count: InOut<'_, i16>, limit: Ref<i16>) -> Result<(), Fault> {
            Ok(())
        }
    };
    let pou = function(args("name = \"BUMP\""), &bump).expect("the function is declared");
    assert!(
        pou.declaration("rust/src/lib.rs").ends_with(
            "FUNCTION BUMP\nVAR_INPUT\n    EN : BOOL := TRUE;\nEND_VAR\nVAR_IN_OUT\n    count : INT;\nEND_VAR\n\
             VAR_INPUT\n    limit : REF_TO INT;\nEND_VAR\n\
             VAR_OUTPUT\n    ENO : BOOL;\nEND_VAR\nEND_FUNCTION\n"
        ),
        "{}",
        pou.declaration("rust/src/lib.rs")
    );
    let fill: ItemStruct = parse_quote! {
        #[budget("1us")]
        pub struct Fill {
            #[input] pub rate: f64,
            #[in_out] pub level: Ref<f64>,
            #[output] pub last: Ref<Array<f64, 1, 4>>,
        }
    };
    let pou = function_block(args("name = \"FILL\""), &fill).expect("the block is declared");
    assert!(
        pou.declaration("rust/src/lib.rs").ends_with(
            "FUNCTION_BLOCK FILL\nVAR_INPUT\n    EN : BOOL := TRUE;\n    rate : LREAL;\nEND_VAR\n\
             VAR_IN_OUT\n    level : LREAL;\nEND_VAR\n\
             VAR_OUTPUT\n    last : REF_TO ARRAY[1..4] OF LREAL;\n    ENO : BOOL;\nEND_VAR\n\
             END_FUNCTION_BLOCK\n"
        ),
        "{}",
        pou.declaration("rust/src/lib.rs")
    );
}

#[test]
fn an_in_out_that_is_no_reference_of_its_last_call_is_refused() {
    // RU-29: a block's in-out keeps the reference of its last call, so it is a `Ref<T>` field with
    // `#[in_out]`, never a borrowed `InOut<T>` nor a value; it names the caller's variable, so it
    // takes no `#[init]` or `#[retain]`; a reference starts NULL and takes no `#[init]`. Catches a
    // declaration the block could not hold or the call could not set.
    for (fields, expected) in [
        (
            "#[in_out] pub level: f64",
            "a field `#[in_out]` of type `Ref<T>`",
        ),
        ("pub level: InOut<f64>", "a function's parameter `InOut<T>`"),
        (
            "#[in_out] #[init(1.0)] pub level: Ref<f64>",
            "no `#[retain]` and no `#[init]`",
        ),
        (
            "#[in_out] #[retain] pub level: Ref<f64>",
            "no `#[retain]` and no `#[init]`",
        ),
        (
            "#[input] #[in_out] pub level: Ref<f64>",
            "a variable has one section",
        ),
        (
            "#[init(1)] pub limit: Ref<i16>",
            "a reference starts `NULL`",
        ),
    ] {
        let item: ItemStruct = syn::parse_str(&format!(
            "#[budget(\"1us\")] pub struct Fields {{ {fields} }}"
        ))
        .expect("a struct");
        let message = function_block(args("name = \"FIELDS\""), &item)
            .expect_err("the field is refused")
            .to_string();
        assert!(message.contains(expected), "{fields}: {message}");
    }
}
