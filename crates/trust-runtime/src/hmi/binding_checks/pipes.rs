//! A page's pipes against the page and the catalogue (specification 39, §6): each end a port of
//! an object of the page, each port taking one pipe, and the flow binding a number or a boolean
//! that the program has. A pipe is named `pipe:<n>` by its place among the pipes.

use std::collections::{BTreeMap, HashSet};

use super::{check_binding_with_remote, check_direction, BindingProblem};
use crate::hmi::catalogue::{Catalogue, ComponentCategory, ComponentDef, PropertyDef, ValueKind};
use crate::hmi::page_files::{HmiBindingDirection, HmiPage};
use crate::memory::VariableStorage;
use crate::runtime::RuntimeMetadata;

pub(super) fn check_pipes(
    page: &HmiPage,
    catalogue: &Catalogue,
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
    remote: &BTreeMap<String, serde_json::Value>,
) -> Vec<BindingProblem> {
    check_pipes_using(page, catalogue, &|component, property, binding| {
        check_binding_with_remote(component, property, binding, metadata, storage, remote)
    })
}

pub(super) fn check_pipes_using(
    page: &HmiPage,
    catalogue: &Catalogue,
    check: &dyn Fn(&ComponentDef, &str, &crate::hmi::page_files::HmiBinding) -> Result<(), String>,
) -> Vec<BindingProblem> {
    let rule = pipe_rule();
    let mut taken = HashSet::new();
    let mut problems = Vec::new();
    for (index, pipe) in page.pipes.iter().enumerate() {
        let name = format!("pipe:{index}");
        for (end, side) in [(&pipe.from, "from"), (&pipe.to, "to")] {
            if let Err(message) = check_end(page, catalogue, end, &mut taken) {
                problems.push(problem(&name, side, message));
            }
        }
        for (property, binding) in &pipe.bind {
            let checked = check_direction(&rule, property, binding, false)
                .and_then(|()| check(&rule, property, binding));
            if let Err(message) = checked {
                problems.push(problem(&name, property, message));
            }
        }
    }
    problems
}

fn problem(pipe: &str, property: &str, message: String) -> BindingProblem {
    BindingProblem {
        object: pipe.to_string(),
        property: Some(property.to_string()),
        message,
    }
}

/// One end: an object of the page, one of its ports, not taken by an earlier pipe.
fn check_end(
    page: &HmiPage,
    catalogue: &Catalogue,
    end: &str,
    taken: &mut HashSet<String>,
) -> Result<(), String> {
    let Some((id, port)) = end.split_once('.') else {
        return Err(format!("\"{end}\" names no port"));
    };
    let object = page
        .objects
        .iter()
        .find(|object| object.id == id)
        .ok_or_else(|| format!("the page has no object \"{id}\""))?;
    // An unknown component type is the object's own problem.
    if let Some(component) = catalogue.component(&object.kind) {
        if !component.ports.iter().any(|known| known == port) {
            return Err(if component.ports.is_empty() {
                format!("a {} has no ports", component.kind)
            } else {
                format!(
                    "a {} has no port \"{port}\"; it has {}",
                    component.kind,
                    listed(&component.ports)
                )
            });
        }
    }
    if !taken.insert(end.to_string()) {
        return Err(format!("the port {end} already has a pipe"));
    }
    Ok(())
}

/// Names joined in plain words: "a", "a and b", "a, b and c".
fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A pipe's one property, as a component's would be defined: its flow, a number or a boolean the
/// pipe reads (§6, item 6).
fn pipe_rule() -> ComponentDef {
    ComponentDef {
        kind: "pipe".to_string(),
        category: ComponentCategory::Equipment,
        phase: 2,
        size: [0, 0],
        variants: Vec::new(),
        properties: BTreeMap::from([(
            "flow".to_string(),
            PropertyDef {
                accepts: vec![ValueKind::Number, ValueKind::Boolean],
                directions: vec![HmiBindingDirection::Read],
            },
        )]),
        additional: None,
        ports: Vec::new(),
        attaches: false,
        group: None,
    }
}
