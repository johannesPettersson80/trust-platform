//! Instrument attachments in the builder (specification 39, §6 item 5; port of the parts of
//! `editor/attachments.ts` that the agent's deletions use: `afterDeleting` and `movedAttachments`).
//!
//! An instrument's `attach` names an equipment by its id, or a pipe by its place, `pipe:<n>`. When
//! pipes are deleted the places move: an attachment follows its pipe, and goes when its pipe or its
//! equipment goes. The `TypeScript`'s `withAttach` only rebuilds an object without its `attach` key;
//! here that is a plain assignment.

use std::collections::BTreeSet;

use trust_runtime::hmi::page_files::HmiPage;

use super::pipe_rules::{pipe_index, pipe_name};

/// The place a pipe keeps once the pipes in `deleted` are gone, or `None` for a deleted pipe
/// (`afterDeleting`, `attachments.ts` line 38): a pipe after deleted pipes moves up by the number of
/// deleted pipes before it.
#[must_use]
fn remap_after_deleting_pipes(deleted: &BTreeSet<usize>, index: usize) -> Option<usize> {
    if deleted.contains(&index) {
        return None;
    }
    Some(index - deleted.range(..index).count())
}

/// `movedAttachments` (`attachments.ts` line 9): the page with each attachment to a pipe moved to that
/// pipe's new place, and with each attachment removed whose pipe is in `deleted_pipes` or whose
/// equipment is in `deleted_objects`. Other attachments stay as written. A `pipe:<n>` attachment is
/// always read as a pipe, as in the `TypeScript`, and a pipe name written with leading zeros comes back
/// in its canonical form.
#[must_use]
pub(super) fn moved_attachments(
    mut page: HmiPage,
    deleted_pipes: &BTreeSet<usize>,
    deleted_objects: &[String],
) -> HmiPage {
    for object in &mut page.objects {
        let Some(attach) = object.attach.as_deref() else {
            continue;
        };
        let next = match pipe_index(attach) {
            Some(index) => remap_after_deleting_pipes(deleted_pipes, index).map(pipe_name),
            None if deleted_objects.iter().any(|id| id == attach) => None,
            None => Some(attach.to_string()),
        };
        if next.as_deref() != Some(attach) {
            object.attach = next;
        }
    }
    page
}

#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;
