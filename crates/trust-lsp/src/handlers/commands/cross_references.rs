//! `trust-lsp.crossReferences` (specification 14 §6.3.2): the cross-reference list of the symbol at
//! a position, for a client with or without an editor. `{ uri, position }` answers `{ symbol,
//! references: [{ uri, range, access, pou }] }`, or null on no symbol. trust-ide builds the list;
//! this module carries it.

use serde::Deserialize;
use serde_json::{json, Value};
use text_size::TextSize;
use tower_lsp::lsp_types::{Position, Range, Url};
use trust_hir::access::Access;
use trust_ide::cross_references::{cross_references, CrossReferenceKind};

use crate::handlers::lsp_utils::{offset_to_position, position_to_offset};
use crate::state::ServerState;

#[derive(Deserialize)]
struct Args {
    uri: Url,
    position: Position,
}

/// The answer to `trust-lsp.crossReferences`, null when the position names no symbol.
pub(super) fn value(state: &ServerState, arguments: Vec<Value>) -> Option<Value> {
    let args: Args = arguments
        .into_iter()
        .next()
        .and_then(|argument| serde_json::from_value(argument).ok())?;
    let doc = state.get_document(&args.uri)?;
    let offset = position_to_offset(&doc.content, args.position)?;
    let Some((symbol, entries)) =
        state.with_database(|db| cross_references(db, doc.file_id, TextSize::from(offset)))
    else {
        return Some(Value::Null);
    };
    let references: Vec<Value> = entries
        .into_iter()
        .filter_map(|entry| {
            let target = state.document_for_file_id(entry.file_id)?;
            let range = Range {
                start: offset_to_position(&target.content, entry.range.start().into()),
                end: offset_to_position(&target.content, entry.range.end().into()),
            };
            Some(json!({
                "uri": target.uri,
                "range": range,
                "access": access_name(entry.kind),
                "pou": entry.pou,
            }))
        })
        .collect();
    Some(json!({ "symbol": symbol.as_str(), "references": references }))
}

fn access_name(kind: CrossReferenceKind) -> &'static str {
    match kind {
        CrossReferenceKind::Declaration => "declaration",
        CrossReferenceKind::Use(Access::Read) => "read",
        CrossReferenceKind::Use(Access::Write) => "write",
        CrossReferenceKind::Use(Access::Call) => "call",
    }
}

#[cfg(test)]
#[path = "cross_references_tests.rs"]
mod tests;
