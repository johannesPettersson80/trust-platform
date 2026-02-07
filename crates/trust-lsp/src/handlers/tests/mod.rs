use super::features::{references, workspace_symbol};
use super::namespace_move_workspace_edit;
use super::*;
use crate::config::{
    BuildConfig, DiagnosticSettings, IndexingConfig, LibraryDependency, LibrarySpec, ProjectConfig,
    RuntimeConfig, StdlibSettings, TargetProfile, TelemetryConfig, WorkspaceSettings,
};
use crate::state::ServerState;
use crate::test_support::test_client;
use expect_test::expect;
use insta::assert_snapshot;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn position_at(source: &str, needle: &str) -> tower_lsp::lsp_types::Position {
    let offset = source
        .find(needle)
        .unwrap_or_else(|| panic!("missing needle '{needle}'"));
    super::lsp_utils::offset_to_position(source, offset as u32)
}

fn inlay_label_contains(label: &tower_lsp::lsp_types::InlayHintLabel, needle: &str) -> bool {
    match label {
        tower_lsp::lsp_types::InlayHintLabel::String(value) => value.contains(needle),
        tower_lsp::lsp_types::InlayHintLabel::LabelParts(parts) => {
            parts.iter().any(|part| part.value.contains(needle))
        }
    }
}

fn temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn document_snapshot(state: &ServerState, uri: &tower_lsp::lsp_types::Url) -> Value {
    match state.get_document(uri) {
        Some(doc) => json!({
            "version": doc.version,
            "isOpen": doc.is_open,
            "content": doc.content,
        }),
        None => Value::Null,
    }
}

mod code_actions_and_commands;
mod completion_hover;
mod core;
mod formatting_and_navigation;

#[test]
fn lsp_golden_multi_root_protocol_snapshot() {
    let source_main = r#"
CONFIGURATION Conf
VAR_GLOBAL CONSTANT
    ANSWER : INT := 42;
END_VAR
END_CONFIGURATION

TYPE MyInt : INT;
END_TYPE

USING Lib;

NAMESPACE Lib
FUNCTION Foo : INT
VAR_INPUT
    a : INT;
END_VAR
Foo := a;
END_FUNCTION
END_NAMESPACE

INTERFACE IFace
METHOD Do : INT;
END_METHOD
END_INTERFACE

CLASS Base
END_CLASS

CLASS Derived EXTENDS Base IMPLEMENTS IFace
METHOD Do : INT
    Do := Lib.Foo(ANSWER);
END_METHOD
END_CLASS

PROGRAM Main
VAR
    x : INT;
    y : INT;
    typed : MyInt;
END_VAR
x := Lib.Foo(ANSWER);
END_PROGRAM
"#;

    let source_aux = r#"
PROGRAM Aux
VAR
    counter : INT;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let config_source = r#"
[project]
include_paths = ["src"]
library_paths = ["libs"]

[[libraries]]
name = "Vendor"
path = "vendor"
"#;

    let state = ServerState::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let client = test_client();
    let root_one = PathBuf::from("/workspace/golden/alpha");
    let root_two = PathBuf::from("/workspace/golden/beta");
    let root_one_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/golden/alpha").unwrap();
    let root_two_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/golden/beta").unwrap();
    state.set_workspace_folders(vec![root_one_uri.clone(), root_two_uri.clone()]);
    state.set_workspace_config(
        root_one_uri.clone(),
        ProjectConfig {
            root: root_one.clone(),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings {
                priority: 10,
                visibility: crate::config::WorkspaceVisibility::Public,
            },
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_workspace_config(
        root_two_uri.clone(),
        ProjectConfig {
            root: root_two.clone(),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings {
                priority: 1,
                visibility: crate::config::WorkspaceVisibility::Private,
            },
            telemetry: TelemetryConfig::default(),
        },
    );

    let main_uri =
        tower_lsp::lsp_types::Url::parse("file:///workspace/golden/alpha/Main.st").unwrap();
    let aux_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/golden/beta/Aux.st").unwrap();
    let config_uri =
        tower_lsp::lsp_types::Url::parse("file:///workspace/golden/alpha/trust-lsp.toml").unwrap();
    state.open_document(main_uri.clone(), 1, source_main.to_string());
    state.open_document(aux_uri.clone(), 1, source_aux.to_string());
    state.open_document(config_uri.clone(), 1, config_source.to_string());

    let hover_params = tower_lsp::lsp_types::HoverParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: position_at(source_main, "Foo(ANSWER"),
        },
        work_done_progress_params: Default::default(),
    };
    let hover_result = hover(&state, hover_params);

    let completion_position = {
        let mut pos = position_at(source_main, "Lib.Foo");
        pos.character += 4;
        pos
    };
    let completion_params = tower_lsp::lsp_types::CompletionParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: completion_position,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };
    let completion_result = completion(&state, completion_params);
    let completion_resolve_result = completion_result.as_ref().and_then(|response| {
        let first = match response {
            tower_lsp::lsp_types::CompletionResponse::Array(items) => items.first().cloned(),
            tower_lsp::lsp_types::CompletionResponse::List(list) => list.items.first().cloned(),
        }?;
        Some(completion_resolve(&state, first))
    });

    let signature_params = tower_lsp::lsp_types::SignatureHelpParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: {
                let mut pos = position_at(source_main, "Foo(ANSWER");
                pos.character += 4;
                pos
            },
        },
        context: None,
        work_done_progress_params: Default::default(),
    };
    let signature_result = signature_help(&state, signature_params);

    let def_params = tower_lsp::lsp_types::GotoDefinitionParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: position_at(source_main, "Foo(ANSWER"),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let def_result = goto_definition(&state, def_params);

    let decl_result = goto_declaration(
        &state,
        tower_lsp::lsp_types::request::GotoDeclarationParams {
            text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: position_at(source_main, "Foo(ANSWER"),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );

    let type_def_result = goto_type_definition(
        &state,
        tower_lsp::lsp_types::request::GotoTypeDefinitionParams {
            text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: position_at(source_main, "typed : MyInt"),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );

    let impl_result = goto_implementation(
        &state,
        tower_lsp::lsp_types::request::GotoImplementationParams {
            text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: position_at(source_main, "IFace"),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );

    let ref_params = tower_lsp::lsp_types::ReferenceParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: position_at(source_main, "Foo(ANSWER"),
        },
        context: tower_lsp::lsp_types::ReferenceContext {
            include_declaration: true,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let ref_result = references(&state, ref_params);

    let highlight_params = tower_lsp::lsp_types::DocumentHighlightParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: position_at(source_main, "x : INT"),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let highlight_result = document_highlight(&state, highlight_params);

    let doc_symbol_params = tower_lsp::lsp_types::DocumentSymbolParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
            uri: main_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let doc_symbol_result = document_symbol(&state, doc_symbol_params);

    let workspace_symbol_empty = workspace_symbol(
        &state,
        tower_lsp::lsp_types::WorkspaceSymbolParams {
            query: "".to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let workspace_symbol_aux = workspace_symbol(
        &state,
        tower_lsp::lsp_types::WorkspaceSymbolParams {
            query: "Aux".to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );

    let diagnostic_params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
            uri: main_uri.clone(),
        },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let diagnostic_result = document_diagnostic(&state, diagnostic_params);

    let workspace_diag_params = tower_lsp::lsp_types::WorkspaceDiagnosticParams {
        previous_result_ids: Vec::new(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        identifier: None,
    };
    let workspace_diag_result = workspace_diagnostic(&state, workspace_diag_params);

    let diagnostic_items = match &diagnostic_result {
        tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
            tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
        ) => full.full_document_diagnostic_report.items.clone(),
        _ => Vec::new(),
    };
    let unused_diag = diagnostic_items
        .iter()
        .find(|diag| {
            diag.code.as_ref().is_some_and(|code| match code {
                tower_lsp::lsp_types::NumberOrString::String(value) => value == "W001",
                _ => false,
            })
        })
        .cloned();
    let code_action_result = unused_diag.as_ref().and_then(|diag| {
        let params = tower_lsp::lsp_types::CodeActionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            range: diag.range,
            context: tower_lsp::lsp_types::CodeActionContext {
                diagnostics: vec![diag.clone()],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        code_action(&state, params)
    });

    let code_lens_result = code_lens(
        &state,
        tower_lsp::lsp_types::CodeLensParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );

    let call_hierarchy_items = prepare_call_hierarchy(
        &state,
        tower_lsp::lsp_types::CallHierarchyPrepareParams {
            text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: position_at(source_main, "Foo : INT"),
            },
            work_done_progress_params: Default::default(),
        },
    )
    .unwrap_or_default();
    let call_hierarchy_incoming = call_hierarchy_items.first().and_then(|item| {
        incoming_calls(
            &state,
            tower_lsp::lsp_types::CallHierarchyIncomingCallsParams {
                item: item.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
    });
    let call_hierarchy_outgoing = call_hierarchy_items.first().and_then(|item| {
        outgoing_calls(
            &state,
            tower_lsp::lsp_types::CallHierarchyOutgoingCallsParams {
                item: item.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
    });

    let type_hierarchy_items = prepare_type_hierarchy(
        &state,
        tower_lsp::lsp_types::TypeHierarchyPrepareParams {
            text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: position_at(source_main, "Derived"),
            },
            work_done_progress_params: Default::default(),
        },
    )
    .unwrap_or_default();
    let type_hierarchy_supertypes = type_hierarchy_items.first().and_then(|item| {
        type_hierarchy_supertypes(
            &state,
            tower_lsp::lsp_types::TypeHierarchySupertypesParams {
                item: item.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
    });
    let type_hierarchy_subtypes = type_hierarchy_items.first().and_then(|item| {
        type_hierarchy_subtypes(
            &state,
            tower_lsp::lsp_types::TypeHierarchySubtypesParams {
                item: item.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
    });

    let rename_params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: position_at(source_main, "x : INT"),
        },
        new_name: "counter".to_string(),
        work_done_progress_params: Default::default(),
    };
    let rename_result = rename(&state, rename_params);

    let prepare_rename_result = prepare_rename(
        &state,
        tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: position_at(source_main, "x : INT"),
        },
    );

    let semantic_full = semantic_tokens_full(
        &state,
        tower_lsp::lsp_types::SemanticTokensParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let previous_result_id = semantic_full.as_ref().and_then(|result| match result {
        tower_lsp::lsp_types::SemanticTokensResult::Tokens(tokens) => tokens.result_id.clone(),
        _ => None,
    });
    let semantic_delta = semantic_tokens_full_delta(
        &state,
        tower_lsp::lsp_types::SemanticTokensDeltaParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            previous_result_id: previous_result_id.unwrap_or_else(|| "0".to_string()),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let semantic_range = semantic_tokens_range(
        &state,
        tower_lsp::lsp_types::SemanticTokensRangeParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            range: tower_lsp::lsp_types::Range {
                start: position_at(source_main, "PROGRAM Main"),
                end: position_at(source_main, "END_PROGRAM"),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );

    let folding_result = folding_range(
        &state,
        tower_lsp::lsp_types::FoldingRangeParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let selection_result = selection_range(
        &state,
        tower_lsp::lsp_types::SelectionRangeParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            positions: vec![position_at(source_main, "Lib.Foo(ANSWER)")],
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let linked_editing_result = linked_editing_range(
        &state,
        tower_lsp::lsp_types::LinkedEditingRangeParams {
            text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: position_at(source_main, "Foo : INT"),
            },
            work_done_progress_params: Default::default(),
        },
    );
    let document_link_st = document_link(
        &state,
        tower_lsp::lsp_types::DocumentLinkParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let document_link_config = document_link(
        &state,
        tower_lsp::lsp_types::DocumentLinkParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: config_uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );

    let inlay_result = inlay_hint(
        &state,
        tower_lsp::lsp_types::InlayHintParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            range: tower_lsp::lsp_types::Range {
                start: position_at(source_main, "x := Lib.Foo"),
                end: position_at(source_main, "END_PROGRAM"),
            },
            work_done_progress_params: Default::default(),
        },
    );

    let inline_value_result = inline_value(
        &state,
        tower_lsp::lsp_types::InlineValueParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            range: tower_lsp::lsp_types::Range {
                start: position_at(source_main, "PROGRAM Main"),
                end: position_at(source_main, "END_PROGRAM"),
            },
            context: tower_lsp::lsp_types::InlineValueContext {
                frame_id: 1,
                stopped_location: tower_lsp::lsp_types::Range {
                    start: tower_lsp::lsp_types::Position::new(0, 0),
                    end: tower_lsp::lsp_types::Position::new(0, 0),
                },
            },
            work_done_progress_params: Default::default(),
        },
    );

    let formatting_result = formatting(
        &state,
        tower_lsp::lsp_types::DocumentFormattingParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            options: tower_lsp::lsp_types::FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            },
            work_done_progress_params: Default::default(),
        },
    );
    let range_formatting_result = range_formatting(
        &state,
        tower_lsp::lsp_types::DocumentRangeFormattingParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            range: tower_lsp::lsp_types::Range {
                start: position_at(source_main, "PROGRAM Main"),
                end: position_at(source_main, "END_PROGRAM"),
            },
            options: tower_lsp::lsp_types::FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            },
            work_done_progress_params: Default::default(),
        },
    );
    let on_type_formatting_result = on_type_formatting(
        &state,
        tower_lsp::lsp_types::DocumentOnTypeFormattingParams {
            text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: main_uri.clone(),
                },
                position: position_at(source_main, "x := Lib.Foo"),
            },
            ch: ";".to_string(),
            options: tower_lsp::lsp_types::FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            },
        },
    );

    let will_rename_result = will_rename_files(
        &state,
        tower_lsp::lsp_types::RenameFilesParams {
            files: vec![tower_lsp::lsp_types::FileRename {
                old_uri: main_uri.to_string(),
                new_uri: "file:///workspace/golden/alpha/MainRenamed.st".to_string(),
            }],
        },
    );

    let execute_command_result = runtime.block_on(execute_command(
        &client,
        &state,
        tower_lsp::lsp_types::ExecuteCommandParams {
            command: PROJECT_INFO_COMMAND.to_string(),
            arguments: vec![json!({ "root_uri": root_one_uri })],
            work_done_progress_params: Default::default(),
        },
    ));

    let notify_summary = {
        let notify_state = Arc::new(ServerState::new());
        let notify_source = r#"
PROGRAM Notify
VAR
    x : INT;
END_VAR
x := 1;
END_PROGRAM
"#;
        let notify_uri =
            tower_lsp::lsp_types::Url::parse("file:///workspace/golden/notify/Notify.st").unwrap();
        let watch_dir = temp_dir("lsp-watch");
        let watch_path = watch_dir.join("Watch.st");
        let watch_source = "PROGRAM Watch\nEND_PROGRAM\n";
        std::fs::write(&watch_path, watch_source).expect("write watch source");
        let watch_uri = tower_lsp::lsp_types::Url::from_file_path(&watch_path).unwrap();

        runtime.block_on(async {
            did_open(
                &client,
                &notify_state,
                tower_lsp::lsp_types::DidOpenTextDocumentParams {
                    text_document: tower_lsp::lsp_types::TextDocumentItem {
                        uri: notify_uri.clone(),
                        language_id: "st".to_string(),
                        version: 1,
                        text: notify_source.to_string(),
                    },
                },
            )
            .await;
            let after_open = document_snapshot(&notify_state, &notify_uri);

            let change_pos = position_at(notify_source, "1;");
            did_change(
                &client,
                &notify_state,
                tower_lsp::lsp_types::DidChangeTextDocumentParams {
                    text_document: tower_lsp::lsp_types::VersionedTextDocumentIdentifier {
                        uri: notify_uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![tower_lsp::lsp_types::TextDocumentContentChangeEvent {
                        range: Some(tower_lsp::lsp_types::Range {
                            start: change_pos,
                            end: tower_lsp::lsp_types::Position::new(
                                change_pos.line,
                                change_pos.character + 1,
                            ),
                        }),
                        range_length: None,
                        text: "2".to_string(),
                    }],
                },
            )
            .await;
            let after_change = document_snapshot(&notify_state, &notify_uri);

            did_save(
                &client,
                &notify_state,
                tower_lsp::lsp_types::DidSaveTextDocumentParams {
                    text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                        uri: notify_uri.clone(),
                    },
                    text: None,
                },
            )
            .await;
            let after_save = document_snapshot(&notify_state, &notify_uri);

            did_close(
                &client,
                &notify_state,
                tower_lsp::lsp_types::DidCloseTextDocumentParams {
                    text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                        uri: notify_uri.clone(),
                    },
                },
            )
            .await;
            let after_close = document_snapshot(&notify_state, &notify_uri);

            let config_value = json!({
                "trust-lsp": {
                    "formatting": { "indent_width": 2 },
                    "diagnostics": { "showIecReferences": true }
                }
            });
            did_change_configuration(
                &notify_state,
                tower_lsp::lsp_types::DidChangeConfigurationParams {
                    settings: config_value.clone(),
                },
            );

            did_change_watched_files(
                &client,
                &notify_state,
                tower_lsp::lsp_types::DidChangeWatchedFilesParams {
                    changes: vec![tower_lsp::lsp_types::FileEvent {
                        uri: watch_uri.clone(),
                        typ: tower_lsp::lsp_types::FileChangeType::CREATED,
                    }],
                },
            )
            .await;
            let after_watch_create = document_snapshot(&notify_state, &watch_uri);

            did_change_watched_files(
                &client,
                &notify_state,
                tower_lsp::lsp_types::DidChangeWatchedFilesParams {
                    changes: vec![tower_lsp::lsp_types::FileEvent {
                        uri: watch_uri.clone(),
                        typ: tower_lsp::lsp_types::FileChangeType::DELETED,
                    }],
                },
            )
            .await;
            let after_watch_delete = document_snapshot(&notify_state, &watch_uri);

            json!({
                "didOpen": after_open,
                "didChange": after_change,
                "didSave": after_save,
                "didClose": after_close,
                "didChangeConfiguration": notify_state.config(),
                "didChangeWatchedFiles": {
                    "afterCreate": after_watch_create,
                    "afterDelete": after_watch_delete,
                }
            })
        })
    };

    let mut output = serde_json::Map::new();
    output.insert(
        "hover".to_string(),
        serde_json::to_value(&hover_result).unwrap(),
    );
    output.insert(
        "completion".to_string(),
        serde_json::to_value(&completion_result).unwrap(),
    );
    output.insert(
        "completionResolve".to_string(),
        serde_json::to_value(&completion_resolve_result).unwrap(),
    );
    output.insert(
        "signatureHelp".to_string(),
        serde_json::to_value(&signature_result).unwrap(),
    );
    output.insert(
        "definition".to_string(),
        serde_json::to_value(&def_result).unwrap(),
    );
    output.insert(
        "declaration".to_string(),
        serde_json::to_value(&decl_result).unwrap(),
    );
    output.insert(
        "typeDefinition".to_string(),
        serde_json::to_value(&type_def_result).unwrap(),
    );
    output.insert(
        "implementation".to_string(),
        serde_json::to_value(&impl_result).unwrap(),
    );
    output.insert(
        "references".to_string(),
        serde_json::to_value(&ref_result).unwrap(),
    );
    output.insert(
        "documentHighlight".to_string(),
        serde_json::to_value(&highlight_result).unwrap(),
    );
    output.insert(
        "documentSymbol".to_string(),
        serde_json::to_value(&doc_symbol_result).unwrap(),
    );
    output.insert(
        "workspaceSymbolEmpty".to_string(),
        serde_json::to_value(&workspace_symbol_empty).unwrap(),
    );
    output.insert(
        "workspaceSymbolAux".to_string(),
        serde_json::to_value(&workspace_symbol_aux).unwrap(),
    );
    output.insert(
        "documentDiagnostic".to_string(),
        serde_json::to_value(&diagnostic_result).unwrap(),
    );
    output.insert(
        "workspaceDiagnostic".to_string(),
        serde_json::to_value(&workspace_diag_result).unwrap(),
    );
    output.insert(
        "codeAction".to_string(),
        serde_json::to_value(&code_action_result).unwrap(),
    );
    output.insert(
        "codeLens".to_string(),
        serde_json::to_value(&code_lens_result).unwrap(),
    );
    output.insert(
        "callHierarchyItems".to_string(),
        serde_json::to_value(&call_hierarchy_items).unwrap(),
    );
    output.insert(
        "callHierarchyIncoming".to_string(),
        serde_json::to_value(&call_hierarchy_incoming).unwrap(),
    );
    output.insert(
        "callHierarchyOutgoing".to_string(),
        serde_json::to_value(&call_hierarchy_outgoing).unwrap(),
    );
    output.insert(
        "typeHierarchyItems".to_string(),
        serde_json::to_value(&type_hierarchy_items).unwrap(),
    );
    output.insert(
        "typeHierarchySupertypes".to_string(),
        serde_json::to_value(&type_hierarchy_supertypes).unwrap(),
    );
    output.insert(
        "typeHierarchySubtypes".to_string(),
        serde_json::to_value(&type_hierarchy_subtypes).unwrap(),
    );
    output.insert(
        "rename".to_string(),
        serde_json::to_value(&rename_result).unwrap(),
    );
    output.insert(
        "prepareRename".to_string(),
        serde_json::to_value(&prepare_rename_result).unwrap(),
    );
    output.insert(
        "semanticTokensFull".to_string(),
        serde_json::to_value(&semantic_full).unwrap(),
    );
    output.insert(
        "semanticTokensDelta".to_string(),
        serde_json::to_value(&semantic_delta).unwrap(),
    );
    output.insert(
        "semanticTokensRange".to_string(),
        serde_json::to_value(&semantic_range).unwrap(),
    );
    output.insert(
        "foldingRange".to_string(),
        serde_json::to_value(&folding_result).unwrap(),
    );
    output.insert(
        "selectionRange".to_string(),
        serde_json::to_value(&selection_result).unwrap(),
    );
    output.insert(
        "linkedEditingRange".to_string(),
        serde_json::to_value(&linked_editing_result).unwrap(),
    );
    output.insert(
        "documentLinkSt".to_string(),
        serde_json::to_value(&document_link_st).unwrap(),
    );
    output.insert(
        "documentLinkConfig".to_string(),
        serde_json::to_value(&document_link_config).unwrap(),
    );
    output.insert(
        "inlayHint".to_string(),
        serde_json::to_value(&inlay_result).unwrap(),
    );
    output.insert(
        "inlineValue".to_string(),
        serde_json::to_value(&inline_value_result).unwrap(),
    );
    output.insert(
        "formatting".to_string(),
        serde_json::to_value(&formatting_result).unwrap(),
    );
    output.insert(
        "rangeFormatting".to_string(),
        serde_json::to_value(&range_formatting_result).unwrap(),
    );
    output.insert(
        "onTypeFormatting".to_string(),
        serde_json::to_value(&on_type_formatting_result).unwrap(),
    );
    output.insert(
        "willRenameFiles".to_string(),
        serde_json::to_value(&will_rename_result).unwrap(),
    );
    output.insert(
        "executeCommandProjectInfo".to_string(),
        serde_json::to_value(&execute_command_result).unwrap(),
    );
    output.insert("notifyWorkflows".to_string(), notify_summary);

    let output = Value::Object(output);
    let output = serde_json::to_string_pretty(&output).expect("serialize snapshot");
    insta::with_settings!({ snapshot_path => "../snapshots" }, {
        assert_snapshot!(output);
    });
}

#[test]
fn lsp_code_action_namespace_disambiguation_non_call() {
    let source = r#"
NAMESPACE LibA
TYPE Foo : INT;
END_TYPE
END_NAMESPACE

NAMESPACE LibB
TYPE Foo : INT;
END_TYPE
END_NAMESPACE

PROGRAM Main
    USING LibA;
    USING LibB;
    VAR
        x : Foo;
    END_VAR
    x := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let type_offset = source.find("x : Foo;").expect("type reference");
    let foo_start = type_offset + "x : ".len();
    let foo_end = foo_start + "Foo".len();
    let start = super::lsp_utils::offset_to_position(source, foo_start as u32);
    let end = super::lsp_utils::offset_to_position(source, foo_end as u32);

    let diagnostic = tower_lsp::lsp_types::Diagnostic {
        range: tower_lsp::lsp_types::Range { start, end },
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "E105".to_string(),
        )),
        source: Some("trust-lsp".to_string()),
        message: "ambiguous reference to 'Foo'; qualify the name".to_string(),
        ..Default::default()
    };

    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: diagnostic.range,
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: vec![diagnostic],
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action(&state, params).expect("code actions");
    let mut titles = actions
        .iter()
        .filter_map(|action| match action {
            tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
                Some(code_action.title.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    titles.sort();
    assert!(
        titles.iter().any(|title| title.contains("LibA.Foo")),
        "expected LibA qualification quick fix"
    );
    assert!(
        titles.iter().any(|title| title.contains("LibB.Foo")),
        "expected LibB qualification quick fix"
    );
}
