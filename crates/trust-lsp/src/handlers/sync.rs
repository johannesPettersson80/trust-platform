//! Document synchronization handlers.

use tower_lsp::lsp_types::*;
use tower_lsp::Client;
use tracing::{info, warn};

use crate::state::ServerState;

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

    let updated = match apply_content_changes(&doc.content, &params.content_changes) {
        Ok(updated) => updated,
        Err(error) => {
            warn!("Failed to apply incremental changes for {}: {}", uri, error);
            client
                .show_message(
                    MessageType::ERROR,
                    format!(
                        "Could not apply editor changes for {uri}: {error}. Reopen the file to resynchronize its full contents."
                    ),
                )
                .await;
            return;
        }
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
) -> Result<String, crate::document_text::ContentChangeError> {
    let changes = changes
        .iter()
        .map(|change| crate::document_text::ContentChange {
            range: change
                .range
                .map(|range| crate::document_text::DocumentRange {
                    start: crate::document_text::DocumentPosition {
                        line: range.start.line,
                        character: range.start.character,
                    },
                    end: crate::document_text::DocumentPosition {
                        line: range.end.line,
                        character: range.end.character,
                    },
                }),
            text: change.text.clone(),
        })
        .collect::<Vec<_>>();
    crate::document_text::apply_content_changes(content, &changes)
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

    let changed = state.close_document(&uri);

    // Clear diagnostics only for push-based clients.
    if !state.use_pull_diagnostics() {
        client.publish_diagnostics(uri, vec![], None).await;
    }
    // The other open documents' diagnostics may rest on what the close discarded, and a client
    // pulls them again only when asked (specification 14 §7.2.1).
    if changed {
        refresh_diagnostics(client, state).await;
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
    fn apply_content_changes_uses_utf16_columns_after_supplementary_scalar() {
        let original = "😀x := 1;\n";
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(0, 2),
                end: Position::new(0, 3),
            }),
            range_length: None,
            text: "y".to_string(),
        };
        let updated = apply_content_changes(original, &[change]).expect("apply change");
        assert_eq!(updated, "😀y := 1;\n");
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

    #[test]
    fn apply_content_changes_reports_out_of_bounds_line_for_full_resync() {
        let original = "x := 1;\n";
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(99, 0),
                end: Position::new(99, 0),
            }),
            range_length: None,
            text: "y := 2;".to_string(),
        };

        let error = apply_content_changes(original, &[change]).expect_err("invalid range");
        assert_eq!(
            error.to_string(),
            "change start line 99 is outside the 2-line document"
        );
    }

    const MAIN: &str = "PROGRAM Main\nEND_PROGRAM\n";

    /// A project folder and a folder outside it, each with a saved `Main.st`, and a server whose
    /// workspace folder is the project, with both files open.
    struct Closing {
        state: ServerState,
        own: Url,
        outside: Url,
        folders: [std::path::PathBuf; 2],
    }

    impl Closing {
        fn new() -> Self {
            let folders = ["project", "elsewhere"].map(|name| {
                let folder = trust_test_support::unique_temp_path(&format!(
                    "trust-lsp-close-refresh-{name}"
                ));
                std::fs::create_dir_all(&folder).expect("create folder");
                std::fs::write(folder.join("Main.st"), MAIN).expect("write Main.st");
                folder
            });
            let state = ServerState::new();
            state.set_workspace_config(
                Url::from_file_path(&folders[0]).expect("project URI"),
                crate::config::ProjectConfig::load(&folders[0]),
            );
            let [own, outside] = folders.each_ref().map(|folder| {
                let uri = Url::from_file_path(folder.join("Main.st")).expect("file URI");
                state.open_document(uri.clone(), 1, MAIN.to_string());
                uri
            });
            Self {
                state,
                own,
                outside,
                folders,
            }
        }
    }

    impl Drop for Closing {
        fn drop(&mut self) {
            for folder in &self.folders {
                std::fs::remove_dir_all(folder).ok();
            }
        }
    }

    fn close(uri: &Url) -> DidCloseTextDocumentParams {
        DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        }
    }

    /// A close that changed the project asks a pull client to pull the open documents'
    /// diagnostics again; a saved file's close asks nothing (specification 14 §7.2.1). Catches
    /// the stale diagnostics VS Code kept: its client pulls another document again only when the
    /// server asks, so the duplicate `PROGRAM Main` of a file that left the project stayed on the
    /// project's own `Main` until its next edit.
    #[tokio::test(flavor = "current_thread")]
    async fn a_close_that_changed_the_project_asks_a_pull_client_to_refresh() {
        use futures_util::{FutureExt, SinkExt, StreamExt};
        let closing = Closing::new();
        let state = &closing.state;
        state.set_diagnostic_pull_supported(true);
        state.set_diagnostic_refresh_supported(true);
        let (client, mut service, mut socket) = crate::test_support::test_client_with_socket();
        crate::test_support::initialize_test_client(&mut service).await;
        let deadline = std::time::Duration::from_secs(10);

        // A refresh request would hold the close until the client answers.
        tokio::time::timeout(deadline, did_close(&client, state, close(&closing.own)))
            .await
            .expect("the saved file's close asks the client nothing");
        assert!(socket.next().now_or_never().is_none());

        let mut closed = std::pin::pin!(did_close(&client, state, close(&closing.outside)));
        let request = tokio::select! {
            () = &mut closed => panic!("the close ended without asking the client to refresh"),
            request = socket.next() => request.expect("the server asks the client"),
        };
        assert_eq!(request.method(), "workspace/diagnostic/refresh");
        let id = request
            .id()
            .cloned()
            .expect("a request, which waits for its answer");
        socket
            .send(tower_lsp::jsonrpc::Response::from_ok(
                id,
                serde_json::Value::Null,
            ))
            .await
            .expect("answer the request");
        tokio::time::timeout(deadline, closed)
            .await
            .expect("the outside file's close ends once the client answered");
    }

    /// A push client receives the empty list for the closed file and then each open document's
    /// recomputed diagnostics, after a close that changed the project (specification 14 §7.2.1).
    /// Catches the project's own `Main` left with the diagnostics published while the closed file
    /// still collided with it.
    #[tokio::test(flavor = "current_thread")]
    async fn a_close_that_changed_the_project_publishes_the_open_documents_to_a_push_client() {
        use futures_util::StreamExt;
        let closing = Closing::new();
        let (client, mut service, mut socket) = crate::test_support::test_client_with_socket();
        crate::test_support::initialize_test_client(&mut service).await;

        let published = async {
            let mut uris = Vec::new();
            while uris.len() < 2 {
                let notification = socket.next().await.expect("a notification");
                assert_eq!(notification.method(), "textDocument/publishDiagnostics");
                let params = notification.params().expect("its parameters");
                uris.push(params["uri"].as_str().expect("its URI").to_string());
            }
            uris
        };
        // Each list arrives as soon as it is published; the deadline only ends a run in which the
        // second list never comes.
        let (_, uris) = tokio::join!(
            did_close(&client, &closing.state, close(&closing.outside)),
            async {
                tokio::time::timeout(std::time::Duration::from_secs(10), published)
                    .await
                    .expect(
                        "after the closed file's empty list, the open project file's diagnostics",
                    )
            }
        );
        assert_eq!(
            uris,
            [closing.outside.to_string(), closing.own.to_string()],
            "the closed file's empty list, then the open project file's diagnostics"
        );
    }
}
