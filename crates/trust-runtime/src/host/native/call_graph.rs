//! The call graph of a linked module (specification 44 §8.5, §8.6; specification 48 RU-45): its
//! functions, each with its raw and its demangled name, and the direct calls between them, read
//! from the call relocations the link keeps (`--emit-relocs`). The checks of a module with Rust
//! code read it: where it can panic, where it recurses, and what it calls of the target's library.

use object::{Object, ObjectSection, ObjectSymbol, RelocationFlags, RelocationTarget};
use trust_runtime_core::native::image::Target;

use super::NativeError;

/// A function of the module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Function {
    /// Its first address.
    pub(super) start: u64,
    /// The address after its last.
    pub(super) end: u64,
    /// Its symbol as the module names it.
    pub(super) raw: String,
    /// Its name as Rust writes it, without the hash of its crate.
    pub(super) name: String,
}

/// The functions of a module, by their addresses, and the direct calls between them, each once.
#[derive(Clone, Debug, Default)]
pub(super) struct CallGraph {
    pub(super) functions: Vec<Function>,
    /// The calls: the caller's and the callee's positions in `functions`.
    pub(super) edges: Vec<(usize, usize)>,
}

/// The call graph of the linked module `elf` built for `target`; an empty graph for a module
/// without code.
///
/// # Errors
///
/// [`NativeError::Module`] for a module whose symbols or relocations do not read.
pub(super) fn read(elf: &[u8], target: Target) -> Result<CallGraph, NativeError> {
    let file = object::File::parse(elf).map_err(|error| NativeError::Module(error.to_string()))?;
    let Some(text) = file.section_by_name(".text") else {
        return Ok(CallGraph::default());
    };
    let mut functions: Vec<Function> = file
        .symbols()
        .filter(|symbol| symbol.kind() == object::SymbolKind::Text && symbol.size() > 0)
        .filter_map(|symbol| {
            let start = symbol.address() & !1;
            let raw = symbol.name().ok()?.to_string();
            let name = demangled(&raw);
            Some(Function {
                start,
                end: start + symbol.size(),
                raw,
                name,
            })
        })
        .collect();
    functions.sort_by_key(|function| function.start);
    functions.dedup_by_key(|function| function.start);
    let containing = |address: u64| {
        let index = functions.partition_point(|function| function.start <= address);
        index
            .checked_sub(1)
            .filter(|index| address < functions[*index].end)
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
    // A function's direct call of itself within its own section leaves no relocation, which the
    // assembler resolves; its call instruction shows it.
    let code = text
        .data()
        .map_err(|error| NativeError::Module(error.to_string()))?;
    let base = text.address();
    for (index, function) in functions.iter().enumerate() {
        let bytes = usize::try_from(function.start.saturating_sub(base))
            .ok()
            .zip(usize::try_from(function.end.saturating_sub(base)).ok())
            .and_then(|(first, end)| code.get(first..end));
        if bytes.is_some_and(|bytes| super::self_calls::calls_itself(bytes, target))
            && !edges.contains(&(index, index))
        {
            edges.push((index, index));
        }
    }
    Ok(CallGraph { functions, edges })
}

/// The cycles of `graph`: each group of functions that call one another, a function that calls
/// itself among them, as their positions in `graph.functions` in their addresses' order, the cycles
/// in the order of their first functions.
pub(super) fn cycles(graph: &CallGraph) -> Vec<Vec<usize>> {
    let mut callees: Vec<Vec<usize>> = vec![Vec::new(); graph.functions.len()];
    for &(caller, callee) in &graph.edges {
        callees[caller].push(callee);
    }
    let (group, groups) = super::stack::cycle_groups(&callees);
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); groups];
    for (function, of) in group.iter().enumerate() {
        members[*of].push(function);
    }
    let mut cycles: Vec<Vec<usize>> = members
        .into_iter()
        .filter(|cycle| {
            // A group of one is a cycle only when its function calls itself.
            cycle.len() > 1
                || cycle
                    .first()
                    .is_some_and(|function| callees[*function].contains(function))
        })
        .collect();
    cycles.sort_by_key(|cycle| cycle.first().copied());
    cycles
}

/// `name` as Rust writes it, without the hash of its crate.
pub(super) fn demangled(name: &str) -> String {
    format!("{:#}", rustc_demangle::demangle(name))
}
