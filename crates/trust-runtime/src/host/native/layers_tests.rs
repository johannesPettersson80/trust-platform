//! Layer 1 of specification 48 §9.2: what the Structured Text that Rust calls reaches (RU-40,
//! RU-42). Every expected set and message is worked out by hand from the graphs below.

use std::collections::{BTreeMap, BTreeSet};

use super::{layer_one, BodyPou};
use crate::native::recursion::CallGraph;
use crate::native::rust_pous::StructuredTextUse;
use crate::native::NativeError;

/// A graph of `bodies`, each with what it calls, and of `dispatchers`, each with what it calls.
fn graph(bodies: &[(&str, &[&str])], dispatchers: &[(&str, &str)]) -> CallGraph {
    let mut graph = CallGraph::default();
    for (name, callees) in bodies {
        graph.body(name, callees.iter().map(ToString::to_string).collect());
    }
    for (name, callee) in dispatchers {
        graph.dispatcher(name, callee);
    }
    graph
}

/// The POUs of the generated bodies `pous`, each with its name and whether it is Rust.
fn pous(pous: &[(&str, &str, bool)]) -> BTreeMap<String, BodyPou> {
    pous.iter()
        .map(|(function, name, rust)| {
            (
                (*function).to_string(),
                BodyPou {
                    name: (*name).to_string(),
                    rust: *rust,
                },
            )
        })
        .collect()
}

/// The call `st::name` at `line` of `rust/src/lib.rs`, of the function `function`.
fn root(function: &str, name: &str, line: usize) -> (String, StructuredTextUse) {
    (
        function.to_string(),
        StructuredTextUse {
            name: name.to_string(),
            file: "rust/src/lib.rs".to_string(),
            line,
            column: 5,
        },
    )
}

#[test]
fn layer_one_holds_what_the_called_structured_text_reaches() {
    // RU-40, RU-42: SCALE, which the crate calls, calls HELPER, which calls LEAF and a method
    // through the interface's dispatcher; the program MAIN calls SCALE and OTHER. Layer 1 is SCALE,
    // HELPER, LEAF, the dispatcher and A.M; MAIN and OTHER, which nothing of it reaches, are layer
    // 3. Catches a layer that misses a callee two calls away or a dispatcher's method, and one that
    // takes in the callers.
    let graph = graph(
        &[
            ("p_MAIN", &["f_SCALE", "f_OTHER"]),
            ("f_SCALE", &["f_HELPER"]),
            ("f_HELPER", &["f_LEAF", "d_I__M"]),
            ("f_LEAF", &[]),
            ("f_OTHER", &[]),
            ("m_A__M", &[]),
        ],
        &[("d_I__M", "m_A__M")],
    );
    let bodies = pous(&[
        ("p_MAIN", "MAIN", false),
        ("f_SCALE", "SCALE", false),
        ("f_HELPER", "HELPER", false),
        ("f_LEAF", "LEAF", false),
        ("f_OTHER", "OTHER", false),
        ("m_A__M", "A.M", false),
    ]);
    let found = layer_one(&graph, &[root("f_SCALE", "SCALE", 9)], &bodies);
    let expected: BTreeSet<String> = ["d_I__M", "f_HELPER", "f_LEAF", "f_SCALE", "m_A__M"]
        .into_iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(found.ok(), Some(expected));
}

/// The message of the refusal `found`.
fn refusal(found: Result<BTreeSet<String>, NativeError>) -> String {
    found.expect_err("the layers are refused").to_string()
}

#[test]
fn structured_text_that_reaches_rust_is_refused_with_its_chain() {
    // RU-40: SCALE calls HELPER, which calls the method M through the interface's dispatcher; the
    // dispatcher reaches A.M of Structured Text and R.M of the Rust function block R. Layer 1 would
    // call the crate, which is compiled after it: the build refuses, naming the call and the POUs
    // on the way, the dispatcher left out. Catches a Rust POU reached through a dispatcher and a
    // chain without the POUs between.
    let graph = graph(
        &[
            ("f_SCALE", &["f_HELPER"]),
            ("f_HELPER", &["d_I__M"]),
            ("m_A__M", &[]),
            ("m_R__M", &[]),
        ],
        &[("d_I__M", "m_A__M"), ("d_I__M", "m_R__M")],
    );
    let bodies = pous(&[
        ("f_SCALE", "SCALE", false),
        ("f_HELPER", "HELPER", false),
        ("m_A__M", "A.M", false),
        ("m_R__M", "R.M", true),
    ]);
    assert_eq!(
        refusal(layer_one(&graph, &[root("f_SCALE", "SCALE", 9)], &bodies)),
        "st::SCALE at rust/src/lib.rs:9 calls SCALE, which calls HELPER, which calls R.M (Rust): \
         Structured Text that Rust calls is compiled before the crate, so it cannot call Rust \
         (specification 48 RU-40)"
    );
}

#[test]
fn the_chain_starts_at_the_call_that_reaches_rust() {
    // RU-40: of the crate's two calls, st::SCALE at line 9 reaches only Structured Text and
    // st::TRIM at line 14 reaches the Rust function TWICE: the refusal names the call at line 14.
    // Catches a chain told from the wrong call.
    let graph = graph(
        &[
            ("f_SCALE", &["f_LEAF"]),
            ("f_LEAF", &[]),
            ("f_TRIM", &["f_TWICE"]),
            ("f_TWICE", &[]),
        ],
        &[],
    );
    let bodies = pous(&[
        ("f_SCALE", "SCALE", false),
        ("f_LEAF", "LEAF", false),
        ("f_TRIM", "TRIM", false),
        ("f_TWICE", "TWICE", true),
    ]);
    assert_eq!(
        refusal(layer_one(
            &graph,
            &[root("f_SCALE", "SCALE", 9), root("f_TRIM", "TRIM", 14)],
            &bodies
        )),
        "st::TRIM at rust/src/lib.rs:14 calls TRIM, which calls TWICE (Rust): Structured Text \
         that Rust calls is compiled before the crate, so it cannot call Rust (specification 48 \
         RU-40)"
    );
}
