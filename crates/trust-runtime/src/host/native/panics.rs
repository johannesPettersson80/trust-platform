//! Where a linked module can panic (specification 44 §8.6; specification 48 RU-14, RU-45): each
//! function that calls the panic machinery, with the call it makes, read from the call relocations
//! the link keeps (`--emit-relocs`), and both named as Rust names them. The build names from them a
//! Rust POU that can panic and the form that returns a fault instead.

use super::call_graph::CallGraph;

/// A call of the panic machinery: the function that makes it and the function it calls, each
/// demangled without its hash (`core::panicking::panic_const::panic_const_div_by_zero`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PanicCall {
    /// The calling function.
    pub(super) caller: String,
    /// The called function of the panic machinery.
    pub(super) panic: String,
}

/// Whether the symbol `name` is a function of the panic machinery, as the module check reads it.
pub(super) fn is_panic_symbol(name: &str) -> bool {
    name.contains("9panicking") || name.contains("panic_bounds_check")
}

/// The calls of the panic machinery in the linked module whose call graph is `graph`: each
/// function outside the machinery that calls a function of it, once per pair, in the order of the
/// callers' addresses. A function of `core` or `alloc` that calls the machinery is part of it
/// (`unwrap`'s `core::option::unwrap_failed`, a slice's range check), so the caller found is the
/// first one outside the standard library: the project's code, the generated code, or a dependency.
pub(super) fn panic_calls(graph: &CallGraph) -> Vec<PanicCall> {
    let (functions, edges) = (&graph.functions, &graph.edges);
    // The machinery: its own functions, and every function of the standard library that calls one.
    let mut machinery: Vec<bool> = functions
        .iter()
        .map(|function| is_panic_symbol(&function.raw))
        .collect();
    let standard = |name: &str| {
        let name = name.trim_start_matches('<');
        name.starts_with("core::") || name.starts_with("alloc::")
    };
    loop {
        let mut grew = false;
        for (caller, callee) in edges {
            if machinery[*callee] && !machinery[*caller] && standard(&functions[*caller].name) {
                machinery[*caller] = true;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut calls: Vec<(u64, PanicCall)> = Vec::new();
    for &(caller, callee) in edges {
        if !machinery[callee] || machinery[caller] {
            continue;
        }
        let call = PanicCall {
            caller: functions[caller].name.clone(),
            panic: functions[callee].name.clone(),
        };
        if !calls.iter().any(|(_, known)| *known == call) {
            calls.push((functions[caller].start, call));
        }
    }
    calls.sort_by_key(|(start, _)| *start);
    calls.into_iter().map(|(_, call)| call).collect()
}

/// What the panic `panic` does and the form of the SDK that returns a fault instead (RU-14).
pub(super) fn advice(panic: &str) -> (&'static str, &'static str) {
    let last = panic.rsplit("::").next().unwrap_or(panic);
    match last {
        "panic_const_div_by_zero" => (
            "an integer division by zero",
            "trust_plc::checked::div returns DivisionByZero instead",
        ),
        "panic_const_div_overflow" => (
            "the smallest integer divided by -1",
            "trust_plc::checked::div returns Overflow instead",
        ),
        "panic_const_rem_by_zero" => (
            "an integer remainder by zero",
            "trust_plc::checked::rem returns ModuloByZero instead",
        ),
        "panic_const_rem_overflow" => (
            "the remainder of the smallest integer divided by -1",
            "trust_plc::checked::rem returns its result instead",
        ),
        "panic_bounds_check" => (
            "an index outside an array or a slice",
            "`get(index)` with `ok_or(...)?` returns a fault instead",
        ),
        "slice_index_order_fail"
        | "slice_start_index_len_fail"
        | "slice_end_index_len_fail"
        | "slice_index_fail" => (
            "a range outside a slice",
            "`get(range)` with `ok_or(...)?` returns a fault instead",
        ),
        "unwrap_failed" | "expect_failed" => (
            "an `unwrap` or `expect` of `None` or `Err`",
            "`ok_or(...)?` or a `match` returns a fault instead",
        ),
        "panic_const_shl_overflow" | "panic_const_shr_overflow" => (
            "a shift past the width of its type",
            "`checked_shl` or `checked_shr` with `ok_or(...)?` returns a fault instead",
        ),
        _ => (
            "a `panic!`, `assert!`, `unreachable!` or another panic",
            "return `Err(rust_fault(cx, ...))` or another fault instead",
        ),
    }
}

#[cfg(test)]
#[path = "panics_tests.rs"]
mod tests;
