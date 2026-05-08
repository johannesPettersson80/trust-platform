//! Document synchronization handlers.

use tower_lsp::lsp_types::*;
use tower_lsp::Client;
use tracing::{info, warn};

use crate::state::ServerState;

use super::lsp_utils::position_to_offset;
use super::refresh::refresh_diagnostics;

pub async fn did_open(client: &Client, state: &ServerState, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    let content = params.text_document.text;

    info!("Document opened: {}", uri);
    state.record_activity();

    state.open_document(uri.clone(), version, content.clone());

    // Push-based clients need a workspace refresh here so dependent open files do not keep
    // stale cross-file diagnostics. Wait for the first workspace-index pass first; clients
    // that don't honor `workspace/diagnostic/refresh` (e.g. Claude Code's LSP tool) latch
    // onto the first publish, so it must already reflect cross-file type resolution.
    if !state.use_pull_diagnostics() {
        state.wait_for_index_first_pass().await;
        refresh_diagnostics(client, state).await;
    }
}

pub async fn did_change(client: &Client, state: &ServerState, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;

    info!("Document changed: {}", uri);
    state.record_activity();

    if params.content_changes.is_empty() {
        return;
    }

    let doc = match state.get_document(&uri) {
        Some(doc) => doc,
        None => {
            warn!("Received change for unknown document: {}", uri);
            return;
        }
    };

    let Some(updated) = apply_content_changes(&doc.content, &params.content_changes) else {
        warn!("Failed to apply incremental changes for {}", uri);
        return;
    };

    state.update_document(&uri, version, updated);

    // Push-based clients need a workspace refresh here so dependent open files do not keep
    // stale cross-file diagnostics.
    if !state.use_pull_diagnostics() {
        refresh_diagnostics(client, state).await;
    }
}

fn apply_content_changes(
    content: &str,
    changes: &[TextDocumentContentChangeEvent],
) -> Option<String> {
    let mut updated = content.to_string();
    for change in changes {
        if let Some(range) = change.range {
            let start = position_to_offset(&updated, range.start)? as usize;
            let end = position_to_offset(&updated, range.end)? as usize;
            if start > end || end > updated.len() {
                return None;
            }
            let mut next = String::with_capacity(
                updated.len().saturating_sub(end.saturating_sub(start)) + change.text.len(),
            );
            next.push_str(&updated[..start]);
            next.push_str(&change.text);
            next.push_str(&updated[end..]);
            updated = next;
        } else {
            updated = change.text.clone();
        }
    }
    Some(updated)
}

pub async fn did_save(client: &Client, state: &ServerState, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;
    info!("Document saved: {}", uri);
    state.record_activity();

    // Push-based clients need a workspace refresh here so dependent open files do not keep
    // stale cross-file diagnostics.
    if !state.use_pull_diagnostics() {
        refresh_diagnostics(client, state).await;
    }
}

pub async fn did_close(client: &Client, state: &ServerState, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    info!("Document closed: {}", uri);
    state.record_activity();

    state.close_document(&uri);

    // Clear diagnostics only for push-based clients.
    if !state.use_pull_diagnostics() {
        client.publish_diagnostics(uri, vec![], None).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{Position, Range};

    #[test]
    fn apply_content_changes_inserts_text() {
        let original = "PROGRAM Test\nEND_PROGRAM\n";
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(1, 0),
                end: Position::new(1, 0),
            }),
            range_length: None,
            text: "    VAR\n    END_VAR\n".to_string(),
        };
        let updated = apply_content_changes(original, &[change]).expect("apply change");
        assert!(updated.contains("VAR"));
        assert!(updated.contains("END_VAR"));
    }

    #[test]
    fn apply_content_changes_replaces_range() {
        let original = "x := 1;\n";
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(0, 5),
                end: Position::new(0, 6),
            }),
            range_length: None,
            text: "2".to_string(),
        };
        let updated = apply_content_changes(original, &[change]).expect("apply change");
        assert_eq!(updated, "x := 2;\n");
    }

    #[test]
    fn apply_content_changes_full_sync() {
        let original = "x := 1;\n";
        let change = TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "y := 2;\n".to_string(),
        };
        let updated = apply_content_changes(original, &[change]).expect("apply change");
        assert_eq!(updated, "y := 2;\n");
    }
}
