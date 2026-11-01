//! Removing an element and a rung's wires, and the text and choice fields of the Ladder
//! properties panel (specification 16 §3 to §5, specification 25 §11.4).
//!
//! Deleting an element takes its term out of the rung's logic, as CODESYS and `TwinCAT` do: its
//! wires go with it, and each node that fed it is wired to each node it fed unless another path
//! still joins the two. So a series element closes the gap, and an element that was a parallel
//! leg on its own leaves the other legs. A wire left to an element that is gone would still block
//! the power of the node it ends at, and the node's input would no longer be what is drawn.

use super::declarations::OPERAND_MEMBERS;
use super::geometry::route;
use super::{edge, fresh_edge_id, items, network_mut, no_rung, reconcile_declarations};
use crate::ladder::schema::{
    COIL_TYPES, COMPARE_OPS, CONTACT_TYPES, COUNTER_TYPES, MATH_OPS, TIMER_TYPES,
};
use crate::Json;

/// The values a choice member takes; `None` for a text member.
type Choices = Option<&'static [&'static str]>;

/// The members of one element kind, by name.
type KindFields = (&'static str, &'static [(&'static str, Choices)]);

/// The text and choice members of each element kind (specification 16 §3), a choice member with
/// the values it takes. The numbers (`presetMs`, `preset`, the position) are
/// [`super::set_number`]'s.
const FIELDS: [KindFields; 6] = [
    (
        "contact",
        &[("contactType", Some(&CONTACT_TYPES)), ("variable", None)],
    ),
    (
        "coil",
        &[("coilType", Some(&COIL_TYPES)), ("variable", None)],
    ),
    (
        "timer",
        &[
            ("timerType", Some(&TIMER_TYPES)),
            ("instance", None),
            ("input", None),
            ("qOutput", None),
            ("etOutput", None),
        ],
    ),
    (
        "counter",
        &[
            ("counterType", Some(&COUNTER_TYPES)),
            ("instance", None),
            ("input", None),
            ("qOutput", None),
            ("cvOutput", None),
        ],
    ),
    (
        "compare",
        &[("op", Some(&COMPARE_OPS)), ("left", None), ("right", None)],
    ),
    (
        "math",
        &[
            ("op", Some(&MATH_OPS)),
            ("left", None),
            ("right", None),
            ("output", None),
        ],
    ),
];

fn no_element(rung: usize, element: usize) -> String {
    format!("Ladder rung {rung} has no element {element}.")
}

/// Removes the element `element` of the rung `rung` with its wires, and wires each node that fed
/// it to each node it fed that the first no longer reaches through another path. The new wires
/// get the smallest free edge ids and an orthogonal route. Implicit locals that no element names
/// any more are removed (specification 16 §5).
///
/// # Errors
/// There is no such rung or element. Nothing changes then.
pub fn remove_element(program: &mut Json, rung: usize, element: usize) -> Result<(), String> {
    let network = network_mut(program, rung).ok_or_else(|| no_rung(rung))?;
    let mut nodes = items(network, "nodes").to_vec();
    if element >= nodes.len() {
        return Err(no_element(rung, element));
    }
    let removed = nodes.remove(element);
    let id = removed.str_at("id").unwrap_or_default();
    let ends = |end: &str, other: &str| -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for wire in items(network, "edges") {
            if wire.str_at(end) == Some(id)
                && let Some(node) = wire.str_at(other)
                && node != id
                && !found.iter().any(|known| known == node)
            {
                found.push(node.to_owned());
            }
        }
        found
    };
    let fed_by = ends("toNodeId", "fromNodeId");
    let feeds = ends("fromNodeId", "toNodeId");
    let mut wires: Vec<Json> = items(network, "edges")
        .iter()
        .filter(|wire| wire.str_at("fromNodeId") != Some(id) && wire.str_at("toNodeId") != Some(id))
        .cloned()
        .collect();
    let find = |node_id: &str| nodes.iter().find(|node| node.str_at("id") == Some(node_id));
    let mut bridges = Vec::new();
    for from in &fed_by {
        for to in &feeds {
            if from != to
                && !reaches(&wires, from, to)
                && let (Some(from_node), Some(to_node)) = (find(from), find(to))
            {
                bridges.push((from_node.clone(), to_node.clone()));
            }
        }
    }
    let mut kept = network.clone();
    kept.set("nodes", Json::Array(nodes));
    kept.set("edges", Json::Array(wires.clone()));
    for (from_node, to_node) in bridges {
        let id = fresh_edge_id(&kept, wires.len(), &[]);
        wires.push(edge(
            id,
            &from_node,
            &to_node,
            route(&from_node, &to_node, false),
        ));
        kept.set("edges", Json::Array(wires.clone()));
    }
    *network = kept;
    reconcile_declarations(program);
    Ok(())
}

/// Whether a path of `wires` leads from the node `from` to the node `to`.
fn reaches(wires: &[Json], from: &str, to: &str) -> bool {
    let mut seen = vec![from.to_owned()];
    let mut next = vec![from.to_owned()];
    while let Some(node) = next.pop() {
        for wire in wires {
            if wire.str_at("fromNodeId") == Some(node.as_str())
                && let Some(target) = wire.str_at("toNodeId")
            {
                if target == to {
                    return true;
                }
                if !seen.iter().any(|known| known == target) {
                    seen.push(target.to_owned());
                    next.push(target.to_owned());
                }
            }
        }
    }
    false
}

/// Removes every wire of the rung `rung`; the rung then runs as one series chain of its elements
/// from left to right (specification 16 §4).
///
/// # Errors
/// There is no such rung.
pub fn clear_wiring(program: &mut Json, rung: usize) -> Result<(), String> {
    let network = network_mut(program, rung).ok_or_else(|| no_rung(rung))?;
    network.set("edges", Json::Array(Vec::new()));
    Ok(())
}

/// Stores `value` in the text or choice member `field` of the element `element` of the rung
/// `rung` (specification 16 §3): a choice member takes one of its values, a text member any
/// text. A changed operand reconciles the implicit locals (specification 16 §5). The element keeps
/// its other members and their order.
///
/// # Errors
/// There is no such rung or element, its kind has no member `field`, or `value` is not one of the
/// member's choices. Nothing is stored then.
pub fn set_field(
    program: &mut Json,
    rung: usize,
    element: usize,
    field: &str,
    value: &str,
) -> Result<(), String> {
    let network = network_mut(program, rung).ok_or_else(|| no_rung(rung))?;
    let node = network
        .get_mut("nodes")
        .and_then(Json::as_array_mut)
        .and_then(|nodes| nodes.get_mut(element))
        .ok_or_else(|| no_element(rung, element))?;
    let kind = node.str_at("type").unwrap_or_default().to_owned();
    let choices = FIELDS
        .iter()
        .find(|(fields_kind, _)| *fields_kind == kind)
        .and_then(|(_, fields)| fields.iter().find(|(name, _)| *name == field))
        .map(|(_, choices)| *choices)
        .ok_or_else(|| format!("A {kind} has no field \"{field}\"."))?;
    if let Some(choices) = choices
        && !choices.contains(&value)
    {
        return Err(format!("{field} must be one of {}.", choices.join(", ")));
    }
    node.set(field, Json::str(value));
    if OPERAND_MEMBERS.contains(&field) {
        reconcile_declarations(program);
    }
    Ok(())
}

/// Turns the contact `element` of the rung `rung` from NO to NC or back, and answers its new
/// type.
///
/// # Errors
/// There is no such rung or element, or the element is not a contact.
pub fn toggle_contact_type(
    program: &mut Json,
    rung: usize,
    element: usize,
) -> Result<&'static str, String> {
    let network = network_mut(program, rung).ok_or_else(|| no_rung(rung))?;
    let node = items(network, "nodes")
        .get(element)
        .ok_or_else(|| no_element(rung, element))?;
    if node.str_at("type") != Some("contact") {
        return Err(format!(
            "Ladder rung {rung} element {element} is not a contact."
        ));
    }
    let next = if node.str_at("contactType") == Some("NC") {
        "NO"
    } else {
        "NC"
    };
    set_field(program, rung, element, "contactType", next)?;
    Ok(next)
}
