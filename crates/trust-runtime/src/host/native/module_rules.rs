//! The rules of Rust in the scan that the build checks on the linked module (specification 48
//! §9.4, RU-45): no `static` that changes (RU-13), no recursion through Rust (RU-18) and no
//! function of the target's library whose results differ from device to device (RU-17). Each
//! refusal names the `static` or the functions; the panic machinery has its own check
//! (`panics`).

use object::{Object, ObjectSection, ObjectSymbol};

use super::call_graph::{demangled, CallGraph};
use super::NativeError;

/// The writable data sections, where only a `static` that changes lies: the generated code keeps
/// every value in the program's areas (specification 44 §7.1).
const WRITABLE: [&str; 4] = [".data", ".bss", ".tdata", ".tbss"];

/// The functions of a target's C library whose results IEEE 754 does not fix, so that they differ
/// between libraries and devices (RU-17). Those it rounds exactly (`sqrt`, `fma`, `floor`, `fmod`
/// and the rest) give one result everywhere and are not among them.
const INEXACT: [&str; 54] = [
    "acos", "acosf", "acosh", "acoshf", "asin", "asinf", "asinh", "asinhf", "atan", "atanf",
    "atan2", "atan2f", "atanh", "atanhf", "cbrt", "cbrtf", "cos", "cosf", "cosh", "coshf", "erf",
    "erff", "erfc", "erfcf", "exp", "expf", "exp2", "exp2f", "expm1", "expm1f", "hypot", "hypotf",
    "lgamma", "lgammaf", "log", "logf", "log10", "log10f", "log1p", "log1pf", "log2", "log2f",
    "pow", "powf", "sin", "sinf", "sinh", "sinhf", "tan", "tanf", "tanh", "tanhf", "tgamma",
    "tgammaf",
];

/// Whether `raw`, a symbol of the module, is a function of the target's library that RU-17 refuses.
pub(super) fn is_inexact_math(raw: &str) -> bool {
    INEXACT.contains(&raw)
}

/// Whether the function `name` is the program's generated code, whose recursion is Structured
/// Text's own (specification 44 §6.4): the generated crate's functions and its entry points.
fn is_generated(name: &str) -> bool {
    name.starts_with("trust_module::") || name.starts_with("trust_entry")
}

/// What the linked module `elf`, whose call graph is `graph`, breaks of RU-13, RU-17 and RU-18,
/// each a line that names the `static` or the functions; none for a module that keeps them.
///
/// # Errors
///
/// [`NativeError::Module`] for a module whose symbols do not read.
pub(super) fn refusals(elf: &[u8], graph: &CallGraph) -> Result<Vec<String>, NativeError> {
    let mut lines = changing_statics(elf)?
        .into_iter()
        .map(|(name, section)| {
            format!(
                "the `static` {name} changes (it lies in {section}): a Rust POU keeps its state in \
                 the program's areas, its instance's variables (specification 48 RU-13)"
            )
        })
        .collect::<Vec<_>>();
    for cycle in rust_cycles(graph) {
        let calls = match cycle.as_slice() {
            [one] => format!("{one} calls itself"),
            many => format!("{} call each other", many.join(", ")),
        };
        lines.push(format!(
            "{calls}: a Rust POU's code calls nothing that calls it again (specification 48 \
             RU-18)"
        ));
    }
    for (caller, callee) in target_math(graph) {
        lines.push(format!(
            "{caller} calls the target's `{callee}`, whose results differ from device to device: \
             use `trust_plc::math`, which gives one result everywhere (specification 48 RU-17)"
        ));
    }
    Ok(lines)
}

/// The symbols that lie in a writable data section, each with its section, demangled.
fn changing_statics(elf: &[u8]) -> Result<Vec<(String, String)>, NativeError> {
    let file = object::File::parse(elf).map_err(|error| NativeError::Module(error.to_string()))?;
    let mut found: Vec<(String, String)> = Vec::new();
    for symbol in file.symbols() {
        // A `static`'s symbol has its size; the linker's own markers (`__bss_start`) have none.
        if !matches!(
            symbol.kind(),
            object::SymbolKind::Data | object::SymbolKind::Tls
        ) || symbol.size() == 0
        {
            continue;
        }
        let Some(section) = symbol
            .section_index()
            .and_then(|index| file.section_by_index(index).ok())
        else {
            continue;
        };
        let Ok(section) = section.name() else {
            continue;
        };
        if !WRITABLE.contains(&section) {
            continue;
        }
        let Ok(raw) = symbol.name() else { continue };
        if raw.is_empty() {
            continue;
        }
        let entry = (demangled(raw), section.to_string());
        if !found.contains(&entry) {
            found.push(entry);
        }
    }
    Ok(found)
}

/// The cycles of `graph` that pass through a function that is not the generated code's, each as
/// its functions' names in their addresses' order, the cycles in the order of their first
/// functions.
fn rust_cycles(graph: &CallGraph) -> Vec<Vec<String>> {
    super::call_graph::cycles(graph)
        .into_iter()
        .filter(|cycle| {
            cycle
                .iter()
                .any(|function| !is_generated(&graph.functions[*function].name))
        })
        .map(|cycle| {
            cycle
                .into_iter()
                .map(|function| graph.functions[function].name.clone())
                .collect()
        })
        .collect()
}

/// The calls of the target library's inexact functions from outside the program library, which
/// computes with its own pure-Rust `libm`: each caller's name and the function it calls.
fn target_math(graph: &CallGraph) -> Vec<(String, String)> {
    let mut calls: Vec<(String, String)> = Vec::new();
    for &(caller, callee) in &graph.edges {
        let (caller, callee) = (&graph.functions[caller], &graph.functions[callee]);
        if !is_inexact_math(&callee.raw) || caller.name.starts_with("trust_plc_lib::") {
            continue;
        }
        let call = (caller.name.clone(), callee.raw.clone());
        if !calls.contains(&call) {
            calls.push(call);
        }
    }
    calls
}

#[cfg(test)]
#[path = "module_rules_tests.rs"]
mod tests;
