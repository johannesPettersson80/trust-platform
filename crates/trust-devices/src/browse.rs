//! The browse panes in Rust (design §4.3 B8; specification 25 §5.4 and §11.2): open a browse at
//! the runtime that owns the named connection, keep its answer in the session's cache, and add
//! the ticked leaves. This slice covers the OPC UA node browser, the `EtherCAT` channel browser,
//! the picker of the globals a server exposes, and the ADS tag browser of B8b1; the ADS tag import
//! (`ads.tags_set`) is B8b2 and OPC UA continuation points are B8c.
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
pub(crate) mod port;
mod run;

pub(crate) use cache::BrowseCache;
pub(crate) use plan::add;
pub(crate) use port::typed_port;
pub(crate) use run::{target_port, Browsed, BrowsedError, BrowsedNode};

use crate::panes::browse::{self, CachedPort};
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
        Err(unresolved) => return browse::failure(request, unresolved.kind, *unresolved.error),
    };
    let answer = session
        .browse()
        .open(&resolved.key, request.seq, move || run::execute(&work));
    let cached = cached_ports(session, &resolved);
    browse::view(
        browse::Browse {
            kind: resolved.kind,
            protocol: &resolved.protocol,
            name: &resolved.name,
            params: &resolved.params,
            runtime: resolved.runtime.as_ref(),
            port: resolved.port,
            request,
            cached: &cached,
        },
        answer.as_ref(),
    )
}

/// The other ADS ports of this connection that the session has browsed (design §3.3 J55): the card
/// of a port that `browse.port` started appears on the pane's next poll, as the old panel's merged
/// ports did (`W/AdsMultiPortTagBrowser.tsx:275-318`). Other kinds show no other port.
fn cached_ports(session: &Session, resolved: &run::Resolved) -> Vec<CachedPort> {
    if resolved.kind != crate::view::types::BrowseKind::AdsTags {
        return Vec::new();
    }
    let Some((prefix, _port)) = resolved.key.rsplit_once('|') else {
        return Vec::new();
    };
    session.browse().answers_under(&format!("{prefix}|"))
}

#[cfg(test)]
mod tests;
