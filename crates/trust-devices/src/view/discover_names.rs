//! The names that the Discover pane shows (specification 25 §2): the ten sources and the protocols that a runtime
//! advertises beyond them, by the name each one shows, and the texts of a search that failed at a runtime. These
//! are presentation facts that the view reads beside one search status.

use std::collections::BTreeMap;

use trust_runtime::search::SourceId;

use super::discover::Scan;
use super::discover_context::Context;
use crate::texts::discover as texts;

/// The text of a search that failed, as the old pane gave the same failure (specification 25 §2), or
/// `None` for a failure that the old pane had no text for. `message` is the failure's own text: the old
/// pane told an authentication failure and a timeout apart by it (`discoveryErrors.ts`).
#[must_use]
pub(crate) fn failure_text(code: &str, message: &str) -> Option<String> {
    match code {
        "unknown_runtime" | "runtime_without_endpoint" => {
            Some(texts::NO_LONGER_REACHABLE.to_owned())
        }
        "runtime_unreachable" => Some(runtime_failure_text(texts::RUNTIME_DISCOVERY_NAME, message)),
        _ => None,
    }
}

/// The text of a request to a runtime that failed, for the discovery named `name` (specification 25 §2). The old
/// pane told an authentication failure and a timeout apart by the failure's own text (`discoveryErrors.ts`).
#[must_use]
pub(crate) fn runtime_failure_text(name: &str, message: &str) -> String {
    let detail = message.to_lowercase();
    if detail.contains("auth") {
        texts::rejected_authentication(name)
    } else if detail.contains("timeout") || detail.contains("timed out") {
        texts::discovery_timed_out(name)
    } else {
        texts::could_not_complete(name)
    }
}

/// The source of `name`, or `None` for a name that this build does not know.
pub(crate) fn known_source(name: &str) -> Option<SourceId> {
    SourceId::ALL
        .into_iter()
        .find(|source| source.as_str() == name)
}

/// The protocols that the origin runtime of `scan` advertises beyond the ten sources, by ID, with the label each
/// advertises (specification 25 §2). A search on this computer advertises none.
pub(crate) fn advertised_labels(scan: &Scan, ctx: &Context) -> BTreeMap<String, String> {
    let Scan::Runtime(id) = scan else {
        return BTreeMap::new();
    };
    ctx.origin(id)
        .and_then(|facts| facts.discover.as_ref())
        .map(|discover| {
            discover
                .beyond
                .iter()
                .map(|advertised| (advertised.id.clone(), advertised.label.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `name` is one of the ten sources, or a protocol that the origin runtime advertises (specification 25
/// §2). A name of neither is left out of the view.
pub(crate) fn known_name(name: &str, advertised: &BTreeMap<String, String>) -> bool {
    known_source(name).is_some() || advertised.contains_key(name)
}

/// The name that a source or an advertised protocol shows: its source name, or the label its runtime advertises.
pub(crate) fn label_of(name: &str, advertised: &BTreeMap<String, String>) -> String {
    advertised
        .get(name)
        .cloned()
        .unwrap_or_else(|| texts::source_name(name))
}
