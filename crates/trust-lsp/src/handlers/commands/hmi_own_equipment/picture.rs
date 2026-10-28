//! The picture of one of the project's own definitions (specification 39, §21.9 items 4 and 5):
//! the symbol it shows, the member that drives each of the symbol's inputs, and the change of
//! both, written to the definition and to every object of the block on the project's pages and
//! popups. A library's definition is the library's: the project does not change it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::{Catalogue, PropertyDef, ValueKind, VISIBLE};
use trust_runtime::hmi::page_files::{
    parse_page, parse_pair, parse_popup, write_page, write_pair, write_popup, HmiBinding,
    HmiBindingDirection, HmiObject, HmiPair,
};

use crate::handlers::commands::hmi_authoring::declarations::Declarations;

/// The `.toml` files of one of the project's own `hmi/` folders, unsaved texts in place of the
/// saved ones, by their path below the project.
fn own_files(root: &Path, open: &BTreeMap<PathBuf, String>, folder: &str) -> Vec<(String, String)> {
    let directory = root.join("hmi").join(folder);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default();
    paths.extend(
        open.keys()
            .filter(|path| path.parent() == Some(directory.as_path()))
            .cloned(),
    );
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "toml")
    });
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| {
            let text = open
                .get(&path)
                .cloned()
                .or_else(|| std::fs::read_to_string(&path).ok())?;
            Some((
                format!("hmi/{folder}/{}", path.file_name()?.to_string_lossy()),
                text,
            ))
        })
        .collect()
}

/// The project's own definition of a block, not a library's: its path and its pair.
pub(in crate::handlers::commands) fn own_pair(
    root: &Path,
    open: &BTreeMap<PathBuf, String>,
    block: &str,
) -> Option<(String, HmiPair)> {
    own_files(root, open, "pairs")
        .into_iter()
        .find_map(|(path, text)| {
            let pair = parse_pair(&path, &text).ok()?;
            pair.block
                .eq_ignore_ascii_case(block)
                .then_some((path, pair))
        })
}

/// The block's members with the kinds of value they hold.
fn members(index: &Declarations, block: &str) -> Vec<(String, Vec<ValueKind>)> {
    index
        .block_members(block)
        .unwrap_or_default()
        .into_iter()
        .map(|(name, type_id)| {
            let kinds = index.node(&name, &name, type_id, false, 0).kinds;
            (name, kinds)
        })
        .filter(|(_, kinds)| !kinds.is_empty())
        .collect()
}

/// Whether a member holding `kinds` fits an input accepting `accepts`.
fn fits(accepts: &[ValueKind], kinds: &[ValueKind]) -> bool {
    accepts
        .iter()
        .any(|accept| *accept == ValueKind::Value || kinds.contains(accept))
}

/// A name compared without case, hyphens or underscores: `out-of-service` is `OutOfService`.
fn plain(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// A picture's inputs: its properties but the object's visibility.
fn inputs<'a>(catalogue: &'a Catalogue, symbol: &str) -> Vec<(&'a String, &'a PropertyDef)> {
    catalogue
        .component(symbol)
        .map(|component| {
            component
                .properties
                .iter()
                .filter(|(name, _)| name.as_str() != VISIBLE)
                .collect()
        })
        .unwrap_or_default()
}

/// The picture as the inspector shows it: the symbol, whether the project may change it, and each
/// input with the member driving it and the members that fit it.
pub(in crate::handlers::commands) fn describe(
    root: &Path,
    open: &BTreeMap<PathBuf, String>,
    index: &Declarations,
    catalogue: &Catalogue,
    block: &str,
) -> Value {
    let Some((_, pair)) = own_pair(root, open, block) else {
        return json!({ "editable": false });
    };
    let members = members(index, block);
    let inputs: Vec<Value> = inputs(catalogue, &pair.symbol)
        .into_iter()
        .map(|(property, definition)| {
            let choices: Vec<&str> = members
                .iter()
                .filter(|(_, kinds)| fits(&definition.accepts, kinds))
                .map(|(name, _)| name.as_str())
                .collect();
            json!({ "property": property, "member": pair.bind.get(property), "choices": choices })
        })
        .collect();
    json!({ "editable": true, "symbol": pair.symbol, "inputs": inputs })
}

/// The edits that give a block's definition the picture `symbol`, its inputs driven by the members
/// `chosen` names, else by the members they had, else by the members of the same name; every
/// object of the block on the project's pages and popups shows it.
pub(in crate::handlers::commands) fn change(
    root: &Path,
    open: &BTreeMap<PathBuf, String>,
    index: &Declarations,
    catalogue: &Catalogue,
    block: &str,
    symbol: &str,
    chosen: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let (path, pair) = own_pair(root, open, block).ok_or_else(|| {
        format!("The definition of {block} belongs to a library; copy it into hmi/pairs to change its picture")
    })?;
    if catalogue.component(symbol).is_none() {
        return Err(format!("There is no picture \"{symbol}\""));
    }
    let members = members(index, block);
    let mut bind = BTreeMap::new();
    for (property, definition) in inputs(catalogue, symbol) {
        let member = chosen
            .get(property)
            .cloned()
            .or_else(|| {
                (symbol == pair.symbol)
                    .then(|| pair.bind.get(property).cloned())
                    .flatten()
            })
            .or_else(|| {
                members
                    .iter()
                    .find(|(name, kinds)| {
                        plain(name) == plain(property) && fits(&definition.accepts, kinds)
                    })
                    .map(|(name, _)| name.clone())
            });
        if let Some(member) = member.filter(|member| !member.is_empty()) {
            bind.insert(property.clone(), member);
        }
    }
    let pair = HmiPair {
        symbol: symbol.to_string(),
        bind,
        ..pair
    };
    let mut edits = vec![json!({ "path": path, "text": write_pair(&pair) })];
    for (file, text) in own_files(root, open, "pages") {
        let Ok(mut page) = parse_page(&file, &text) else {
            continue;
        };
        if show(&mut page.objects, &pair) {
            edits.push(json!({ "path": file, "text": write_page(&page) }));
        }
    }
    for (file, text) in own_files(root, open, "popups") {
        let Ok(mut popup) = parse_popup(&file, &text) else {
            continue;
        };
        if show(&mut popup.page.objects, &pair) {
            edits.push(json!({ "path": file, "text": write_popup(&popup) }));
        }
    }
    Ok(json!({ "ok": true, "edits": edits }))
}

/// Gives the objects of the definition's block its picture and bindings; whether any changed.
fn show(objects: &mut [HmiObject], pair: &HmiPair) -> bool {
    let mut changed = false;
    for object in objects.iter_mut().filter(|object| {
        object
            .block
            .as_deref()
            .is_some_and(|block| block.eq_ignore_ascii_case(&pair.block))
    }) {
        let mut bind: BTreeMap<String, HmiBinding> = object
            .bind
            .iter()
            .filter(|(name, _)| name.as_str() == VISIBLE)
            .map(|(name, binding)| (name.clone(), binding.clone()))
            .collect();
        if let Some(instance) = &object.instance {
            for (property, member) in &pair.bind {
                bind.insert(
                    property.clone(),
                    HmiBinding {
                        quality: pair
                            .quality
                            .get(property)
                            .map(|quality| format!("{instance}.{quality}")),
                        path: format!("{instance}.{member}"),
                        unit: None,
                        range: None,
                        direction: HmiBindingDirection::Read,
                    },
                );
            }
        }
        if object.kind != pair.symbol || object.bind != bind {
            object.kind = pair.symbol.clone();
            object.bind = bind;
            changed = true;
        }
    }
    changed
}
