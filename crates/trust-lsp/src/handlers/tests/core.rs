use super::*;

fn normalize_path_for_assert(path: &std::path::Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut normalized = canonical.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = normalized.strip_prefix("//?/") {
        normalized = stripped.to_string();
    }
    if cfg!(windows) {
        normalized = normalized.to_ascii_lowercase();
    }
    normalized
}

#[test]
fn lsp_diagnostics_short_circuit_when_request_ticket_is_cancelled() {
    let source = r#"
PROGRAM Test
    missing := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let doc = state.get_document(&uri).expect("tracked document");

    let stale_ticket = state.begin_semantic_request();
    let _active_ticket = state.begin_semantic_request();

    let diagnostics = collect_diagnostics_with_ticket_for_tests(
        &state,
        &uri,
        &doc.content,
        doc.file_id,
        stale_ticket,
    );
    assert!(
        diagnostics.is_empty(),
        "cancelled diagnostics ticket should return cleanly without semantic work"
    );
}

#[test]
fn lsp_references_returns_none_when_request_ticket_is_cancelled() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
    x := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let stale_ticket = state.begin_semantic_request();
    let _active_ticket = state.begin_semantic_request();

    let params = tower_lsp::lsp_types::ReferenceParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "x : INT"),
        },
        context: tower_lsp::lsp_types::ReferenceContext {
            include_declaration: true,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let refs = references_with_ticket_for_tests(&state, params, stale_ticket);
    assert!(
        refs.is_none(),
        "cancelled references ticket should short-circuit without semantic work"
    );
}

#[test]
fn lsp_workspace_symbol_returns_none_when_request_ticket_is_cancelled() {
    let source = r#"
PROGRAM TestProgram
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri, 1, source.to_string());

    let stale_ticket = state.begin_semantic_request();
    let _active_ticket = state.begin_semantic_request();

    let params = tower_lsp::lsp_types::WorkspaceSymbolParams {
        query: "Test".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let symbols = workspace_symbol_with_ticket_for_tests(&state, params, stale_ticket);
    assert!(
        symbols.is_none(),
        "cancelled workspace symbol ticket should short-circuit without semantic work"
    );
}

#[test]
fn lsp_rename_returns_none_when_request_ticket_is_cancelled() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let stale_ticket = state.begin_semantic_request();
    let _active_ticket = state.begin_semantic_request();

    let params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "x : INT"),
        },
        new_name: "y".to_string(),
        work_done_progress_params: Default::default(),
    };
    let edit = rename_with_ticket_for_tests(&state, params, stale_ticket);
    assert!(
        edit.is_none(),
        "cancelled rename ticket should short-circuit without semantic work"
    );
}

#[test]
fn lsp_code_action_returns_none_when_request_ticket_is_cancelled() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := TRUE;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let stale_ticket = state.begin_semantic_request();
    let _active_ticket = state.begin_semantic_request();

    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range {
            start: tower_lsp::lsp_types::Position {
                line: 0,
                character: 0,
            },
            end: tower_lsp::lsp_types::Position {
                line: 10,
                character: 0,
            },
        },
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: Vec::new(),
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action_with_ticket_for_tests(&state, params, stale_ticket);
    assert!(
        actions.is_none(),
        "cancelled code action ticket should short-circuit without semantic work"
    );
}

#[test]
fn lsp_hover_variable() {
    let source = r#"
PROGRAM Test
    VAR
        speed : INT;
    END_VAR

    speed := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::HoverParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "speed := 1"),
        },
        work_done_progress_params: Default::default(),
    };
    let hover = hover(&state, params).expect("hover result");
    let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(markup.value.contains("speed"));
    assert!(markup.value.contains("INT"));
}

#[test]
fn lsp_references_variable() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
    x := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::ReferenceParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "x : INT"),
        },
        context: tower_lsp::lsp_types::ReferenceContext {
            include_declaration: true,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let refs = references(&state, params).expect("references");
    assert!(refs.len() >= 2);
}

#[test]
fn lsp_rename_variable() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
    x := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "x : INT"),
        },
        new_name: "y".to_string(),
        work_done_progress_params: Default::default(),
    };
    let edit = rename(&state, params).expect("rename edits");
    let changes = edit.changes.expect("workspace edits");
    let edits = changes.get(&uri).expect("uri edits");
    assert!(edits.len() >= 2);
    assert!(edits.iter().all(|edit| edit.new_text == "y"));
}

#[test]
fn lsp_rename_namespace_path_updates_using_and_qualified_names() {
    let source = r#"
NAMESPACE LibA
TYPE Foo : INT;
END_TYPE
FUNCTION FooFunc : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
    USING LibA;
    VAR
        x : LibA.Foo;
    END_VAR
    x := LibA.FooFunc();
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "LibA\nTYPE"),
        },
        new_name: "Company.LibA".to_string(),
        work_done_progress_params: Default::default(),
    };
    let edit = rename(&state, params).expect("rename edits");
    let changes = edit.changes.expect("workspace edits");
    let edits = changes.get(&uri).expect("uri edits");
    assert!(edits.iter().any(|edit| edit.new_text == "Company.LibA"));
    assert!(edits.iter().any(|edit| edit.new_text == "Company.LibA.Foo"));
    assert!(edits
        .iter()
        .any(|edit| edit.new_text == "Company.LibA.FooFunc"));
}

#[test]
fn lsp_rename_primary_pou_renames_file() {
    let source = r#"
FUNCTION_BLOCK OldName
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///OldName.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "OldName"),
        },
        new_name: "NewName".to_string(),
        work_done_progress_params: Default::default(),
    };
    let edit = rename(&state, params).expect("rename edits");
    assert!(edit.changes.is_none(), "expected document changes");

    let document_changes = edit.document_changes.expect("document changes");
    let document_changes = match document_changes {
        tower_lsp::lsp_types::DocumentChanges::Operations(ops) => ops,
        _ => panic!("expected document change operations"),
    };

    let new_uri = tower_lsp::lsp_types::Url::parse("file:///NewName.st").unwrap();
    let has_rename = document_changes.iter().any(|change| {
        matches!(
            change,
            tower_lsp::lsp_types::DocumentChangeOperation::Op(
                tower_lsp::lsp_types::ResourceOp::Rename(rename)
            ) if rename.old_uri == uri && rename.new_uri == new_uri
        )
    });
    assert!(has_rename, "expected rename file operation");

    let has_text_edit = document_changes.iter().any(|change| match change {
        tower_lsp::lsp_types::DocumentChangeOperation::Edit(edit) => {
            edit.edits.iter().any(|edit| {
                matches!(
                    edit,
                    tower_lsp::lsp_types::OneOf::Left(edit) if edit.new_text == "NewName"
                )
            })
        }
        _ => false,
    });
    assert!(has_text_edit, "expected text edits for new POU name");
}

#[test]
fn lsp_pull_diagnostics_returns_unchanged_and_explainer() {
    let source = r#"
PROGRAM Test
    VAR
        A__B : INT;
    END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///diag.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
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
    let result_id = full
        .full_document_diagnostic_report
        .result_id
        .clone()
        .expect("result id");
    let diagnostics = full.full_document_diagnostic_report.items;
    let invalid_identifier = diagnostics
        .iter()
        .find(|diag| match diag.code.as_ref() {
            Some(tower_lsp::lsp_types::NumberOrString::String(code)) => code == "E106",
            _ => false,
        })
        .expect("E106 diagnostic");
    let data = invalid_identifier
        .data
        .as_ref()
        .and_then(|value| value.as_object());
    let explain = data.and_then(|map| map.get("explain"));
    assert!(explain.is_some(), "expected IEC explainer data");

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: Some(result_id),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let report = document_diagnostic(&state, params);
    let report = match report {
        tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(report) => report,
        _ => panic!("expected diagnostic report"),
    };
    assert!(
        matches!(
            report,
            tower_lsp::lsp_types::DocumentDiagnosticReport::Unchanged(_)
        ),
        "expected unchanged diagnostic report"
    );
}

#[test]
fn lsp_supports_virtual_document_uris() {
    let state = ServerState::new();
    let uri =
        tower_lsp::lsp_types::Url::parse("vscode-notebook-cell:/workspace/notebook#cell1").unwrap();
    state.open_document(uri.clone(), 1, "PROGRAM Test END_PROGRAM".to_string());

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
    assert!(
        matches!(
            report,
            tower_lsp::lsp_types::DocumentDiagnosticReport::Full(_)
        ),
        "expected full diagnostic report"
    );
}

#[test]
fn lsp_diagnostics_respect_config_toggles() {
    let mut body = String::new();
    for _ in 0..15 {
        body.push_str("    IF TRUE THEN x := x + 1; END_IF;\n");
    }
    let source = format!(
        r#"
PROGRAM Test
    VAR
        x : INT;
        y : REAL;
    END_VAR
    CASE x OF
        1: x := 1;
    END_CASE;
{body}
    y := 1;
END_PROGRAM
"#
    );
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            diagnostics: DiagnosticSettings {
                warn_unused: true,
                warn_unreachable: true,
                warn_missing_else: false,
                warn_implicit_conversion: false,
                warn_shadowed: true,
                warn_deprecated: true,
                warn_complexity: false,
                warn_nondeterminism: true,
                severity_overrides: Default::default(),
            },
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/diag.st").unwrap();
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
    let codes: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .filter_map(|diag| diag.code.as_ref())
        .map(|code| match code {
            tower_lsp::lsp_types::NumberOrString::String(value) => value.clone(),
            tower_lsp::lsp_types::NumberOrString::Number(value) => value.to_string(),
        })
        .collect();

    assert!(
        !codes.iter().any(|code| code == "W004"),
        "expected MissingElse warning to be filtered"
    );
    assert!(
        !codes.iter().any(|code| code == "W005"),
        "expected ImplicitConversion warning to be filtered"
    );
    assert!(
        !codes.iter().any(|code| code == "W008"),
        "expected HighComplexity warning to be filtered"
    );
}

#[test]
fn lsp_learner_diagnostics_include_did_you_mean_and_conversion_guidance() {
    let source = r#"
TYPE MotorConfig : STRUCT
    speed : INT;
END_STRUCT
END_TYPE

PROGRAM Test
VAR
    speedValue : INT;
    cfg : MotroConfig;
    flag : BOOL;
END_VAR
    speadValue := 1;
    flag := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///learner-hints.st").unwrap();
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

    let undefined_identifier = diagnostics
        .iter()
        .find(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E101"
            )
        })
        .expect("expected E101 diagnostic");
    let id_hints: Vec<&str> = undefined_identifier
        .related_information
        .as_ref()
        .map(|items| items.iter().map(|item| item.message.as_str()).collect())
        .unwrap_or_default();
    assert!(
        id_hints
            .iter()
            .any(|hint| hint.contains("Did you mean 'speedValue'?")),
        "expected did-you-mean hint for E101, got {id_hints:?}"
    );

    let undefined_type = diagnostics
        .iter()
        .find(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E102"
            )
        })
        .expect("expected E102 diagnostic");
    let type_hints: Vec<&str> = undefined_type
        .related_information
        .as_ref()
        .map(|items| items.iter().map(|item| item.message.as_str()).collect())
        .unwrap_or_default();
    assert!(
        type_hints
            .iter()
            .any(|hint| hint.contains("Did you mean 'MotorConfig'?")),
        "expected did-you-mean hint for E102, got {type_hints:?}"
    );

    let incompatible_assignment = diagnostics
        .iter()
        .find(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E203"
            )
        })
        .expect("expected E203 diagnostic");
    let conversion_hints: Vec<&str> = incompatible_assignment
        .related_information
        .as_ref()
        .map(|items| items.iter().map(|item| item.message.as_str()).collect())
        .unwrap_or_default();
    assert!(
        conversion_hints
            .iter()
            .any(|hint| hint.contains("_TO_BOOL(<expr>)")),
        "expected explicit conversion hint, got {conversion_hints:?}"
    );
}

#[test]
fn lsp_learner_diagnostics_include_syntax_habit_hints() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
    y : INT;
END_VAR
IF x == y THEN
    x = 1;
END_IF;
IF TRUE && FALSE THEN
    x := 2;
END_IF;
}
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///syntax-hints.st").unwrap();
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
    let hints: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .flat_map(|diag| {
            diag.related_information
                .iter()
                .flat_map(|items| items.iter().map(|item| item.message.clone()))
        })
        .collect();

    assert!(
        hints
            .iter()
            .any(|hint| hint.contains("use '=' for comparison")),
        "expected == guidance, got {hints:?}"
    );
    assert!(
        hints
            .iter()
            .any(|hint| hint.contains("assignments use ':='")),
        "expected assignment guidance, got {hints:?}"
    );
    assert!(
        hints.iter().any(|hint| hint.contains("AND instead of &&")),
        "expected && guidance, got {hints:?}"
    );
    assert!(
        hints
            .iter()
            .any(|hint| hint.contains("END_* keywords for block endings")),
        "expected brace guidance, got {hints:?}"
    );
}

#[test]
fn lsp_learner_diagnostics_no_hint_noise_on_valid_code() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
    y : INT;
END_VAR
x := 1;
y := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///no-noise.st").unwrap();
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

    let hint_messages: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .flat_map(|diag| {
            diag.related_information
                .iter()
                .flat_map(|items| items.iter().map(|item| item.message.clone()))
        })
        .filter(|message| message.starts_with("Hint:"))
        .collect();
    assert!(
        hint_messages.is_empty(),
        "expected no learner hints on valid code, got {hint_messages:?}"
    );
}

#[test]
fn lsp_config_diagnostics_report_library_dependency_issues() {
    let config = r#"
[project]
include_paths = ["src"]

[[libraries]]
name = "Core"
path = "libs/core"
version = "1.0"

[[libraries]]
name = "App"
path = "libs/app"
version = "1.0"
dependencies = [{ name = "Core", version = "2.0" }, { name = "Missing" }]
"#;
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri]);

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/trust-lsp.toml").unwrap();
    state.open_document(uri.clone(), 1, config.to_string());

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
    let codes: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .filter_map(|diag| diag.code.as_ref())
        .map(|code| match code {
            tower_lsp::lsp_types::NumberOrString::String(value) => value.clone(),
            tower_lsp::lsp_types::NumberOrString::Number(value) => value.to_string(),
        })
        .collect();

    assert!(codes.contains(&"L001".to_string()));
    assert!(codes.contains(&"L002".to_string()));
}

#[test]
fn lsp_config_diagnostics_report_dependency_cycle_issues() {
    let root = temp_dir("trustlsp-cycle-config");
    let dep_a = root.join("deps/lib-a");
    let dep_b = root.join("deps/lib-b");
    std::fs::create_dir_all(&dep_a).expect("create dep a");
    std::fs::create_dir_all(&dep_b).expect("create dep b");

    let config = r#"
[dependencies]
LibA = { path = "deps/lib-a" }
"#;
    std::fs::write(root.join("trust-lsp.toml"), config).expect("write root config");
    std::fs::write(
        dep_a.join("trust-lsp.toml"),
        r#"
[dependencies]
LibB = { path = "../lib-b" }
"#,
    )
    .expect("write dep a");
    std::fs::write(
        dep_b.join("trust-lsp.toml"),
        r#"
[dependencies]
LibA = { path = "../lib-a" }
"#,
    )
    .expect("write dep b");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_folders(vec![root_uri.clone()]);

    let uri =
        tower_lsp::lsp_types::Url::from_file_path(root.join("trust-lsp.toml")).expect("config uri");
    state.open_document(uri.clone(), 1, config.to_string());

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
    let codes: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .filter_map(|diag| diag.code.as_ref())
        .map(|code| match code {
            tower_lsp::lsp_types::NumberOrString::String(value) => value.clone(),
            tower_lsp::lsp_types::NumberOrString::Number(value) => value.to_string(),
        })
        .collect();

    assert!(codes.contains(&"L004".to_string()));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_workspace_symbols_include_dependency_sources() {
    let root = temp_dir("trustlsp-dependency-symbols");
    let dep = root.join("deps/vendor");
    std::fs::create_dir_all(root.join("sources")).expect("create sources");
    std::fs::create_dir_all(dep.join("sources")).expect("create dependency sources");
    std::fs::write(
        root.join("trust-lsp.toml"),
        r#"
[project]
include_paths = ["sources"]

[dependencies]
Vendor = { path = "deps/vendor", version = "1.0.0" }
"#,
    )
    .expect("write root config");
    std::fs::write(
        dep.join("trust-lsp.toml"),
        r#"
[package]
version = "1.0.0"
"#,
    )
    .expect("write dependency config");
    std::fs::write(
        root.join("sources/main.st"),
        r#"
PROGRAM Main
VAR
    out : INT;
END_VAR
out := VendorDouble(2);
END_PROGRAM
"#,
    )
    .expect("write root source");
    std::fs::write(
        dep.join("sources/vendor.st"),
        r#"
FUNCTION VendorDouble : INT
VAR_INPUT
    x : INT;
END_VAR
VendorDouble := x * 2;
END_FUNCTION
"#,
    )
    .expect("write dependency source");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_folders(vec![root_uri]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        let client = test_client();
        index_workspace(&client, &state).await;
    });

    let params = tower_lsp::lsp_types::WorkspaceSymbolParams {
        query: "VendorDouble".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let symbols = workspace_symbol(&state, params).expect("workspace symbols");
    let dep_source = dep
        .join("sources/vendor.st")
        .canonicalize()
        .expect("dep source");
    let found_dependency_symbol = symbols.iter().any(|symbol| {
        symbol.name == "VendorDouble"
            && symbol.location.uri.to_file_path().ok().is_some_and(|path| {
                normalize_path_for_assert(&path) == normalize_path_for_assert(&dep_source)
            })
    });
    assert!(
        found_dependency_symbol,
        "expected dependency symbol to be indexed"
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_external_diagnostics_provide_quick_fixes() {
    let root = temp_dir("trustlsp-external-diag");
    let lint_path = root.join("lint.json");
    std::fs::write(
        &lint_path,
        r#"
[
  {
    "path": "main.st",
    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 7 } },
    "severity": "warning",
    "code": "X001",
    "message": "External issue",
    "fix": {
      "title": "Fix external issue",
      "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 7 } },
      "new_text": "PROGRAM"
    }
  }
]
"#,
    )
    .expect("write lint json");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri.clone(),
        ProjectConfig {
            root: root.clone(),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: vec![lint_path.clone()],
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::from_file_path(root.join("main.st")).unwrap();
    let source = "PROGRAM Main\nEND_PROGRAM\n";
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
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
    let external = diagnostics
        .iter()
        .find(|diag| {
            diag.code.as_ref().is_some_and(|code| {
            matches!(code, tower_lsp::lsp_types::NumberOrString::String(value) if value == "X001")
        })
        })
        .expect("external diagnostic");

    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: external.range,
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: vec![external.clone()],
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let actions = code_action(&state, params).expect("code actions");
    let has_external = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(action) => {
            action.title.contains("Fix external issue")
        }
        _ => false,
    });
    assert!(has_external, "expected external quick fix");

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_document_symbols_include_configuration_hierarchy() {
    let source = r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION

PROGRAM Main
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///config.st").unwrap();
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
    let names: Vec<String> = symbols.iter().map(|symbol| symbol.name.clone()).collect();
    assert!(names.iter().any(|name| name.contains("Conf")));
    assert!(names.iter().any(|name| name.contains("R")));
    assert!(names.iter().any(|name| name.contains("Fast")));
    assert!(names.iter().any(|name| name.contains("P1")));

    let task_container = symbols
        .iter()
        .find(|symbol| symbol.name.contains("Fast"))
        .and_then(|symbol| symbol.container_name.clone());
    assert_eq!(task_container.as_deref(), Some("R"));
}

#[test]
fn lsp_document_symbols_include_members() {
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
fn lsp_oop_access_diagnostics_include_explainer_and_hint() {
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
fn lsp_workspace_diagnostics_supports_unchanged_reports() {
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
fn lsp_will_rename_files_updates_pou_name() {
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

#[test]
fn lsp_will_rename_files_updates_using_namespace() {
    let source_decl = r#"
NAMESPACE Lib
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE
"#;
    let source_ref = r#"
USING Lib;
PROGRAM Main
    VAR
        x : INT;
    END_VAR
    x := Foo();
END_PROGRAM
"#;
    let state = ServerState::new();
    let decl_uri = tower_lsp::lsp_types::Url::parse("file:///Lib.st").unwrap();
    let ref_uri = tower_lsp::lsp_types::Url::parse("file:///Main.st").unwrap();
    state.open_document(decl_uri.clone(), 1, source_decl.to_string());
    state.open_document(ref_uri.clone(), 1, source_ref.to_string());

    let params = tower_lsp::lsp_types::RenameFilesParams {
        files: vec![tower_lsp::lsp_types::FileRename {
            old_uri: decl_uri.to_string(),
            new_uri: "file:///NewLib.st".to_string(),
        }],
    };
    let edit = will_rename_files(&state, params).expect("rename edits");
    let changes = edit.changes.expect("workspace edits");
    let decl_edits = changes.get(&decl_uri).expect("namespace edits");
    let ref_edits = changes.get(&ref_uri).expect("using edits");
    assert!(decl_edits.iter().any(|edit| edit.new_text == "NewLib"));
    assert!(ref_edits.iter().any(|edit| edit.new_text == "NewLib"));
}

#[test]
fn lsp_workspace_symbols() {
    let source_one = r#"
FUNCTION_BLOCK Counter
    VAR
        value : INT;
    END_VAR
END_FUNCTION_BLOCK
"#;
    let source_two = r#"
PROGRAM Main
    VAR
        counter : Counter;
    END_VAR
END_PROGRAM
"#;

    let state = ServerState::new();
    let uri_one = tower_lsp::lsp_types::Url::parse("file:///one.st").unwrap();
    let uri_two = tower_lsp::lsp_types::Url::parse("file:///two.st").unwrap();
    state.open_document(uri_one.clone(), 1, source_one.to_string());
    state.open_document(uri_two.clone(), 1, source_two.to_string());

    let params = tower_lsp::lsp_types::WorkspaceSymbolParams {
        query: "counter".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let items = workspace_symbol(&state, params).expect("workspace symbols");
    assert!(
        items
            .iter()
            .any(|item| { item.name.starts_with("Counter") && item.location.uri == uri_one }),
        "Expected to find Counter symbol from first file"
    );
}

#[test]
fn lsp_workspace_symbols_respect_root_visibility_and_priority() {
    let source = r#"
FUNCTION_BLOCK Counter
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let root_one = temp_dir("trustlsp-root-one");
    let root_two = temp_dir("trustlsp-root-two");
    let root_one_uri = tower_lsp::lsp_types::Url::from_file_path(&root_one).unwrap();
    let root_two_uri = tower_lsp::lsp_types::Url::from_file_path(&root_two).unwrap();
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

    let uri_one = tower_lsp::lsp_types::Url::from_file_path(root_one.join("one.st")).unwrap();
    let uri_two = tower_lsp::lsp_types::Url::from_file_path(root_two.join("two.st")).unwrap();
    state.open_document(uri_one.clone(), 1, source.to_string());
    state.open_document(uri_two.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::WorkspaceSymbolParams {
        query: "".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let items = workspace_symbol(&state, params).expect("workspace symbols");
    assert!(items.iter().any(|item| item.location.uri == uri_one));
    assert!(!items.iter().any(|item| item.location.uri == uri_two));

    let params = tower_lsp::lsp_types::WorkspaceSymbolParams {
        query: "counter".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let items = workspace_symbol(&state, params).expect("workspace symbols");
    let counters: Vec<_> = items
        .iter()
        .filter(|item| item.name.starts_with("Counter"))
        .collect();
    assert!(counters.len() >= 2);
    assert_eq!(counters[0].location.uri, uri_one);
    assert_eq!(counters[1].location.uri, uri_two);
}

#[test]
fn lsp_document_highlight_variable() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
    x := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentHighlightParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "x := 1"),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let highlights = document_highlight(&state, params).expect("document highlights");
    assert!(highlights.len() >= 3);
}

#[test]
fn lsp_semantic_tokens_delta() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
END_PROGRAM
"#;
    let updated = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let full_params = tower_lsp::lsp_types::SemanticTokensParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let full = semantic_tokens_full(&state, full_params).expect("semantic tokens full");
    let tower_lsp::lsp_types::SemanticTokensResult::Tokens(tokens) = full else {
        panic!("expected semantic tokens");
    };
    let previous_result_id = tokens.result_id.expect("semantic tokens result id");

    state.update_document(&uri, 2, updated.to_string());

    let delta_params = tower_lsp::lsp_types::SemanticTokensDeltaParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        previous_result_id,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let delta = semantic_tokens_full_delta(&state, delta_params).expect("semantic tokens delta");
    match delta {
        tower_lsp::lsp_types::SemanticTokensFullDeltaResult::TokensDelta(delta) => {
            assert!(delta.result_id.is_some());
            assert!(!delta.edits.is_empty());
        }
        _ => panic!("expected semantic tokens delta response"),
    }
}

#[test]
fn lsp_linked_editing_ranges() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
    x := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::LinkedEditingRangeParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "x := 1"),
        },
        work_done_progress_params: Default::default(),
    };

    let ranges = linked_editing_range(&state, params).expect("linked editing ranges");
    assert_eq!(ranges.ranges.len(), 4);
}

#[test]
fn lsp_inlay_hints_parameters() {
    let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(1, 2);
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "Add(1");
    let end_offset = source.find(");").expect("call end");
    let end = super::lsp_utils::offset_to_position(source, end_offset as u32);

    let params = tower_lsp::lsp_types::InlayHintParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range { start, end },
        work_done_progress_params: Default::default(),
    };

    let hints = inlay_hint(&state, params).expect("inlay hints");
    assert_eq!(hints.len(), 2);
    assert!(hints
        .iter()
        .any(|hint| inlay_label_contains(&hint.label, "A")));
    assert!(hints
        .iter()
        .any(|hint| inlay_label_contains(&hint.label, "B")));
}

#[test]
fn lsp_inline_values_constants() {
    let constants = r#"
CONFIGURATION Conf
VAR_GLOBAL CONSTANT
    ANSWER : INT := 42;
END_VAR
END_CONFIGURATION
"#;
    let program = r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
VAR_EXTERNAL CONSTANT
    ANSWER : INT;
END_VAR
    x := ANSWER;
END_PROGRAM
"#;
    let state = ServerState::new();
    let const_uri = tower_lsp::lsp_types::Url::parse("file:///constants.st").unwrap();
    let prog_uri = tower_lsp::lsp_types::Url::parse("file:///main.st").unwrap();
    state.open_document(const_uri, 1, constants.to_string());
    state.open_document(prog_uri.clone(), 1, program.to_string());

    let end = super::lsp_utils::offset_to_position(program, program.len() as u32);
    let params = tower_lsp::lsp_types::InlineValueParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
            uri: prog_uri.clone(),
        },
        range: tower_lsp::lsp_types::Range {
            start: tower_lsp::lsp_types::Position::new(0, 0),
            end,
        },
        context: tower_lsp::lsp_types::InlineValueContext {
            frame_id: 0,
            stopped_location: tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position::new(0, 0),
                end,
            },
        },
        work_done_progress_params: Default::default(),
    };

    let values = inline_value(&state, params).expect("inline values");
    let has_answer = values.iter().any(|value| match value {
        tower_lsp::lsp_types::InlineValue::Text(text) => text.text == " = 42",
        _ => false,
    });
    assert!(has_answer);
}

fn runtime_inline_values_source() -> &'static str {
    r#"
CONFIGURATION Conf
VAR_GLOBAL
    g : INT;
END_VAR
VAR_GLOBAL RETAIN
    r : INT;
END_VAR
END_CONFIGURATION

PROGRAM Test
VAR
    x : INT;
END_VAR
    x := x + g + r;
END_PROGRAM
"#
}

fn runtime_inline_values_params(
    uri: tower_lsp::lsp_types::Url,
    source: &str,
) -> tower_lsp::lsp_types::InlineValueParams {
    tower_lsp::lsp_types::InlineValueParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        range: tower_lsp::lsp_types::Range {
            start: position_at(source, "x := x"),
            end: position_at(source, "END_PROGRAM"),
        },
        context: tower_lsp::lsp_types::InlineValueContext {
            frame_id: 1,
            stopped_location: tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position::new(0, 0),
                end: tower_lsp::lsp_types::Position::new(0, 0),
            },
        },
        work_done_progress_params: Default::default(),
    }
}

#[test]
fn lsp_inline_values_fetch_runtime_values_from_control_stub() {
    let (endpoint, handle) = spawn_control_stub();
    let source = runtime_inline_values_source();
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            runtime: RuntimeConfig {
                control_endpoint: Some(endpoint),
                control_auth_token: None,
            },
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = runtime_inline_values_params(uri, source);

    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();

    assert!(texts.iter().any(|text| text == " = DInt(7)"));
    assert!(texts.iter().any(|text| text == " = DInt(11)"));
    assert!(texts.iter().any(|text| text == " = DInt(42)"));

    handle.join().expect("control stub thread");
}

#[test]
fn lsp_inline_values_runtime_override_accepts_camel_case_client_settings() {
    let (endpoint, handle) = spawn_control_stub();
    let source = runtime_inline_values_source();
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_config(json!({
        "stLsp": {
            "runtime": {
                "inlineValuesEnabled": true,
                "controlEndpointEnabled": true,
                "controlEndpoint": endpoint,
            }
        }
    }));

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let params = runtime_inline_values_params(uri, source);
    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|text| text == " = DInt(11)"));
    assert!(texts.iter().any(|text| text == " = DInt(42)"));

    handle.join().expect("control stub thread");
}

#[test]
fn lsp_inline_values_runtime_override_accepts_snake_case_client_settings() {
    let (endpoint, handle) = spawn_control_stub();
    let source = runtime_inline_values_source();
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_config(json!({
        "trust_lsp": {
            "runtime": {
                "inline_values_enabled": true,
                "control_endpoint_enabled": true,
                "control_endpoint": endpoint,
            }
        }
    }));

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let params = runtime_inline_values_params(uri, source);
    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|text| text == " = DInt(11)"));
    assert!(texts.iter().any(|text| text == " = DInt(42)"));

    handle.join().expect("control stub thread");
}

#[test]
fn lsp_inline_values_runtime_override_prefers_camel_case_when_aliases_conflict() {
    let (endpoint, handle) = spawn_control_stub();
    let endpoint_addr = endpoint
        .strip_prefix("tcp://")
        .map(str::to_string)
        .expect("tcp endpoint");
    let source = runtime_inline_values_source();
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_config(json!({
        "stLsp": {
            "runtime": {
                "inlineValuesEnabled": false,
                "inline_values_enabled": true,
                "controlEndpointEnabled": false,
                "control_endpoint_enabled": true,
                "controlEndpoint": endpoint.clone(),
                "control_endpoint": endpoint,
            }
        }
    }));

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let params = runtime_inline_values_params(uri, source);
    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();

    assert!(
        texts.iter().all(|text| text != " = DInt(11)"),
        "camelCase control flag should disable runtime fetch"
    );
    assert!(
        texts.iter().all(|text| text != " = DInt(42)"),
        "camelCase control flag should disable runtime fetch"
    );

    let _ = std::net::TcpStream::connect(endpoint_addr);
    handle.join().expect("control stub thread");
}

#[test]
fn lsp_inline_values_merge_instances_into_locals() {
    let (endpoint, handle) = spawn_control_stub_with_instances("TestProgram#1");
    let source = r#"
PROGRAM TestProgram
VAR
    x : DINT;
END_VAR
    x := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            runtime: RuntimeConfig {
                control_endpoint: Some(endpoint),
                control_auth_token: None,
            },
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::InlineValueParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        range: tower_lsp::lsp_types::Range {
            start: position_at(source, "x := x"),
            end: position_at(source, "END_PROGRAM"),
        },
        context: tower_lsp::lsp_types::InlineValueContext {
            frame_id: 1,
            stopped_location: tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position::new(0, 0),
                end: tower_lsp::lsp_types::Position::new(0, 0),
            },
        },
        work_done_progress_params: Default::default(),
    };

    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();

    assert!(texts.iter().any(|text| text == " = DInt(9)"));

    handle.join().expect("control stub thread");
}

#[test]
fn lsp_inline_values_merge_instances_with_namespace() {
    let (endpoint, handle) = spawn_control_stub_with_instances("Ns.TestProgram#1");
    let source = r#"
NAMESPACE Ns
PROGRAM TestProgram
VAR
    x : DINT;
END_VAR
    x := x + 1;
END_PROGRAM
END_NAMESPACE
"#;
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            runtime: RuntimeConfig {
                control_endpoint: Some(endpoint),
                control_auth_token: None,
            },
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::InlineValueParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        range: tower_lsp::lsp_types::Range {
            start: position_at(source, "x := x"),
            end: position_at(source, "END_PROGRAM"),
        },
        context: tower_lsp::lsp_types::InlineValueContext {
            frame_id: 1,
            stopped_location: tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position::new(0, 0),
                end: tower_lsp::lsp_types::Position::new(0, 0),
            },
        },
        work_done_progress_params: Default::default(),
    };

    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();

    assert!(texts.iter().any(|text| text == " = DInt(9)"));

    handle.join().expect("control stub thread");
}

#[test]
fn lsp_tutorial_examples_no_unexpected_diagnostics_snapshot() {
    let tutorials = [
        (
            "01_hello_counter.st",
            include_str!("../../../../../examples/tutorials/01_hello_counter.st"),
        ),
        (
            "02_blinker.st",
            include_str!("../../../../../examples/tutorials/02_blinker.st"),
        ),
        (
            "03_traffic_light.st",
            include_str!("../../../../../examples/tutorials/03_traffic_light.st"),
        ),
        (
            "04_tank_level.st",
            include_str!("../../../../../examples/tutorials/04_tank_level.st"),
        ),
        (
            "05_motor_starter.st",
            include_str!("../../../../../examples/tutorials/05_motor_starter.st"),
        ),
        (
            "06_recipe_manager.st",
            include_str!("../../../../../examples/tutorials/06_recipe_manager.st"),
        ),
        (
            "07_pid_loop.st",
            include_str!("../../../../../examples/tutorials/07_pid_loop.st"),
        ),
        (
            "08_conveyor_system.st",
            include_str!("../../../../../examples/tutorials/08_conveyor_system.st"),
        ),
        (
            "09_simulation_coupling.st",
            include_str!("../../../../../examples/tutorials/09_simulation_coupling.st"),
        ),
    ];

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").expect("workspace uri");
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
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
            diagnostics: DiagnosticSettings {
                warn_unused: false,
                warn_unreachable: false,
                warn_missing_else: false,
                warn_implicit_conversion: false,
                warn_shadowed: false,
                warn_deprecated: false,
                warn_complexity: false,
                warn_nondeterminism: false,
                severity_overrides: Default::default(),
            },
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    let mut output = serde_json::Map::new();

    for (name, source) in tutorials {
        let uri = tower_lsp::lsp_types::Url::parse(&format!(
            "file:///workspace/examples/tutorials/{name}"
        ))
        .expect("tutorial uri");
        state.open_document(uri.clone(), 1, source.to_string());

        let file_id = state.get_document(&uri).expect("tutorial document").file_id;
        let diagnostics = super::diagnostics::collect_diagnostics_with_ticket(
            &state, &uri, source, file_id, None,
        );

        let summary: Vec<String> = diagnostics
            .iter()
            .map(|diag| {
                let code = match diag.code.as_ref() {
                    Some(tower_lsp::lsp_types::NumberOrString::String(value)) => value.clone(),
                    Some(tower_lsp::lsp_types::NumberOrString::Number(value)) => value.to_string(),
                    None => "NO_CODE".to_string(),
                };
                let severity = match diag.severity {
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR) => "error",
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING) => "warning",
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION) => "info",
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT) => "hint",
                    _ => "none",
                };
                format!(
                    "{code}|{severity}|{}:{}-{}:{}|{}",
                    diag.range.start.line,
                    diag.range.start.character,
                    diag.range.end.line,
                    diag.range.end.character,
                    diag.message
                )
            })
            .collect();

        assert!(
            summary.is_empty(),
            "expected no diagnostics for {name}, got {summary:?}"
        );
        output.insert(
            name.to_string(),
            serde_json::to_value(summary).expect("serialize diagnostics"),
        );
    }

    let rendered =
        serde_json::to_string_pretty(&Value::Object(output)).expect("serialize diagnostics");
    insta::with_settings!({ snapshot_path => "../snapshots" }, {
        insta::assert_snapshot!("lsp_tutorial_examples_diagnostics", rendered);
    });
}

fn spawn_control_stub() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind control stub");
    let addr = listener.local_addr().expect("control stub addr");
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept control stub");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut writer = std::io::BufWriter::new(stream);

        for _ in 0..4 {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read line") == 0 {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let payload: serde_json::Value =
                serde_json::from_str(line.trim()).expect("parse payload");
            let id = payload
                .get("id")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let kind = payload
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let response = match kind {
                "debug.scopes" => json!({
                    "id": id,
                    "ok": true,
                    "result": {
                        "scopes": [
                            { "name": "Locals", "variablesReference": 1 },
                            { "name": "Globals", "variablesReference": 2 },
                            { "name": "Retain", "variablesReference": 3 },
                        ]
                    }
                }),
                "debug.variables" => {
                    let reference = payload
                        .get("params")
                        .and_then(|value| value.get("variables_reference"))
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0);
                    let variables = match reference {
                        1 => vec![json!({
                            "name": "x",
                            "value": "DInt(7)",
                            "variablesReference": 0
                        })],
                        2 => vec![json!({
                            "name": "g",
                            "value": "DInt(11)",
                            "variablesReference": 0
                        })],
                        3 => vec![json!({
                            "name": "r",
                            "value": "DInt(42)",
                            "variablesReference": 0
                        })],
                        _ => Vec::new(),
                    };
                    json!({
                        "id": id,
                        "ok": true,
                        "result": { "variables": variables }
                    })
                }
                _ => json!({ "id": id, "ok": false, "error": "unknown request" }),
            };
            writeln!(writer, "{response}").expect("write response");
            writer.flush().expect("flush response");
        }
    });

    (format!("tcp://{addr}"), handle)
}

fn spawn_control_stub_with_instances(instance_name: &str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind control stub");
    let addr = listener.local_addr().expect("control stub addr");
    let instance_name = instance_name.to_string();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept control stub");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut writer = std::io::BufWriter::new(stream);

        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read line") == 0 {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let payload: serde_json::Value =
                serde_json::from_str(line.trim()).expect("parse payload");
            let id = payload
                .get("id")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let kind = payload
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let response = match kind {
                "debug.scopes" => json!({
                    "id": id,
                    "ok": true,
                    "result": {
                        "scopes": [
                            { "name": "Locals", "variablesReference": 1 },
                            { "name": "Instances", "variablesReference": 2 },
                        ]
                    }
                }),
                "debug.variables" => {
                    let reference = payload
                        .get("params")
                        .and_then(|value| value.get("variables_reference"))
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0);
                    let variables = match reference {
                        1 => vec![json!({
                            "name": "temp",
                            "value": "DInt(3)",
                            "variablesReference": 0
                        })],
                        2 => vec![json!({
                            "name": instance_name.clone(),
                            "value": "Instance(1)",
                            "variablesReference": 10
                        })],
                        10 => vec![json!({
                            "name": "x",
                            "value": "DInt(9)",
                            "variablesReference": 0
                        })],
                        _ => Vec::new(),
                    };
                    json!({
                        "id": id,
                        "ok": true,
                        "result": { "variables": variables }
                    })
                }
                _ => json!({ "id": id, "ok": false, "error": "unknown request" }),
            };
            writeln!(writer, "{response}").expect("write response");
            writer.flush().expect("flush response");
        }
    });

    (format!("tcp://{addr}"), handle)
}
