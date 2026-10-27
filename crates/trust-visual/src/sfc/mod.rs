//! SFC programs, `*.sfc.json` and `*.sfc` (specification 25 §11.4, specification 17 §3,
//! `sfc-profile.md`).
//!
//! An SFC program is the JSON file the SFC editor saves: its steps (with their actions), its
//! transitions, its parallel splits and joins, its variables and its metadata. This module holds
//! every rule about that file:
//!
//! - [`generate`]: the Structured Text companion, a `FUNCTION_BLOCK` that executes the chart by
//!   IEC 61131-3 §6.7: step flags, action control with final scan, transitions in priority order
//!   and the evolution (`sfc-profile.md`, "Execution of a visual SFC"). The runtime refuses
//!   textual SFC (`sfc-profile.md`), so the companion is plain Structured Text.
//! - [`validate`]: the editor's Validate, the checks of a chart the companion can be made of.
//! - [`graph`] and [`file()`]: the file as the editor's canvas graph (React Flow nodes and edges) and
//!   back, exactly as the editor wrote it before the rules moved here.
//! - [`connect`], [`set_branch_count`] and [`add_node`]: the edits the editor stores in the file.
//!
//! Where a transition attaches to a step on the canvas (top, side or bottom handle) and where a new
//! node is placed are drawing, and stay in the editor.

mod chart;
mod edit;
mod generate;
mod graph;
mod validate;

pub use edit::{add_node, connect, set_branch_count, Added, BranchCount, NodeType};
pub use generate::generate;
pub use graph::{file, graph, keep_identity};
pub use validate::{validate, Issue, IssueKind};

use crate::js;
use crate::Json;

/// The fewest branches a parallel split or join has (IEC 61131-3 parallel divergence).
const MIN_PARALLEL_BRANCHES: f64 = 2.0;
/// The input handle of a parallel split.
const SPLIT_INPUT_HANDLE: &str = "in";
/// The output handle of a parallel join.
const JOIN_OUTPUT_HANDLE: &str = "out";
/// The prefix of a split's branch handles: `branch-out-<index>`.
const SPLIT_BRANCH_PREFIX: &str = "branch-out-";
/// The prefix of a join's branch handles: `branch-in-<index>`.
const JOIN_BRANCH_PREFIX: &str = "branch-in-";

/// The message when a file is not an SFC program.
const FORMAT_MESSAGE: &str =
    "Invalid SFC workspace format. Expected JSON object with steps and transitions.";

/// The parts of an SFC file that the checks and the canvas read.
struct Workspace<'a> {
    steps: &'a [Json],
    transitions: &'a [Json],
    splits: &'a [Json],
    joins: &'a [Json],
}

impl<'a> Workspace<'a> {
    /// The steps, transitions, parallel splits and parallel joins of `file`.
    ///
    /// Steps and transitions must be arrays; splits and joins are optional, but when present each
    /// must be an array whose entries have a `branchIds` array.
    fn read(file: &'a Json) -> Result<Self, String> {
        let steps = file
            .get("steps")
            .and_then(Json::as_array)
            .ok_or(FORMAT_MESSAGE)?;
        let transitions = file
            .get("transitions")
            .and_then(Json::as_array)
            .ok_or(FORMAT_MESSAGE)?;
        let splits = parallel_nodes(file, "parallelSplits", "Parallel split")?;
        let joins = parallel_nodes(file, "parallelJoins", "Parallel join")?;
        Ok(Self {
            steps,
            transitions,
            splits,
            joins,
        })
    }
}

/// The parallel splits or joins under `key`: none when the member is missing or falsy.
fn parallel_nodes<'a>(file: &'a Json, key: &str, what: &str) -> Result<&'a [Json], String> {
    let value = file.get(key);
    if !js::truthy(value) {
        return Ok(&[]);
    }
    let nodes = value
        .and_then(Json::as_array)
        .ok_or_else(|| format!("Invalid SFC workspace format. {key} must be an array."))?;
    for node in nodes {
        if branch_ids(node).is_none() {
            return Err(format!(
                "Invalid SFC workspace format. {what} {} needs a branchIds array.",
                js::string(js::member(Some(node), "name"))
            ));
        }
    }
    Ok(nodes)
}

/// The `branchIds` of a parallel split or join.
fn branch_ids(node: &Json) -> Option<&[Json]> {
    node.get("branchIds")
        .and_then(Json::as_array)
        .map(Vec::as_slice)
}

/// The branch count of a parallel node: the number given, rounded down and at least two; two for
/// anything that is not a finite number.
fn normalize_branch_count(value: Option<&Json>) -> f64 {
    match value.and_then(Json::as_finite) {
        Some(count) => count.floor().max(MIN_PARALLEL_BRANCHES),
        None => MIN_PARALLEL_BRANCHES,
    }
}

/// The branch index of the handle `handle` when it is `<prefix><index>` with an index from zero.
fn branch_handle_index(handle: Option<&Json>, prefix: &str) -> Option<f64> {
    let index = handle?
        .as_str()?
        .strip_prefix(prefix)
        .and_then(js::parse_int)?;
    (index.is_finite() && index >= 0.0).then_some(index)
}
