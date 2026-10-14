//! Deleting objects and naming new ones for the agent's edits (specification 39, §3.3 and §6; port of
//! `deleteObjects` and `nextId` in `editor/editing.ts`, lines 20 and 136-146).

use std::collections::BTreeSet;

use trust_runtime::hmi::page_files::HmiPage;

use super::attachments::moved_attachments;
use super::pipe_rules::without_pipes;

/// The first free id of the form `<kind>-<n>`, counting from 1 (`nextId`, `editing.ts` line 20).
#[must_use]
pub(super) fn next_object_id(page: &HmiPage, kind: &str) -> String {
    let mut number = 1_usize;
    loop {
        let id = format!("{kind}-{number}");
        if page.objects.iter().all(|object| object.id != id) {
            return id;
        }
        number += 1;
    }
}

/// The places of the pipes that join one of `ids`: a pipe whose `from` or `to` port belongs to a
/// named object, whether or not that object is on the page (`deleteObjects`, `editing.ts` line 137).
#[must_use]
pub(super) fn pipes_joined_to(page: &HmiPage, ids: &[String]) -> BTreeSet<usize> {
    page.pipes
        .iter()
        .enumerate()
        .filter(|(_, pipe)| port_belongs_to(&pipe.from, ids) || port_belongs_to(&pipe.to, ids))
        .map(|(index, _)| index)
        .collect()
}

/// Whether the object part of `port` is one of `ids`. The object part is the text before the last
/// dot. A port without a dot has no such text: `slice(0, -1)` of `editing.ts` then drops the last
/// UTF-16 unit, so that prefix is compared unit by unit and matches only what the `TypeScript` matches.
fn port_belongs_to(port: &str, ids: &[String]) -> bool {
    match port.rfind('.') {
        Some(dot) => ids.iter().any(|id| id == &port[..dot]),
        None => {
            let mut units: Vec<u16> = port.encode_utf16().collect();
            units.pop();
            ids.iter()
                .any(|id| id.encode_utf16().eq(units.iter().copied()))
        }
    }
}

/// `deleteObjects` (`editing.ts` lines 136-146): the page without the objects named in `ids`, without
/// the pipes that join them, and with the attachments to either removed (specification 39, §6). A name
/// on no object is ignored, except that the pipes joined to it still go, as in the `TypeScript`.
#[must_use]
pub(super) fn delete_objects(page: &HmiPage, ids: &[String]) -> HmiPage {
    let gone = pipes_joined_to(page, ids);
    let mut kept = without_pipes(page, &gone);
    kept.objects.retain(|object| !ids.contains(&object.id));
    moved_attachments(kept, &gone, ids)
}

#[cfg(test)]
#[path = "object_rules_tests.rs"]
mod tests;
