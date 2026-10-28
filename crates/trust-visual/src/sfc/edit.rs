//! The edits the SFC editor stores in the file (specification 25 §11.4): a transition the user
//! draws on the canvas, the branch count of a parallel split or join, and a new node.
//!
//! The editor sends its canvas graph (`nodes`, `edges`); each edit answers what changes. A
//! connection that the rules refuse changes nothing, and the editor draws nothing. Where a new node
//! is placed and where a transition attaches to a step are drawing, so the editor computes them and
//! sends them along.

use super::graph::graph_parts;
use super::{
    branch_handle_index, normalize_branch_count, JOIN_BRANCH_PREFIX, JOIN_OUTPUT_HANDLE,
    MIN_PARALLEL_BRANCHES, SPLIT_BRANCH_PREFIX, SPLIT_INPUT_HANDLE,
};
use crate::js::{as_number, member, nullish_or, strict_eq, truthy};
use crate::Json;

/// The kind of a node the editor adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// A step.
    Step,
    /// A parallel split (divergence).
    ParallelSplit,
    /// A parallel join (convergence).
    ParallelJoin,
}

impl NodeType {
    /// The node type called `name` on the canvas: `step`, `parallelSplit` or `parallelJoin`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "step" => Some(Self::Step),
            "parallelSplit" => Some(Self::ParallelSplit),
            "parallelJoin" => Some(Self::ParallelJoin),
            _ => None,
        }
    }
}

/// A new node, and the transition that wires a new step onto the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct Added {
    /// The new canvas node.
    pub node: Json,
    /// The new transition into it, when it was wired.
    pub edge: Option<Json>,
}

/// The branch count a parallel node gets and the transitions that no longer have a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchCount {
    /// The branch count: the number asked for, rounded down and at least two.
    pub branch_count: f64,
    /// The ids of the transitions on a branch handle at or beyond the new count.
    pub removed_edges: Vec<Json>,
}

/// A new transition: condition `TRUE`, labelled `TRUE`, with the id `trans_<stamp>`.
fn transition(stamp: &str, source: Option<&Json>, target: Option<&Json>) -> Vec<(String, Json)> {
    let mut members = vec![("id".to_owned(), Json::String(format!("trans_{stamp}")))];
    if let Some(source) = source {
        members.push(("source".to_owned(), source.clone()));
    }
    if let Some(target) = target {
        members.push(("target".to_owned(), target.clone()));
    }
    members
}

fn finish_transition(mut members: Vec<(String, Json)>) -> Json {
    members.push(("type".to_owned(), Json::str("transition")));
    members.push((
        "data".to_owned(),
        Json::Object(vec![
            ("condition".to_owned(), Json::str("TRUE")),
            ("label".to_owned(), Json::str("TRUE")),
        ]),
    ));
    members.push(("label".to_owned(), Json::str("TRUE")));
    Json::Object(members)
}

/// The first node whose id is `id`.
fn node_with_id<'a>(nodes: &'a [Json], id: Option<&Json>) -> Option<&'a Json> {
    nodes.iter().find(|node| strict_eq(node.get("id"), id))
}

/// The branch count of a parallel node.
fn branch_count(node: &Json) -> f64 {
    normalize_branch_count(member(node.get("data"), "branchCount"))
}

/// Whether some edge has `end` equal to `id` and `handle` equal to `value`.
fn handle_taken(
    edges: &[Json],
    end: &str,
    id: Option<&Json>,
    handle: &str,
    value: Option<&Json>,
) -> bool {
    edges
        .iter()
        .any(|edge| strict_eq(edge.get(end), id) && strict_eq(edge.get(handle), value))
}

/// The transition the user drew, `connection` (`source`, `target`, `sourceHandle`,
/// `targetHandle`), as the canvas edge to add; `fallback` holds the step handles the editor drew
/// for an end the connection names no handle for. The new transition gets the id `trans_<stamp>`.
///
/// A connection is refused when it has no source or target or loops on one node, when its nodes
/// are gone, when the same connection exists, or when it breaks a parallel node: a split has one
/// input (`in`) and starts each branch once on a handle `branch-out-<index>` below its branch
/// count; a join ends each branch once on `branch-in-<index>` and has one output (`out`).
///
/// # Errors
/// The connection is refused; the message says why.
pub fn connect(
    graph: &Json,
    connection: &Json,
    fallback: &Json,
    stamp: &str,
) -> Result<Json, String> {
    let (nodes, edges) = graph_parts(graph)?;
    let source = connection.get("source");
    let target = connection.get("target");
    if !truthy(source) || !truthy(target) {
        return Err("A transition needs a source and a target.".to_owned());
    }
    if strict_eq(source, target) {
        return Err("A transition cannot start and end at the same node.".to_owned());
    }
    let (Some(source_node), Some(target_node)) =
        (node_with_id(nodes, source), node_with_id(nodes, target))
    else {
        return Err("A transition must connect two nodes of the chart.".to_owned());
    };
    let source_handle = connection.get("sourceHandle");
    let target_handle = connection.get("targetHandle");
    let same_connection = edges.iter().any(|edge| {
        strict_eq(edge.get("source"), source)
            && strict_eq(edge.get("target"), target)
            && strict_eq(edge.get("sourceHandle"), source_handle)
            && strict_eq(edge.get("targetHandle"), target_handle)
    });
    if same_connection {
        return Err("This transition already exists.".to_owned());
    }
    check_source(edges, source_node, source_handle)?;
    check_target(edges, target_node, target_handle)?;

    let source_handle = nullish_or(source_handle, fallback.get("sourceHandle"));
    let target_handle = nullish_or(target_handle, fallback.get("targetHandle"));
    // The same transition, compared as the canvas compares new edges: a missing or empty handle
    // matches a missing or empty handle.
    let handle_matches = |existing: Option<&Json>, new: Option<&Json>| {
        strict_eq(existing, new) || (!truthy(existing) && !truthy(new))
    };
    let exists = edges.iter().any(|edge| {
        strict_eq(edge.get("source"), source)
            && strict_eq(edge.get("target"), target)
            && handle_matches(edge.get("sourceHandle"), source_handle)
            && handle_matches(edge.get("targetHandle"), target_handle)
    });
    if exists {
        return Err("This transition already exists.".to_owned());
    }
    let mut members = transition(stamp, source, target);
    // A `null` handle is left out of the new edge.
    for (key, handle) in [
        ("sourceHandle", source_handle),
        ("targetHandle", target_handle),
    ] {
        if let Some(handle) = handle.filter(|handle| **handle != Json::Null) {
            members.push((key.to_owned(), handle.clone()));
        }
    }
    Ok(finish_transition(members))
}

fn check_source(edges: &[Json], node: &Json, handle: Option<&Json>) -> Result<(), String> {
    let id = node.get("id");
    match node.str_at("type") {
        Some("parallelSplit") => {
            let free = branch_handle_index(handle, SPLIT_BRANCH_PREFIX)
                .is_some_and(|index| index < branch_count(node))
                && !handle_taken(edges, "source", id, "sourceHandle", handle);
            if !free {
                return Err(
                    "A parallel split starts each branch once, from a branch handle.".to_owned(),
                );
            }
        }
        Some("parallelJoin") => {
            let output = Json::str(JOIN_OUTPUT_HANDLE);
            let free = strict_eq(handle, Some(&output))
                && !handle_taken(edges, "source", id, "sourceHandle", Some(&output));
            if !free {
                return Err("A parallel join has one outgoing transition.".to_owned());
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_target(edges: &[Json], node: &Json, handle: Option<&Json>) -> Result<(), String> {
    let id = node.get("id");
    match node.str_at("type") {
        Some("parallelSplit") => {
            let input = Json::str(SPLIT_INPUT_HANDLE);
            let free = strict_eq(handle, Some(&input))
                && !handle_taken(edges, "target", id, "targetHandle", Some(&input));
            if !free {
                return Err("A parallel split has one incoming transition.".to_owned());
            }
        }
        Some("parallelJoin") => {
            let free = branch_handle_index(handle, JOIN_BRANCH_PREFIX)
                .is_some_and(|index| index < branch_count(node))
                && !handle_taken(edges, "target", id, "targetHandle", handle);
            if !free {
                return Err("A parallel join ends each branch once, on a branch handle.".to_owned());
            }
        }
        _ => {}
    }
    Ok(())
}

/// Sets the branch count of the parallel node `node_id` to `count`: the number rounded down and at
/// least two, or two for anything that is not a finite number. The transitions that leave the node
/// on a split branch handle, or enter it on a join branch handle, at or beyond the new count lose
/// their branch and are removed.
///
/// # Errors
/// The graph has no `nodes` or `edges` array.
pub fn set_branch_count(
    graph: &Json,
    node_id: &str,
    count: Option<&Json>,
) -> Result<BranchCount, String> {
    let (_, edges) = graph_parts(graph)?;
    let branch_count = normalize_branch_count(count);
    let id = Json::str(node_id);
    let beyond = |edge: &Json, end: &str, handle: &str, prefix: &str| {
        strict_eq(edge.get(end), Some(&id))
            && branch_handle_index(edge.get(handle), prefix)
                .is_some_and(|index| index >= branch_count)
    };
    let removed_edges = edges
        .iter()
        .filter(|edge| {
            beyond(edge, "source", "sourceHandle", SPLIT_BRANCH_PREFIX)
                || beyond(edge, "target", "targetHandle", JOIN_BRANCH_PREFIX)
        })
        .map(|edge| edge.get("id").cloned().unwrap_or(Json::Null))
        .collect();
    Ok(BranchCount {
        branch_count,
        removed_edges,
    })
}

/// The step a new step is wired onto: among the steps without an outgoing transition (or all
/// steps when every step has one) the lowest on the canvas, the first of equals.
fn terminal_step<'a>(nodes: &'a [Json], edges: &[Json]) -> Option<&'a Json> {
    let steps: Vec<&Json> = nodes
        .iter()
        .filter(|node| node.str_at("type") == Some("step"))
        .collect();
    let open: Vec<&Json> = steps
        .iter()
        .copied()
        .filter(|step| {
            !edges
                .iter()
                .any(|edge| strict_eq(edge.get("source"), step.get("id")))
        })
        .collect();
    let pool = if open.is_empty() { steps } else { open };
    let y = |node: &Json| as_number(member(node.get("position"), "y"));
    pool.into_iter()
        .reduce(|lowest, step| if y(step) > y(lowest) { step } else { lowest })
}

/// A new node of `node_type` at `position` (`{ x, y }`, placed by the editor), with the id
/// `step_<stamp>`, `split_<stamp>` or `join_<stamp>`. A step is called `Step<n>` for the n-th step
/// of the chart; a split or join starts with two branches. With `wire`, a new step is connected
/// by a `TRUE` transition from the step at the open end of the chart (the lowest step without an
/// outgoing transition), so a step added from the toolbar joins the flow.
///
/// # Errors
/// The graph has no `nodes` or `edges` array.
pub fn add_node(
    graph: &Json,
    node_type: NodeType,
    position: &Json,
    wire: bool,
    stamp: &str,
) -> Result<Added, String> {
    let (nodes, edges) = graph_parts(graph)?;
    let (id, data) = match node_type {
        NodeType::Step => {
            let steps = nodes
                .iter()
                .filter(|node| node.str_at("type") == Some("step"))
                .count();
            (
                format!("step_{stamp}"),
                vec![
                    (
                        "label".to_owned(),
                        Json::String(format!("Step{}", steps + 1)),
                    ),
                    ("type".to_owned(), Json::str("normal")),
                    ("actions".to_owned(), Json::Array(Vec::new())),
                ],
            )
        }
        NodeType::ParallelSplit => (
            format!("split_{stamp}"),
            parallel_data("Parallel Split", "parallelSplit"),
        ),
        NodeType::ParallelJoin => (
            format!("join_{stamp}"),
            parallel_data("Parallel Join", "parallelJoin"),
        ),
    };
    let type_name = match node_type {
        NodeType::Step => "step",
        NodeType::ParallelSplit => "parallelSplit",
        NodeType::ParallelJoin => "parallelJoin",
    };
    let node = Json::Object(vec![
        ("id".to_owned(), Json::String(id.clone())),
        ("type".to_owned(), Json::str(type_name)),
        ("position".to_owned(), position.clone()),
        ("data".to_owned(), Json::Object(data)),
    ]);
    let edge = if wire && node_type == NodeType::Step {
        terminal_step(nodes, edges).map(|terminal| {
            finish_transition(transition(
                stamp,
                terminal.get("id"),
                Some(&Json::String(id)),
            ))
        })
    } else {
        None
    };
    Ok(Added { node, edge })
}

fn parallel_data(label: &str, node_type: &str) -> Vec<(String, Json)> {
    vec![
        ("label".to_owned(), Json::str(label)),
        ("nodeType".to_owned(), Json::str(node_type)),
        (
            "branchCount".to_owned(),
            Json::Number(MIN_PARALLEL_BRANCHES),
        ),
    ]
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
