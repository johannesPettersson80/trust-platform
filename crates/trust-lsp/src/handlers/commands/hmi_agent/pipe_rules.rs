//! The builder's pipe rules for the agent's edits (specification 39, §6; port of `addPipe`,
//! `deletePipes` and `pipeIndex` in `editor/pipes.ts`, lines 1-36).
//!
//! A pipe is named `pipe:<n>` by its place among the page's pipes. The bend, branch, split and
//! hit-test functions of `pipes.ts` are canvas interaction and stay in `TypeScript`.

use std::collections::{BTreeMap, BTreeSet};

use trust_runtime::hmi::page_files::{HmiPage, HmiPipe, HmiPipeStyle};

use super::attachments::moved_attachments;

/// The name of the pipe at `index` among the page's pipes, `pipe:<n>`.
#[must_use]
pub(super) fn pipe_name(index: usize) -> String {
    format!("pipe:{index}")
}

/// The place of the pipe named `id`: `pipe:<n>` with ASCII digits only, the expression
/// `^pipe:(\d+)$` of `pipeIndex` (`pipes.ts` line 30). A number that does not fit in `usize` names
/// no pipe of any page, so it has no place here.
#[must_use]
pub(super) fn pipe_index(id: &str) -> Option<usize> {
    let digits = id.strip_prefix("pipe:")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The page with the pipes at the places in `gone` removed; the other pipes keep their order.
#[must_use]
pub(super) fn without_pipes(page: &HmiPage, gone: &BTreeSet<usize>) -> HmiPage {
    let mut next = page.clone();
    next.pipes = page
        .pipes
        .iter()
        .enumerate()
        .filter(|(index, _)| !gone.contains(index))
        .map(|(_, pipe)| pipe.clone())
        .collect();
    next
}

/// `addPipe` (`pipes.ts` line 15): the page with a pipe from `from` to `to` appended, or `None` where
/// the builder leaves the page as it is. That is the case when the two ports are the same, or when
/// either port already has a pipe: a pipe joins two different ports that have no pipe yet
/// (specification 39, §6). The appended pipe has the defaults of the object `{ from, to }`: a process
/// line, no bends and no bindings.
#[must_use]
pub(super) fn add_pipe(page: &HmiPage, from: &str, to: &str) -> Option<HmiPage> {
    if from == to {
        return None;
    }
    let has_pipe = |port: &str| {
        page.pipes
            .iter()
            .any(|pipe| pipe.from == port || pipe.to == port)
    };
    if has_pipe(from) || has_pipe(to) {
        return None;
    }
    let mut next = page.clone();
    next.pipes.push(HmiPipe {
        from: from.to_string(),
        to: to.to_string(),
        style: HmiPipeStyle::default(),
        waypoints: Vec::new(),
        bind: BTreeMap::new(),
    });
    Some(next)
}

/// `deletePipes` (`pipes.ts` line 23): the page without the pipes named in `ids`. The attachments to
/// those pipes go, and the attachments to later pipes follow their pipes (specification 39, §6
/// item 5). Names that are not `pipe:<n>` are ignored, and a page with no pipe to delete comes back
/// unchanged. A well-formed name beyond the page's pipes still counts, as in the `TypeScript`: it
/// removes the attachments that name that place.
#[must_use]
pub(super) fn delete_pipes(page: &HmiPage, ids: &[String]) -> HmiPage {
    let gone: BTreeSet<usize> = ids
        .iter()
        .map(String::as_str)
        .filter_map(pipe_index)
        .collect();
    if gone.is_empty() {
        return page.clone();
    }
    moved_attachments(without_pipes(page, &gone), &gone, &[])
}

#[cfg(test)]
#[path = "pipe_rules_tests.rs"]
mod tests;
