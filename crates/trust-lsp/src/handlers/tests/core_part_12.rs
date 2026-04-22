use super::*;

#[test]
pub(super) fn lsp_completion_constant_parameter_uses_constant_kind() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    CONSTANT A : INT;
END_VAR
VAR_OUTPUT
    C : INT;
END_VAR
    C := A
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///const-completion.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::CompletionParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            position: position_at(source, "A\nEND_FUNCTION_BLOCK"),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };

    let response = completion(&state, params).expect("completion response");
    let items = match response {
        tower_lsp::lsp_types::CompletionResponse::Array(items) => items,
        tower_lsp::lsp_types::CompletionResponse::List(list) => list.items,
    };
    let a_item = items
        .iter()
        .find(|item| item.label.eq_ignore_ascii_case("A"))
        .expect("A completion");
    assert_eq!(
        a_item.kind,
        Some(tower_lsp::lsp_types::CompletionItemKind::CONSTANT)
    );
}

#[test]
pub(super) fn lsp_hover_constant_parameter_mentions_constant_and_array_star() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    CONSTANT A : ARRAY[*] OF BYTE;
END_VAR
VAR_OUTPUT
    X : BYTE;
END_VAR
    X := A[0];
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///const-hover.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::HoverParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            position: position_at(source, "A[0]"),
        },
        work_done_progress_params: Default::default(),
    };
    let hover = hover(&state, params).expect("hover");
    let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(markup.value.contains("CONSTANT"));
    assert!(markup.value.contains("ARRAY[*] OF BYTE"));
}

#[test]
pub(super) fn lsp_signature_help_constant_parameter_mentions_constant() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_IN_OUT
    CONSTANT Z : ARRAY[*] OF INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    data : ARRAY[0..2] OF INT;
    fb : Fb;
END_VAR
    fb();
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///const-signature.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::SignatureHelpParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            position: position_at(source, "fb();"),
        },
        work_done_progress_params: Default::default(),
        context: None,
    };
    let result = signature_help(&state, params).expect("signature help");
    assert!(result.signatures[0].label.contains("CONSTANT"));
    assert!(result.signatures[0].label.contains("ARRAY[*] OF INT"));
    let parameters = result.signatures[0]
        .parameters
        .as_ref()
        .expect("signature parameters");
    assert_eq!(
        parameters[0].label,
        tower_lsp::lsp_types::ParameterLabel::Simple(
            "Z: ARRAY[*] OF INT (IN_OUT CONSTANT)".to_string()
        )
    );
}

#[test]
pub(super) fn lsp_signature_help_method_var_input_mentions_method_parameters() {
    let source = r#"
FUNCTION_BLOCK Motor
METHOD PUBLIC Start : BOOL
VAR_INPUT
    Var1 : BOOL;
    Var2 : BOOL;
END_VAR
    Start := Var1 AND Var2;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    motor : Motor;
    result : BOOL;
END_VAR
    result := motor.Start();
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///method-signature.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let cursor = source.find("motor.Start(").expect("method call") + "motor.Start(".len();

    let params = tower_lsp::lsp_types::SignatureHelpParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            position: super::super::lsp_utils::offset_to_position(source, cursor as u32),
        },
        work_done_progress_params: Default::default(),
        context: None,
    };
    let result = signature_help(&state, params).expect("signature help");
    assert!(result.signatures[0].label.contains("Start("));
    assert!(result.signatures[0].label.contains("Var1: BOOL"));
    assert!(result.signatures[0].label.contains("Var2: BOOL"));
}

#[test]
pub(super) fn lsp_workspace_symbols_mark_constant_parameters_as_constants() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    CONSTANT A : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace-const.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::WorkspaceSymbolParams {
        query: "A".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let items = workspace_symbol(&state, params).expect("workspace symbols");
    let a_item = items
        .iter()
        .find(|item| item.name == "A")
        .expect("A symbol");
    assert_eq!(a_item.kind, tower_lsp::lsp_types::SymbolKind::CONSTANT);
}

#[test]
pub(super) fn lsp_diagnostics_report_fb_instance_in_constant_sections() {
    let source = r#"
FUNCTION_BLOCK Inner
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
VAR_INPUT
    CONSTANT Inst : Inner;
END_VAR
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///fb-constant-diagnostic.st").unwrap();
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
    assert!(
        diagnostics.iter().any(|diag| diag
            .message
            .contains("function block instances shall not be declared in CONSTANT sections")),
        "expected FB-in-CONSTANT diagnostic, got {:?}",
        diagnostics
            .iter()
            .map(|diag| diag.message.clone())
            .collect::<Vec<_>>()
    );
}
