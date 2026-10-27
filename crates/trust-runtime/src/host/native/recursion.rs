//! The recursion of a compiled program (specification 44 §6.4): the call graph of the generated
//! crate, its cycles, and the calls that count against the call depth.
//!
//! A program is generated once, which records what every body and every dispatcher calls. When that
//! call graph has a cycle, the program is generated again, with every call that a body makes to a
//! function of its own cycle taking one from the call depth before it runs and giving it back when it
//! returns. Every cycle goes through a body, since a dispatcher calls only bodies and methods, so
//! every turn of a recursion takes at least one.

use std::collections::{BTreeMap, BTreeSet};

use super::emit::{self, Generated};
use super::front::{Front, StatementIds};
use super::model::Model;
use super::NativeError;
use trust_runtime_core::native::image::Target;

/// What the generated functions call, by their Rust names.
#[derive(Debug, Default)]
pub(super) struct CallGraph {
    callees: BTreeMap<String, BTreeSet<String>>,
    /// The functions that are bodies of POUs: their calls into their own cycle count, a
    /// dispatcher's do not.
    bodies: BTreeSet<String>,
}

impl CallGraph {
    /// Records the body `name`, which calls `callees`.
    pub(super) fn body(&mut self, name: &str, callees: BTreeSet<String>) {
        self.bodies.insert(name.to_string());
        self.callees
            .entry(name.to_string())
            .or_default()
            .extend(callees);
    }

    /// Records that the dispatcher `name` calls `callee`.
    pub(super) fn dispatcher(&mut self, name: &str, callee: &str) {
        self.callees
            .entry(name.to_string())
            .or_default()
            .insert(callee.to_string());
    }

    /// The recursive calls: for each body, the functions of its own cycle that it calls, itself
    /// included.
    pub(super) fn recursion(&self) -> Recursion {
        let names: Vec<&str> = self
            .callees
            .iter()
            .flat_map(|(caller, callees)| {
                std::iter::once(caller.as_str()).chain(callees.iter().map(String::as_str))
            })
            .collect::<BTreeSet<&str>>()
            .into_iter()
            .collect();
        let index: BTreeMap<&str, usize> = names
            .iter()
            .enumerate()
            .map(|(position, name)| (*name, position))
            .collect();
        let edges: Vec<Vec<usize>> = names
            .iter()
            .map(|name| {
                self.callees
                    .get(*name)
                    .into_iter()
                    .flatten()
                    .map(|callee| index[callee.as_str()])
                    .collect()
            })
            .collect();
        let (group, groups) = super::stack::cycle_groups(&edges);
        let mut members = vec![0usize; groups];
        for of in &group {
            members[*of] += 1;
        }
        let mut recursive: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for body in &self.bodies {
            let caller = index[body.as_str()];
            for &callee in &edges[caller] {
                let cyclic = callee == caller || members[group[caller]] > 1;
                if group[callee] == group[caller] && cyclic {
                    recursive
                        .entry(body.clone())
                        .or_default()
                        .insert(names[callee].to_string());
                }
            }
        }
        Recursion(recursive)
    }
}

/// For each body, the callees whose calls take one from the call depth (specification 44 §6.4).
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Recursion(BTreeMap<String, BTreeSet<String>>);

impl Recursion {
    /// Whether the program makes no recursive call.
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The callees whose calls by the body `name` are recursive.
    pub(super) fn callees_of(&self, name: &str) -> BTreeSet<String> {
        self.0.get(name).cloned().unwrap_or_default()
    }
}

/// Generates the crate of `model` for `target`: once, and once more with its recursive calls bounded
/// by the call depth when its call graph has a cycle.
///
/// # Errors
///
/// [`NativeError`] for a construct the generator does not compile.
pub(super) fn generate(
    front: &Front,
    model: &mut Model<'_>,
    ids: &StatementIds,
    target: Target,
    hooks: bool,
) -> Result<Generated, NativeError> {
    let (generated, graph) =
        emit::generate(front, model, ids, target, hooks, &Recursion::default())?;
    let recursion = graph.recursion();
    if recursion.is_empty() {
        return Ok(generated);
    }
    let (generated, _) = emit::generate(front, model, ids, target, hooks, &recursion)?;
    Ok(generated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn recursion(entries: &[(&str, &[&str])]) -> Recursion {
        Recursion(
            entries
                .iter()
                .map(|(body, callees)| ((*body).to_string(), set(callees)))
                .collect(),
        )
    }

    #[test]
    fn calls_without_a_cycle_are_no_recursion() {
        // Specification 44 §6.4: only a call into the caller's own cycle counts. A program that
        // calls a function that calls another, and a function block body that the program and a
        // method both call, has no cycle. Catches a recursion found in a shared callee.
        let mut graph = CallGraph::default();
        graph.body("prog_MAIN", set(&["f_SCALE", "fb_VALVE"]));
        graph.body("f_SCALE", set(&["f_CLAMP"]));
        graph.body("f_CLAMP", set(&[]));
        graph.body("fb_VALVE", set(&["f_CLAMP"]));
        assert!(graph.recursion().is_empty());
    }

    #[test]
    fn a_function_that_calls_itself_counts_that_call() {
        // A function that calls itself is a cycle of one: its call of itself counts, and the
        // program's call into it does not. Catches a cycle of one function taken for none, and the
        // entry into a recursion counted as a turn of it.
        let mut graph = CallGraph::default();
        graph.body("prog_MAIN", set(&["f_FACTORIAL"]));
        graph.body("f_FACTORIAL", set(&["f_FACTORIAL", "f_CLAMP"]));
        graph.body("f_CLAMP", set(&[]));
        assert_eq!(
            graph.recursion(),
            recursion(&[("f_FACTORIAL", &["f_FACTORIAL"])])
        );
    }

    #[test]
    fn functions_that_call_one_another_count_each_call_between_them() {
        // Two functions that call each other form one cycle: each one's call of the other counts.
        // Catches a cycle counted at one of its calls only.
        let mut graph = CallGraph::default();
        graph.body("prog_MAIN", set(&["f_EVEN"]));
        graph.body("f_EVEN", set(&["f_ODD"]));
        graph.body("f_ODD", set(&["f_EVEN"]));
        assert_eq!(
            graph.recursion(),
            recursion(&[("f_EVEN", &["f_ODD"]), ("f_ODD", &["f_EVEN"])])
        );
    }

    #[test]
    fn a_cycle_through_a_dispatcher_counts_the_bodys_call_only() {
        // Specification 44 §6.4: a decorator's method calls the interface's dispatcher, which calls
        // the method of every type that implements it, the decorator's own included. The
        // decorator's call of the dispatcher counts; the dispatcher's calls do not, and the method
        // of the other type, outside the cycle, makes no recursive call. Catches a dispatcher's
        // call counted, which counts a turn twice, and a cycle through a dispatcher missed.
        let mut graph = CallGraph::default();
        graph.body("prog_MAIN", set(&["i_SHAPE__AREA"]));
        graph.body("m_SCALED__AREA", set(&["i_SHAPE__AREA"]));
        graph.body("m_SQUARE__AREA", set(&[]));
        graph.dispatcher("i_SHAPE__AREA", "m_SCALED__AREA");
        graph.dispatcher("i_SHAPE__AREA", "m_SQUARE__AREA");
        assert_eq!(
            graph.recursion(),
            recursion(&[("m_SCALED__AREA", &["i_SHAPE__AREA"])])
        );
    }
}
