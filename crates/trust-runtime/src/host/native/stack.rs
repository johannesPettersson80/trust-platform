//! The stack bound of a compiled program (specification 44 §8.5): the deepest chain of calls from
//! each entry point of the linked module, a recursion's cycle counted once, every function's frame
//! from the compiler's `prolog-epilog` remarks, the kernel functions a statement may call on the same
//! stack, and a margin of a quarter; and the bytes of one call level of a recursion, which the call
//! depth bounds (§6.4). The call graph comes from the call relocations the link keeps
//! (`--emit-relocs`) and the functions' calls of themselves (`call_graph`); a cycle no call depth
//! bounds is refused.

use std::collections::HashMap;

use object::{Object, ObjectSymbol};
use trust_runtime_core::native::image::Target;

use super::call_graph::CallGraph;
use super::NativeError;

/// The stack a kernel function may take on the task's stack when a module calls it: a stop of
/// the debugger, a queued write, the runtime's reader of reals (specification 44 §8.2).
const KERNEL_FUNCTIONS: u64 = 4096;

/// The frame counted for a function the compiler gave no remark for: one of the target's
/// builtins, which the module links outside its link-time optimization.
const UNKNOWN_FRAME: u64 = 512;

/// The call relocations of each target, and the distance from a relocation's place to the
/// called address that its addend leaves out: an x86-64 call's displacement counts from the end
/// of its four bytes.
pub(super) fn calls(target: Target) -> (&'static [u32], i64) {
    match target {
        // R_X86_64_PC32, R_X86_64_PLT32, R_X86_64_GOTPCREL (a call through the table the image
        // builder relaxes).
        Target::X86_64 => (&[2, 4, 9], 4),
        // R_RISCV_JAL, R_RISCV_CALL, R_RISCV_CALL_PLT.
        Target::Rv32imac => (&[17, 18, 19], 0),
        // R_ARM_THM_CALL, R_ARM_THM_JUMP24.
        Target::ThumbV8mMainHf => (&[10, 30], 0),
        // R_AARCH64_JUMP26, R_AARCH64_CALL26.
        Target::Aarch64 => (&[282, 283], 0),
    }
}

/// The stack a module takes (specification 44 §8.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StackFigure {
    /// The bound of the stack every entry point may take, a recursion's cycle counted once.
    pub(super) bound: u64,
    /// The most one recursive call adds until the next: the frames of the largest cycle, and a
    /// quarter more; 0 for a module without a cycle.
    pub(super) call_level: u64,
}

/// The stack every entry point named in `entries` may take, from the linked module `elf` built for
/// `target` and the compiler's frames `frames`: the deepest chain over all of them, with the kernel's
/// functions, and a quarter more. A chain that comes back to a function it passed is a recursion
/// (§6.4), whose cycle counts once in the bound, all its frames together; each further turn takes a
/// call level of the call depth.
///
/// # Errors
///
/// [`NativeError::Module`] for a module the bound cannot be read from.
pub(super) fn bound(
    elf: &[u8],
    target: Target,
    frames: &[(String, u64)],
    entries: &[String],
) -> Result<StackFigure, NativeError> {
    let refuse = |reason: String| NativeError::Module(reason);
    // The functions and their calls, each function's call of itself among them. Functions the
    // linker folded into one, such as two entry points of the same code, share one start: the first
    // name stays in the graph, and every name keeps its start in `starts`.
    let graph = super::call_graph::read(elf, target)?;
    if graph.functions.is_empty() {
        return Err(refuse("the module has no code".into()));
    }
    let unbounded = unbounded_cycles(&graph);
    if !unbounded.is_empty() {
        let cycles: Vec<String> = unbounded
            .iter()
            .map(|cycle| match cycle.as_slice() {
                [one] => format!("{one} calls itself"),
                many => format!("{} call each other", many.join(", ")),
            })
            .collect();
        return Err(refuse(format!(
            "{}: no call depth of a Structured Text recursion bounds it, so the stack has no bound \
             (specification 44 §6.4, §8.5)",
            cycles.join("; ")
        )));
    }
    let file = object::File::parse(elf).map_err(|error| refuse(format!("{error}")))?;
    let starts: HashMap<String, u64> = file
        .symbols()
        .filter(|symbol| symbol.kind() == object::SymbolKind::Text)
        .filter_map(|symbol| Some((symbol.name().ok()?.to_string(), symbol.address() & !1)))
        .collect();
    let functions = &graph.functions;
    let containing = |address: u64| {
        let index = functions.partition_point(|function| function.start <= address);
        index
            .checked_sub(1)
            .filter(|index| address < functions[*index].end)
    };
    let mut callees: Vec<Vec<usize>> = vec![Vec::new(); functions.len()];
    for &(caller, callee) in &graph.edges {
        callees[caller].push(callee);
    }
    // A function's frame by its start, the largest of the names folded there.
    let mut frame_at: HashMap<u64, u64> = HashMap::new();
    for (name, bytes) in frames {
        if let Some(start) = starts.get(name) {
            let frame = frame_at.entry(*start).or_insert(0);
            *frame = (*frame).max(*bytes);
        }
    }
    // A call on x86-64 pushes its return address outside the frame the compiler reports; the
    // other targets keep it in a register or in the frame.
    let return_address = if target == Target::X86_64 { 8 } else { 0 };
    let frames: Vec<u64> = functions
        .iter()
        .map(|function| {
            frame_at
                .get(&function.start)
                .copied()
                .unwrap_or(UNKNOWN_FRAME)
                + return_address
        })
        .collect();
    let (deepest, cycle) = deepest_chains(&callees, &frames);
    let mut worst = 0u64;
    for entry in entries {
        let Some(index) = starts.get(entry).copied().and_then(containing) else {
            return Err(refuse(format!("the module has no entry point {entry}")));
        };
        worst = worst.max(deepest[index]);
    }
    let total = worst.saturating_add(KERNEL_FUNCTIONS);
    Ok(StackFigure {
        bound: total.saturating_add(total.div_ceil(4)),
        call_level: cycle.saturating_add(cycle.div_ceil(4)),
    })
}

/// The cycles of `graph` that pass through no `recurse` of the program library's interface
/// (§6.4), whose turns no call depth counts: each as its functions' names in their addresses'
/// order. Structured Text's recursion passes through `recurse`; a Rust POU's never does, and
/// specification 48 RU-18 refuses it first.
pub(super) fn unbounded_cycles(graph: &CallGraph) -> Vec<Vec<String>> {
    super::call_graph::cycles(graph)
        .into_iter()
        .filter(|cycle| {
            !cycle.iter().any(|function| {
                graph.functions[*function]
                    .name
                    .starts_with("trust_plc_abi::module::recurse")
            })
        })
        .map(|cycle| {
            cycle
                .into_iter()
                .map(|function| graph.functions[function].name.clone())
                .collect()
        })
        .collect()
}

/// The deepest chain of frames from each function of the call graph `callees`, each function's
/// frame in `frames`, a group of functions that call one another in a cycle counting all its frames
/// once; and the frames of the largest cycle, 0 for a graph without one. A group is a cycle when it
/// has several functions or its one function calls itself.
fn deepest_chains(callees: &[Vec<usize>], frames: &[u64]) -> (Vec<u64>, u64) {
    let (group, groups) = cycle_groups(callees);
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); groups];
    for (function, &of) in group.iter().enumerate() {
        members[of].push(function);
    }
    // The groups are numbered callers first, so every callee's group is done before its caller's.
    let mut deepest_group = vec![0u64; groups];
    let mut largest_cycle = 0u64;
    for of in (0..groups).rev() {
        let own: u64 = members[of]
            .iter()
            .map(|&function| frames[function])
            .fold(0, u64::saturating_add);
        let cycle = members[of].len() > 1
            || members[of]
                .iter()
                .any(|&function| callees[function].contains(&function));
        if cycle {
            largest_cycle = largest_cycle.max(own);
        }
        let below = members[of]
            .iter()
            .flat_map(|&function| &callees[function])
            .filter(|&&callee| group[callee] != of)
            .map(|&callee| deepest_group[group[callee]])
            .max()
            .unwrap_or(0);
        deepest_group[of] = own.saturating_add(below);
    }
    (
        group.iter().map(|&of| deepest_group[of]).collect(),
        largest_cycle,
    )
}

/// The group of each function, the functions that call one another in a cycle in one group, and
/// the number of groups, numbered so that a group's callers come before it (Kosaraju's two walks,
/// without recursion).
pub(super) fn cycle_groups(callees: &[Vec<usize>]) -> (Vec<usize>, usize) {
    let count = callees.len();
    // The order in which a depth-first walk of the calls finishes each function.
    let mut finished = Vec::with_capacity(count);
    let mut seen = vec![false; count];
    for root in 0..count {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut walk = vec![(root, 0usize)];
        while let Some(&(function, next)) = walk.last() {
            if let Some(&callee) = callees[function].get(next) {
                if let Some(top) = walk.last_mut() {
                    top.1 += 1;
                }
                if !seen[callee] {
                    seen[callee] = true;
                    walk.push((callee, 0));
                }
            } else {
                finished.push(function);
                walk.pop();
            }
        }
    }
    let mut callers: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (caller, called) in callees.iter().enumerate() {
        for &callee in called {
            callers[callee].push(caller);
        }
    }
    // Walking the callers from the function finished last gathers one group at a time.
    let mut group = vec![usize::MAX; count];
    let mut groups = 0;
    for &root in finished.iter().rev() {
        if group[root] != usize::MAX {
            continue;
        }
        group[root] = groups;
        let mut walk = vec![root];
        while let Some(function) = walk.pop() {
            for &caller in &callers[function] {
                if group[caller] == usize::MAX {
                    group[caller] = groups;
                    walk.push(caller);
                }
            }
        }
        groups += 1;
    }
    (group, groups)
}

#[cfg(test)]
mod tests {
    use super::deepest_chains;

    /// The deepest chain takes the deeper callee at every level: `a` (16) calls `b` (32) and `c`
    /// (64), `b` calls `c`, so the deepest chain is a, b, c, 112 bytes, where a and c alone are 80;
    /// `d` (100) is called by nobody and counts nothing, so the bound is not the 212 of all frames.
    #[test]
    fn the_deepest_chain_follows_the_deepest_callee() {
        let callees = vec![vec![1, 2], vec![2], vec![], vec![]];
        let frames = [16, 32, 64, 100];
        let (deepest, cycle) = deepest_chains(&callees, &frames);
        assert_eq!(deepest[0], 112);
        assert_eq!(deepest[1], 96);
        assert_eq!(deepest[3], 100);
        assert_eq!(cycle, 0, "a graph without a cycle has no call level");
    }

    /// Specification 44 §8.5: a recursion's cycle counts once in the chain, all its frames
    /// together, and its frames are the call level that each further turn takes of the call depth.
    /// `e` (8) calls `a` (16), `a` and `b` (32) call each other, and `b` calls `c` (64): `e` is
    /// 8 + (16 + 32) + 64 = 120, and the call level 48. Catches a cycle counted once per class
    /// instance, as before 23 October 2026, or not at all, and a call level of one frame.
    #[test]
    fn a_cycle_counts_once_and_its_frames_are_the_call_level() {
        let callees = vec![vec![1], vec![2], vec![1, 3], vec![]];
        let frames = [8, 16, 32, 64];
        let (deepest, cycle) = deepest_chains(&callees, &frames);
        assert_eq!(deepest[0], 120);
        assert_eq!(
            deepest[1], 112,
            "a function of the cycle starts the whole cycle"
        );
        assert_eq!(deepest[3], 64);
        assert_eq!(cycle, 48);
    }

    /// A function that calls itself is a cycle of one: its frame is the call level. `f` (24) calls
    /// itself and `g` (40): `f` is 24 + 40 = 64, the call level 24. Catches a call of a function to
    /// itself left out of the graph, which no call level would bound.
    #[test]
    fn a_function_that_calls_itself_is_a_cycle_of_one() {
        let callees = vec![vec![0, 1], vec![]];
        let frames = [24, 40];
        let (deepest, cycle) = deepest_chains(&callees, &frames);
        assert_eq!(deepest[0], 64);
        assert_eq!(cycle, 24);
    }

    #[test]
    fn a_cycle_without_a_recursive_call_has_no_bound() {
        // Specification 44 §6.4 and §8.5: Structured Text's recursion passes through `recurse`,
        // whose call depth bounds it; a function that calls itself, or two that call each other,
        // without it have no bound and are named, in their addresses' order. Catches such a cycle
        // taken for a bounded recursion, and Structured Text's refused.
        use super::super::call_graph::{CallGraph, Function};
        let function = |index: u64, name: &str| Function {
            start: index * 16,
            end: index * 16 + 16,
            raw: name.to_string(),
            name: name.to_string(),
        };
        let graph = CallGraph {
            functions: vec![
                function(0, "trust_module::f_FACT"),
                function(1, "trust_plc_abi::module::recurse"),
                function(2, "libm::helper"),
                function(3, "lib::even"),
                function(4, "lib::odd"),
            ],
            edges: vec![(0, 1), (1, 0), (2, 2), (3, 4), (4, 3)],
        };
        assert_eq!(
            super::unbounded_cycles(&graph),
            [
                vec!["libm::helper".to_string()],
                vec!["lib::even".to_string(), "lib::odd".to_string()]
            ]
        );
    }
}
