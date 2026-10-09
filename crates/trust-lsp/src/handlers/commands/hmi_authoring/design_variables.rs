//! Pending equipment is visible in pickers before source generation. These nodes are
//! declaration-shaped design intent, never instantiated or passed to a runtime.
use super::super::ProjectInputs;
use super::declarations::Declarations;
use std::collections::BTreeMap;
use trust_runtime::hmi::variable_tree::{VariableKind, VariableNode};
pub(in crate::handlers::commands) fn variables(
    inputs: &ProjectInputs,
    index: &Declarations,
) -> Result<Vec<VariableNode>, String> {
    let bundle = super::bundle::read_edited(&inputs.root, &inputs.open)
        .map_err(|error| error.to_string())?;
    let mut pending = BTreeMap::<String, String>::new();
    for (file, bytes) in &bundle.files {
        if !file.starts_with("hmi/pages/") || !file.ends_with(".toml") {
            continue;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        let Ok(table) = toml::from_str::<toml::Table>(text) else {
            continue;
        };
        for object in table
            .get("object")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            if let (Some(instance), Some(block)) = (
                object.get("instance").and_then(toml::Value::as_str),
                object.get("block").and_then(toml::Value::as_str),
            ) {
                pending.insert(instance.into(), block.into());
            }
        }
    }
    // A design that does not load adds nothing here; the inspector reports why.
    if let Ok((design, _)) =
        super::super::hmi_plant_project::read_design(&inputs.root, &inputs.open)
    {
        pending.extend(
            design
                .equipment
                .into_iter()
                .map(|(path, entry)| (path, entry.block)),
        );
    }
    let mut variables = index.variables.clone();
    for (path, block) in pending {
        if index.resolve(&path).is_some() {
            continue;
        }
        let Some((root, name)) = path.split_once('.') else {
            continue;
        };
        if name.contains('.') {
            continue;
        }
        let Some(ty) = index.registry.lookup(&block) else {
            continue;
        };
        let target = if let Some(position) = variables
            .iter()
            .position(|node| node.path.eq_ignore_ascii_case(root))
        {
            position
        } else {
            variables.push(VariableNode {
                name: root.into(),
                path: root.into(),
                type_name: root.into(),
                kind: VariableKind::Program,
                kinds: vec![],
                writable: false,
                write_block: None,
                access: None,
                children: vec![],
                array: None,
            });
            variables.len() - 1
        };
        if !variables[target]
            .children
            .iter()
            .any(|node| node.path.eq_ignore_ascii_case(&path))
        {
            variables[target]
                .children
                .push(index.node(name, &path, ty, false, 0));
        }
    }
    Ok(variables)
}

pub(in crate::handlers::commands) fn json(
    inputs: &ProjectInputs,
    index: &Declarations,
) -> Result<serde_json::Value, String> {
    let mut value =
        serde_json::to_value(variables(inputs, index)?).map_err(|error| error.to_string())?;
    fn mark(node: &mut serde_json::Value, index: &Declarations) {
        if node["path"]
            .as_str()
            .is_some_and(|path| index.resolve(path).is_none())
        {
            node["pending"] = serde_json::json!(true);
        }
        if let Some(children) = node
            .get_mut("children")
            .and_then(serde_json::Value::as_array_mut)
        {
            for child in children {
                mark(child, index);
            }
        }
        if let Some(element) = node
            .get_mut("array")
            .and_then(|array| array.get_mut("element"))
        {
            mark(element, index);
        }
    }
    if let Some(nodes) = value.as_array_mut() {
        for node in nodes {
            mark(node, index);
        }
    }
    Ok(value)
}
