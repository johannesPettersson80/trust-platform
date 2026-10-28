//! Create HMI (specification 39, §9): an overview page with the symbol of each declared equipment
//! instance not yet on a page, bound to its members by its component definition and opening its
//! panel, in rows of measurements, controllers and actuators. Declarations only: nothing is
//! compiled or run, and the page is written by the runtime's page writer (§21.1 item 5).
use super::{bundle, components::EquipmentDefinition, declarations::Declarations};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use trust_runtime::hmi::page_files::{self as native, HmiCallPosition};
use trust_runtime::hmi::variable_tree::{VariableKind, VariableNode};

/// The margin around the symbols and the gap between them.
const MARGIN: u32 = 80;
const GAP: u32 = 64;
/// The most instances one page proposes.
const MAX_INSTANCES: usize = 2048;
/// The longest base chain followed for a definition.
const MAX_BASES: usize = 8;

/// The definition of a type: its own, else its nearest base's, so a block extending a library
/// block takes that block's symbol and panel.
fn definition_for<'a>(
    index: &Declarations,
    definitions: &'a [EquipmentDefinition],
    type_name: &str,
) -> Option<&'a EquipmentDefinition> {
    let mut current = type_name.to_string();
    for _ in 0..MAX_BASES {
        if let Some(found) =
            super::super::hmi_generate::named(definitions, &current, |definition| {
                definition.block.as_str()
            })
        {
            return Some(found);
        }
        current = index.base_of(&current)?;
    }
    None
}

/// The row of a definition: measurements, controllers or actuators.
fn row_of(definition: &EquipmentDefinition) -> usize {
    match definition.position {
        Some(HmiCallPosition::Measurements) => 0,
        Some(HmiCallPosition::Controllers) | None => 1,
        Some(HmiCallPosition::Actuators) => 2,
    }
}

/// The instances the project's pages already show.
fn shown(bundle: &trust_runtime::hmi::bundle::HmiBundle) -> Result<BTreeSet<String>, String> {
    let mut shown = BTreeSet::new();
    for (file, bytes) in &bundle.files {
        if !file.starts_with("hmi/pages/") || !file.ends_with(".toml") {
            continue;
        }
        let page = native::parse_page(file, &String::from_utf8_lossy(bytes)).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| format!("{}:{}: {}", error.file, error.line, error.message))
                .collect::<Vec<_>>()
                .join("\n")
        })?;
        for object in &page.objects {
            let instance = object
                .instance
                .clone()
                .or_else(|| trust_runtime::hmi::faceplate_fit::common_instance(object));
            if let Some(instance) = instance {
                shown.insert(instance.to_ascii_uppercase());
            }
        }
    }
    Ok(shown)
}

/// One proposed object.
fn object(
    node: &VariableNode,
    definition: &EquipmentDefinition,
    ordinal: usize,
    at: [u32; 2],
    size: [u32; 2],
) -> Value {
    let bindings: serde_json::Map<String, Value> = definition
        .bindings
        .iter()
        .map(|(property, member)| {
            let path =
                |member: &str| trust_runtime::hmi::page_files::pair_member_path(&node.path, member);
            let mut binding = json!({"path": path(member)});
            if let Some(quality) = definition.quality.get(property) {
                binding["quality"] = json!(path(quality));
            }
            (property.clone(), binding)
        })
        .collect();
    json!({
        "id": format!("equipment-{ordinal}"),
        "type": definition.symbol,
        "block": node.type_name,
        "instance": node.path,
        "tag": node.path.rsplit('.').next(),
        "faceplate": definition.panel,
        "bind": bindings,
        "at": at,
        "size": size,
    })
}

pub(in crate::handlers::commands) fn page(
    inputs: &super::super::ProjectInputs,
) -> Result<Value, String> {
    let index = Declarations::read(&inputs.sources);
    let bundle = bundle::read_edited(&inputs.root, &inputs.open).map_err(|e| e.to_string())?;
    let catalogue = super::super::hmi_symbols::project_catalogue(&inputs.root, &inputs.open)?;
    let (definitions, errors) = super::components::equipment_definitions(&bundle, &catalogue);
    let problems: Vec<Value> = errors
        .into_iter()
        .map(|error| json!({"file": error.file, "line": error.line, "message": error.message}))
        .collect();
    let width = bundle
        .files
        .get("hmi/screen.toml")
        .and_then(|bytes| {
            native::parse_screen("hmi/screen.toml", &String::from_utf8_lossy(bytes)).ok()
        })
        .map_or(1920, |screen| screen.size[0]);
    let shown = shown(&bundle)?;
    let nodes = super::declaration_walk::matching(&index.variables, MAX_INSTANCES, &|node| {
        node.kind == VariableKind::FunctionBlock
            && definition_for(&index, &definitions, &node.type_name).is_some()
    })?;
    let mut rows: [Vec<(&VariableNode, &EquipmentDefinition)>; 3] = Default::default();
    for node in &nodes {
        if shown.contains(&node.path.to_ascii_uppercase()) {
            continue;
        }
        if let Some(definition) = definition_for(&index, &definitions, &node.type_name) {
            rows[row_of(definition)].push((node, definition));
        }
    }
    let mut objects = Vec::new();
    let mut y = MARGIN + 40;
    for row in &rows {
        let (mut x, mut height) = (MARGIN, 0);
        for (node, definition) in row {
            let size = catalogue
                .component(&definition.symbol)
                .map_or([128, 108], |component| component.size);
            if x + size[0] + MARGIN > width && x > MARGIN {
                x = MARGIN;
                y += height + GAP;
                height = 0;
            }
            objects.push(object(node, definition, objects.len() + 1, [x, y], size));
            x += size[0] + GAP;
            height = height.max(size[1]);
        }
        if height > 0 {
            y += height + GAP;
        }
    }
    let count = objects.len();
    let model = json!({"title": "Overview", "icon": "overview", "object": objects});
    let text = super::documents::write("page", model).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let unmapped: Vec<Value> =
        super::declaration_walk::matching(&index.variables, MAX_INSTANCES, &|node| {
            node.kind == VariableKind::FunctionBlock
        })?
        .into_iter()
        .filter(|node| {
            !node.type_name.eq_ignore_ascii_case("Hmi.Display")
                && definition_for(&index, &definitions, &node.type_name).is_none()
        })
        .map(|node| json!({"instance": node.path, "block": node.type_name}))
        .collect();
    Ok(
        json!({"ok": true, "text": text, "count": count, "unmapped": unmapped, "problems": problems}),
    )
}
