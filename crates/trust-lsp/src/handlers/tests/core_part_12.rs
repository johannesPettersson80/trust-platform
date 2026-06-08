use super::*;

#[test]
pub(super) fn lsp_openot_completion_returns_documented_values_and_keys() {
    let source = r#"
TYPE E_Step : (Idle := 0, Filling := 1) END_TYPE

PROGRAM Main
VAR
    Level : REAL {attribute 'oot' := 'value', };
    Step : E_Step {attribute 'oot' := 'state', 'category' := ''};
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', };
    Started : BOOL {attribute 'oot' := 'message', };
    AckHighPh : BOOL {attribute 'oot' := 'condition', };
    AckEvent : BOOL {attribute 'oot' := 'condition', 'event' := ''};
END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///openot-completion.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let category_cursor =
        source.find("'category' := '").expect("category") + "'category' := '".len();
    let category_labels = completion_labels(&state, &uri, source, category_cursor);
    assert!(category_labels.iter().any(|label| label == "process"));
    assert!(category_labels.iter().any(|label| label == "mode"));
    assert!(category_labels.iter().any(|label| label == "procedural"));

    let value_key_cursor = source.find("'value', ").expect("value comma") + "'value', ".len();
    let value_key_labels = completion_labels(&state, &uri, source, value_key_cursor);
    assert!(value_key_labels.iter().any(|label| label == "sampling"));
    assert!(value_key_labels.iter().any(|label| label == "interval"));

    let key_cursor = source.find("'alarm', ").expect("alarm comma") + "'alarm', ".len();
    let key_labels = completion_labels(&state, &uri, source, key_cursor);
    assert!(key_labels.iter().any(|label| label == "class"));
    assert!(key_labels.iter().any(|label| label == "severity"));
    assert!(key_labels.iter().any(|label| label == "cause"));

    let message_key_cursor =
        source.find("'message', ").expect("message comma") + "'message', ".len();
    let message_key_labels = completion_labels(&state, &uri, source, message_key_cursor);
    assert!(message_key_labels.iter().any(|label| label == "template"));
    assert!(message_key_labels.iter().any(|label| label == "severity"));
    assert!(message_key_labels.iter().any(|label| label == "arg1"));

    let condition_key_cursor =
        source.find("'condition', ").expect("condition comma") + "'condition', ".len();
    let condition_key_labels = completion_labels(&state, &uri, source, condition_key_cursor);
    assert!(condition_key_labels.iter().any(|label| label == "of"));
    assert!(condition_key_labels.iter().any(|label| label == "event"));
    assert!(condition_key_labels.iter().any(|label| label == "by"));
    assert!(condition_key_labels.iter().any(|label| label == "seconds"));
    assert!(condition_key_labels.iter().any(|label| label == "reason"));
    assert!(condition_key_labels.iter().any(|label| label == "comment"));
    assert!(condition_key_labels
        .iter()
        .any(|label| label == "new-priority"));
    assert!(condition_key_labels
        .iter()
        .any(|label| label == "previous-priority"));

    let event_cursor = source.find("'event' := '").expect("event") + "'event' := '".len();
    let event_labels = completion_labels(&state, &uri, source, event_cursor);
    assert!(event_labels.iter().any(|label| label == "acknowledge"));
    assert!(event_labels.iter().any(|label| label == "confirm"));
    assert!(event_labels.iter().any(|label| label == "shelve"));
    assert!(event_labels.iter().any(|label| label == "unshelve"));
    assert!(event_labels.iter().any(|label| label == "suppress"));
    assert!(event_labels.iter().any(|label| label == "unsuppress"));
    assert!(event_labels.iter().any(|label| label == "out-of-service"));
    assert!(event_labels.iter().any(|label| label == "in-service"));
    assert!(event_labels.iter().any(|label| label == "reset"));
    assert!(event_labels.iter().any(|label| label == "comment"));
    assert!(event_labels.iter().any(|label| label == "priority-changed"));
}

#[test]
pub(super) fn lsp_openot_validation_reports_bad_value_and_accepts_good_value() {
    let bad = r#"
PROGRAM Main
VAR
    Step : INT {attribute 'oot' := 'state', 'category' := 'banana'};
END_VAR
END_PROGRAM
"#;
    let bad_state = ServerState::new();
    let bad_uri = tower_lsp::lsp_types::Url::parse("file:///openot-bad.st").unwrap();
    bad_state.open_document(bad_uri.clone(), 1, bad.to_string());
    let bad_diagnostics = document_diagnostics(&bad_state, bad_uri);
    assert!(
        bad_diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_ref().is_some_and(|code| {
                matches!(
                    code,
                    tower_lsp::lsp_types::NumberOrString::String(value) if value == "E308"
                )
            }) && diagnostic.message.contains("unknown OpenOT category")
        }),
        "{bad_diagnostics:#?}"
    );

    let good = r#"
TYPE E_Step : (Idle := 0, Filling := 1) END_TYPE

PROGRAM Main
VAR
    Step : E_Step {attribute 'oot' := 'state', 'category' := 'process'};
END_VAR
END_PROGRAM
"#;
    let good_state = ServerState::new();
    let good_uri = tower_lsp::lsp_types::Url::parse("file:///openot-good.st").unwrap();
    good_state.open_document(good_uri.clone(), 1, good.to_string());
    let good_diagnostics = document_diagnostics(&good_state, good_uri);
    assert!(
        !good_diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_ref().is_some_and(|code| {
                matches!(
                    code,
                    tower_lsp::lsp_types::NumberOrString::String(value) if value == "E308"
                )
            })
        }),
        "{good_diagnostics:#?}"
    );
}

#[test]
pub(super) fn lsp_openot_inlay_hint_shows_emitted_record() {
    let source = r#"
PROGRAM Main
VAR
    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L', 'deadband' := '0.5'};
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm'};
    AckHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'acknowledge'};
END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///openot-inlay.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let params = tower_lsp::lsp_types::InlayHintParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        range: tower_lsp::lsp_types::Range {
            start: super::super::lsp_utils::offset_to_position(source, 0),
            end: super::super::lsp_utils::offset_to_position(source, source.len() as u32),
        },
        work_done_progress_params: Default::default(),
    };
    let hints = inlay_hint(&state, params).expect("inlay hints");
    assert!(
        hints
            .iter()
            .any(|hint| inlay_label_contains(&hint.label, "ValueChanged on delta>0.5 L")),
        "{hints:#?}"
    );
    assert!(
        hints.iter().any(|hint| {
            inlay_label_contains(
                &hint.label,
                "Condition acknowledge for HighPhAlarm on TRUE edge",
            )
        }),
        "{hints:#?}"
    );
}

fn completion_labels(
    state: &ServerState,
    uri: &tower_lsp::lsp_types::Url,
    source: &str,
    offset: usize,
) -> Vec<String> {
    let params = tower_lsp::lsp_types::CompletionParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: super::super::lsp_utils::offset_to_position(source, offset as u32),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };
    let response = completion(state, params).expect("completion response");
    let items = match response {
        tower_lsp::lsp_types::CompletionResponse::Array(items) => items,
        tower_lsp::lsp_types::CompletionResponse::List(list) => list.items,
    };
    items.into_iter().map(|item| item.label).collect()
}

fn document_diagnostics(
    state: &ServerState,
    uri: tower_lsp::lsp_types::Url,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let report = document_diagnostic(state, params);
    let report = match report {
        tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(report) => report,
        _ => panic!("expected diagnostic report"),
    };
    let full = match report {
        tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full) => full,
        _ => panic!("expected full diagnostic report"),
    };
    full.full_document_diagnostic_report.items
}

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
