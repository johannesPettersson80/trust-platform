//! The variable picker: what a binding is offered and what a dropped variable creates
//! (specification 39, §7.1; row H10). The rules are those of the binding checks (§7.6): a
//! binding takes a variable of a kind its property accepts, and one that writes takes only a
//! variable that can be written. The editor shows the answers; searching the tree and folding it
//! are the editor's own.
//!
//! - `offered { variables, fields?, component?, type, property, direction? }` → `{ rows }`: the
//!   tree a binding's picker shows, each variable marked `offered` when the binding may have it,
//!   with only the programs, blocks and structures that hold one. A faceplate's fields come first,
//!   as `$Name` (§8.7). `type` is the bound object's component type, `pipe` for a pipe's flow
//!   (§6 item 6); `component` its catalogue definition; `direction` the binding's own.
//! - `bind { component?, object, property, path? | variable? }` → `{ object }`: the object with
//!   the binding set to the path, or to the dropped variable, in the binding's direction; an empty
//!   path removes it. A dropped variable the binding cannot have answers `{ refused }`, saying why.
//! - `drop { components, variable }` → `{ type, bind, label }`: the object a variable dropped on
//!   the canvas creates, or `{ refused }` saying why it creates none.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::{ComponentDef, PropertyDef, ValueKind};
use trust_runtime::hmi::page_files::{HmiBinding, HmiBindingDirection, HmiField, HmiObject};

use super::decode;

/// The component type that stands for a pipe, whose one property is its flow (§6 item 6).
const PIPE: &str = "pipe";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Request {
    Offered {
        variables: Vec<VariableNode>,
        #[serde(default)]
        fields: BTreeMap<String, HmiField>,
        #[serde(default)]
        component: Option<Box<ComponentDef>>,
        #[serde(rename = "type")]
        kind: String,
        property: String,
        #[serde(default)]
        direction: Option<HmiBindingDirection>,
    },
    Bind {
        #[serde(default)]
        component: Option<Box<ComponentDef>>,
        object: Box<HmiObject>,
        property: String,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        variable: Option<Box<VariableNode>>,
    },
    Drop {
        components: Vec<ComponentDef>,
        variable: Box<VariableNode>,
    },
}

/// One variable of the program as the language server's variable tree gives it
/// (`trust-lsp.hmiBindings`, §17.3), or one of another runtime's (§7 item 9).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(super) struct VariableNode {
    #[serde(default)]
    pub(super) name: String,
    /// Its runtime instance path, for example `FeedArea.Pumps[1].Running`.
    pub(super) path: String,
    /// `program`, `function-block`, `structure`, `array` or `value`.
    #[serde(default)]
    pub(super) kind: String,
    /// Its declared type, for example `REAL` or `Plant.FB_Pump`.
    #[serde(default, rename = "type")]
    pub(super) type_name: String,
    /// The kinds of value it holds, as the catalogue names them (§5.6).
    #[serde(default)]
    pub(super) kinds: Vec<String>,
    /// Whether a binding may write it (§7.6).
    #[serde(default)]
    pub(super) writable: bool,
    /// Why a binding cannot write it, in the binding checks' words.
    #[serde(
        default,
        rename = "writeBlock",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) write_block: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    access: Option<String>,
    /// Design intent not yet generated into PLC code (§21.9).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) pending: bool,
    /// Function block members or structure fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) children: Vec<VariableNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    array: Option<ArrayShape>,
}

/// An array's element, standing for all of them.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ArrayShape {
    element: Box<VariableNode>,
}

impl VariableNode {
    /// What the picker lists under it: an array's element, else its members or fields.
    fn listed(&self) -> &[VariableNode] {
        match &self.array {
            Some(array) => std::slice::from_ref(&*array.element),
            None => &self.children,
        }
    }

    /// The variable without what it holds, as one row of the picker carries it.
    fn alone(&self) -> VariableNode {
        VariableNode {
            name: self.name.clone(),
            path: self.path.clone(),
            kind: self.kind.clone(),
            type_name: self.type_name.clone(),
            kinds: self.kinds.clone(),
            writable: self.writable,
            write_block: self.write_block.clone(),
            access: self.access.clone(),
            pending: self.pending,
            children: Vec::new(),
            array: None,
        }
    }

    fn holds(&self, kind: &str) -> bool {
        self.kinds.iter().any(|held| held == kind)
    }
}

/// Every variable of a tree with the members and fields under it, in tree order; an array's
/// element is not among them.
pub(super) fn every_variable(nodes: &[VariableNode]) -> Vec<&VariableNode> {
    let mut all = Vec::new();
    for node in nodes {
        all.push(node);
        all.extend(every_variable(&node.children));
    }
    all
}

/// One row of a binding's picker.
#[derive(Debug, Serialize)]
struct Row {
    node: VariableNode,
    /// Whether the binding may have it; a row that is not only holds rows that are.
    offered: bool,
    children: Vec<Row>,
}

/// The answer of `trust-lsp.hmiVariablePicker`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Request>(argument)? {
        Request::Offered {
            variables,
            fields,
            component,
            kind,
            property,
            direction,
        } => {
            let rule = rule_of(&kind, &property, component.as_deref());
            let direction = binding_direction(rule.as_ref(), direction);
            let mut nodes = field_nodes(&fields);
            nodes.extend(variables);
            let offered = |node: &VariableNode| {
                rule.as_ref()
                    .is_some_and(|rule| refusal(&kind, &property, rule, node, direction).is_none())
            };
            Ok(json!({ "ok": true, "rows": rows(&nodes, &offered) }))
        }
        Request::Bind {
            component,
            object,
            property,
            path,
            variable,
        } => bind(
            component.as_deref(),
            *object,
            &property,
            path,
            variable.map(|node| *node),
        ),
        Request::Drop {
            components,
            variable,
        } => Ok(dropped(&components, &variable)),
    }
}

/// A property's rule: a pipe's flow, or the component's property, or its rule for further
/// properties (a trend's pens).
fn rule_of(kind: &str, property: &str, component: Option<&ComponentDef>) -> Option<PropertyDef> {
    if kind == PIPE {
        return (property == "flow").then(|| PropertyDef {
            accepts: vec![ValueKind::Number, ValueKind::Boolean],
            directions: vec![HmiBindingDirection::Read],
        });
    }
    component
        .and_then(|component| component.property(property))
        .cloned()
}

/// The direction a binding of a property has: the binding's own, else the first its property
/// allows, else reading (§7.4).
fn binding_direction(
    rule: Option<&PropertyDef>,
    existing: Option<HmiBindingDirection>,
) -> HmiBindingDirection {
    existing
        .or_else(|| rule.and_then(|rule| rule.directions.first().copied()))
        .unwrap_or_default()
}

/// Why a variable cannot be bound to a property, in the binding checks' words (§7.6): a kind of
/// value the property does not accept, or a write to what cannot be written.
fn refusal(
    component: &str,
    property: &str,
    rule: &PropertyDef,
    node: &VariableNode,
    direction: HmiBindingDirection,
) -> Option<String> {
    if !rule.accepts.iter().any(|kind| node.holds(kind_name(*kind))) {
        let needed = rule
            .accepts
            .iter()
            .map(|kind| kind.words())
            .collect::<Vec<_>>()
            .join(" or ");
        return Some(format!(
            "\"{}\" is {}; a {}'s {property} needs {needed}",
            node.path,
            node.type_name,
            component.replace('-', " ")
        ));
    }
    if direction != HmiBindingDirection::Read && !node.writable {
        return Some(format!(
            "\"{}\" {} and cannot be written",
            node.path,
            node.write_block.as_deref().unwrap_or("is read-only")
        ));
    }
    None
}

/// A kind of value as the catalogue and the variable tree name it.
fn kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Number => "number",
        ValueKind::Boolean => "boolean",
        ValueKind::Text => "text",
        ValueKind::Time => "time",
        ValueKind::Enumeration => "enumeration",
        ValueKind::Structure => "structure",
        ValueKind::Array => "array",
        ValueKind::Value => "value",
    }
}

/// The kinds of a single value.
const SINGLE: [&str; 5] = ["number", "boolean", "text", "time", "enumeration"];

/// A faceplate's fields as the picker lists variables (§8.7): `$Name`, with the kinds it takes,
/// any single value when it names none; a single value is also a "value", as a variable's is.
fn field_nodes(fields: &BTreeMap<String, HmiField>) -> Vec<VariableNode> {
    fields
        .iter()
        .map(|(name, field)| {
            let taken: Vec<String> =
                if field.accepts.is_empty() || field.accepts.iter().any(|kind| kind == "value") {
                    SINGLE.iter().map(|kind| (*kind).to_string()).collect()
                } else {
                    field.accepts.clone()
                };
            let single = taken.iter().all(|kind| SINGLE.contains(&kind.as_str()));
            let mut kinds = taken;
            if single {
                kinds.push("value".to_string());
            }
            VariableNode {
                name: format!("${name}"),
                path: format!("${name}"),
                kind: "value".to_string(),
                type_name: if field.accepts.is_empty() {
                    "any value".to_string()
                } else {
                    field.accepts.join(" or ")
                },
                kinds,
                writable: true,
                ..VariableNode::default()
            }
        })
        .collect()
}

/// The rows of a tree with only the variables `offered` takes and those that hold them.
fn rows(nodes: &[VariableNode], offered: &dyn Fn(&VariableNode) -> bool) -> Vec<Row> {
    nodes
        .iter()
        .filter_map(|node| {
            let children = rows(node.listed(), offered);
            let offers = offered(node);
            (offers || !children.is_empty()).then(|| Row {
                node: node.alone(),
                offered: offers,
                children,
            })
        })
        .collect()
}

/// `bind`: the object with its binding set; a dropped variable it cannot have is refused.
fn bind(
    component: Option<&ComponentDef>,
    mut object: HmiObject,
    property: &str,
    path: Option<String>,
    variable: Option<VariableNode>,
) -> Result<Value, String> {
    let rule = component.and_then(|component| component.property(property));
    let existing = object.bind.get(property).map(|binding| binding.direction);
    let direction = binding_direction(rule, existing);
    let path = match (variable, path) {
        (Some(node), _) => {
            let refused = match rule {
                None => Some(format!(
                    "a {} has no property \"{property}\"",
                    object.kind.replace('-', " ")
                )),
                Some(rule) => refusal(&object.kind, property, rule, &node, direction),
            };
            if let Some(reason) = refused {
                return Ok(json!({ "ok": true, "refused": reason }));
            }
            node.path
        }
        (None, Some(path)) => path,
        (None, None) => return Err("bind needs a path or a variable.".into()),
    };
    if path.is_empty() {
        object.bind.remove(property);
    } else {
        let binding = object
            .bind
            .entry(property.to_string())
            .or_insert_with(|| HmiBinding {
                quality: None,
                path: String::new(),
                unit: None,
                range: None,
                direction,
            });
        binding.path = path;
        binding.direction = direction;
    }
    Ok(json!({ "ok": true, "object": object }))
}

/// `drop`: the component a variable dropped on the canvas creates (§7.1): a boolean a Status, a
/// structure or an array a Table, any other single value a Value, bound by the first property
/// that accepts it and labelled with the last part of its path.
fn dropped(components: &[ComponentDef], variable: &VariableNode) -> Value {
    let kind = if variable.holds("boolean") {
        Some("status")
    } else if variable.holds("structure") || variable.holds("array") {
        Some("table")
    } else if variable.holds("value") {
        Some("value")
    } else {
        None
    };
    let made = kind
        .and_then(|kind| components.iter().find(|component| component.kind == kind))
        .and_then(|component| {
            component
                .properties
                .iter()
                .find(|(_, rule)| {
                    rule.accepts
                        .iter()
                        .any(|kind| variable.holds(kind_name(*kind)))
                })
                .map(|(property, rule)| (component, property, rule))
        });
    let Some((component, property, rule)) = made else {
        return json!({ "ok": true, "refused": why_nothing(variable) });
    };
    let mut bind = serde_json::Map::new();
    bind.insert(
        property.clone(),
        json!({ "path": variable.path, "direction": binding_direction(Some(rule), None) }),
    );
    json!({
        "ok": true,
        "type": component.kind,
        "bind": bind,
        "label": variable.path.rsplit('.').next().unwrap_or(&variable.path),
    })
}

/// What the editor says when a dropped variable creates nothing: a program or function block
/// instance, whose members can be dropped instead.
fn why_nothing(variable: &VariableNode) -> String {
    match variable.kind.as_str() {
        "program" => format!(
            "\"{}\" is a program: drag one of its variables onto the page",
            variable.path
        ),
        "function-block" => format!(
            "\"{}\" is a function block instance ({}): drag one of its members onto the page",
            variable.path, variable.type_name
        ),
        _ => format!(
            "\"{}\" ({}) has no component to show it",
            variable.path, variable.type_name
        ),
    }
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
