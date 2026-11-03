//! The edits the Ladder editor stores in a program (specification 16, specification 25 §11.4).
//!
//! The editor draws the program and handles the pointer (drag, snap, the wire being drawn); every
//! change it stores goes through these edits. Each edit changes the program in place and keeps
//! the member order of the file. New elements, rungs and wires get the smallest free numbered id
//! of their kind (`contact_1`, `rung_2`, `edge_rung_2_0`), so an edit is deterministic. The
//! fields of the properties panel are stored here too ([`set_number`], [`set_field`]), and
//! deleting an element takes its term out of the rung's logic ([`remove_element`]).

mod branch;
mod declarations;
mod elements;
mod geometry;
mod numbers;

pub use branch::{add_parallel_contact, EditError, Leg};
pub use declarations::reconcile_declarations;
pub use elements::{clear_wiring, remove_element, set_field, toggle_contact_type};
pub use numbers::{set_number, NumberField};

use crate::js;
use crate::Json;
use geometry::{route, snap, RUNG_HEIGHT};

/// The members holding a symbol or address, per element kind, for find and replace.
const SYMBOL_MEMBERS: [(&str, &[&str]); 6] = [
    ("contact", &["variable"]),
    ("coil", &["variable"]),
    ("timer", &["instance", "input", "qOutput", "etOutput"]),
    ("counter", &["instance", "input", "qOutput", "cvOutput"]),
    ("compare", &["left", "right"]),
    ("math", &["left", "right", "output"]),
];

/// The networks of a program, to change.
fn networks_mut(program: &mut Json) -> Option<&mut Vec<Json>> {
    program.get_mut("networks").and_then(Json::as_array_mut)
}

/// The network at `index`, to change.
fn network_mut(program: &mut Json, index: usize) -> Option<&mut Json> {
    networks_mut(program)?.get_mut(index)
}

/// The items of the array member `key`.
fn items<'a>(value: &'a Json, key: &str) -> &'a [Json] {
    value
        .get(key)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// Whether any node of the program has the id `id`.
fn node_id_taken(program: &Json, id: &str) -> bool {
    items(program, "networks")
        .iter()
        .flat_map(|network| items(network, "nodes"))
        .any(|node| node.str_at("id") == Some(id))
}

/// The first name `name(n)`, n counting up from `start`, that `taken` does not refuse. A program
/// has finitely many names, so one is always free.
fn first_free(
    start: usize,
    name: impl Fn(usize) -> String,
    taken: impl Fn(&str) -> bool,
) -> String {
    let mut n = start;
    loop {
        let candidate = name(n);
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// The smallest free `<prefix>_<n>` (n from 1) no node of the program and no id in `taken` has.
fn fresh_node_id(program: &Json, prefix: &str, taken: &[String]) -> String {
    first_free(
        1,
        |n| format!("{prefix}_{n}"),
        |id| node_id_taken(program, id) || taken.iter().any(|known| known == id),
    )
}

/// The smallest free `edge_<network id>_<n>`, n from `start`, in `network`.
fn fresh_edge_id(network: &Json, start: usize, taken: &[String]) -> String {
    let network_id = network.str_at("id").unwrap_or_default();
    first_free(
        start,
        |n| format!("edge_{network_id}_{n}"),
        |id| {
            taken.iter().any(|known| known == id)
                || items(network, "edges")
                    .iter()
                    .any(|edge| edge.str_at("id") == Some(id))
        },
    )
}

/// A rung position as a number of the file.
fn position_number(index: usize) -> f64 {
    f64::from(u32::try_from(index).unwrap_or(u32::MAX))
}

/// Gives every rung its position as its `order` and puts it on the rung grid
/// (`layout.y` = 100 × position + 100).
fn renumber_rungs(program: &mut Json) {
    let Some(networks) = networks_mut(program) else {
        return;
    };
    for (index, network) in networks.iter_mut().enumerate() {
        let position = position_number(index);
        network.set("order", Json::Number(position));
        let mut layout = network.get("layout").cloned().unwrap_or_else(Json::object);
        layout.set("y", Json::Number(position * RUNG_HEIGHT + RUNG_HEIGHT));
        network.set("layout", layout);
    }
}

/// The wires of a network routed as one series chain in the node order: an edge from each node to
/// the next, `edge_<network id>_<index>`, with an orthogonal route through the grid point between
/// them. A network with fewer than two nodes gets no wires.
#[must_use]
pub fn routed_network(network: &Json) -> Json {
    let nodes = super::power::sorted_nodes(network);
    let network_id = network.str_at("id").unwrap_or_default();
    let edges: Vec<Json> = nodes
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            edge(
                format!("edge_{network_id}_{index}"),
                pair[0],
                pair[1],
                route(pair[0], pair[1], false),
            )
        })
        .collect();
    let mut routed = network.clone();
    routed.set("edges", Json::Array(edges));
    routed
}

/// An edge object: id, ends and route.
fn edge(id: String, from: &Json, to: &Json, points: Json) -> Json {
    Json::Object(vec![
        ("id".to_owned(), Json::String(id)),
        (
            "fromNodeId".to_owned(),
            Json::str(from.str_at("id").unwrap_or_default()),
        ),
        (
            "toNodeId".to_owned(),
            Json::str(to.str_at("id").unwrap_or_default()),
        ),
        ("points".to_owned(), points),
    ])
}

/// Routes the wires of the rung `rung`, or of every rung ([`routed_network`]).
pub fn auto_route(program: &mut Json, rung: Option<usize>) {
    let Some(networks) = networks_mut(program) else {
        return;
    };
    for (index, network) in networks.iter_mut().enumerate() {
        if rung.is_none_or(|only| only == index) {
            *network = routed_network(network);
        }
    }
}

/// Replaces every occurrence of `find` by `replace` in the variable names and addresses and in
/// the symbol members of every element (find and replace), and answers how many were replaced.
/// An empty `find` replaces nothing.
pub fn replace_symbol(program: &mut Json, find: &str, replace: &str) -> usize {
    if find.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut replace_in = |value: &mut Json| {
        if let Json::String(text) = value
            && !text.is_empty()
        {
            count += text.matches(find).count();
            *text = text.replace(find, replace);
        }
    };
    if let Some(variables) = program.get_mut("variables").and_then(Json::as_array_mut) {
        for variable in variables {
            for member in ["name", "address"] {
                if let Some(value) = variable.get_mut(member) {
                    replace_in(value);
                }
            }
        }
    }
    if let Some(networks) = networks_mut(program) {
        for network in networks {
            let Some(nodes) = network.get_mut("nodes").and_then(Json::as_array_mut) else {
                continue;
            };
            for node in nodes {
                let kind = node.str_at("type").unwrap_or_default().to_owned();
                let members = SYMBOL_MEMBERS
                    .iter()
                    .find(|(candidate, _)| *candidate == kind)
                    .map_or(&[][..], |(_, members)| members);
                for member in members {
                    if let Some(value) = node.get_mut(member) {
                        replace_in(value);
                    }
                }
            }
        }
    }
    count
}

/// Pastes a copy of `element` into the rung `rung`: a new id of its kind, `x` moved 40 to the
/// right and snapped to the grid, `y` on the rung. Answers the index of the new node.
///
/// # Errors
/// There is no such rung or `element` is not a Ladder element.
pub fn paste_element(program: &mut Json, rung: usize, element: &Json) -> Result<usize, String> {
    let kind = element
        .str_at("type")
        .filter(|kind| super::ELEMENT_TYPES.contains(kind))
        .ok_or("The pasted element is not a Ladder element.")?
        .to_owned();
    let id = fresh_node_id(program, &kind, &[]);
    let network = network_mut(program, rung).ok_or_else(|| no_rung(rung))?;
    let rung_y = network
        .get("layout")
        .and_then(|layout| layout.get("y"))
        .and_then(Json::as_f64)
        .unwrap_or(0.0);
    let x = element
        .get("position")
        .and_then(|position| position.get("x"))
        .and_then(Json::as_f64)
        .unwrap_or(0.0);
    let mut copy = element.clone();
    copy.set("id", Json::String(id));
    copy.set(
        "position",
        Json::Object(vec![
            ("x".to_owned(), Json::Number(snap(x + 40.0))),
            ("y".to_owned(), Json::Number(rung_y)),
        ]),
    );
    let nodes = nodes_mut(network);
    nodes.push(copy);
    Ok(nodes.len() - 1)
}

/// The nodes of a network, to change (created when missing).
fn nodes_mut(network: &mut Json) -> &mut Vec<Json> {
    if network.get("nodes").and_then(Json::as_array).is_none() {
        network.set("nodes", Json::Array(Vec::new()));
    }
    match network.get_mut("nodes") {
        Some(Json::Array(nodes)) => nodes,
        _ => unreachable!("the nodes member was just made an array"),
    }
}

fn no_rung(rung: usize) -> String {
    format!("Ladder rung {rung} does not exist.")
}

/// Pastes a copy of the rung `source` after the rung `after` (before the first rung when `after`
/// is negative): a new rung id, new node ids, nodes moved 100 down, no wires; then every rung is
/// renumbered (order and row from its position). Answers the index of the new rung.
pub fn paste_rung(program: &mut Json, source: &Json, after: i64) -> usize {
    let count = items(program, "networks").len();
    let index = usize::try_from(after.saturating_add(1).max(0))
        .unwrap_or(usize::MAX)
        .min(count);
    let rung_id = fresh_rung_id(program);
    let mut taken: Vec<String> = Vec::new();
    let mut nodes: Vec<Json> = Vec::new();
    for node in items(source, "nodes") {
        let id = fresh_node_id(program, node.str_at("type").unwrap_or("element"), &taken);
        taken.push(id.clone());
        let position = node.get("position");
        let coordinate = |axis: &str| {
            position
                .and_then(|position| position.get(axis))
                .and_then(Json::as_f64)
                .unwrap_or(0.0)
        };
        let mut copy = node.clone();
        copy.set("id", Json::String(id));
        copy.set(
            "position",
            Json::Object(vec![
                ("x".to_owned(), Json::Number(coordinate("x"))),
                ("y".to_owned(), Json::Number(coordinate("y") + 100.0)),
            ]),
        );
        nodes.push(copy);
    }
    let mut rung = source.clone();
    rung.set("id", Json::String(rung_id));
    rung.set("nodes", Json::Array(nodes));
    rung.set("edges", Json::Array(Vec::new()));
    if let Some(networks) = networks_mut(program) {
        networks.insert(index, rung);
    } else {
        program.set("networks", Json::Array(vec![rung]));
    }
    renumber_rungs(program);
    index
}

/// The smallest free `rung_<n>`.
fn fresh_rung_id(program: &Json) -> String {
    first_free(
        1,
        |n| format!("rung_{n}"),
        |id| {
            items(program, "networks")
                .iter()
                .any(|network| network.str_at("id") == Some(id))
        },
    )
}

/// Adds an empty rung at `index` (clamped to the rung count) and renumbers every rung. Answers
/// the index of the new rung.
pub fn add_rung(program: &mut Json, index: usize) -> usize {
    let count = items(program, "networks").len();
    let index = index.min(count);
    let position = position_number(index);
    let rung = Json::Object(vec![
        ("id".to_owned(), Json::String(fresh_rung_id(program))),
        ("order".to_owned(), Json::Number(position)),
        ("nodes".to_owned(), Json::Array(Vec::new())),
        ("edges".to_owned(), Json::Array(Vec::new())),
        (
            "layout".to_owned(),
            Json::Object(vec![(
                "y".to_owned(),
                Json::Number(position * RUNG_HEIGHT + RUNG_HEIGHT),
            )]),
        ),
    ]);
    if let Some(networks) = networks_mut(program) {
        networks.insert(index, rung);
    } else {
        program.set("networks", Json::Array(vec![rung]));
    }
    renumber_rungs(program);
    index
}

/// Removes the rung at `index` and renumbers the others.
///
/// # Errors
/// There is no rung at `index`.
pub fn remove_rung(program: &mut Json, index: usize) -> Result<(), String> {
    let networks = networks_mut(program).ok_or_else(|| no_rung(index))?;
    if index >= networks.len() {
        return Err(no_rung(index));
    }
    networks.remove(index);
    renumber_rungs(program);
    Ok(())
}

/// What a wire request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// The wire was added.
    Added,
    /// The two nodes were already wired in that direction; nothing changed.
    AlreadyConnected,
    /// The rung or a node does not exist, or both ends are the same node; nothing changed.
    Invalid,
}

impl Wire {
    /// The name of the outcome in the command answer.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::AlreadyConnected => "already-connected",
            Self::Invalid => "invalid",
        }
    }
}

/// Wires the node `from` to the node `to` in the rung `rung`. A rung without wires is first routed
/// as a series chain ([`routed_network`]), so drawing the first wire keeps the chain the rung had.
pub fn connect(program: &mut Json, rung: usize, from: &str, to: &str) -> Wire {
    let Some(network) = network_mut(program, rung) else {
        return Wire::Invalid;
    };
    if from == to {
        return Wire::Invalid;
    }
    let mut base = if items(network, "edges").is_empty() {
        routed_network(network)
    } else {
        network.clone()
    };
    let find = |id: &str| {
        items(&base, "nodes")
            .iter()
            .find(|node| node.str_at("id") == Some(id))
            .cloned()
    };
    let (Some(from_node), Some(to_node)) = (find(from), find(to)) else {
        return Wire::Invalid;
    };
    let exists = items(&base, "edges")
        .iter()
        .any(|edge| edge.str_at("fromNodeId") == Some(from) && edge.str_at("toNodeId") == Some(to));
    if exists {
        return Wire::AlreadyConnected;
    }
    let id = fresh_edge_id(&base, items(&base, "edges").len(), &[]);
    let wire = edge(id, &from_node, &to_node, route(&from_node, &to_node, false));
    if let Some(Json::Array(edges)) = base.get_mut("edges") {
        edges.push(wire);
    }
    *network = base;
    Wire::Added
}

/// One kind of element the tools place, with its defaults.
struct Tool {
    kind: &'static str,
    /// The prefix of the instance name, for elements that name one.
    instance: Option<&'static str>,
}

const TOOLS: [Tool; 9] = [
    Tool {
        kind: "contact",
        instance: None,
    },
    Tool {
        kind: "coil",
        instance: None,
    },
    Tool {
        kind: "timer",
        instance: Some("T"),
    },
    Tool {
        kind: "counter",
        instance: Some("C"),
    },
    Tool {
        kind: "compare",
        instance: Some("CMP"),
    },
    Tool {
        kind: "math",
        instance: Some("MATH"),
    },
    Tool {
        kind: "branchSplit",
        instance: None,
    },
    Tool {
        kind: "branchMerge",
        instance: None,
    },
    Tool {
        kind: "junction",
        instance: None,
    },
];

/// The suffixes of the locals an element of `kind` declares, with their types; a `BOOL` starts
/// `FALSE`, any other type 0.
fn declared_locals(kind: &str) -> &'static [(&'static str, &'static str)] {
    match kind {
        "timer" => &[("Q", "BOOL"), ("ET", "TIME")],
        "counter" => &[("Q", "BOOL"), ("CV", "INT")],
        "compare" => &[("IN1", "INT"), ("IN2", "INT")],
        "math" => &[("IN1", "INT"), ("IN2", "INT"), ("OUT", "INT")],
        _ => &[],
    }
}

/// Whether a variable called `name` (any case) is declared in `scope` (no scope is global).
fn declared_in(program: &Json, name: &str, scope: &str) -> bool {
    let key = super::references::key(name);
    items(program, "variables").iter().any(|variable| {
        super::references::key(variable.str_at("name").unwrap_or_default()) == key
            && variable.str_at("scope").unwrap_or("global") == scope
    })
}

/// Places a new element of the tool `tool` at (`x`, `y`) in the rung `rung`, with the defaults of
/// its kind: a contact on `%IX0.0` (NO), a coil on `%QX0.0` (NORMAL), a TON timer of 1000 ms, a
/// CTU counter preset 10, a GT compare, an ADD math block, or a bare topology node. A timer,
/// counter, compare or math block gets the smallest free instance name (`T_1`, `C_1`, `CMP_1`,
/// `MATH_1`) and declares its operands as locals (`T_1_Q : BOOL := FALSE`, `T_1_ET : TIME := 0`,
/// …) unless a variable of that name is already declared locally. Answers the index of the new
/// node in its rung.
///
/// # Errors
/// There is no such rung or tool.
pub fn insert_element(
    program: &mut Json,
    rung: usize,
    tool: &str,
    x: f64,
    y: f64,
) -> Result<usize, String> {
    let tool = TOOLS
        .iter()
        .find(|candidate| candidate.kind == tool)
        .ok_or_else(|| format!("The Ladder editor has no tool \"{tool}\"."))?;
    if network_mut(program, rung).is_none() {
        return Err(no_rung(rung));
    }
    let id = fresh_node_id(program, tool.kind, &[]);
    let instance = tool
        .instance
        .map(|prefix| fresh_instance(program, prefix, tool.kind));
    let operand = |suffix: &str| {
        Json::String(format!(
            "{}_{suffix}",
            instance.as_deref().unwrap_or_default()
        ))
    };
    let mut members: Vec<(String, Json)> = vec![
        ("id".to_owned(), Json::String(id)),
        ("type".to_owned(), Json::str(tool.kind)),
    ];
    let mut add = |key: &str, value: Json| members.push((key.to_owned(), value));
    match tool.kind {
        "contact" => {
            add("contactType", Json::str("NO"));
            add("variable", Json::str("%IX0.0"));
        }
        "coil" => {
            add("coilType", Json::str("NORMAL"));
            add("variable", Json::str("%QX0.0"));
        }
        "timer" => {
            add("timerType", Json::str("TON"));
            add("instance", Json::str(instance.clone().unwrap_or_default()));
            add("input", Json::str(""));
            add("presetMs", Json::Number(1000.0));
            add("qOutput", operand("Q"));
            add("etOutput", operand("ET"));
        }
        "counter" => {
            add("counterType", Json::str("CTU"));
            add("instance", Json::str(instance.clone().unwrap_or_default()));
            add("input", Json::str(""));
            add("preset", Json::Number(10.0));
            add("qOutput", operand("Q"));
            add("cvOutput", operand("CV"));
        }
        "compare" => {
            add("op", Json::str("GT"));
            add("left", operand("IN1"));
            add("right", operand("IN2"));
        }
        "math" => {
            add("op", Json::str("ADD"));
            add("left", operand("IN1"));
            add("right", operand("IN2"));
            add("output", operand("OUT"));
        }
        _ => {}
    }
    add(
        "position",
        Json::Object(vec![
            ("x".to_owned(), Json::Number(x)),
            ("y".to_owned(), Json::Number(y)),
        ]),
    );
    let element = Json::Object(members);
    if let Some(instance) = &instance {
        for (suffix, ty) in declared_locals(tool.kind) {
            let name = format!("{instance}_{suffix}");
            if declared_in(program, &name, "local") {
                continue;
            }
            let initial = if *ty == "BOOL" {
                Json::Bool(false)
            } else {
                Json::Number(0.0)
            };
            let variable = Json::Object(vec![
                ("name".to_owned(), Json::String(name)),
                ("type".to_owned(), Json::str(*ty)),
                ("scope".to_owned(), Json::str("local")),
                ("initialValue".to_owned(), initial),
            ]);
            match program.get_mut("variables").and_then(Json::as_array_mut) {
                Some(variables) => variables.push(variable),
                None => program.set("variables", Json::Array(vec![variable])),
            }
        }
    }
    let network = network_mut(program, rung).ok_or_else(|| no_rung(rung))?;
    let nodes = nodes_mut(network);
    nodes.push(element);
    Ok(nodes.len() - 1)
}

/// The smallest free instance name `<prefix>_<n>`: no element names it as its instance and no
/// variable (any scope, any case) has one of the names its operands would get.
fn fresh_instance(program: &Json, prefix: &str, kind: &str) -> String {
    first_free(
        1,
        |n| format!("{prefix}_{n}"),
        |candidate| {
            let used_as_instance = items(program, "networks")
                .iter()
                .flat_map(|network| items(network, "nodes"))
                .any(|node| node.str_at("instance").map(js::trim) == Some(candidate));
            let operand_taken = declared_locals(kind).iter().any(|(suffix, _)| {
                let key = super::references::key(&format!("{candidate}_{suffix}"));
                items(program, "variables").iter().any(|variable| {
                    super::references::key(variable.str_at("name").unwrap_or_default()) == key
                })
            });
            used_as_instance || operand_taken
        },
    )
}
