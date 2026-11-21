//! The cases with which `trust-dev rust bench` measures a Rust POU (specification 48 RU-48). Every
//! expected value is the specification's or worked out by hand from it.

use proc_macro2::TokenStream;
use syn::{parse_quote, ImplItemFn, ItemFn, ItemStruct};

use crate::{function, function_block, method, program, Case, Value};

fn args(text: &str) -> TokenStream {
    text.parse().expect("the arguments are tokens")
}

/// RU-22's `FILTER_EMA` with the case attributes `cases` written above its fields.
fn filter_with(cases: &str) -> Result<crate::Pou, syn::Error> {
    let cases: TokenStream = cases.parse().expect("the cases are tokens");
    let item: ItemStruct = parse_quote! {
        #[budget("15us")]
        #cases
        pub struct FilterEma {
            #[input] pub x: f64,
            #[input] #[init(0.1)] pub alpha: f64,
            #[output] pub y: f64,
            primed: bool,
        }
    };
    function_block(args("name = \"FILTER_EMA\""), &item)
}

fn refusal<T: std::fmt::Debug>(result: Result<T, syn::Error>) -> String {
    result.expect_err("the case is refused").to_string()
}

#[test]
fn a_blocks_cases_are_read_in_their_order_with_their_inputs_types() {
    // RU-48: one case for each attribute, in the order written, each value of its input's type
    // (an f64 input is an LREAL). Catches a lost or reordered case, a value read as another type,
    // and an input the case does not name given a value instead of keeping its initial one.
    let pou = filter_with("#[bench_case(x = 1.0e6, alpha = 0.001)] #[bench_case(x = -1.0e6)]")
        .expect("the block is read");
    assert_eq!(
        pou.bench,
        vec![
            Case {
                values: vec![
                    ("x".to_string(), Value::Lreal(1.0e6)),
                    ("alpha".to_string(), Value::Lreal(0.001)),
                ],
            },
            Case {
                values: vec![("x".to_string(), Value::Lreal(-1.0e6))],
            },
        ]
    );
}

#[test]
fn a_pou_without_a_case_has_none_and_bench_alone_is_its_initial_values() {
    // RU-48: a POU without a case is measured with its declared initial values, and
    // `#[bench_case]` without values is that case written out. Catches a case invented for a POU
    // without one and `#[bench_case]` alone refused.
    assert_eq!(filter_with("").expect("read").bench, Vec::<Case>::new());
    let pou = filter_with("#[bench_case]").expect("read");
    assert_eq!(pou.bench, vec![Case { values: Vec::new() }]);
}

#[test]
fn a_functions_case_gives_values_to_its_parameters_and_in_outs() {
    // RU-48: a function's case names its parameters, an in-out too, whose variable the bench
    // provides. Catches an in-out refused for a function and an integer read as another type.
    let item: ItemFn = parse_quote! {
        #[budget("1us")]
        #[bench_case(gain = 7, total = 3)]
        pub fn scale(_cx: &mut Cx, gain: i32, total: InOut<'_, i32>) -> Result<i32, Fault> {
            Ok(0)
        }
    };
    let pou = function(args("name = \"SCALE\""), &item).expect("the function is read");
    assert_eq!(
        pou.bench,
        vec![Case {
            values: vec![
                ("gain".to_string(), Value::Integer(7)),
                ("total".to_string(), Value::Integer(3)),
            ],
        }]
    );
}

#[test]
fn a_case_that_names_anything_but_an_input_or_a_name_twice_is_refused() {
    // RU-48: the build refuses a case that names an output, an internal variable, a name the POU
    // has not, or one name twice, and a value its type cannot take. Catches a case that would set
    // an output or a state the measurement must start from as declared.
    let output = refusal(filter_with("#[bench_case(y = 1.0)]"));
    assert!(
        output.contains("`y` is no input or in-out of the POU"),
        "{output}"
    );
    let internal = refusal(filter_with("#[bench_case(primed = true)]"));
    assert!(
        internal.contains("`primed` is no input or in-out of the POU"),
        "{internal}"
    );
    let unknown = refusal(filter_with("#[bench_case(z = 1.0)]"));
    assert!(
        unknown.contains("`z` is no variable of the POU"),
        "{unknown}"
    );
    let twice = refusal(filter_with("#[bench_case(x = 1.0, x = 2.0)]"));
    assert!(twice.contains("the case names `x` twice"), "{twice}");
    let wrong = refusal(filter_with("#[bench_case(x = true)]"));
    assert!(
        wrong.contains("the case's value of `x` is written as `#[init(...)]` writes it"),
        "{wrong}"
    );
}

#[test]
fn a_programs_case_names_its_inputs_only() {
    // RU-48: a program's instances are the configuration's, so a case gives values to its inputs
    // and the refusal offers no in-out. Catches a program's output set by a case.
    let item: ItemStruct = parse_quote! {
        #[budget("5us")]
        #[bench_case(start = true)]
        pub struct Cell {
            #[input] pub start: bool,
            #[output] pub done: bool,
        }
    };
    let pou = program(args("name = \"CELL\""), &item).expect("the program is read");
    assert_eq!(
        pou.bench,
        vec![Case {
            values: vec![("start".to_string(), Value::Bool(true))],
        }]
    );
    let item: ItemStruct = parse_quote! {
        #[budget("5us")]
        #[bench_case(done = true)]
        pub struct Cell {
            #[input] pub start: bool,
            #[output] pub done: bool,
        }
    };
    let refused = refusal(program(args("name = \"CELL\""), &item));
    assert!(
        refused.contains("`done` is no input of the POU"),
        "{refused}"
    );
}

#[test]
fn a_methods_cases_name_its_parameters() {
    // RU-48: a method has cases of its own, measured on an instance of its block; an output
    // parameter takes no value. Catches a method's cases lost and an output accepted.
    let item: ImplItemFn = parse_quote! {
        #[budget("2us")]
        #[bench_case(distance = 0.5, limit = 10.0)]
        pub fn jog(
            &mut self,
            cx: &mut Cx,
            distance: f64,
            limit: InOut<'_, f64>,
            reached: &mut bool,
        ) -> Result<f64, Fault> {
            Ok(0.0)
        }
    };
    let read = method(TokenStream::new(), &item).expect("the method is read");
    assert_eq!(
        read.bench,
        vec![Case {
            values: vec![
                ("distance".to_string(), Value::Lreal(0.5)),
                ("limit".to_string(), Value::Lreal(10.0)),
            ],
        }]
    );
    let item: ImplItemFn = parse_quote! {
        #[budget("2us")]
        #[bench_case(reached = true)]
        pub fn jog(&mut self, cx: &mut Cx, reached: &mut bool) -> Result<(), Fault> {
            Ok(())
        }
    };
    let refused = refusal(method(TokenStream::new(), &item));
    assert!(
        refused.contains("`reached` is no input or in-out of the POU"),
        "{refused}"
    );
}
