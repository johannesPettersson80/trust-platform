//! Power flow through one Ladder network (specification 15 §4.2, specification 16 §4).
//!
//! The power reaching each node is a Boolean expression: the left rail is `TRUE`, a contact passes
//! the power reaching it AND its operand (NOT its operand for NC), every other node passes the
//! power reaching it, and a node reached from several nodes gets their OR. Nodes are ordered
//! deterministically by position and then id; a network without edges is a series chain in that
//! order, and in a network with edges every node without an incoming edge hangs on the left rail.

use std::cmp::Ordering;

use crate::js;
use crate::Json;

/// The id the left rail has in the topology.
const LEFT_RAIL: &str = "__LEFT_RAIL__";

/// The deterministic node order: by `position.x`, then `position.y`, then id
/// (specification 15 §4.3).
pub(crate) fn compare_nodes(left: &Json, right: &Json) -> Ordering {
    let coordinate = |node: &Json, axis: &str| {
        node.get("position")
            .and_then(|position| position.get(axis))
            .and_then(Json::as_f64)
            .unwrap_or(0.0)
    };
    // Positions are finite; `-0` and `0` compare equal, as their difference is zero in JavaScript.
    let by = |axis: &str| {
        coordinate(left, axis)
            .partial_cmp(&coordinate(right, axis))
            .unwrap_or(Ordering::Equal)
    };
    by("x")
        .then_with(|| by("y"))
        .then_with(|| js::locale_compare(node_id(left), node_id(right)))
}

/// The nodes of a network in the deterministic order.
pub(crate) fn sorted_nodes(network: &Json) -> Vec<&Json> {
    let mut nodes: Vec<&Json> = network
        .get("nodes")
        .and_then(Json::as_array)
        .map(|nodes| nodes.iter().collect())
        .unwrap_or_default();
    nodes.sort_by(|left, right| compare_nodes(left, right));
    nodes
}

/// The id of a node.
pub(crate) fn node_id(node: &Json) -> &str {
    node.str_at("id").unwrap_or_default()
}

/// The incoming and outgoing links of each node, in the order the links were added.
struct Topology<'a> {
    nodes: Vec<&'a Json>,
    incoming: Vec<(String, Vec<String>)>,
    outgoing: Vec<(String, Vec<String>)>,
}

impl Topology<'_> {
    fn incoming(&self, id: &str) -> &[String] {
        self.incoming
            .iter()
            .find(|(node, _)| node == id)
            .map_or(&[], |(_, sources)| sources.as_slice())
    }

    fn outgoing(&self, id: &str) -> &[String] {
        self.outgoing
            .iter()
            .find(|(node, _)| node == id)
            .map_or(&[], |(_, targets)| targets.as_slice())
    }

    /// Adds the link `from → to` once; a link to a node that is not in the network is ignored.
    fn link(&mut self, from: &str, to: &str) {
        let Some(sources) = self
            .incoming
            .iter_mut()
            .find(|(node, _)| node == to)
            .map(|(_, sources)| sources)
        else {
            return;
        };
        if !sources.iter().any(|source| source == from) {
            sources.push(from.to_owned());
        }
        match self.outgoing.iter_mut().find(|(node, _)| node == from) {
            Some((_, targets)) => {
                if !targets.iter().any(|target| target == to) {
                    targets.push(to.to_owned());
                }
            }
            None => self.outgoing.push((from.to_owned(), vec![to.to_owned()])),
        }
    }
}

fn topology(network: &Json) -> Topology<'_> {
    let nodes = sorted_nodes(network);
    let mut topology = Topology {
        incoming: nodes
            .iter()
            .map(|node| (node_id(node).to_owned(), Vec::new()))
            .collect(),
        outgoing: nodes
            .iter()
            .map(|node| (node_id(node).to_owned(), Vec::new()))
            .collect(),
        nodes,
    };
    topology.outgoing.push((LEFT_RAIL.to_owned(), Vec::new()));
    let edges = network
        .get("edges")
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if edges.is_empty() {
        let ids: Vec<String> = topology
            .nodes
            .iter()
            .map(|node| node_id(node).to_owned())
            .collect();
        if let Some(first) = ids.first() {
            topology.link(LEFT_RAIL, first);
        }
        for pair in ids.windows(2) {
            topology.link(&pair[0], &pair[1]);
        }
    } else {
        for edge in edges {
            let from = edge.str_at("fromNodeId").unwrap_or_default();
            let to = edge.str_at("toNodeId").unwrap_or_default();
            topology.link(from, to);
        }
        let unfed: Vec<String> = topology
            .nodes
            .iter()
            .map(|node| node_id(node).to_owned())
            .filter(|id| topology.incoming(id).is_empty())
            .collect();
        for id in unfed {
            topology.link(LEFT_RAIL, &id);
        }
    }
    topology
}

/// The evaluation order: Kahn's order over the links (ties broken by the node order), then every
/// node a cycle or an unknown source kept back, in the node order.
fn evaluation_order<'a>(topology: &Topology<'a>) -> Vec<&'a Json> {
    let mut indegree: Vec<(String, usize)> = topology
        .nodes
        .iter()
        .map(|node| (node_id(node).to_owned(), 0))
        .collect();
    for (from, targets) in &topology.outgoing {
        if from == LEFT_RAIL {
            continue;
        }
        for target in targets {
            if let Some((_, count)) = indegree.iter_mut().find(|(id, _)| id == target) {
                *count += 1;
            }
        }
    }
    let degree = |indegree: &[(String, usize)], id: &str| {
        indegree
            .iter()
            .find(|(node, _)| node == id)
            .map_or(0, |(_, count)| *count)
    };
    let mut queue: Vec<&Json> = topology
        .nodes
        .iter()
        .copied()
        .filter(|node| degree(&indegree, node_id(node)) == 0)
        .collect();
    queue.sort_by(|left, right| compare_nodes(left, right));
    let mut order: Vec<&Json> = Vec::new();
    while !queue.is_empty() {
        let next = queue.remove(0);
        order.push(next);
        for target in topology.outgoing(node_id(next)) {
            let Some((_, count)) = indegree.iter_mut().find(|(id, _)| id == target) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0
                && let Some(node) = topology.nodes.iter().find(|node| node_id(node) == target)
            {
                queue.push(node);
                queue.sort_by(|left, right| compare_nodes(left, right));
            }
        }
    }
    for node in &topology.nodes {
        if !order.iter().any(|done| node_id(done) == node_id(node)) {
            order.push(node);
        }
    }
    order
}

/// The OR of power terms: blank terms are dropped, `TRUE` absorbs every term, `FALSE` terms are
/// dropped, repeated terms are kept once; no term is `FALSE`.
fn join_or(terms: &[String]) -> String {
    let mut kept: Vec<&str> = Vec::new();
    for term in terms
        .iter()
        .map(|term| js::trim(term))
        .filter(|term| !term.is_empty())
    {
        if term == "TRUE" {
            return "TRUE".to_owned();
        }
        if term != "FALSE" && !kept.contains(&term) {
            kept.push(term);
        }
    }
    if kept.is_empty() {
        "FALSE".to_owned()
    } else {
        kept.join(" OR ")
    }
}

/// Whether `term` holds the word `OR`, so it needs parentheses inside an AND.
fn holds_or(term: &str) -> bool {
    let bytes = term.as_bytes();
    let word = |at: usize| {
        bytes
            .get(at)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    };
    term.match_indices("OR")
        .any(|(at, _)| (at == 0 || !word(at - 1)) && !word(at + 2))
}

/// The AND of two power terms, with `FALSE` absorbing and `TRUE` neutral.
pub(crate) fn join_and(left: &str, right: &str) -> String {
    if left == "FALSE" || right == "FALSE" {
        return "FALSE".to_owned();
    }
    if left == "TRUE" {
        return right.to_owned();
    }
    if right == "TRUE" {
        return left.to_owned();
    }
    let wrap = |term: &str| {
        if holds_or(term) {
            format!("({term})")
        } else {
            term.to_owned()
        }
    };
    format!("{} AND {}", wrap(left), wrap(right))
}

/// The power reaching each node of `network`, by node id. `contact_term` gives a contact's own
/// term (its operand, or `NOT (operand)` for NC).
pub(crate) fn incoming_power(
    network: &Json,
    contact_term: &dyn Fn(&Json) -> String,
) -> Vec<(String, String)> {
    let topology = topology(network);
    let order = evaluation_order(&topology);
    let mut passed: Vec<(String, String)> = Vec::new();
    let mut reaching: Vec<(String, String)> = Vec::new();
    for node in order {
        let id = node_id(node);
        let sources = topology.incoming(id);
        let terms: Vec<String> = if sources.is_empty() {
            vec!["FALSE".to_owned()]
        } else {
            sources
                .iter()
                .map(|source| {
                    if source == LEFT_RAIL {
                        "TRUE".to_owned()
                    } else {
                        passed
                            .iter()
                            .find(|(node, _)| node == source)
                            .map_or_else(|| "FALSE".to_owned(), |(_, term)| term.clone())
                    }
                })
                .collect()
        };
        let power = join_or(&terms);
        let out = if node.str_at("type") == Some("contact") {
            join_and(&power, &contact_term(node))
        } else {
            power.clone()
        };
        reaching.push((id.to_owned(), power));
        passed.push((id.to_owned(), out));
    }
    reaching
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(text: &str) -> Json {
        Json::parse(text).expect("network")
    }

    fn contact_operand(node: &Json) -> String {
        let operand = node.str_at("variable").unwrap_or_default().to_owned();
        if node.str_at("contactType") == Some("NC") {
            format!("NOT ({operand})")
        } else {
            operand
        }
    }

    fn power_of(network: &Json, id: &str) -> String {
        incoming_power(network, &contact_operand)
            .into_iter()
            .find(|(node, _)| node == id)
            .map(|(_, power)| power)
            .expect("node power")
    }

    #[test]
    fn a_network_without_edges_is_a_series_chain_by_position() {
        // Specification 15 §4.2: series contacts AND; the chain follows x even when the array
        // order differs. Catches a chain in array order (`B AND A`).
        let rung = network(
            r#"{"nodes":[
              {"id":"q","type":"coil","position":{"x":300,"y":100}},
              {"id":"b","type":"contact","contactType":"NC","variable":"B","position":{"x":200,"y":100}},
              {"id":"a","type":"contact","contactType":"NO","variable":"A","position":{"x":100,"y":100}}
            ],"edges":[]}"#,
        );
        assert_eq!(power_of(&rung, "q"), "A AND NOT (B)");
        assert_eq!(power_of(&rung, "a"), "TRUE");
    }

    #[test]
    fn parallel_legs_are_or_and_an_or_is_parenthesised_inside_and() {
        // Specification 15 §4.2: parallel paths are alternatives. Worked by hand: split → (A | B)
        // → merge → C → coil gives `(A OR B) AND C`. Catches a missing parenthesis, which ST
        // would read as `A OR (B AND C)`.
        let rung = network(
            r#"{"nodes":[
              {"id":"s","type":"branchSplit","position":{"x":80,"y":100}},
              {"id":"a","type":"contact","contactType":"NO","variable":"A","position":{"x":120,"y":100}},
              {"id":"b","type":"contact","contactType":"NO","variable":"B","position":{"x":120,"y":160}},
              {"id":"m","type":"branchMerge","position":{"x":200,"y":100}},
              {"id":"c","type":"contact","contactType":"NO","variable":"C","position":{"x":240,"y":100}},
              {"id":"q","type":"coil","position":{"x":320,"y":100}}
            ],"edges":[
              {"id":"e1","fromNodeId":"s","toNodeId":"a"},{"id":"e2","fromNodeId":"s","toNodeId":"b"},
              {"id":"e3","fromNodeId":"a","toNodeId":"m"},{"id":"e4","fromNodeId":"b","toNodeId":"m"},
              {"id":"e5","fromNodeId":"m","toNodeId":"c"},{"id":"e6","fromNodeId":"c","toNodeId":"q"}
            ]}"#,
        );
        assert_eq!(power_of(&rung, "m"), "A OR B");
        assert_eq!(power_of(&rung, "q"), "(A OR B) AND C");
    }

    #[test]
    fn a_node_fed_only_by_an_unknown_node_gets_no_power() {
        // An edge from a node that is not in the network carries no power. Catches a port that
        // hangs such a node on the left rail (it has an incoming link, so it is not unfed).
        let rung = network(
            r#"{"nodes":[{"id":"q","type":"coil","position":{"x":300,"y":100}}],
                "edges":[{"id":"e","fromNodeId":"ghost","toNodeId":"q"}]}"#,
        );
        assert_eq!(power_of(&rung, "q"), "FALSE");
    }

    #[test]
    fn or_terms_absorb_true_drop_false_and_repeat_once() {
        let terms = |list: &[&str]| {
            list.iter()
                .map(|term| (*term).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(join_or(&terms(&["A", "TRUE"])), "TRUE");
        assert_eq!(join_or(&terms(&["FALSE", " "])), "FALSE");
        assert_eq!(join_or(&terms(&["A", "B", "A"])), "A OR B");
        assert_eq!(join_and("MOTOR", "B"), "MOTOR AND B");
        assert_eq!(join_and("A OR B", "TRUE"), "A OR B");
        assert_eq!(join_and("FALSE", "A"), "FALSE");
    }
}
