//! Copy, paste and duplicate (specification 39, §3.3 item 8; row H11). The copied objects travel
//! through the clipboard as text, so they paste on any page. A copy keeps every property and
//! binding and gets the first free id of its type (§3.3 item 6); pipes between copied objects
//! come along, pipes to objects not copied are dropped, and an instrument keeps what it measures
//! only when that came along too. A copied binding moves to the next number when that variable
//! exists in the program (`TT101` to `TT102`, `TT009` to `TT010`).
//!
//! - `copy { page, ids }` → `{ text }`: the objects of `ids` with the pipes between them as
//!   clipboard text; no `text` when none of them is on the page.
//! - `paste { page, text, step, variables }` → `{ page, ids }`: the page with the clipboard's
//!   objects added `step` from where they were copied, and their new ids; neither when the text
//!   holds no objects.
//! - `duplicate { page, ids, step, variables }` → `{ page, ids }`: a copy pasted at once.
//!
//! `page` is the editor's model of a page, popup or panel (§15); only its objects and pipes
//! change. `variables` is the program's variable tree, whose paths decide the numbering.

use std::collections::{BTreeMap, HashSet};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{HmiBinding, HmiObject, HmiPipe};

use super::decode;
use super::variables::{every_variable, VariableNode};

/// The key that marks clipboard text as copied objects.
const MARK: &str = "trust-hmi-objects";

/// An attachment to a pipe, `pipe:<n>` (§6 item 5).
const PIPE_ATTACHMENT: &str = "pipe:";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Request {
    Copy {
        page: Value,
        ids: Vec<String>,
    },
    Paste {
        page: Value,
        text: String,
        step: [i64; 2],
        #[serde(default)]
        variables: Vec<VariableNode>,
    },
    Duplicate {
        page: Value,
        ids: Vec<String>,
        step: [i64; 2],
        #[serde(default)]
        variables: Vec<VariableNode>,
    },
}

/// Copied objects and the pipes between them; an instrument on a copied pipe names it by its
/// place among them.
struct Copied {
    objects: Vec<HmiObject>,
    pipes: Vec<HmiPipe>,
}

/// The answer of `trust-lsp.hmiClipboard`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Request>(argument)? {
        Request::Copy { page, ids } => {
            let (objects, pipes) = parts(&page)?;
            let Some(copied) = copied(&objects, &pipes, &ids) else {
                return Ok(json!({ "ok": true }));
            };
            let text = text(&copied)?;
            Ok(json!({ "ok": true, "text": text }))
        }
        Request::Paste {
            page,
            text,
            step,
            variables,
        } => match from_text(&text)? {
            Some(copied) => pasted(page, copied, step, &variables),
            None => Ok(json!({ "ok": true })),
        },
        Request::Duplicate {
            page,
            ids,
            step,
            variables,
        } => {
            let (objects, pipes) = parts(&page)?;
            match copied(&objects, &pipes, &ids) {
                Some(copied) => pasted(page, copied, step, &variables),
                None => Ok(json!({ "ok": true })),
            }
        }
    }
}

/// A page's objects and pipes.
fn parts(page: &Value) -> Result<(Vec<HmiObject>, Vec<HmiPipe>), String> {
    if !page.is_object() {
        return Err("The page is not a page model.".into());
    }
    let objects = listed(page, "object")
        .map_err(|error| format!("The page's objects do not load: {error}."))?;
    let pipes =
        listed(page, "pipe").map_err(|error| format!("The page's pipes do not load: {error}."))?;
    Ok((objects, pipes))
}

/// A list of a model, empty when the model has none.
pub(super) fn listed<T: DeserializeOwned>(
    model: &Value,
    key: &str,
) -> Result<Vec<T>, serde_json::Error> {
    match model.get(key) {
        Some(list) => serde_json::from_value(list.clone()),
        None => Ok(Vec::new()),
    }
}

/// The object a port `<object id>.<port>` belongs to.
fn object_of(port: &str) -> &str {
    port.rsplit_once('.').map_or(port, |(object, _)| object)
}

/// The pipe a `pipe:<n>` attachment names.
fn attached_pipe(attach: &str) -> Option<usize> {
    attach.strip_prefix(PIPE_ATTACHMENT)?.parse().ok()
}

/// The objects of `ids` in page order, with the pipes between them; none when no object is.
fn copied(objects: &[HmiObject], pipes: &[HmiPipe], ids: &[String]) -> Option<Copied> {
    let chosen: Vec<&HmiObject> = objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .collect();
    if chosen.is_empty() {
        return None;
    }
    let copied_ids: HashSet<&str> = chosen.iter().map(|object| object.id.as_str()).collect();
    // The places on the page of the pipes that come along.
    let places: Vec<usize> = pipes
        .iter()
        .enumerate()
        .filter(|(_, pipe)| {
            copied_ids.contains(object_of(&pipe.from)) && copied_ids.contains(object_of(&pipe.to))
        })
        .map(|(place, _)| place)
        .collect();
    let objects = chosen
        .into_iter()
        .map(|object| {
            let mut object = object.clone();
            // An instrument on a copied pipe names it by its place among the copied pipes; on a
            // pipe left behind it stands free. One attached to an object keeps its id.
            if let Some(attach) = object
                .attach
                .as_deref()
                .filter(|attach| attach.starts_with(PIPE_ATTACHMENT))
            {
                object.attach = attached_pipe(attach)
                    .and_then(|pipe| places.iter().position(|place| *place == pipe))
                    .map(|place| format!("{PIPE_ATTACHMENT}{place}"));
            }
            object
        })
        .collect();
    Some(Copied {
        objects,
        pipes: places.iter().map(|place| pipes[*place].clone()).collect(),
    })
}

/// Copied objects as clipboard text.
fn text(copied: &Copied) -> Result<String, String> {
    let mut value = json!({ "objects": copied.objects, "pipes": copied.pipes });
    value[MARK] = json!(1);
    serde_json::to_string(&value).map_err(|error| format!("The copy cannot be written: {error}."))
}

/// The copied objects of clipboard text; none when it holds no objects.
fn from_text(text: &str) -> Result<Option<Copied>, String> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Ok(None);
    };
    let objects = match value.get("objects") {
        Some(Value::Array(objects)) if !objects.is_empty() => objects.clone(),
        _ => return Ok(None),
    };
    if value.get(MARK).and_then(Value::as_u64) != Some(1) {
        return Ok(None);
    }
    let objects = serde_json::from_value(Value::Array(objects))
        .map_err(|error| format!("The copied objects do not load: {error}."))?;
    let pipes = match value.get("pipes") {
        Some(pipes @ Value::Array(_)) => serde_json::from_value(pipes.clone())
            .map_err(|error| format!("The copied pipes do not load: {error}."))?,
        _ => Vec::new(),
    };
    Ok(Some(Copied { objects, pipes }))
}

/// The first free id of the form `<type>-<n>` (§3.3 item 6, §5.1).
fn next_id(objects: &[HmiObject], kind: &str) -> String {
    let used: HashSet<&str> = objects.iter().map(|object| object.id.as_str()).collect();
    let mut number = 1_u64;
    loop {
        let id = format!("{kind}-{number}");
        if !used.contains(id.as_str()) {
            return id;
        }
        number += 1;
    }
}

/// A path with its last number moved on by one, its zero padding kept: `Main.P101.Running` to
/// `Main.P102.Running`, `TT009` to `TT010`; none without a number.
fn next_numbered(path: &str) -> Option<String> {
    let rest = path.len() - path.trim_end_matches(|ch: char| !ch.is_ascii_digit()).len();
    let before_rest = &path[..path.len() - rest];
    let digits_start = before_rest
        .trim_end_matches(|ch: char| ch.is_ascii_digit())
        .len();
    let digits = &before_rest[digits_start..];
    if digits.is_empty() {
        return None;
    }
    let mut next: Vec<u8> = digits.bytes().collect();
    let mut carry = true;
    for digit in next.iter_mut().rev() {
        if !carry {
            break;
        }
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            carry = false;
        }
    }
    if carry {
        next.insert(0, b'1');
    }
    let next = String::from_utf8(next).ok()?;
    Some(format!(
        "{}{next}{}",
        &before_rest[..digits_start],
        &path[path.len() - rest..]
    ))
}

/// Bindings moved to their next number where that variable exists.
fn renumbered(
    bind: &BTreeMap<String, HmiBinding>,
    exists: &HashSet<String>,
) -> BTreeMap<String, HmiBinding> {
    bind.iter()
        .map(|(property, binding)| {
            let mut binding = binding.clone();
            if let Some(next) = next_numbered(&binding.path)
                && exists.contains(&next.to_lowercase())
            {
                binding.path = next;
            }
            (property.clone(), binding)
        })
        .collect()
}

/// The page with the copied objects added `step` from where they were copied.
fn pasted(
    mut page: Value,
    copied: Copied,
    step: [i64; 2],
    variables: &[VariableNode],
) -> Result<Value, String> {
    let (mut objects, mut pipes) = parts(&page)?;
    let exists: HashSet<String> = every_variable(variables)
        .into_iter()
        .map(|node| node.path.to_lowercase())
        .collect();
    // Each copied id with the id of its copy, in the order copied.
    let mut renamed: Vec<(String, String)> = Vec::new();
    let first_object = objects.len();
    for object in copied.objects {
        let id = next_id(&objects, &object.kind);
        match renamed.iter_mut().find(|(old, _)| *old == object.id) {
            Some(entry) => entry.1.clone_from(&id),
            None => renamed.push((object.id.clone(), id.clone())),
        }
        let bind = renumbered(&object.bind, &exists);
        objects.push(HmiObject {
            id,
            at: [object.at[0] + step[0], object.at[1] + step[1]],
            bind,
            ..object
        });
    }
    let renamed_to = |old: &str| {
        renamed
            .iter()
            .find(|(from, _)| from == old)
            .map(|(_, new)| new.clone())
    };
    let port = |end: &str| {
        let object = object_of(end);
        let renamed = renamed_to(object).unwrap_or_else(|| object.to_string());
        format!("{renamed}{}", &end[object.len()..])
    };
    let first_pipe = pipes.len();
    let copied_pipes = copied.pipes.len();
    for pipe in copied.pipes {
        pipes.push(HmiPipe {
            from: port(pipe.from.as_str()),
            to: port(pipe.to.as_str()),
            waypoints: pipe
                .waypoints
                .iter()
                .map(|[x, y]| [x + step[0], y + step[1]])
                .collect(),
            bind: renumbered(&pipe.bind, &exists),
            ..pipe
        });
    }
    // An instrument measures what came along with it, or stands free.
    for object in &mut objects[first_object..] {
        let Some(attach) = object.attach.take() else {
            continue;
        };
        object.attach = if attach.starts_with(PIPE_ATTACHMENT) {
            attached_pipe(&attach)
                .filter(|index| *index < copied_pipes)
                .map(|index| format!("{PIPE_ATTACHMENT}{}", first_pipe + index))
        } else {
            renamed_to(attach.as_str())
        };
    }
    let ids: Vec<&str> = renamed.iter().map(|(_, new)| new.as_str()).collect();
    page["object"] = serde_json::to_value(&objects)
        .map_err(|error| format!("The page's objects cannot be written: {error}."))?;
    page["pipe"] = serde_json::to_value(&pipes)
        .map_err(|error| format!("The page's pipes cannot be written: {error}."))?;
    Ok(json!({ "ok": true, "page": page, "ids": ids }))
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
