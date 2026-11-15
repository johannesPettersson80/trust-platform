//! The module's rules for Rust code (specification 48 RU-13, RU-17, RU-18, RU-45) on call graphs
//! written by hand; the end-to-end refusals of a built project are `tests/native_rust_pous.rs`'s.

use super::super::call_graph::{CallGraph, Function};
use super::{is_inexact_math, rust_cycles, target_math};

/// A graph of the functions `names`, at consecutive addresses, with the calls `edges`.
fn graph(names: &[(&str, &str)], edges: &[(usize, usize)]) -> CallGraph {
    CallGraph {
        functions: names
            .iter()
            .zip(0_u64..)
            .map(|((raw, name), index)| Function {
                start: index * 16,
                end: index * 16 + 16,
                raw: (*raw).to_string(),
                name: (*name).to_string(),
            })
            .collect(),
        edges: edges.to_vec(),
    }
}

#[test]
fn only_the_targets_inexact_functions_are_target_math() {
    // RU-17: the target's elementary functions differ from library to library; IEEE 754 fixes the
    // results of `sqrt`, `fma`, `floor` and `fmod`, so they compute alike everywhere. Catches a
    // check that refuses an exact operation, or lets `sin` through.
    for name in ["sin", "sinf", "exp", "log10", "pow", "atan2f", "tanh"] {
        assert!(is_inexact_math(name), "{name}");
    }
    for name in [
        "sqrt",
        "sqrtf",
        "fma",
        "floor",
        "fmod",
        "fabs",
        "sinus",
        "trust_plc_lib::sin",
    ] {
        assert!(!is_inexact_math(name), "{name}");
    }
}

#[test]
fn a_cycle_through_rust_is_named_and_structured_texts_own_is_not() {
    // RU-18: a cycle through a Rust function is refused with its functions; a cycle of the
    // generated code alone is Structured Text's bounded recursion (specification 44 §6.4). Catches
    // a check that misses a function calling itself or two calling each other, or that refuses
    // Structured Text's recursion.
    let found = rust_cycles(&graph(
        &[
            ("trust_entry_p0", "trust_entry_p0"),
            ("a", "trust_module::f_FACT"),
            ("b", "rules::fib"),
            ("c", "rules::ping"),
            ("d", "rules::pong"),
            ("e", "rules::leaf"),
        ],
        &[
            (0, 1),
            (1, 1),
            (0, 2),
            (2, 2),
            (0, 3),
            (3, 4),
            (4, 3),
            (3, 5),
        ],
    ));
    assert_eq!(
        found,
        [
            vec!["rules::fib".to_string()],
            vec!["rules::ping".to_string(), "rules::pong".to_string()],
        ]
    );
}

#[test]
fn target_math_is_named_outside_the_program_library() {
    // RU-17: the program library computes with its own pure-Rust `libm`; any other caller of the
    // target's `sin` is named, with the function it calls. Catches a caller the check misses and
    // the program library refused.
    let found = target_math(&graph(
        &[
            ("a", "rules::wave"),
            ("b", "trust_plc_lib::math::sin"),
            ("sin", "sin"),
            ("sqrt", "sqrt"),
        ],
        &[(0, 2), (1, 2), (0, 3)],
    ));
    assert_eq!(found, [("rules::wave".to_string(), "sin".to_string())]);
}
