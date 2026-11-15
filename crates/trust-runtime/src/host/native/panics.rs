//! Where a linked module can panic (specification 44 §8.6; specification 48 RU-14, RU-45): each
//! function that calls the panic machinery, with the call it makes, read from the call relocations
//! the link keeps (`--emit-relocs`), and both named as Rust names them. The build names from them a
//! Rust POU that can panic and the form that returns a fault instead.

use object::{Object, ObjectSection, ObjectSymbol, RelocationFlags, RelocationTarget};
use trust_runtime_core::native::image::Target;

use super::NativeError;

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

/// The calls of the panic machinery in the linked module `elf` built for `target`: each function
/// outside the machinery that calls a function of it, once per pair, in the order of the callers'
/// addresses. A function of `core` or `alloc` that calls the machinery is part of it (`unwrap`'s
/// `core::option::unwrap_failed`, a slice's range check), so the caller found is the first one
/// outside the standard library: the project's code, the generated code, or a dependency.
///
/// # Errors
///
/// [`NativeError::Module`] for a module whose symbols or relocations do not read.
pub(super) fn panic_calls(elf: &[u8], target: Target) -> Result<Vec<PanicCall>, NativeError> {
    let refuse = |reason: String| NativeError::Module(reason);
    let file = object::File::parse(elf).map_err(|error| refuse(error.to_string()))?;
    let Some(text) = file.section_by_name(".text") else {
        return Ok(Vec::new());
    };
    // The functions by their start, each with its end and its raw and demangled names.
    let mut functions: Vec<(u64, u64, String, String)> = file
        .symbols()
        .filter(|symbol| symbol.kind() == object::SymbolKind::Text && symbol.size() > 0)
        .filter_map(|symbol| {
            let start = symbol.address() & !1;
            let raw = symbol.name().ok()?.to_string();
            let name = demangled(&raw);
            Some((start, start + symbol.size(), raw, name))
        })
        .collect();
    functions.sort_by_key(|(start, _, _, _)| *start);
    functions.dedup_by_key(|(start, _, _, _)| *start);
    let containing = |address: u64| {
        let index = functions.partition_point(|(start, _, _, _)| *start <= address);
        index
            .checked_sub(1)
            .filter(|index| address < functions[*index].1)
    };
    // The calls between the functions, from the call relocations as the stack bound reads them.
    let (kinds, displacement) = super::stack::calls(target);
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (offset, relocation) in text.relocations() {
        let RelocationFlags::Elf { r_type } = relocation.flags() else {
            continue;
        };
        if !kinds.contains(&r_type) {
            continue;
        }
        let (Some(caller), RelocationTarget::Symbol(index)) =
            (containing(offset), relocation.target())
        else {
            continue;
        };
        let Ok(symbol) = file.symbol_by_index(index) else {
            continue;
        };
        let place = if symbol.kind() == object::SymbolKind::Section {
            u64::try_from(
                i64::try_from(symbol.address()).unwrap_or(i64::MAX)
                    + relocation.addend()
                    + displacement,
            )
            .ok()
        } else if symbol.is_undefined() {
            None
        } else {
            Some(symbol.address() & !1)
        };
        if let Some(callee) = place.and_then(containing)
            && !edges.contains(&(caller, callee))
        {
            edges.push((caller, callee));
        }
    }
    // The machinery: its own functions, and every function of the standard library that calls one.
    let mut machinery: Vec<bool> = functions
        .iter()
        .map(|(_, _, raw, _)| is_panic_symbol(raw))
        .collect();
    let standard = |name: &str| {
        let name = name.trim_start_matches('<');
        name.starts_with("core::") || name.starts_with("alloc::")
    };
    loop {
        let mut grew = false;
        for (caller, callee) in &edges {
            if machinery[*callee] && !machinery[*caller] && standard(&functions[*caller].3) {
                machinery[*caller] = true;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut calls: Vec<(u64, PanicCall)> = Vec::new();
    for (caller, callee) in edges {
        if !machinery[callee] || machinery[caller] {
            continue;
        }
        let call = PanicCall {
            caller: functions[caller].3.clone(),
            panic: functions[callee].3.clone(),
        };
        if !calls.iter().any(|(_, known)| *known == call) {
            calls.push((functions[caller].0, call));
        }
    }
    calls.sort_by_key(|(start, _)| *start);
    Ok(calls.into_iter().map(|(_, call)| call).collect())
}

/// `name` as Rust writes it, without the hash of its crate.
fn demangled(name: &str) -> String {
    format!("{:#}", rustc_demangle::demangle(name))
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
