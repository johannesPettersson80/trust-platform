use super::*;

#[test]
pub(super) fn lsp_hover_variable() {
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
pub(super) fn lsp_hover_uses_utf16_position_and_range_after_supplementary_scalar() {
    let source = r#"
PROGRAM Test
VAR
    speed : INT;
END_VAR
(* 😀 *) speed := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///utf16-hover.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let mut speed_position = utf16_position_at(source, "speed := 1");
    speed_position.character += 1;
    let params = tower_lsp::lsp_types::HoverParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            position: speed_position,
        },
        work_done_progress_params: Default::default(),
    };

    let hover = hover(&state, params).expect("hover after supplementary scalar");
    let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(markup.value.contains("speed"));
    assert!(markup.value.contains("INT"));

    let range = hover.range.expect("hover range");
    let expected_start = utf16_position_at(source, "speed := 1");
    assert_eq!(range.start, expected_start);
    assert_eq!(
        range.end,
        tower_lsp::lsp_types::Position::new(
            expected_start.line,
            expected_start.character + utf16_len("speed"),
        )
    );
}

#[test]
pub(super) fn lsp_references_variable() {
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
pub(super) fn lsp_rename_variable() {
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
    let edit = rename(&state, params)
        .expect("rename response")
        .expect("rename edits");
    let changes = edit.changes.expect("workspace edits");
    let edits = changes.get(&uri).expect("uri edits");
    assert!(edits.len() >= 2);
    assert!(edits.iter().all(|edit| edit.new_text == "y"));
}

#[test]
pub(super) fn lsp_rename_shadow_capture_returns_response_error() {
    let source = r#"
VAR_GLOBAL
    Speed : INT;
END_VAR

PROGRAM Main
VAR
    Temp : INT;
END_VAR
Temp := Speed + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            position: position_at(source, "Speed : INT"),
        },
        new_name: "Temp".to_string(),
        work_done_progress_params: Default::default(),
    };
    let err = rename(&state, params).expect_err("unsafe rename must return a response error");
    assert_eq!(err.code, tower_lsp::jsonrpc::ErrorCode::InvalidParams);
    assert!(
        err.message.contains("different symbol"),
        "expected shadow-capture reason in response error, got {err:?}"
    );
}

#[test]
pub(super) fn lsp_rename_namespace_path_updates_using_and_qualified_names() {
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
    let edit = rename(&state, params)
        .expect("rename response")
        .expect("rename edits");
    let changes = edit.changes.expect("workspace edits");
    let edits = changes.get(&uri).expect("uri edits");
    assert!(edits.iter().any(|edit| edit.new_text == "Company.LibA"));
    assert!(edits.iter().any(|edit| edit.new_text == "Company.LibA.Foo"));
    assert!(edits
        .iter()
        .any(|edit| edit.new_text == "Company.LibA.FooFunc"));
}

#[test]
pub(super) fn lsp_rename_primary_pou_renames_file() {
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
    let edit = rename(&state, params)
        .expect("rename response")
        .expect("rename edits");
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
pub(super) fn lsp_pull_diagnostics_returns_unchanged_and_explainer() {
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
pub(super) fn lsp_diagnostics_no_burst_baseline_reports_real_errors() {
    let source = r#"
PROGRAM Test
    VAR
        A__B : INT;
    END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///diag-baseline.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let doc = state.get_document(&uri).expect("document");

    let ticket = state.begin_semantic_request();
    let diagnostics = super::diagnostics::collect_diagnostics_with_ticket_for_tests(
        &state,
        &uri,
        source,
        doc.file_id,
        ticket,
    );

    assert!(
        diagnostics.iter().any(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E106"
            )
        }),
        "baseline diagnostics must report the real invalid-identifier error, got {diagnostics:?}"
    );
}

#[test]
pub(super) fn lsp_pull_diagnostics_cancelled_request_returns_content_modified() {
    let source = r#"
PROGRAM Test
    VAR
        A__B : INT;
    END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///diag-content-modified.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let stale_ticket = state.begin_semantic_request();
    let _newer_ticket = state.begin_semantic_request();
    let err = super::diagnostics::document_diagnostic_result_with_ticket_for_tests(
        &state,
        tower_lsp::lsp_types::DocumentDiagnosticParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
        stale_ticket,
    )
    .expect_err("cancelled pull diagnostics should return ContentModified");

    assert_eq!(err.code, tower_lsp::jsonrpc::ErrorCode::ContentModified);
}

#[test]
pub(super) fn lsp_push_diagnostics_cancelled_collection_is_skipped_not_empty_publish() {
    let source = r#"
PROGRAM Test
    VAR
        A__B : INT;
    END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///diag-push-cancelled.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let doc = state.get_document(&uri).expect("document");

    let complete_ticket = state.begin_semantic_request();
    let complete = super::diagnostics::publish_diagnostics_items_with_ticket_for_tests(
        &state,
        &uri,
        source,
        doc.file_id,
        complete_ticket,
    )
    .expect("completed publish diagnostics");
    assert!(
        complete.iter().any(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E106"
            )
        }),
        "completed push diagnostics must publish real errors, got {complete:?}"
    );

    let stale_ticket = state.begin_semantic_request();
    let _newer_ticket = state.begin_semantic_request();
    let cancelled = super::diagnostics::publish_diagnostics_items_with_ticket_for_tests(
        &state,
        &uri,
        source,
        doc.file_id,
        stale_ticket,
    );

    assert!(
        cancelled.is_none(),
        "cancelled push diagnostics must skip publish instead of publishing an empty success, got {cancelled:?}"
    );
}
