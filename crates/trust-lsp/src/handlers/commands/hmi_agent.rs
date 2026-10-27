//! The HMI agent's authoring engine in Rust (specification 43, §3 and §6; checklist H4).
//!
//! The HMI agent's authoring engine in Rust (specification 43, §3 to §6; checklist H4).
//!
//! The composition (`patterns`, `layout`, `compose`, `pattern_references`, `intent`), the page edits
//! (`edits`, `geometric_edits`, `pipe_edits`, `review`), the builder's editing rules they use
//! (`attachments`, `object_rules`, `pipe_rules`, `align`, `clipboard`), the quality checks (`adaptive`,
//! `ports`, `routes`, `geometry_quality`, `routing_quality`), the draft store (`documents`,
//! `draft_records`, `drafts`, `review_record`), the request boundary (`boundary`), the tool input schemas
//! (`schema`), the command argument (`argument`), the existing HMI commands it calls (`native`), the
//! context, library and inspection answers (`queries`, `query_metadata`), and the operations of the
//! authoring commands (`service`, `review_apply`).
//!
//! The extension keeps what presents and wires: the rendering and preview (AT-07, AT-08) and the editor
//! transaction (AT-09). The editor's quality warnings are decided by `trust-lsp.hmiQuality`
//! (`hmi_quality`); the extension adds only the texts its drawing cuts. Whether a simulator journey's
//! target is a local simulator is decided by `trust-lsp.hmiAgent.checkTarget` (`target`).

use serde_json::Value;

use crate::state::ServerState;

mod argument;
mod native;
mod queries;
mod query_metadata;
mod review_apply;
mod schema;
mod service;
mod target;

/// Whether `name` is an HMI agent command: an authoring command or the simulator target check.
#[must_use]
pub(super) fn is_command(name: &str) -> bool {
    service::is_command(name) || name == target::CHECK_TARGET
}

/// Runs the HMI agent command `name` on its argument, or `None` when it is not one.
#[must_use]
pub(super) fn execute(state: &ServerState, name: &str, arguments: Vec<Value>) -> Option<Value> {
    if name == target::CHECK_TARGET {
        return Some(target::check_target(&arguments));
    }
    service::execute(state, name, arguments)
}

mod error;
mod geometry;

mod align;
mod attachments;
mod clipboard;
mod object_rules;
mod pipe_rules;

mod adaptive;
mod geometry_quality;
mod ports;
mod routes;
mod routing_quality;

mod documents;
mod draft_records;
mod drafts;
mod review_record;

mod compose;
mod edits;
mod geometric_edits;
mod intent;
mod layout;
mod pattern_references;
mod patterns;
mod pipe_edits;
mod review;

mod boundary;
