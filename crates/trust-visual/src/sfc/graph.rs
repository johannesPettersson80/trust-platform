//! An SFC file as the editor's canvas graph, and the graph as the file the editor saves
//! (specification 25 §11.4).
//!
//! The graph is what the editor's canvas (React Flow) holds: `nodes` (steps, parallel splits and
//! parallel joins), `edges` (the transitions) and the program's `variables`. A transition from a
//! split to one of its branches leaves the split at the branch's handle, `branch-out-<index>`, and
//! a transition from a branch into a join enters at `branch-in-<index>`; the handle index is the
//! branch's place in the split's or join's `branchIds`, so the branch order survives the canvas.
//! Where a transition attaches to a step is drawing; the editor adds it, and a saved file never
//! holds it.
//!
//! A graph is written as the name `SFC_Program`, the steps, the transitions, the parallel splits
//! and joins (only when there are any), the variables and the metadata `{ modified, version:
//! "1.0" }`; saving over a file then keeps that file's name and metadata ([`keep_identity`]).

use std::cmp::Ordering;

use super::{
    branch_handle_index, branch_ids, normalize_branch_count, Workspace, JOIN_BRANCH_PREFIX,
    JOIN_OUTPUT_HANDLE, SPLIT_BRANCH_PREFIX, SPLIT_INPUT_HANDLE,
};
use crate::js::{member, nullish_or, or, strict_eq, truthy};
use crate::Json;

/// The name every saved SFC file gets.
const SAVED_NAME: &str = "SFC_Program";
/// The metadata version every saved SFC file gets.
const SAVED_VERSION: &str = "1.0";

/// `saved` (a file written by [`file()`]) with the name and metadata of the file it replaces,
/// `previous`: a non-empty `name` is kept instead of `SFC_Program`, and the metadata (`author`,
/// `description`, `created`, `version`, …) is kept with `modified` set to `now` and `version` added
/// as `1.0` when it has none.
pub fn keep_identity(saved: &mut Json, previous: &Json, now: &str) {
    if let Some(name) = previous.str_at("name").filter(|name| !name.is_empty()) {
        saved.set("name", Json::str(name));
    }
    if let Some(metadata) = previous
        .get("metadata")
        .filter(|metadata| metadata.is_object())
    {
        let mut kept = metadata.clone();
        kept.set("modified", Json::str(now));
        if kept.get("version").is_none() {
            kept.set("version", Json::str(SAVED_VERSION));
        }
        saved.set("metadata", kept);
    }
}

/// An object with the members `members`, leaving out each one that is `undefined` (`None`), as
/// `JSON.stringify` leaves it out.
fn object(members: Vec<(&str, Option<Json>)>) -> Json {
    Json::Object(
        members
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value)))
            .collect(),
    )
}

/// A canvas position `{ x: x || default_x, y: y || default_y }`.
fn position(node: &Json, default_x: f64, default_y: f64) -> Json {
    let default_x = Json::Number(default_x);
    let default_y = Json::Number(default_y);
    object(vec![
        ("x", Some(or(node.get("x"), &default_x).clone())),
        ("y", Some(or(node.get("y"), &default_y).clone())),
    ])
}

/// The canvas graph of the SFC file `file`.
///
/// # Errors
/// The file is not an SFC program: steps or transitions are not arrays, or a parallel split or
/// join has no `branchIds` array.
pub fn graph(file: &Json) -> Result<Json, String> {
    let workspace = Workspace::read(file)?;
    let empty = Json::Array(Vec::new());
    let mut nodes: Vec<Json> = workspace
        .steps
        .iter()
        .map(|step| step_node(step, &empty))
        .collect();
    nodes.extend(
        workspace
            .splits
            .iter()
            .map(|split| parallel_node(split, "parallelSplit", 150.0)),
    );
    nodes.extend(
        workspace
            .joins
            .iter()
            .map(|join| parallel_node(join, "parallelJoin", 300.0)),
    );
    let edges = workspace
        .transitions
        .iter()
        .map(|transition| edge(&workspace, transition))
        .collect();
    Ok(object(vec![
        ("nodes", Some(Json::Array(nodes))),
        ("edges", Some(Json::Array(edges))),
        ("variables", Some(or(file.get("variables"), &empty).clone())),
    ]))
}

fn step_node(step: &Json, empty: &Json) -> Json {
    let kind = if truthy(step.get("initial")) {
        "initial"
    } else {
        "normal"
    };
    object(vec![
        ("id", step.get("id").cloned()),
        ("type", Some(Json::str("step"))),
        ("position", Some(position(step, 250.0, 50.0))),
        (
            "data",
            Some(object(vec![
                ("label", step.get("name").cloned()),
                ("type", Some(Json::str(kind))),
                ("actions", Some(or(step.get("actions"), empty).clone())),
            ])),
        ),
    ])
}

fn parallel_node(node: &Json, node_type: &str, default_y: f64) -> Json {
    let branches = branch_ids(node).map_or(0, <[Json]>::len);
    // A branch count is a small whole number; the list of a real file is far below 2^53.
    #[allow(clippy::cast_precision_loss)]
    let count = Json::Number(branches as f64);
    object(vec![
        ("id", node.get("id").cloned()),
        ("type", Some(Json::str(node_type))),
        ("position", Some(position(node, 250.0, default_y))),
        (
            "data",
            Some(object(vec![
                ("label", node.get("name").cloned()),
                ("nodeType", Some(Json::str(node_type))),
                (
                    "branchCount",
                    Some(Json::Number(normalize_branch_count(Some(&count)))),
                ),
            ])),
        ),
    ])
}

/// The last parallel node in `nodes` whose id is `id` (a later node with the same id wins).
fn parallel_with_id<'a>(nodes: &'a [Json], id: Option<&Json>) -> Option<&'a Json> {
    nodes
        .iter()
        .rev()
        .find(|node| strict_eq(node.get("id"), id))
}

/// The place of `id` in the branches of `node`.
fn branch_index(node: &Json, id: Option<&Json>) -> Option<usize> {
    branch_ids(node)?
        .iter()
        .position(|branch| strict_eq(Some(branch), id))
}

fn edge(workspace: &Workspace<'_>, transition: &Json) -> Json {
    let source = transition.get("sourceStepId");
    let target = transition.get("targetStepId");
    let condition = transition.get("condition");
    let name = transition.get("name");
    // `trans.name || trans.condition`.
    let label = if truthy(name) { name } else { condition };
    let default_condition = Json::str("TRUE");
    let data_condition = nullish_or(condition, Some(&default_condition));
    let data_label = nullish_or(label, data_condition);
    let priority = nullish_or(transition.get("priority"), None);

    // Each later rule overrides an earlier one only when it applies.
    let mut source_handle = None;
    let mut target_handle = None;
    if let Some(index) =
        parallel_with_id(workspace.splits, source).and_then(|split| branch_index(split, target))
    {
        source_handle = Some(Json::String(format!("{SPLIT_BRANCH_PREFIX}{index}")));
    }
    if parallel_with_id(workspace.joins, source).is_some() {
        source_handle = Some(Json::str(JOIN_OUTPUT_HANDLE));
    }
    if parallel_with_id(workspace.splits, target).is_some() {
        target_handle = Some(Json::str(SPLIT_INPUT_HANDLE));
    }
    if let Some(index) =
        parallel_with_id(workspace.joins, target).and_then(|join| branch_index(join, source))
    {
        target_handle = Some(Json::String(format!("{JOIN_BRANCH_PREFIX}{index}")));
    }

    object(vec![
        ("id", transition.get("id").cloned()),
        ("source", source.cloned()),
        ("target", target.cloned()),
        ("type", Some(Json::str("transition"))),
        (
            "data",
            Some(object(vec![
                ("condition", data_condition.cloned()),
                ("label", data_label.cloned()),
                ("priority", priority.cloned()),
            ])),
        ),
        ("label", label.cloned()),
        ("sourceHandle", source_handle),
        ("targetHandle", target_handle),
    ])
}

/// The nodes of `kind` (`step`, `parallelSplit`, `parallelJoin`) of a graph.
fn nodes_of<'a>(nodes: &'a [Json], kind: &'a str) -> impl Iterator<Item = &'a Json> + 'a {
    nodes
        .iter()
        .filter(move |node| node.str_at("type") == Some(kind))
}

/// The nodes and edges of a canvas graph.
pub(super) fn graph_parts(graph: &Json) -> Result<(&[Json], &[Json]), String> {
    let nodes = graph.get("nodes").and_then(Json::as_array);
    let edges = graph.get("edges").and_then(Json::as_array);
    match (nodes, edges) {
        (Some(nodes), Some(edges)) => Ok((nodes.as_slice(), edges.as_slice())),
        _ => Err("The SFC editor sent a graph without nodes and edges.".to_owned()),
    }
}

/// The SFC file of the canvas graph `graph`, saved at `now` (an ISO 8601 time).
///
/// # Errors
/// The graph has no `nodes` or `edges` array.
pub fn file(graph: &Json, now: &str) -> Result<Json, String> {
    let (nodes, edges) = graph_parts(graph)?;
    let empty = Json::Array(Vec::new());
    let steps = nodes_of(nodes, "step")
        .map(|node| saved_step(node, &empty))
        .collect();
    let transitions = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| saved_transition(index, edge))
        .collect();
    let splits: Vec<Json> = nodes_of(nodes, "parallelSplit")
        .map(|node| {
            let branches = branch_ends(
                edges,
                node,
                "source",
                "sourceHandle",
                SPLIT_BRANCH_PREFIX,
                "target",
            );
            parallel_entry(node, branches, None)
        })
        .collect();
    let joins: Vec<Json> = nodes_of(nodes, "parallelJoin")
        .map(|node| {
            let branches = branch_ends(
                edges,
                node,
                "target",
                "targetHandle",
                JOIN_BRANCH_PREFIX,
                "source",
            );
            let next = edges
                .iter()
                .find(|edge| {
                    strict_eq(edge.get("source"), node.get("id"))
                        && edge.str_at("sourceHandle") == Some(JOIN_OUTPUT_HANDLE)
                })
                .and_then(|edge| edge.get("target"))
                .cloned();
            parallel_entry(node, branches, next)
        })
        .collect();
    Ok(object(vec![
        ("name", Some(Json::str(SAVED_NAME))),
        ("steps", Some(Json::Array(steps))),
        ("transitions", Some(Json::Array(transitions))),
        (
            "parallelSplits",
            (!splits.is_empty()).then_some(Json::Array(splits)),
        ),
        (
            "parallelJoins",
            (!joins.is_empty()).then_some(Json::Array(joins)),
        ),
        (
            "variables",
            Some(or(graph.get("variables"), &empty).clone()),
        ),
        (
            "metadata",
            Some(object(vec![
                ("modified", Some(Json::str(now))),
                ("version", Some(Json::str(SAVED_VERSION))),
            ])),
        ),
    ]))
}

/// A step node as the file writes it.
fn saved_step(node: &Json, empty: &Json) -> Json {
    let data = node.get("data");
    object(vec![
        ("id", node.get("id").cloned()),
        ("name", member(data, "label").cloned()),
        (
            "initial",
            Some(Json::Bool(
                member(data, "type").and_then(Json::as_str) == Some("initial"),
            )),
        ),
        ("x", member(node.get("position"), "x").cloned()),
        ("y", member(node.get("position"), "y").cloned()),
        ("actions", Some(or(member(data, "actions"), empty).clone())),
    ])
}

/// The `index`-th edge as the file writes its transition: a missing label is `T<index + 1>`, a
/// missing condition `TRUE`.
fn saved_transition(index: usize, edge: &Json) -> Json {
    let data = edge.get("data");
    let fallback_name = Json::String(format!("T{}", index + 1));
    let fallback_condition = Json::str("TRUE");
    object(vec![
        ("id", edge.get("id").cloned()),
        (
            "name",
            Some(or(member(data, "label"), &fallback_name).clone()),
        ),
        (
            "condition",
            Some(or(member(data, "condition"), &fallback_condition).clone()),
        ),
        ("sourceStepId", edge.get("source").cloned()),
        ("targetStepId", edge.get("target").cloned()),
        ("priority", member(data, "priority").cloned()),
    ])
}

/// The branch ends of a parallel node: the `far_end` of each edge whose `near_end` is the node and
/// whose `handle` is a branch handle, in the order of the branch index (a stable order for equal
/// indexes). An end that is missing is written as `null`, as `JSON.stringify` writes it in a list.
fn branch_ends(
    edges: &[Json],
    node: &Json,
    near_end: &str,
    handle: &str,
    prefix: &str,
    far_end: &str,
) -> Vec<Json> {
    let mut branches: Vec<(f64, &Json)> = edges
        .iter()
        .filter(|edge| strict_eq(edge.get(near_end), node.get("id")))
        .filter_map(|edge| branch_handle_index(edge.get(handle), prefix).map(|index| (index, edge)))
        .collect();
    // `(left.index ?? 0) - (right.index ?? 0)`: -0 and 0 are the same place.
    branches.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(Ordering::Equal));
    branches
        .into_iter()
        .map(|(_, edge)| edge.get(far_end).cloned().unwrap_or(Json::Null))
        .collect()
}

fn parallel_entry(node: &Json, branches: Vec<Json>, next: Option<Json>) -> Json {
    object(vec![
        ("id", node.get("id").cloned()),
        ("name", member(node.get("data"), "label").cloned()),
        ("x", member(node.get("position"), "x").cloned()),
        ("y", member(node.get("position"), "y").cloned()),
        ("branchIds", Some(Json::Array(branches))),
        ("nextStepId", next),
    ])
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
