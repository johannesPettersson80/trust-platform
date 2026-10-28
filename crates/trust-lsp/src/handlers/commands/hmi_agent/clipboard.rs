//! The duplicate of the agent's edits (specification 43, AT-05 and repair 4; port of `duplicateObjects`
//! in `agent/pipeEdits.ts`, lines 89-124, with the copy and paste rules of `editor/clipboard.ts` that it
//! uses: `copyText`, `pasted` and `objectOf`).
//!
//! The builder's copy travels through the clipboard as text. That transport belongs to the editor and is
//! not ported: the rules the text carries are applied to the page directly. The agent's duplicate pastes
//! with no program variables (the `TypeScript` passes an empty set), so the bindings of a copy stay as they
//! are. `PASTE_STEP` is not ported, because only the canvas's own paste reads it.

use rustc_hash::FxHashSet;
use trust_runtime::hmi::page_files::{HmiObject, HmiPage, HmiPipe};

use super::error::{AgentError, AgentResult};
use super::object_rules::next_object_id;
use super::pipe_rules::{pipe_index, pipe_name};

/// The objects of a copy and the pipes between them, as `copyText` writes them.
struct Copied {
    /// The selected objects in the page's order. An instrument whose pipe came along names it by its
    /// place among the copied pipes; an instrument whose pipe did not come along stands free.
    objects: Vec<HmiObject>,
    /// The pipes whose two ends are both copied objects, in the page's order.
    pipes: Vec<HmiPipe>,
}

/// The result of a paste: the page with the copies appended, and the id each original's copy was given,
/// in the order of the originals.
struct Pasted {
    page: HmiPage,
    renamed: Vec<(String, String)>,
}

/// The object a port `<object id>.<port>` belongs to: the text before its last dot, or the whole port
/// when it has no dot (`objectOf`, `clipboard.ts` line 23).
fn object_of(port: &str) -> &str {
    port.rfind('.').map_or(port, |dot| &port[..dot])
}

/// An instrument's `attach` as `copyText` writes it (`located`, `clipboard.ts` lines 40-46): a pipe is
/// named by its place among the copied pipes, and a pipe that did not come along is dropped. An
/// attachment to equipment is kept here; the paste decides it.
fn located(object: &HmiObject, places: &[usize]) -> HmiObject {
    let mut copy = object.clone();
    if let Some(attach) = object
        .attach
        .as_deref()
        .filter(|attach| attach.starts_with("pipe:"))
    {
        copy.attach = pipe_index(attach)
            .and_then(|index| places.iter().position(|&place| place == index))
            .map(pipe_name);
    }
    copy
}

/// `copyText` (`clipboard.ts` line 29), without the text: the objects named in `ids`, in the page's
/// order, and the pipes whose two ends are both copied objects, in the page's order.
fn copy_selection(page: &HmiPage, ids: &[String]) -> Copied {
    let selected: Vec<&HmiObject> = page
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .collect();
    let copied: FxHashSet<&str> = selected.iter().map(|&object| object.id.as_str()).collect();
    let mut places = Vec::new();
    let mut pipes = Vec::new();
    for (index, pipe) in page.pipes.iter().enumerate() {
        if copied.contains(object_of(&pipe.from)) && copied.contains(object_of(&pipe.to)) {
            places.push(index);
            pipes.push(pipe.clone());
        }
    }
    let objects = selected
        .into_iter()
        .map(|object| located(object, &places))
        .collect();
    Copied { objects, pipes }
}

/// The copy of an offset value: `value` moved by `step`, as `pasted` adds the step to each position. The
/// `TypeScript` numbers do not overflow here; a sum outside the range of a page coordinate is refused.
fn offset(value: i64, step: i64) -> AgentResult<i64> {
    value.checked_add(step).ok_or_else(|| {
        AgentError::invalid(format!(
            "the duplicate offset {step} moves {value} beyond the range of a page coordinate; choose a smaller delta."
        ))
    })
}

/// The id a pasted original's copy was given, when that original was copied (`renamed.get` in `pasted`).
fn copy_id<'a>(renamed: &'a [(String, String)], original: &str) -> Option<&'a str> {
    renamed
        .iter()
        .find(|(from, _)| from == original)
        .map(|(_, copy)| copy.as_str())
}

/// `pipe:<first + n>`: the place on the page of a copied pipe that an instrument names by its place
/// among the copied pipes (`copiedPipeIndex`, `clipboard.ts` line 137). `None` for a place that is not
/// one of the copied pipes.
fn copied_pipe_place(attach: &str, copied_pipes: usize, first_pipe: usize) -> Option<String> {
    let index = pipe_index(attach)?;
    (index < copied_pipes).then(|| pipe_name(first_pipe + index))
}

/// A port of a copied pipe on the page (`port` in `pasted`, `clipboard.ts` line 111): the copy of its
/// object when that object was copied, else the object itself, with the port name kept. A port without a
/// dot names its object as a whole.
fn port_on_copy(port: &str, renamed: &[(String, String)]) -> String {
    let object = object_of(port);
    format!(
        "{}{}",
        copy_id(renamed, object).unwrap_or(object),
        &port[object.len()..]
    )
}

/// `pasted` (`clipboard.ts` line 83), with no program variables: the copies, moved by `step`, each under
/// the next free id of its kind; the copied pipes after the page's own pipes, with their ends on the
/// copies and their bends moved by `step`; and each instrument's attachment kept only where what it
/// measures came along too.
fn paste(page: &HmiPage, copied: &Copied, step: [i64; 2]) -> AgentResult<Pasted> {
    let mut next = page.clone();
    let mut renamed: Vec<(String, String)> = Vec::new();
    for object in &copied.objects {
        let id = next_object_id(&next, &object.kind);
        let mut copy = object.clone();
        copy.id = id.clone();
        copy.at = [
            offset(object.at[0], step[0])?,
            offset(object.at[1], step[1])?,
        ];
        next.objects.push(copy);
        renamed.push((object.id.clone(), id));
    }
    let first_pipe = page.pipes.len();
    let mut pasted_pipes = Vec::with_capacity(copied.pipes.len());
    for pipe in &copied.pipes {
        let mut copy = pipe.clone();
        copy.from = port_on_copy(&pipe.from, &renamed);
        copy.to = port_on_copy(&pipe.to, &renamed);
        copy.waypoints = pipe
            .waypoints
            .iter()
            .map(|&[x, y]| -> AgentResult<[i64; 2]> {
                Ok([offset(x, step[0])?, offset(y, step[1])?])
            })
            .collect::<AgentResult<Vec<[i64; 2]>>>()?;
        pasted_pipes.push(copy);
    }
    for object in next.objects.iter_mut().skip(page.objects.len()) {
        let Some(attach) = object.attach.take() else {
            continue;
        };
        object.attach = if attach.starts_with("pipe:") {
            copied_pipe_place(&attach, copied.pipes.len(), first_pipe)
        } else {
            copy_id(&renamed, &attach).map(str::to_string)
        };
    }
    next.pipes.extend(pasted_pipes);
    Ok(Pasted {
        page: next,
        renamed,
    })
}

/// Whether the agent chose an id for the pasted id `id` (`chosen.has` in `duplicateObjects`). A pasted id
/// can be listed with no chosen id, when the agent gave fewer ids than there are objects.
fn has_choice(chosen: &[(String, Option<String>)], id: &str) -> bool {
    chosen.iter().any(|(key, _)| key == id)
}

/// The id a pasted id is renamed to (`rename` in `duplicateObjects`): the id the agent chose for it, or
/// the pasted id itself when no id was chosen.
fn rename<'a>(chosen: &'a [(String, Option<String>)], id: &'a str) -> &'a str {
    chosen
        .iter()
        .find(|(key, _)| key == id)
        .and_then(|(_, choice)| choice.as_deref())
        .unwrap_or(id)
}

/// A port of a copied pipe after the agent's ids are applied (`end` in `duplicateObjects`): the object
/// part renamed, the port name kept. A port without a dot is left as it is.
fn renamed_port(port: &str, chosen: &[(String, Option<String>)]) -> String {
    match port.rfind('.') {
        None => port.to_string(),
        Some(dot) => format!("{}{}", rename(chosen, &port[..dot]), &port[dot..]),
    }
}

/// `duplicateObjects` (`agent/pipeEdits.ts` line 92): copies of the objects named in `ids`, moved by
/// `delta`, each copy taking the id the agent chose for its original. `new_ids` pair with `ids` by
/// position: the copy of `ids[n]` takes `new_ids[n]` (specification 43, repair 4). The copies follow the
/// builder's copy and paste (specification 39, §3.3 item 8): the pipes between copied objects come along
/// after the page's pipes, an instrument keeps what it measures only when that came along too, the bends
/// of the copied pipes move with the copies, and bindings are unchanged.
///
/// # Errors
///
/// `INVALID_ARGUMENT` when no name in `ids` is an object of the page, or when a copy would move past the
/// range of a page coordinate.
pub(super) fn duplicate_objects(
    page: &HmiPage,
    ids: &[String],
    new_ids: &[String],
    delta: [i64; 2],
) -> AgentResult<HmiPage> {
    let copied = copy_selection(page, ids);
    if copied.objects.is_empty() {
        return Err(AgentError::invalid(
            "provide objects on this page to duplicate.",
        ));
    }
    let Pasted {
        page: mut result,
        renamed,
    } = paste(page, &copied, delta)?;
    let chosen: Vec<(String, Option<String>)> = renamed
        .iter()
        .map(|(original, copy)| {
            let position = ids.iter().position(|id| id == original);
            (
                copy.clone(),
                position.and_then(|at| new_ids.get(at)).cloned(),
            )
        })
        .collect();
    for object in &mut result.objects {
        if !has_choice(&chosen, &object.id) {
            continue;
        }
        let id = rename(&chosen, &object.id).to_string();
        let attach = object
            .attach
            .as_deref()
            .filter(|attach| has_choice(&chosen, attach))
            .map(|attach| rename(&chosen, attach).to_string());
        object.id = id;
        if let Some(attach) = attach {
            object.attach = Some(attach);
        }
    }
    for pipe in result.pipes.iter_mut().skip(page.pipes.len()) {
        pipe.from = renamed_port(&pipe.from, &chosen);
        pipe.to = renamed_port(&pipe.to, &chosen);
    }
    Ok(result)
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
