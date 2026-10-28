//! A page and its P&ID kept one, and a process screen from a P&ID (specification 39, §18 phase
//! 4 items 1 to 3; row H8). A P&ID is `hmi/pid/<name>.toml`; a page names its P&ID with
//! `pid = "<name>"`. A process object with the same id in both is the same object: its type,
//! variant, tag and bindings, whether it exists, and the pipes between such objects are one in
//! both views. Positions, sizes and bends are each view's own; texts, lines, displays and
//! controls belong to their view alone. The extension reads and writes the files; it decides
//! nothing.
//!
//! Files are named by their path in the project, with `/` between the parts.
//!
//! - `linked { file, model? }` → `{ pid, files, pattern? }`: whether the file is a P&ID, and
//!   the files that may share its process objects: a page's P&ID, or for a P&ID the pages that
//!   `pattern` finds (the pages naming it are among them).
//! - `sync { file, model, size, linked }` → `{ writes }`: after a change of `file` to `model`,
//!   the new text of each of the `linked` files (`{ file, text }`) that shares its process
//!   objects and has changed. Both views are drawn at the screen's design size `size`.
//! - `processScreen { file, text, size }` → `{ name, file, text }`: Generate Process Screen from
//!   P&ID, the page linked to the P&ID `file`, with its title, its process objects and their
//!   pipes, placed as on the P&ID scaled to the screen's size and bound as on the P&ID.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{
    parse_page, write_page, HmiObject, HmiPage, HmiPipe, PageFileError,
};
use trust_runtime::hmi::symbol_geometry::symbol_shape;

use super::clipboard::listed;
use super::decode;

/// The folder of the project's pages, which a P&ID's linked pages are among.
const PAGES: &str = "hmi/pages/*.toml";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Request {
    Linked {
        file: String,
        #[serde(default)]
        model: Option<Value>,
    },
    Sync {
        file: String,
        model: Value,
        size: [u32; 2],
        #[serde(default)]
        linked: Vec<FileText>,
    },
    ProcessScreen {
        file: String,
        text: String,
        size: [u32; 2],
    },
}

/// A file of the project with its text, unsaved changes included.
#[derive(Deserialize)]
struct FileText {
    file: String,
    text: String,
}

/// The answer of `trust-lsp.hmiPidSync`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Request>(argument)? {
        Request::Linked { file, model } => Ok(match pid_name(&file) {
            Some(_) => json!({ "ok": true, "pid": true, "files": [], "pattern": PAGES }),
            None => {
                let files: Vec<String> = model
                    .as_ref()
                    .and_then(page_pid)
                    .map(|pid| vec![pid_file(&pid)])
                    .unwrap_or_default();
                json!({ "ok": true, "pid": false, "files": files })
            }
        }),
        Request::Sync {
            file,
            model,
            size,
            linked,
        } => {
            check_size(size)?;
            let (objects, pipes) = parts(&model)?;
            let name = pid_name(&file);
            let own_pid = page_pid(&model).map(|pid| pid_file(&pid));
            let mut writes = Vec::new();
            for candidate in linked {
                if candidate.file == file {
                    continue;
                }
                // A page's P&ID, or for a P&ID a page that names it.
                let shares = match &name {
                    Some(_) => is_page(&candidate.file),
                    None => own_pid.as_deref() == Some(candidate.file.as_str()),
                };
                if !shares {
                    continue;
                }
                let Ok(target) = parse_page(&candidate.file, &candidate.text) else {
                    continue;
                };
                if name.is_some() && target.pid != name {
                    continue;
                }
                let Some(next) = linked_update(&objects, &pipes, size, &target, size) else {
                    continue;
                };
                if let Ok(text) = written(&candidate.file, &next) {
                    writes.push(json!({ "file": candidate.file, "text": text }));
                }
            }
            Ok(json!({ "ok": true, "writes": writes }))
        }
        Request::ProcessScreen { file, text, size } => process_screen(&file, &text, size),
    }
}

/// The name of a P&ID, `hmi/pid/<name>.toml`; none for any other file.
fn pid_name(file: &str) -> Option<String> {
    let parts: Vec<&str> = file.split(['/', '\\']).collect();
    match parts.as_slice() {
        [.., "hmi", "pid", last] => Some(last.strip_suffix(".toml").unwrap_or(*last).to_string()),
        _ => None,
    }
}

/// Whether a file is a page, `hmi/pages/<name>.toml`.
fn is_page(file: &str) -> bool {
    let parts: Vec<&str> = file.split(['/', '\\']).collect();
    matches!(parts.as_slice(), [.., "hmi", "pages", _])
}

/// A P&ID's file.
fn pid_file(name: &str) -> String {
    format!("hmi/pid/{name}.toml")
}

/// The P&ID a page model names.
fn page_pid(model: &Value) -> Option<String> {
    model["pid"]
        .as_str()
        .filter(|pid| !pid.is_empty())
        .map(str::to_string)
}

/// The objects and pipes of an edited model, of whatever kind of file it is.
fn parts(model: &Value) -> Result<(Vec<HmiObject>, Vec<HmiPipe>), String> {
    let objects = listed(model, "object")
        .map_err(|error| format!("The model's objects do not load: {error}."))?;
    let pipes = listed(model, "pipe")
        .map_err(|error| format!("The model's pipes do not load: {error}."))?;
    Ok((objects, pipes))
}

/// Both views have the screen's design size; a size without area scales nothing.
fn check_size(size: [u32; 2]) -> Result<(), String> {
    if size.contains(&0) {
        return Err(format!(
            "The screen's size {} × {} has no area.",
            size[0], size[1]
        ));
    }
    Ok(())
}

/// The text of a page, which loads again as it is.
fn written(file: &str, page: &HmiPage) -> Result<String, Vec<PageFileError>> {
    let text = write_page(page);
    parse_page(file, &text)?;
    Ok(text)
}

/// Whether an object is a process object, which a page and its P&ID share: equipment, an
/// instrument, a junction or one of the project's own symbols (`my:<name>`) — a type with a
/// shipped symbol (the symbol table the editor's drawing reads, §6 item 1) or a symbol of the
/// project's.
fn is_process_object(kind: &str) -> bool {
    kind == "junction" || kind.starts_with("my:") || symbol_shape(kind).is_some()
}

/// The object a pipe end `<object id>.<port>` belongs to.
fn object_of(end: &str) -> &str {
    end.rsplit_once('.').map_or(end, |(object, _)| object)
}

/// A coordinate scaled from one view's size to another's, to the nearest whole pixel (halves
/// upward).
fn scaled(at: i64, from: u32, to: u32) -> i64 {
    ((at as f64) * f64::from(to) / f64::from(from) + 0.5).floor() as i64
}

/// The target with the source's process objects and the pipes between them; none when it has
/// them already. An object new to the target comes at its place in the source, scaled from the
/// source's size to the target's; an object deleted in the source leaves the target. A pipe keeps
/// the target's own bends; one new to the target comes without bends.
fn linked_update(
    source_objects: &[HmiObject],
    source_pipes: &[HmiPipe],
    source_size: [u32; 2],
    target: &HmiPage,
    target_size: [u32; 2],
) -> Option<HmiPage> {
    // The source's process objects by id, in the order first seen.
    let mut shared: Vec<&HmiObject> = Vec::new();
    for object in source_objects
        .iter()
        .filter(|object| is_process_object(&object.kind))
    {
        match shared.iter_mut().find(|known| known.id == object.id) {
            Some(known) => *known = object,
            None => shared.push(object),
        }
    }
    let shared_by = |id: &str| shared.iter().copied().find(|object| object.id == id);
    let mut objects = Vec::new();
    let mut present = HashSet::new();
    for object in &target.objects {
        if !is_process_object(&object.kind) {
            objects.push(object.clone());
            continue;
        }
        let Some(from) = shared_by(object.id.as_str()) else {
            continue;
        };
        present.insert(object.id.as_str());
        objects.push(HmiObject {
            kind: from.kind.clone(),
            variant: from.variant.clone(),
            tag: from.tag.clone(),
            bind: from.bind.clone(),
            ..object.clone()
        });
    }
    for from in &shared {
        if !present.contains(from.id.as_str()) {
            objects.push(HmiObject {
                at: [
                    scaled(from.at[0], source_size[0], target_size[0]),
                    scaled(from.at[1], source_size[1], target_size[1]),
                ],
                ..(*from).clone()
            });
        }
    }
    let pipes = {
        let linked: HashSet<&str> = objects
            .iter()
            .filter(|object| is_process_object(&object.kind))
            .map(|object| object.id.as_str())
            .collect();
        let between = |pipe: &HmiPipe| {
            linked.contains(object_of(&pipe.from)) && linked.contains(object_of(&pipe.to))
        };
        let own: Vec<&HmiPipe> = target.pipes.iter().filter(|&pipe| between(pipe)).collect();
        let mut pipes: Vec<HmiPipe> = source_pipes
            .iter()
            .filter(|&pipe| between(pipe))
            .map(|pipe| HmiPipe {
                waypoints: own
                    .iter()
                    .find(|candidate| candidate.from == pipe.from && candidate.to == pipe.to)
                    .map(|candidate| candidate.waypoints.clone())
                    .unwrap_or_default(),
                ..pipe.clone()
            })
            .collect();
        pipes.extend(target.pipes.iter().filter(|&pipe| !between(pipe)).cloned());
        pipes
    };
    let next = HmiPage {
        objects,
        pipes,
        ..target.clone()
    };
    (next != *target).then_some(next)
}

/// `processScreen`: the page linked to a P&ID.
fn process_screen(file: &str, text: &str, size: [u32; 2]) -> Result<Value, String> {
    let name = pid_name(file)
        .ok_or("Open a P&ID (hmi/pid/<name>.toml) to generate its process screen.")?;
    check_size(size)?;
    let pid = parse_page(file, text).map_err(|errors| {
        format!(
            "The P&ID does not load: {}",
            errors
                .iter()
                .map(|error| format!("line {}: {}", error.line, error.message))
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    let empty = HmiPage {
        regions: Vec::new(),
        title: pid.title.clone(),
        order: None,
        icon: None,
        group: None,
        hidden: false,
        incomplete: false,
        pid: Some(name.clone()),
        objects: Vec::new(),
        pipes: Vec::new(),
    };
    // The P&ID is drawn at the screen's size too, so its places carry over as they are.
    let page = linked_update(&pid.objects, &pid.pipes, size, &empty, size).unwrap_or(empty);
    let file = format!("hmi/pages/{name}.toml");
    let text = written(&file, &page).map_err(|errors| {
        format!(
            "The process screen cannot be written: {}",
            errors
                .iter()
                .map(|error| error.message.clone())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    Ok(json!({ "ok": true, "name": name, "file": file, "text": text }))
}

#[cfg(test)]
#[path = "pid_tests.rs"]
mod tests;
