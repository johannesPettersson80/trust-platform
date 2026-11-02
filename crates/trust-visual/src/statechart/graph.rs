//! The Statechart editor's graph and the file it saves (specification 25 §11.4).
//!
//! The editor draws each state as a node and each transition as an edge. [`graph`] reads a
//! `.statechart.json` file into that graph and [`file()`] writes a graph back as the file the
//! editor saves; the editor itself only places, moves and selects what the graph holds.
//!
//! The graph is an object of three members:
//!
//! - `states`: `{ id, label, type, entry, exit }` per state in file order, `id` being
//!   `state_<name>`, `type` one of `initial`, `final`, `compound` or `normal`;
//! - `transitions`: `{ id, source, target, event, guard?, actions?, after? }` per transition, the
//!   ends being state ids (`state_<target>`), the optional members present when the file has them;
//! - `actionMappings`: the file's action mappings as they are.
//!
//! A file written from a graph is the machine `stateMachine` (saving over a file keeps that file's
//! id, [`keep_identity`]): its `initial` state (left out when
//! no state is initial), every state under its label with its non-empty `entry` and `exit`, its
//! `final` or `compound` type and its transitions by event (`NEXT` for an edge without an event;
//! a later edge of the same event replaces an earlier one), and the action mappings. Transitions to
//! a state the graph does not hold are left out. Members are written in the order JavaScript
//! writes them, so a file round-trips exactly as the editor wrote it.

use crate::js;
use crate::Json;

/// The message when a graph does not have the members the file is written from.
const GRAPH_MESSAGE: &str = "The statechart graph must hold a list of states and of transitions.";

/// The editor graph of the statechart file `file`.
///
/// # Errors
/// The file is not an object with a `states` object, or a state or transition is `null`.
pub fn graph(file: &Json) -> Result<Json, String> {
    let Some(Json::Object(members)) = file.get("states") else {
        return Err(super::FORMAT_MESSAGE.to_owned());
    };
    let initial = file.get("initial");
    let mut states = Vec::new();
    let mut transitions = Vec::new();
    for (name, config) in js::ordered(members) {
        if matches!(config, Json::Null) {
            return Err(format!("The state \"{name}\" is null."));
        }
        let kind = if initial.and_then(Json::as_str) == Some(name.as_str()) {
            "initial"
        } else {
            match config.str_at("type") {
                Some("final") => "final",
                Some("compound") => "compound",
                _ => "normal",
            }
        };
        let id = format!("state_{name}");
        let list = |member: &str| {
            config
                .get(member)
                .filter(|value| js::truthy(Some(value)))
                .cloned()
                .unwrap_or_else(|| Json::Array(Vec::new()))
        };
        states.push(Json::Object(vec![
            ("id".to_owned(), Json::str(id.clone())),
            ("label".to_owned(), Json::str(name.clone())),
            ("type".to_owned(), Json::str(kind)),
            ("entry".to_owned(), list("entry")),
            ("exit".to_owned(), list("exit")),
        ]));
        let Some(on) = config.get("on").filter(|on| js::truthy(Some(on))) else {
            continue;
        };
        for (event, transition) in js::entries(on) {
            if matches!(transition, Json::Null) {
                return Err(format!(
                    "The transition \"{event}\" of the state \"{name}\" is null."
                ));
            }
            let target = match transition {
                Json::String(_) => Some(transition),
                other => other.get("target"),
            };
            let target_id = format!("state_{}", js::string(target));
            let mut edge = vec![
                (
                    "id".to_owned(),
                    Json::str(format!("edge_{id}_{target_id}_{event}")),
                ),
                ("source".to_owned(), Json::str(id.clone())),
                ("target".to_owned(), Json::str(target_id)),
                ("event".to_owned(), Json::str(event)),
            ];
            for member in ["guard", "actions", "after"] {
                if let Some(value) = transition.get(member) {
                    edge.push((member.to_owned(), value.clone()));
                }
            }
            transitions.push(Json::Object(edge));
        }
    }
    let mappings = file
        .get("actionMappings")
        .filter(|mappings| js::truthy(Some(mappings)))
        .cloned()
        .unwrap_or_else(Json::object);
    Ok(Json::Object(vec![
        ("states".to_owned(), Json::Array(states)),
        ("transitions".to_owned(), Json::Array(transitions)),
        ("actionMappings".to_owned(), mappings),
    ]))
}

/// Which actions a statechart names and which it maps (the editor's action-mapping panel).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionReport {
    /// Actions a state's `entry` or `exit` or a transition's `actions` names that no action
    /// mapping defines, in the order they are first named (states first, then transitions).
    pub unmapped: Vec<String>,
    /// Action mappings no state or transition names, in the order of the mappings.
    pub unused: Vec<String>,
}

/// The [`ActionReport`] of the editor graph `graph`. Blank action names are left out.
#[must_use]
pub fn action_report(graph: &Json) -> ActionReport {
    let names = |item: &Json, member: &str| -> Vec<String> {
        item.get(member)
            .and_then(Json::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(Json::as_str)
                    .filter(|name| !crate::js::trim(name).is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let list = |member: &str| {
        graph
            .get(member)
            .and_then(Json::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
    };
    let mut referenced: Vec<String> = Vec::new();
    let mut note = |name: String| {
        if !referenced.contains(&name) {
            referenced.push(name);
        }
    };
    for state in list("states") {
        names(state, "entry").into_iter().for_each(&mut note);
        names(state, "exit").into_iter().for_each(&mut note);
    }
    for transition in list("transitions") {
        names(transition, "actions").into_iter().for_each(&mut note);
    }
    let mapped: Vec<&String> = match graph.get("actionMappings") {
        Some(Json::Object(members)) => js::ordered(members)
            .into_iter()
            .map(|(name, _)| name)
            .collect(),
        _ => Vec::new(),
    };
    ActionReport {
        unmapped: referenced
            .iter()
            .filter(|name| !mapped.contains(name))
            .cloned()
            .collect(),
        unused: mapped
            .into_iter()
            .filter(|name| !referenced.contains(name))
            .cloned()
            .collect(),
    }
}

/// `saved` (a file written by [`file()`]) with the `id` of the file it replaces, `previous`, when
/// that file has a non-empty one: saving keeps the chart's own id instead of `stateMachine`.
pub fn keep_identity(saved: &mut Json, previous: &Json) {
    if let Some(id) = previous.str_at("id").filter(|id| !id.is_empty()) {
        saved.set("id", Json::str(id));
    }
}

/// Whether the file can hold `states`: it names each state once and starts in one.
///
/// # Errors
/// Two states share a name, or more than one is initial.
fn one_name_each_and_one_start(states: &[Json]) -> Result<(), String> {
    let mut names: Vec<String> = Vec::new();
    for state in states {
        let name = js::string(state.get("label"));
        if names.contains(&name) {
            return Err(format!(
                "Two states are named \"{name}\"; give each state its own name."
            ));
        }
        names.push(name);
    }
    let initial_names: Vec<String> = states
        .iter()
        .filter(|state| state.str_at("type") == Some("initial"))
        .map(|state| js::string(state.get("label")))
        .collect();
    if initial_names.len() > 1 {
        return Err(format!(
            "The states \"{}\" are all initial; a statechart starts in one state.",
            initial_names.join("\", \"")
        ));
    }
    Ok(())
}

/// The statechart file of the editor graph `graph`, with its members in the order JavaScript
/// writes them.
///
/// # Errors
/// The graph has no list of states or of transitions, or a state or transition is no object; or
/// two states share a name or more than one is initial. The file names each state once and starts
/// in one, so writing such a graph would silently drop a state or change which state starts.
pub fn file(graph: &Json) -> Result<Json, String> {
    let states = graph
        .get("states")
        .and_then(Json::as_array)
        .ok_or(GRAPH_MESSAGE)?;
    let transitions = graph
        .get("transitions")
        .and_then(Json::as_array)
        .ok_or(GRAPH_MESSAGE)?;
    if !states.iter().chain(transitions).all(Json::is_object) {
        return Err(GRAPH_MESSAGE.to_owned());
    }
    one_name_each_and_one_start(states)?;
    let mut written = Json::object();
    let mut initial: Option<&Json> = None;
    for state in states {
        let mut config = Json::object();
        for member in ["entry", "exit"] {
            let value = state.get(member);
            if js::has_length(value) {
                config.set(member, value.cloned().unwrap_or(Json::Null));
            }
        }
        match state.str_at("type") {
            Some("final") => config.set("type", Json::str("final")),
            Some("compound") => config.set("type", Json::str("compound")),
            _ => {}
        }
        let mut on = Json::object();
        for edge in transitions
            .iter()
            .filter(|edge| edge.get("source") == state.get("id"))
        {
            let Some(target) = states
                .iter()
                .find(|candidate| candidate.get("id") == edge.get("target"))
            else {
                continue;
            };
            let mut transition = Json::object();
            if let Some(label) = target.get("label") {
                transition.set("target", label.clone());
            }
            if js::truthy(edge.get("guard")) {
                transition.set("guard", edge.get("guard").cloned().unwrap_or(Json::Null));
            }
            if js::truthy(edge.get("actions")) && js::has_length(edge.get("actions")) {
                transition.set(
                    "actions",
                    edge.get("actions").cloned().unwrap_or(Json::Null),
                );
            }
            if let Some(after) = edge.get("after") {
                transition.set("after", after.clone());
            }
            let event = edge.get("event");
            let event = if js::truthy(event) {
                js::string(event)
            } else {
                "NEXT".to_owned()
            };
            on.set(&event, transition);
        }
        if on.as_object().is_some_and(|members| !members.is_empty()) {
            config.set("on", on);
        }
        written.set(&js::string(state.get("label")), config);
        if state.str_at("type") == Some("initial") {
            initial = state.get("label");
        }
    }
    let mut machine = Json::object();
    machine.set("id", Json::str("stateMachine"));
    if let Some(initial) = initial {
        machine.set("initial", initial.clone());
    }
    machine.set("states", written);
    machine.set(
        "actionMappings",
        graph
            .get("actionMappings")
            .cloned()
            .unwrap_or_else(Json::object),
    );
    Ok(js::normalized(&machine))
}
