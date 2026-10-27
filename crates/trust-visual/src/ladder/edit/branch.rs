//! Parallel contacts: the branch shortcut of the Ladder editor (specification 25 §11.4).
//!
//! On a contact in a series rung, the shortcut puts the contact between a new split and merge and
//! adds a second, empty contact 60 below it; on a contact that already is a leg of a branch, it adds
//! one more empty leg below the lowest. The split and merge sit 100 left and right of the contact,
//! kept 80 clear of its neighbours and 20 clear of the rails, all on the grid. When the branch
//! grows into the next rung, that rung and every rung below move down by the overlap, on the grid.
//! A request that does not fit changes nothing.

use std::fmt;

use super::geometry::{position, route, snap, LEFT_RAIL_X, RIGHT_RAIL_X};
use super::{edge, fresh_edge_id, fresh_node_id, items, network_mut};
use crate::ladder::power::{node_id, sorted_nodes};
use crate::Json;

/// The clearance of a split or merge from a rail.
const RAIL_CLEARANCE: f64 = 20.0;
/// The least horizontal distance between a split and its merge, and from a neighbour.
const MIN_SPAN: f64 = 80.0;
/// The vertical distance between two legs.
const LEG_PITCH: f64 = 60.0;
/// The margin below the lowest element of a rung.
const CONTENT_MARGIN: f64 = 40.0;
/// The clearance between that margin and the next rung.
const RUNG_CLEARANCE: f64 = 40.0;

/// Why a parallel contact cannot be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    /// The rung does not exist.
    RungNotFound,
    /// The selected element is not a contact of that rung.
    ContactNotFound,
    /// The rung already has topology nodes, so the contact is on no plain series path.
    UnsupportedTopology,
    /// The neighbours leave no room for a split and merge around the contact.
    InsufficientHorizontalSpace,
    /// The contact is wired in a way no leg can be added to.
    InvalidTopology,
}

impl EditError {
    /// The code of the refusal in the command answer.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::RungNotFound => "rung-not-found",
            Self::ContactNotFound => "contact-not-found",
            Self::UnsupportedTopology => "unsupported-topology",
            Self::InsufficientHorizontalSpace => "insufficient-horizontal-space",
            Self::InvalidTopology => "invalid-topology",
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RungNotFound => "Selected rung no longer exists.",
            Self::ContactNotFound => "Selected contact is no longer valid.",
            Self::UnsupportedTopology => "This contact is not part of a branchable path.",
            Self::InsufficientHorizontalSpace => {
                "Not enough horizontal space in this rung; move the contact right and retry."
            }
            Self::InvalidTopology => {
                "Cannot add another parallel contact on this topology. Use Clear Wiring and retry."
            }
        })
    }
}

/// A leg the shortcut added.
#[derive(Debug, Clone, PartialEq)]
pub struct Leg {
    /// The id of the new, empty contact (the editor selects it).
    pub selected_node_id: String,
    /// How far the rungs below moved down.
    pub rung_shift_px: f64,
    /// The vertical distance between the highest and the lowest leg.
    pub added_vertical_span_px: f64,
    /// The legs of the branch now.
    pub total_parallel_legs: usize,
}

fn node<'a>(network: &'a Json, id: &str) -> Option<&'a Json> {
    // The last node of an id wins, as a JavaScript Map built from the node list keeps it.
    items(network, "nodes")
        .iter()
        .rev()
        .find(|node| node_id(node) == id)
}

/// The split and merge a contact is a leg of: a split wired into it and a merge it is wired to.
fn branch_of(network: &Json, contact: &str) -> Option<(String, String)> {
    let edges = items(network, "edges");
    for incoming in edges
        .iter()
        .filter(|edge| edge.str_at("toNodeId") == Some(contact))
    {
        let split = node(network, incoming.str_at("fromNodeId").unwrap_or_default());
        if split.and_then(|split| split.str_at("type")) != Some("branchSplit") {
            continue;
        }
        for outgoing in edges
            .iter()
            .filter(|edge| edge.str_at("fromNodeId") == Some(contact))
        {
            let merge = node(network, outgoing.str_at("toNodeId").unwrap_or_default());
            if merge.and_then(|merge| merge.str_at("type")) == Some("branchMerge") {
                return Some((
                    node_id(split.unwrap_or(&Json::Null)).to_owned(),
                    node_id(merge.unwrap_or(&Json::Null)).to_owned(),
                ));
            }
        }
    }
    None
}

/// The contacts wired from `split` to `merge`, in the order of the split's wires.
fn legs<'a>(network: &'a Json, split: &str, merge: &str) -> Vec<&'a Json> {
    let edges = items(network, "edges");
    let into_merge: Vec<&str> = edges
        .iter()
        .filter(|edge| edge.str_at("toNodeId") == Some(merge))
        .filter_map(|edge| edge.str_at("fromNodeId"))
        .collect();
    edges
        .iter()
        .filter(|edge| edge.str_at("fromNodeId") == Some(split))
        .filter_map(|edge| edge.str_at("toNodeId"))
        .filter(|id| into_merge.contains(id))
        .filter_map(|id| node(network, id))
        .filter(|node| node.str_at("type") == Some("contact"))
        .collect()
}

/// The vertical span of a set of legs.
fn span(legs: &[&Json]) -> f64 {
    let ys: Vec<f64> = legs.iter().map(|leg| position(leg).1).collect();
    let max = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min = ys.iter().copied().fold(f64::INFINITY, f64::min);
    max - min
}

/// A copy of `contact` with a new id, no operand and the position (`x`, `y`).
fn empty_leg(program: &Json, contact: &Json, x: f64, y: f64) -> Json {
    let mut leg = contact.clone();
    leg.set("id", Json::String(fresh_node_id(program, "contact", &[])));
    leg.set("variable", Json::str(""));
    leg.set("position", point(x, y));
    leg
}

fn point(x: f64, y: f64) -> Json {
    Json::Object(vec![
        ("x".to_owned(), Json::Number(x)),
        ("y".to_owned(), Json::Number(y)),
    ])
}

/// A topology node of `kind` at (`x`, `y`).
fn topology_node(program: &Json, kind: &str, x: f64, y: f64) -> Json {
    Json::Object(vec![
        (
            "id".to_owned(),
            Json::String(fresh_node_id(program, kind, &[])),
        ),
        ("type".to_owned(), Json::str(kind)),
        ("position".to_owned(), point(x, y)),
    ])
}

/// Where a new branch goes: the contact on the grid, its split and its merge.
struct Placement {
    anchor_x: f64,
    anchor_y: f64,
    split_x: f64,
    merge_x: f64,
}

/// The placement of a branch around `anchor` between its neighbours `previous` and `next` (see
/// the module documentation).
fn placement(
    network: &Json,
    anchor: &Json,
    previous: Option<&Json>,
    next: Option<&Json>,
) -> Result<Placement, EditError> {
    let rung_y = network
        .get("layout")
        .and_then(|layout| layout.get("y"))
        .and_then(Json::as_f64)
        .unwrap_or(0.0);
    let (anchor_x, anchor_y) = position(anchor);
    let anchor_x = snap(anchor_x);
    // A contact at y 0 sits on the rung (JavaScript's `y || rungY`).
    let anchor_y = snap(if anchor_y == 0.0 { rung_y } else { anchor_y });
    let min_split = snap(LEFT_RAIL_X + RAIL_CLEARANCE);
    let max_merge = snap(RIGHT_RAIL_X - RAIL_CLEARANCE);
    let lower = previous.map_or(min_split, |node| {
        min_split.max(snap(position(node).0 + MIN_SPAN))
    });
    let upper = next.map_or(max_merge, |node| {
        max_merge.min(snap(position(node).0 - MIN_SPAN))
    });
    if upper - lower < MIN_SPAN {
        return Err(EditError::InsufficientHorizontalSpace);
    }
    let clamp = |value: f64, min: f64, max: f64| {
        if min > max {
            min
        } else {
            value.clamp(min, max)
        }
    };
    let split_x = clamp(snap(anchor_x - 100.0), lower, upper - MIN_SPAN);
    let merge_x = clamp(snap(anchor_x + 100.0), split_x + MIN_SPAN, upper);
    if split_x >= anchor_x || merge_x <= anchor_x {
        return Err(EditError::InsufficientHorizontalSpace);
    }
    if split_x < min_split || merge_x > max_merge || split_x >= merge_x {
        return Err(EditError::InvalidTopology);
    }
    Ok(Placement {
        anchor_x,
        anchor_y,
        split_x,
        merge_x,
    })
}

/// A new branch around the series contact `anchor` of `network`; the network keeps its other
/// elements, which are rewired as a series chain around the branch. Answers the new network and
/// the id of the new, empty leg.
fn new_branch(
    program: &Json,
    network: &Json,
    anchor_id: &str,
) -> Result<(Json, String), EditError> {
    let anchor = items(network, "nodes")
        .iter()
        .find(|node| node_id(node) == anchor_id && node.str_at("type") == Some("contact"))
        .ok_or(EditError::ContactNotFound)?;
    let has_topology = items(network, "nodes").iter().any(|node| {
        matches!(
            node.str_at("type"),
            Some("branchSplit" | "branchMerge" | "junction")
        )
    });
    if has_topology {
        return Err(EditError::UnsupportedTopology);
    }
    let sorted = sorted_nodes(network);
    let at = sorted
        .iter()
        .position(|node| node_id(node) == anchor_id)
        .ok_or(EditError::ContactNotFound)?;
    let previous = at.checked_sub(1).map(|index| sorted[index]);
    let next = sorted.get(at + 1).copied();
    let place = placement(network, anchor, previous, next)?;
    let (anchor_x, anchor_y, split_x, merge_x) =
        (place.anchor_x, place.anchor_y, place.split_x, place.merge_x);
    let split = topology_node(program, "branchSplit", split_x, anchor_y);
    let merge = topology_node(program, "branchMerge", merge_x, anchor_y);
    let mut upper_leg = anchor.clone();
    upper_leg.set("position", point(anchor_x, anchor_y));
    let lower_leg = empty_leg(program, anchor, anchor_x, snap(anchor_y + LEG_PITCH));
    let lower_id = node_id(&lower_leg).to_owned();

    let mut nodes: Vec<Json> = items(network, "nodes")
        .iter()
        .map(|node| {
            if node_id(node) == anchor_id {
                upper_leg.clone()
            } else {
                node.clone()
            }
        })
        .collect();
    nodes.extend([lower_leg.clone(), split.clone(), merge.clone()]);
    let mut branched = network.clone();
    branched.set("nodes", Json::Array(nodes));

    let mut wires: Vec<(Json, Json)> = Vec::new();
    for pair in sorted.windows(2) {
        if node_id(pair[0]) != anchor_id && node_id(pair[1]) != anchor_id {
            wires.push((pair[0].clone(), pair[1].clone()));
        }
    }
    if let Some(previous) = previous {
        wires.push((previous.clone(), split.clone()));
    }
    wires.push((split.clone(), upper_leg.clone()));
    wires.push((split.clone(), lower_leg.clone()));
    wires.push((upper_leg.clone(), merge.clone()));
    wires.push((lower_leg, merge.clone()));
    if let Some(next) = next {
        wires.push((merge, next.clone()));
    }
    let network_id = network.str_at("id").unwrap_or_default();
    let edges: Vec<Json> = wires
        .iter()
        .filter(|(from, to)| node_id(from) != node_id(to))
        .enumerate()
        .map(|(index, (from, to))| {
            edge(
                format!("edge_{network_id}_{index}"),
                from,
                to,
                route(from, to, true),
            )
        })
        .collect();
    branched.set("edges", Json::Array(edges));
    Ok((branched, lower_id))
}

/// One more empty leg below the lowest leg of the branch `contact` belongs to.
fn new_leg(program: &Json, network: &Json, contact_id: &str) -> Result<(Json, String), EditError> {
    let contact = items(network, "nodes")
        .iter()
        .find(|node| node_id(node) == contact_id && node.str_at("type") == Some("contact"))
        .ok_or(EditError::ContactNotFound)?;
    let (split_id, merge_id) = branch_of(network, contact_id).ok_or(EditError::InvalidTopology)?;
    let current = legs(network, &split_id, &merge_id);
    if current.len() < 2 || !current.iter().any(|leg| node_id(leg) == contact_id) {
        return Err(EditError::InvalidTopology);
    }
    let lowest = current
        .iter()
        .map(|leg| position(leg).1)
        .fold(f64::NEG_INFINITY, f64::max);
    let leg = empty_leg(
        program,
        contact,
        snap(position(contact).0),
        snap(lowest + LEG_PITCH),
    );
    let (Some(split), Some(merge)) = (
        node(network, &split_id).cloned(),
        node(network, &merge_id).cloned(),
    ) else {
        return Err(EditError::InvalidTopology);
    };
    let leg_id = node_id(&leg).to_owned();
    let count = items(network, "edges").len();
    let into_leg = fresh_edge_id(network, count, &[]);
    let out_of_leg = fresh_edge_id(network, count + 1, std::slice::from_ref(&into_leg));
    let mut grown = network.clone();
    let mut nodes = items(network, "nodes").to_vec();
    nodes.push(leg.clone());
    grown.set("nodes", Json::Array(nodes));
    let mut edges = items(network, "edges").to_vec();
    edges.push(edge(into_leg, &split, &leg, route(&split, &leg, true)));
    edges.push(edge(out_of_leg, &leg, &merge, route(&leg, &merge, true)));
    grown.set("edges", Json::Array(edges));
    Ok((grown, leg_id))
}

/// Moves a rung, its elements and its wire routes down by `delta`.
fn shift_down(network: &mut Json, delta: f64) {
    let shift = |value: &mut Json, key: &str| {
        if let Some(Json::Number(number)) = value.get_mut(key) {
            *number += delta;
        }
    };
    if let Some(layout) = network.get_mut("layout") {
        shift(layout, "y");
    }
    if let Some(Json::Array(nodes)) = network.get_mut("nodes") {
        for node in nodes {
            let (x, y) = position(node);
            node.set("position", point(x, y + delta));
        }
    }
    if let Some(Json::Array(edges)) = network.get_mut("edges") {
        for edge in edges {
            if let Some(Json::Array(points)) = edge.get_mut("points") {
                for point_value in points.iter_mut() {
                    let x = point_value.get("x").cloned().unwrap_or(Json::Null);
                    let y = point_value.get("y").and_then(Json::as_f64).unwrap_or(0.0);
                    *point_value = Json::Object(vec![
                        ("x".to_owned(), x),
                        ("y".to_owned(), Json::Number(y + delta)),
                    ]);
                }
            }
        }
    }
}

/// How far the rung below must move so it starts 40 below the lowest element of `network` plus a
/// margin of 40, on the grid; 0 when it already does or there is none.
fn shift_for(network: &Json, next_rung_y: Option<f64>) -> f64 {
    let Some(next_rung_y) = next_rung_y else {
        return 0.0;
    };
    let lowest = items(network, "nodes")
        .iter()
        .map(|node| position(node).1)
        .fold(f64::NEG_INFINITY, f64::max);
    let required = snap(lowest + CONTENT_MARGIN + RUNG_CLEARANCE);
    if next_rung_y >= required {
        0.0
    } else {
        snap(required - next_rung_y)
    }
}

/// Adds a parallel contact to the contact `contact` of the rung `rung` (see the module
/// documentation) and moves the rungs below when the branch reaches them.
///
/// # Errors
/// The rung or contact does not exist, or the branch does not fit; the program is unchanged.
pub fn add_parallel_contact(
    program: &mut Json,
    rung: usize,
    contact: &str,
) -> Result<Leg, EditError> {
    let network = items(program, "networks")
        .get(rung)
        .ok_or(EditError::RungNotFound)?;
    let selected = items(network, "nodes")
        .iter()
        .find(|node| node_id(node) == contact);
    if selected.and_then(|node| node.str_at("type")) != Some("contact") {
        return Err(EditError::ContactNotFound);
    }
    let (grown, leg_id) = if branch_of(network, contact).is_some() {
        new_leg(program, network, contact)?
    } else {
        let (branched, leg_id) =
            new_branch(program, network, contact).map_err(|error| match error {
                EditError::UnsupportedTopology | EditError::InsufficientHorizontalSpace => error,
                _ => EditError::InvalidTopology,
            })?;
        if branch_of(&branched, &leg_id).is_none() {
            return Err(EditError::InvalidTopology);
        }
        (branched, leg_id)
    };
    let (split, merge) = branch_of(&grown, &leg_id).ok_or(EditError::InvalidTopology)?;
    let all_legs = legs(&grown, &split, &merge);
    let total = all_legs.len();
    let vertical_span = span(&all_legs);
    let next_rung_y = items(program, "networks")
        .get(rung + 1)
        .and_then(|next| next.get("layout"))
        .and_then(|layout| layout.get("y"))
        .and_then(Json::as_f64);
    let delta = shift_for(&grown, next_rung_y);
    if let Some(slot) = network_mut(program, rung) {
        *slot = grown;
    }
    if delta > 0.0
        && let Some(Json::Array(networks)) = program.get_mut("networks")
    {
        for network in networks.iter_mut().skip(rung + 1) {
            shift_down(network, delta);
        }
    }
    Ok(Leg {
        selected_node_id: leg_id,
        rung_shift_px: delta,
        added_vertical_span_px: vertical_span,
        total_parallel_legs: total,
    })
}
