//! Performance and stress harnesses for trust-lsp.

#[cfg(test)]
mod tests {
    use crate::handlers::{completion, document_diagnostic, hover, index_workspace, rename};
    use crate::state::ServerState;
    use crate::test_support::test_client;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use tower_lsp::lsp_types::{
        CompletionParams, DocumentDiagnosticParams, Position, RenameParams, TextDocumentIdentifier,
        TextDocumentPositionParams, WorkspaceSymbolParams,
    };

    fn position_at(source: &str, needle: &str) -> Position {
        let offset = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing needle '{needle}'"));
        let mut line = 0u32;
        let mut character = 0u32;
        for (idx, ch) in source.char_indices() {
            if idx >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                character = 0;
            } else {
                character += 1;
            }
        }
        Position::new(line, character)
    }

    fn env_usize(name: &str, default: usize) -> usize {
        env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }

    fn env_u64(name: &str, default: u64) -> u64 {
        env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }

    fn avg_duration(iterations: usize, mut f: impl FnMut()) -> Duration {
        let iterations = iterations.max(1);
        let start = Instant::now();
        for _ in 0..iterations {
            f();
        }
        let total = start.elapsed();
        Duration::from_nanos((total.as_nanos() / iterations as u128) as u64)
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn perf_state(source: &str) -> (ServerState, tower_lsp::lsp_types::Url) {
        let state = ServerState::new();
        let uri = tower_lsp::lsp_types::Url::parse("file:///perf/Main.st").unwrap();
        state.open_document(uri.clone(), 1, source.to_string());
        (state, uri)
    }

    #[test]
    #[ignore]
    fn perf_hover_budget() {
        let source = r#"
FUNCTION Foo : INT
VAR_INPUT
    a : INT;
END_VAR
Foo := a;
END_FUNCTION

PROGRAM Main
VAR
    x : INT;
END_VAR
x := Foo(1);
END_PROGRAM
"#;
        let (state, uri) = perf_state(source);
        let params = tower_lsp::lsp_types::HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: position_at(source, "Foo(1)"),
            },
            work_done_progress_params: Default::default(),
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 50);
        let budget_ms = env_u64("ST_LSP_PERF_HOVER_MS", 50);
        let avg = avg_duration(iterations, || {
            let _ = hover(&state, params.clone());
        });
        assert!(
            avg.as_millis() <= budget_ms as u128,
            "hover avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_completion_budget() {
        let source = r#"
FUNCTION Foo : INT
VAR_INPUT
    a : INT;
END_VAR
Foo := a;
END_FUNCTION

PROGRAM Main
VAR
    x : INT;
END_VAR
x := F
END_PROGRAM
"#;
        let (state, uri) = perf_state(source);
        let mut position = position_at(source, "x := F");
        position.character += 6;
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 50);
        let budget_ms = env_u64("ST_LSP_PERF_COMPLETION_MS", 200);
        let avg = avg_duration(iterations, || {
            let _ = completion(&state, params.clone());
        });
        assert!(
            avg.as_millis() <= budget_ms as u128,
            "completion avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_rename_budget() {
        let source = r#"
PROGRAM Main
VAR
    x : INT;
END_VAR
x := 1;
x := x + 1;
END_PROGRAM
"#;
        let (state, uri) = perf_state(source);
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: position_at(source, "x : INT"),
            },
            new_name: "counter".to_string(),
            work_done_progress_params: Default::default(),
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 20);
        let budget_ms = env_u64("ST_LSP_PERF_RENAME_MS", 100);
        let avg = avg_duration(iterations, || {
            let _ = rename(&state, params.clone());
        });
        assert!(
            avg.as_millis() <= budget_ms as u128,
            "rename avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_large_workspace_index_budget() {
        let file_count = env_usize("ST_LSP_PERF_STRESS_FILES", 10_000);
        let budget_ms = env_u64("ST_LSP_PERF_INDEX_MS", 5_000);
        let root = temp_dir("trust-lsp-perf");
        for idx in 0..file_count {
            let path = root.join(format!("Program{idx:05}.st"));
            let content = format!(
                "PROGRAM Program{idx:05}\nVAR\n    x : INT;\nEND_VAR\nx := {idx};\nEND_PROGRAM\n"
            );
            fs::write(&path, content).expect("write perf file");
        }

        let client = test_client();
        let state = ServerState::new();
        let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).unwrap();
        state.set_workspace_folders(vec![root_uri.clone()]);

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let start = Instant::now();
        runtime.block_on(index_workspace(&client, &state));
        let elapsed = start.elapsed();

        let _ = fs::remove_dir_all(&root);

        assert!(
            elapsed.as_millis() <= budget_ms as u128,
            "indexing avg {:?} exceeded budget {}ms",
            elapsed,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_workspace_symbol_budget() {
        let source = r#"
PROGRAM Main
VAR
    counter : INT;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;
        let (state, _uri) = perf_state(source);
        let params = WorkspaceSymbolParams {
            query: "Main".to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 20);
        let budget_ms = env_u64("ST_LSP_PERF_WORKSPACE_SYMBOL_MS", 200);
        let client = test_client();
        let avg = avg_duration(iterations, || {
            let params = params.clone();
            runtime.block_on(async {
                let _ =
                    crate::handlers::workspace_symbol_with_progress(&client, &state, params).await;
            });
        });

        assert!(
            avg.as_millis() <= budget_ms as u128,
            "workspace/symbol avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_diagnostics_budget() {
        let declarations = env_usize("ST_LSP_PERF_DIAGNOSTIC_DECLS", 200);
        let mut source = String::from(
            r#"
TYPE MotorConfig : STRUCT
    speed : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    cfg : MotroConfig;
    flag : BOOL;
"#,
        );
        for idx in 0..declarations {
            source.push_str(&format!("    speedValue{idx} : DINT;\n"));
        }
        source.push_str(
            r#"END_VAR
    speadValue0 := 1;
    flag := 1;
"#,
        );
        for idx in 0..declarations {
            source.push_str(&format!("    speedValue{idx} := {idx};\n"));
        }
        source.push_str("END_PROGRAM\n");

        let (state, uri) = perf_state(&source);
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 30);
        let budget_ms = env_u64("ST_LSP_PERF_DIAGNOSTICS_MS", 900);
        let avg = avg_duration(iterations, || {
            let _ = document_diagnostic(&state, params.clone());
        });

        assert!(
            avg.as_millis() <= budget_ms as u128,
            "diagnostics avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }
}
