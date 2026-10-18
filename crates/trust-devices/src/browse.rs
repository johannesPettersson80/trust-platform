//! The browse panes in Rust (design §4.3 B8; specification 25 §5.4 and §11.2): open a browse at
//! the runtime that owns the named connection, keep its answer in the session's cache, and add
//! the ticked leaves. This slice covers the OPC UA node browser, the `EtherCAT` channel browser
//! and the picker of the globals a server exposes; the ADS tag browser and OPC UA continuation
//! points are B8b and B8c.
//!
//! - `run`: the I/O of one browse, at the runtime's control endpoint or in the open project's
//!   files, returning typed facts.
//! - `cache`: the session's browse answers, filled by a background thread so `open` answers at
//!   once (design §1.5; specification 25 §5.4 "one configured client connection, not the whole
//!   connection array").
//! - `add`: the `browse.add` action, which builds the plan of its kind from the selection.
//!
//! The view projection is the `panes::browse` namespace.

mod cache;
mod plan;
mod run;

pub(crate) use cache::BrowseCache;
pub(crate) use plan::add;
pub(crate) use run::{Browsed, BrowsedError, BrowsedNode};

use crate::panes::browse;
use crate::request::BrowseRequest;
use crate::session::Session;
use crate::view::types::BrowseView;

/// Opens or refreshes the browse pane that `request` names (design §1.5 and §4.3 B8;
/// specification 25 §5.4): answer at once from the session's cache, run the browse in the
/// background when the cached answer is older, and project the answer into the pane's view. A
/// request that names no browsable connection answers its own failure pane.
#[must_use]
pub fn open(session: &Session, request: &BrowseRequest) -> BrowseView {
    let (resolved, work) = match run::resolve(session, request) {
        Ok(resolved) => resolved,
        Err(error) => return browse::failure(request, *error),
    };
    let answer = session
        .browse()
        .open(&resolved.key, request.seq, move || run::execute(&work));
    browse::view(
        browse::Browse {
            kind: resolved.kind,
            protocol: &resolved.protocol,
            name: &resolved.name,
            params: &resolved.params,
            runtime: resolved.runtime.as_ref(),
            request,
        },
        answer.as_ref(),
    )
}

#[cfg(test)]
mod tests;
