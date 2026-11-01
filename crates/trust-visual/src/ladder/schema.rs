//! The schema-v2 check of a Ladder program (specification 16 §2).
//!
//! A file passes only when every member the profile names has its exact shape; one message per
//! problem names its JSON path, and the file is refused as a whole. Unknown members are kept and
//! not checked. Legacy files (no `schemaVersion: 2`) are refused, never migrated.

use crate::js;
use crate::Json;

/// The Ladder element kinds (specification 16 §3).
pub const ELEMENT_TYPES: [&str; 9] = [
    "contact",
    "coil",
    "timer",
    "counter",
    "compare",
    "math",
    "branchSplit",
    "branchMerge",
    "junction",
];

/// The variable types a Ladder program may declare (specification 16 §2).
pub const VARIABLE_TYPES: [&str; 6] = ["BOOL", "INT", "REAL", "TIME", "DINT", "LREAL"];

pub(super) const CONTACT_TYPES: [&str; 2] = ["NO", "NC"];
pub(super) const COIL_TYPES: [&str; 4] = ["NORMAL", "SET", "RESET", "NEGATED"];
pub(super) const TIMER_TYPES: [&str; 3] = ["TON", "TOF", "TP"];
pub(super) const COUNTER_TYPES: [&str; 3] = ["CTU", "CTD", "CTUD"];
pub(super) const COMPARE_OPS: [&str; 3] = ["GT", "LT", "EQ"];
pub(super) const MATH_OPS: [&str; 4] = ["ADD", "SUB", "MUL", "DIV"];

/// Checks that `program` is a schema-v2 Ladder program.
///
/// # Errors
/// The program breaks the schema: `Unsupported ladder schema: root object is missing.` for a
/// value that is no object, otherwise `Unsupported ladder schema. Expected strict schemaVersion: 2
/// contract.` followed by one `- <problem>` line per problem.
pub fn validate(program: &Json) -> Result<(), String> {
    if !program.is_object() {
        return Err("Unsupported ladder schema: root object is missing.".to_owned());
    }
    let mut errors: Vec<String> = Vec::new();
    if program.get("schemaVersion").and_then(Json::as_f64) != Some(2.0) {
        errors.push("schemaVersion must be exactly 2.".to_owned());
    }
    check_metadata(program.get("metadata"), &mut errors);
    check_variables(program.get("variables"), &mut errors);
    match program.get("networks").and_then(Json::as_array) {
        None => errors.push("networks must be an array.".to_owned()),
        Some(networks) => {
            for (index, network) in networks.iter().enumerate() {
                check_network(network, &format!("networks[{index}]"), &mut errors);
            }
        }
    }
    if errors.is_empty() {
        return Ok(());
    }
    let details: Vec<String> = errors.iter().map(|error| format!("- {error}")).collect();
    Err(format!(
        "Unsupported ladder schema. Expected strict schemaVersion: 2 contract.\n{}",
        details.join("\n")
    ))
}

/// Whether `value` is a string that is not blank.
pub(crate) fn non_blank(value: Option<&Json>) -> bool {
    value
        .and_then(Json::as_str)
        .is_some_and(|text| !js::trim(text).is_empty())
}

fn is_string(value: Option<&Json>) -> bool {
    value.and_then(Json::as_str).is_some()
}

fn is_finite(value: Option<&Json>) -> bool {
    value.and_then(Json::as_finite).is_some()
}

fn one_of(value: Option<&Json>, allowed: &[&str]) -> bool {
    value
        .and_then(Json::as_str)
        .is_some_and(|text| allowed.contains(&text))
}

fn check_metadata(metadata: Option<&Json>, errors: &mut Vec<String>) {
    let Some(metadata) = metadata.filter(|value| value.is_object()) else {
        errors.push("metadata object is required.".to_owned());
        return;
    };
    if !non_blank(metadata.get("name")) {
        errors.push("metadata.name must be a non-empty string.".to_owned());
    }
    if !is_string(metadata.get("description")) {
        errors.push("metadata.description must be a string.".to_owned());
    }
    for key in ["created", "modified"] {
        if metadata.get(key).is_some() && !is_string(metadata.get(key)) {
            errors.push(format!("metadata.{key} must be a string when provided."));
        }
    }
}

fn check_variables(variables: Option<&Json>, errors: &mut Vec<String>) {
    let Some(variables) = variables.and_then(Json::as_array) else {
        errors.push("variables must be an array.".to_owned());
        return;
    };
    for (index, variable) in variables.iter().enumerate() {
        let prefix = format!("variables[{index}]");
        if !variable.is_object() {
            errors.push(format!("{prefix} must be an object."));
            continue;
        }
        if !non_blank(variable.get("name")) {
            errors.push(format!("{prefix}.name must be a non-empty string."));
        }
        if !one_of(variable.get("type"), &VARIABLE_TYPES) {
            errors.push(format!(
                "{prefix}.type must be one of BOOL|INT|REAL|TIME|DINT|LREAL."
            ));
        }
        if variable.get("scope").is_some() && !one_of(variable.get("scope"), &["local", "global"]) {
            errors.push(format!(
                "{prefix}.scope must be 'local' or 'global' when provided."
            ));
        }
        if variable.get("address").is_some() && !is_string(variable.get("address")) {
            errors.push(format!("{prefix}.address must be a string when provided."));
        }
    }
}

fn check_network(network: &Json, prefix: &str, errors: &mut Vec<String>) {
    if !network.is_object() {
        errors.push(format!("{prefix} must be an object."));
        return;
    }
    if !non_blank(network.get("id")) {
        errors.push(format!("{prefix}.id must be a non-empty string."));
    }
    if !is_finite(network.get("order")) {
        errors.push(format!("{prefix}.order must be a finite number."));
    }
    let layout_y = network
        .get("layout")
        .filter(|layout| layout.is_object())
        .and_then(|layout| layout.get("y"));
    if !is_finite(layout_y) {
        errors.push(format!("{prefix}.layout.y must be a finite number."));
    }
    match network.get("nodes").and_then(Json::as_array) {
        None => errors.push(format!("{prefix}.nodes must be an array.")),
        Some(nodes) => {
            for (index, node) in nodes.iter().enumerate() {
                check_node(node, &format!("{prefix}.nodes[{index}]"), errors);
            }
        }
    }
    match network.get("edges").and_then(Json::as_array) {
        None => errors.push(format!("{prefix}.edges must be an array.")),
        Some(edges) => {
            for (index, edge) in edges.iter().enumerate() {
                check_edge(edge, &format!("{prefix}.edges[{index}]"), errors);
            }
        }
    }
}

fn check_node(node: &Json, prefix: &str, errors: &mut Vec<String>) {
    if !node.is_object() {
        errors.push(format!("{prefix} must be an object."));
        return;
    }
    if !non_blank(node.get("id")) {
        errors.push(format!("{prefix}.id must be a non-empty string."));
    }
    let Some(kind) = node
        .str_at("type")
        .filter(|kind| ELEMENT_TYPES.contains(kind))
    else {
        errors.push(format!("{prefix}.type is unsupported."));
        return;
    };
    let Some(position) = node.get("position").filter(|value| value.is_object()) else {
        errors.push(format!("{prefix}.position must be an object."));
        return;
    };
    if !is_finite(position.get("x")) || !is_finite(position.get("y")) {
        errors.push(format!("{prefix}.position.x/y must be finite numbers."));
    }
    match kind {
        "contact" => {
            if !one_of(node.get("contactType"), &CONTACT_TYPES) {
                errors.push(format!("{prefix}.contactType must be NO or NC."));
            }
            if !is_string(node.get("variable")) {
                errors.push(format!("{prefix}.variable must be a string."));
            }
        }
        "coil" => {
            if !one_of(node.get("coilType"), &COIL_TYPES) {
                errors.push(format!(
                    "{prefix}.coilType must be NORMAL, SET, RESET, or NEGATED."
                ));
            }
            if !is_string(node.get("variable")) {
                errors.push(format!("{prefix}.variable must be a string."));
            }
        }
        "timer" => {
            if !one_of(node.get("timerType"), &TIMER_TYPES) {
                errors.push(format!("{prefix}.timerType must be TON, TOF, or TP."));
            }
            check_block(node, prefix, "presetMs", "etOutput", errors);
        }
        "counter" => {
            if !one_of(node.get("counterType"), &COUNTER_TYPES) {
                errors.push(format!("{prefix}.counterType must be CTU, CTD, or CTUD."));
            }
            check_block(node, prefix, "preset", "cvOutput", errors);
        }
        "compare" => {
            if !one_of(node.get("op"), &COMPARE_OPS) {
                errors.push(format!("{prefix}.op must be GT, LT, or EQ."));
            }
            if !is_string(node.get("left")) || !is_string(node.get("right")) {
                errors.push(format!("{prefix}.left and .right must be strings."));
            }
        }
        "math" => {
            if !one_of(node.get("op"), &MATH_OPS) {
                errors.push(format!("{prefix}.op must be ADD, SUB, MUL, or DIV."));
            }
            if !is_string(node.get("left"))
                || !is_string(node.get("right"))
                || !is_string(node.get("output"))
            {
                errors.push(format!(
                    "{prefix}.left, .right, and .output must be strings."
                ));
            }
        }
        _ => {}
    }
}

/// The members a timer and a counter share: `instance`, an optional `input`, a preset number and
/// two output targets.
fn check_block(
    node: &Json,
    prefix: &str,
    preset: &str,
    value_output: &str,
    errors: &mut Vec<String>,
) {
    if !non_blank(node.get("instance")) {
        errors.push(format!("{prefix}.instance must be a non-empty string."));
    }
    if node.get("input").is_some() && !is_string(node.get("input")) {
        errors.push(format!("{prefix}.input must be a string when provided."));
    }
    if !is_finite(node.get(preset)) {
        errors.push(format!("{prefix}.{preset} must be a finite number."));
    }
    if !non_blank(node.get("qOutput")) {
        errors.push(format!("{prefix}.qOutput must be a non-empty string."));
    }
    if !non_blank(node.get(value_output)) {
        errors.push(format!(
            "{prefix}.{value_output} must be a non-empty string."
        ));
    }
}

fn check_edge(edge: &Json, prefix: &str, errors: &mut Vec<String>) {
    if !edge.is_object() {
        errors.push(format!("{prefix} must be an object."));
        return;
    }
    if !non_blank(edge.get("id")) {
        errors.push(format!("{prefix}.id must be a non-empty string."));
    }
    if !non_blank(edge.get("fromNodeId")) || !non_blank(edge.get("toNodeId")) {
        errors.push(format!(
            "{prefix}.fromNodeId and .toNodeId must be non-empty strings."
        ));
    }
    let Some(points) = edge.get("points") else {
        return;
    };
    let Some(points) = points.as_array() else {
        errors.push(format!("{prefix}.points must be an array when provided."));
        return;
    };
    for (index, point) in points.iter().enumerate() {
        if !is_finite(point.get("x")) || !is_finite(point.get("y")) {
            errors.push(format!(
                "{prefix}.points[{index}] must contain numeric x/y."
            ));
        }
    }
}
