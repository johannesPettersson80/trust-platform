use super::*;

#[test]
pub(super) fn lsp_document_symbols_include_members() {
    let source = r#"
INTERFACE ICounter
    METHOD Next : DINT
    END_METHOD
    PROPERTY Value : DINT
        GET
        END_GET
    END_PROPERTY
END_INTERFACE

FUNCTION_BLOCK CounterFb IMPLEMENTS ICounter
VAR
    x : DINT;
END_VAR

METHOD PUBLIC Next : DINT
    x := x + 1;
    Next := x;
END_METHOD

PUBLIC PROPERTY Value : DINT
    GET
        Value := x;
    END_GET
END_PROPERTY
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///members.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentSymbolParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let response = document_symbol(&state, params).expect("document symbols");
    let symbols = match response {
        tower_lsp::lsp_types::DocumentSymbolResponse::Flat(symbols) => symbols,
        tower_lsp::lsp_types::DocumentSymbolResponse::Nested(_) => {
            panic!("expected flat document symbols")
        }
    };

    let has_next = symbols.iter().any(|symbol| symbol.name.contains("Next"));
    let has_value = symbols.iter().any(|symbol| symbol.name.contains("Value"));
    assert!(has_next, "expected Next in document symbols");
    assert!(has_value, "expected Value in document symbols");

    let has_next_in_fb = symbols.iter().any(|symbol| {
        symbol.name.contains("Next") && symbol.container_name.as_deref() == Some("CounterFb")
    });
    assert!(has_next_in_fb, "expected Next under CounterFb");
}

#[test]
pub(super) fn lsp_oop_access_diagnostics_include_explainer_and_hint() {
    let source = r#"
CLASS Foo
VAR PRIVATE
    secret : INT;
END_VAR
END_CLASS

PROGRAM Test
VAR
    f : Foo;
    x : INT;
END_VAR
    x := f.secret;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///access.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let report = document_diagnostic(&state, params);
    let report = match report {
        tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(report) => report,
        _ => panic!("expected diagnostic report"),
    };
    let full = match report {
        tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full) => full,
        _ => panic!("expected full diagnostic report"),
    };
    let diagnostics = full.full_document_diagnostic_report.items;
    let access_diag = diagnostics
        .iter()
        .find(|diag| diag.message.contains("cannot access PRIVATE member"))
        .expect("expected access violation diagnostic");
    let explain = access_diag
        .data
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|map| map.get("explain"))
        .and_then(|value| value.get("iec"))
        .and_then(|value| value.as_str());
    assert!(
        explain.is_some_and(|iec| iec.contains("6.6.5")),
        "expected IEC 6.6.5 explainer"
    );
    let related = access_diag.related_information.as_ref();
    assert!(
        related.is_some_and(|items| items.iter().any(|item| item.message.contains("Hint:"))),
        "expected access hint related information"
    );
}

#[test]
pub(super) fn lsp_workspace_diagnostics_supports_unchanged_reports() {
    let source = r#"
PROGRAM Test
    VAR
        A__B : INT;
    END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace-diag.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::WorkspaceDiagnosticParams {
        identifier: None,
        previous_result_ids: Vec::new(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let report = workspace_diagnostic(&state, params);
    let report = match report {
        tower_lsp::lsp_types::WorkspaceDiagnosticReportResult::Report(report) => report,
        _ => panic!("expected workspace diagnostic report"),
    };
    let first_item = report
        .items
        .iter()
        .find(|item| match item {
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) => full.uri == uri,
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(unchanged) => {
                unchanged.uri == uri
            }
        })
        .expect("expected workspace diagnostic item");
    let result_id = match first_item {
        tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) => full
            .full_document_diagnostic_report
            .result_id
            .clone()
            .expect("result id"),
        _ => panic!("expected full diagnostic report"),
    };

    let params = tower_lsp::lsp_types::WorkspaceDiagnosticParams {
        identifier: None,
        previous_result_ids: vec![tower_lsp::lsp_types::PreviousResultId {
            uri: uri.clone(),
            value: result_id,
        }],
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let report = workspace_diagnostic(&state, params);
    let report = match report {
        tower_lsp::lsp_types::WorkspaceDiagnosticReportResult::Report(report) => report,
        _ => panic!("expected workspace diagnostic report"),
    };
    let unchanged = report
        .items
        .iter()
        .find(|item| match item {
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) => full.uri == uri,
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(unchanged) => {
                unchanged.uri == uri
            }
        })
        .expect("expected workspace diagnostic item");
    assert!(
        matches!(
            unchanged,
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(_)
        ),
        "expected unchanged workspace diagnostic report"
    );
}

#[test]
pub(super) fn lsp_push_sync_refreshes_dependent_open_document_diagnostics() {
    let add_v1 = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION
"#;
    let add_v2 = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
END_VAR
    Add := A;
END_FUNCTION
"#;
    let main = r#"
PROGRAM Main
VAR
    Result : INT;
END_VAR
    Result := Add(1);
END_PROGRAM
"#;

    let state = ServerState::new();
    let client = test_client();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let add_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/Add.st").unwrap();
    let main_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/Main.st").unwrap();

    runtime.block_on(async {
        did_open(
            &client,
            &state,
            tower_lsp::lsp_types::DidOpenTextDocumentParams {
                text_document: tower_lsp::lsp_types::TextDocumentItem {
                    uri: add_uri.clone(),
                    language_id: "st".to_string(),
                    version: 1,
                    text: add_v1.to_string(),
                },
            },
        )
        .await;
        did_open(
            &client,
            &state,
            tower_lsp::lsp_types::DidOpenTextDocumentParams {
                text_document: tower_lsp::lsp_types::TextDocumentItem {
                    uri: main_uri.clone(),
                    language_id: "st".to_string(),
                    version: 1,
                    text: main.to_string(),
                },
            },
        )
        .await;
    });

    let before_report = document_diagnostic(
        &state,
        tower_lsp::lsp_types::DocumentDiagnosticParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let before_items = match before_report {
        tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
            tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
        ) => full.full_document_diagnostic_report.items,
        _ => panic!("expected full document diagnostic report"),
    };
    assert!(
        before_items
            .iter()
            .any(|diag| diag.message.contains("expected 2 arguments, found 1")),
        "expected initial wrong-argument-count diagnostic, got {:?}",
        before_items
            .iter()
            .map(|diag| diag.message.clone())
            .collect::<Vec<_>>()
    );

    let before_result_id = state
        .diagnostic_result_id(&main_uri)
        .expect("diagnostic cache for dependent document");

    runtime.block_on(async {
        did_change(
            &client,
            &state,
            tower_lsp::lsp_types::DidChangeTextDocumentParams {
                text_document: tower_lsp::lsp_types::VersionedTextDocumentIdentifier {
                    uri: add_uri,
                    version: 2,
                },
                content_changes: vec![tower_lsp::lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: add_v2.to_string(),
                }],
            },
        )
        .await;
    });

    let after_result_id = state
        .diagnostic_result_id(&main_uri)
        .expect("refreshed diagnostic cache for dependent document");
    assert_ne!(
        before_result_id, after_result_id,
        "dependent document diagnostics should be republished after source edits"
    );

    let after_report = document_diagnostic(
        &state,
        tower_lsp::lsp_types::DocumentDiagnosticParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: main_uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let after_items = match after_report {
        tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
            tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
        ) => full.full_document_diagnostic_report.items,
        _ => panic!("expected full document diagnostic report"),
    };
    assert!(
        after_items
            .iter()
            .all(|diag| !diag.message.contains("expected 2 arguments, found 1")),
        "dependent document should no longer carry the stale wrong-argument-count diagnostic, got {:?}",
        after_items
            .iter()
            .map(|diag| diag.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
pub(super) fn lsp_did_close_unsaved_dependency_reverts_cross_file_semantics_to_disk() {
    let root = temp_dir("trustlsp-didclose-disk-reload");
    let add_path = root.join("Add.st");
    let main_path = root.join("Main.st");
    let add_on_disk = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION
"#;
    let add_unsaved = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
END_VAR
    Add := A;
END_FUNCTION
"#;
    let main = r#"
PROGRAM Main
VAR
    Result : INT;
END_VAR
    Result := Add(1, 2);
END_PROGRAM
"#;
    std::fs::write(&add_path, add_on_disk).expect("write disk dependency");
    std::fs::write(&main_path, main).expect("write main");

    let state = ServerState::new();
    let client = test_client();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let add_uri = tower_lsp::lsp_types::Url::from_file_path(&add_path).expect("add uri");
    let main_uri = tower_lsp::lsp_types::Url::from_file_path(&main_path).expect("main uri");

    runtime.block_on(async {
        did_open(
            &client,
            &state,
            tower_lsp::lsp_types::DidOpenTextDocumentParams {
                text_document: tower_lsp::lsp_types::TextDocumentItem {
                    uri: add_uri.clone(),
                    language_id: "st".to_string(),
                    version: 1,
                    text: add_on_disk.to_string(),
                },
            },
        )
        .await;
        did_open(
            &client,
            &state,
            tower_lsp::lsp_types::DidOpenTextDocumentParams {
                text_document: tower_lsp::lsp_types::TextDocumentItem {
                    uri: main_uri.clone(),
                    language_id: "st".to_string(),
                    version: 1,
                    text: main.to_string(),
                },
            },
        )
        .await;
        did_change(
            &client,
            &state,
            tower_lsp::lsp_types::DidChangeTextDocumentParams {
                text_document: tower_lsp::lsp_types::VersionedTextDocumentIdentifier {
                    uri: add_uri.clone(),
                    version: 2,
                },
                content_changes: vec![tower_lsp::lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: add_unsaved.to_string(),
                }],
            },
        )
        .await;
    });

    let unsaved_items = full_document_diagnostics(&state, main_uri.clone());
    assert!(
        unsaved_items
            .iter()
            .any(|diag| diag.message.contains("arguments")),
        "sanity check: unsaved dependency edit should affect dependent diagnostics, got {:?}",
        diagnostic_messages(&unsaved_items)
    );

    runtime.block_on(async {
        did_close(
            &client,
            &state,
            tower_lsp::lsp_types::DidCloseTextDocumentParams {
                text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                    uri: add_uri.clone(),
                },
            },
        )
        .await;
    });

    let after_close_items = full_document_diagnostics(&state, main_uri);
    assert!(
        after_close_items
            .iter()
            .all(|diag| !diag.message.contains("arguments")),
        "didClose without save must discard the unsaved dependency signature and restore disk semantics, got {:?}",
        diagnostic_messages(&after_close_items)
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
pub(super) fn lsp_memory_budget_eviction_keeps_closed_dependency_semantically_indexed() {
    let root = temp_dir("trustlsp-memory-budget-semantics");
    let config_path = root.join("trust-lsp.toml");
    std::fs::write(
        &config_path,
        r#"
[indexing]
memory_budget_mb = 1
evict_to_percent = 75
"#,
    )
    .expect("write config");

    let padding = format!("(* {} *)", "A".repeat(600_000));
    let motor_source = format!(
        r#"
FUNCTION_BLOCK Motor
END_FUNCTION_BLOCK
{padding}
"#
    );
    let main_source = format!(
        r#"
PROGRAM Main
VAR
    fb : Motor;
END_VAR
    fb();
END_PROGRAM
{padding}
"#
    );
    let motor_path = root.join("Motor.st");
    let main_path = root.join("Main.st");
    std::fs::write(&motor_path, &motor_source).expect("write motor");
    std::fs::write(&main_path, &main_source).expect("write main");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_config(root_uri, ProjectConfig::load(&root));
    let motor_uri = tower_lsp::lsp_types::Url::from_file_path(&motor_path).expect("motor uri");
    let main_uri = tower_lsp::lsp_types::Url::from_file_path(&main_path).expect("main uri");

    state.index_document(motor_uri.clone(), motor_source);
    state.index_document(main_uri.clone(), main_source);
    assert!(
        state.get_document(&motor_uri).is_none(),
        "test fixture should evict the older closed document from the text cache"
    );

    let diagnostics = full_document_diagnostics(&state, main_uri);
    let errors = diagnostics
        .iter()
        .filter(|diag| diag.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "evicting closed text must not remove the dependency from semantic project sources, got {:?}",
        diagnostic_messages(&diagnostics)
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
pub(super) fn lsp_hmi_toml_diagnostics_use_open_source_buffers() {
    let root = temp_dir("trustlsp-hmi-open-buffer-diagnostics");
    let source_path = root.join("src/main.st");
    let hmi_path = root.join("hmi/overview.toml");
    std::fs::create_dir_all(source_path.parent().expect("source parent")).expect("create src");
    std::fs::create_dir_all(hmi_path.parent().expect("hmi parent")).expect("create hmi");

    let disk_source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL;
END_VAR
END_PROGRAM
"#;
    let open_source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL;
    velocity : REAL;
END_VAR
END_PROGRAM
"#;
    let page = r#"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "gauge"
bind = "Main.velocity"
"#;
    std::fs::write(&source_path, disk_source).expect("write disk source");
    std::fs::write(&hmi_path, page).expect("write hmi page");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_config(root_uri, ProjectConfig::load(&root));
    let source_uri = tower_lsp::lsp_types::Url::from_file_path(&source_path).expect("source uri");
    let hmi_uri = tower_lsp::lsp_types::Url::from_file_path(&hmi_path).expect("hmi uri");
    state.open_document(source_uri, 1, open_source.to_string());
    state.open_document(hmi_uri.clone(), 1, page.to_string());

    let diagnostics = full_document_diagnostics(&state, hmi_uri);
    assert!(
        diagnostics.iter().all(|diag| {
            !(diag.message.contains("unknown binding path")
                && diag.message.contains("Main.velocity"))
        }),
        "HMI diagnostics must validate against open ST buffers before disk snapshots, got {:?}",
        diagnostic_messages(&diagnostics)
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
pub(super) fn lsp_hmi_toml_local_property_diagnostics_do_not_require_runtime_compile() {
    let root = temp_dir("trustlsp-hmi-local-property-diagnostics");
    let hmi_path = root.join("hmi/overview.toml");
    std::fs::create_dir_all(hmi_path.parent().expect("hmi parent")).expect("create hmi");

    let page = r#"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "gauge"
bind = "Main.speed"
min = 10
max = 1
"#;
    std::fs::write(&hmi_path, page).expect("write hmi page");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_config(root_uri, ProjectConfig::load(&root));
    let hmi_uri = tower_lsp::lsp_types::Url::from_file_path(&hmi_path).expect("hmi uri");
    state.open_document(hmi_uri.clone(), 1, page.to_string());

    let diagnostics = full_document_diagnostics(&state, hmi_uri);
    assert!(
        diagnostics.iter().any(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code))
                    if code == "HMI_INVALID_WIDGET_PROPERTIES"
            )
        }),
        "local HMI widget-property diagnostics must not depend on runtime compilation, got {:?}",
        diagnostic_messages(&diagnostics)
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
pub(super) fn lsp_will_rename_files_updates_pou_name() {
    let source_decl = r#"
FUNCTION_BLOCK OldName
END_FUNCTION_BLOCK
"#;
    let source_ref = r#"
PROGRAM Main
    VAR
        fb : OldName;
    END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let decl_uri = tower_lsp::lsp_types::Url::parse("file:///OldName.st").unwrap();
    let ref_uri = tower_lsp::lsp_types::Url::parse("file:///Ref.st").unwrap();
    state.open_document(decl_uri.clone(), 1, source_decl.to_string());
    state.open_document(ref_uri.clone(), 1, source_ref.to_string());

    let params = tower_lsp::lsp_types::RenameFilesParams {
        files: vec![tower_lsp::lsp_types::FileRename {
            old_uri: decl_uri.to_string(),
            new_uri: "file:///NewName.st".to_string(),
        }],
    };
    let edit = will_rename_files(&state, params).expect("rename edits");
    let changes = edit.changes.expect("workspace edits");
    let decl_edits = changes.get(&decl_uri).expect("declaration edits");
    let ref_edits = changes.get(&ref_uri).expect("reference edits");
    assert!(decl_edits.iter().any(|edit| edit.new_text == "NewName"));
    assert!(ref_edits.iter().any(|edit| edit.new_text == "NewName"));
}

fn full_document_diagnostics(
    state: &ServerState,
    uri: tower_lsp::lsp_types::Url,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let report = document_diagnostic(
        state,
        tower_lsp::lsp_types::DocumentDiagnosticParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    match report {
        tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
            tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
        ) => full.full_document_diagnostic_report.items,
        other => panic!("expected full document diagnostic report, got {other:?}"),
    }
}

fn diagnostic_messages(diagnostics: &[tower_lsp::lsp_types::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diag| diag.message.clone())
        .collect()
}
