//! The layers of a project whose Rust POUs call Structured Text (specification 48 §9.2): layer 1 is
//! what the Structured Text that Rust calls reaches, compiled before the project's crate, so none of
//! it may call a Rust POU (RU-40).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::recursion::CallGraph;
use super::rust_pous::StructuredTextUse;
use super::NativeError;

/// The POU whose body a generated function is.
pub(super) struct BodyPou {
    /// Its Structured Text name, `Owner.Method` for a method.
    pub(super) name: String,
    /// Whether it is a Rust POU, whose body calls the crate (RU-28).
    pub(super) rust: bool,
}

/// Layer 1 (RU-40, RU-42): every generated function that `roots` reach in `graph`, each root the
/// function of a Structured Text POU that the crate calls, with the call; `bodies` names the POU of
/// each generated body.
///
/// # Errors
///
/// [`NativeError::Compile`] naming the chain from a call to the Rust POU it reaches: layer 1 is
/// compiled before the crate, so the layers would be a cycle.
pub(super) fn layer_one(
    graph: &CallGraph,
    roots: &[(String, StructuredTextUse)],
    bodies: &BTreeMap<String, BodyPou>,
) -> Result<BTreeSet<String>, NativeError> {
    // Each reached function, with the function that reached it first and the root it came from.
    let mut reached: BTreeMap<&str, (Option<&str>, usize)> = BTreeMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    for (index, (root, _)) in roots.iter().enumerate() {
        if !reached.contains_key(root.as_str()) {
            reached.insert(root, (None, index));
            queue.push_back(root);
        }
    }
    while let Some(function) = queue.pop_front() {
        if bodies.get(function).is_some_and(|pou| pou.rust) {
            return Err(cycle(function, &reached, roots, bodies));
        }
        let origin = reached[function].1;
        for callee in graph.callees(function) {
            if !reached.contains_key(callee) {
                reached.insert(callee, (Some(function), origin));
                queue.push_back(callee);
            }
        }
    }
    Ok(reached.into_keys().map(str::to_string).collect())
}

/// The refusal of the Rust POU whose body is `rust`, which a root reached: the call and the POUs on
/// the way, the dispatchers between them left out.
fn cycle(
    rust: &str,
    reached: &BTreeMap<&str, (Option<&str>, usize)>,
    roots: &[(String, StructuredTextUse)],
    bodies: &BTreeMap<String, BodyPou>,
) -> NativeError {
    let mut chain = vec![rust];
    while let Some((Some(caller), _)) = chain.last().and_then(|at| reached.get(at)) {
        chain.push(*caller);
    }
    chain.reverse();
    let names: Vec<&str> = chain
        .iter()
        .filter_map(|function| bodies.get(*function))
        .map(|pou| pou.name.as_str())
        .collect();
    let used = &roots[reached[rust].1].1;
    let mut text = format!("st::{} at {}:{}", used.name, used.file, used.line);
    for (index, name) in names.iter().enumerate() {
        text.push_str(if index == 0 {
            " calls "
        } else {
            ", which calls "
        });
        text.push_str(name);
    }
    NativeError::Compile(format!(
        "{text} (Rust): Structured Text that Rust calls is compiled before the crate, so it \
         cannot call Rust (specification 48 RU-40)"
    ))
}

#[cfg(test)]
#[path = "layers_tests.rs"]
mod tests;
