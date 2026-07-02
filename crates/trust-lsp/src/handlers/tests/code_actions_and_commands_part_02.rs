use super::*;

#[test]
fn lsp_code_action_incompatible_assignment_conversion() {
    let source = r#"
PROGRAM Test
VAR
    x : BOOL;
END_VAR
    x := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "1;");
    let end = super::lsp_utils::offset_to_position(source, (source.find("1;").unwrap() + 1) as u32);

    let diagnostic = tower_lsp::lsp_types::Diagnostic {
        range: tower_lsp::lsp_types::Range { start, end },
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "E203".to_string(),
        )),
        source: Some("truST".to_string()),
        message: "cannot assign 'INT' to 'BOOL'".to_string(),
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
    let has_conversion_action = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
            code_action.title.contains("conversion")
                && code_action
                    .edit
                    .as_ref()
                    .and_then(|edit| edit.changes.as_ref())
                    .and_then(|changes| changes.values().next())
                    .and_then(|edits| edits.first())
                    .map(|edit| edit.new_text.contains("INT_TO_BOOL"))
                    .unwrap_or(false)
        }
        _ => false,
    });
    assert!(has_conversion_action, "expected conversion code action");
}

#[test]
fn lsp_code_action_convert_call_style() {
    let source = r#"
FUNCTION Foo : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Foo := A + B;
END_FUNCTION

PROGRAM Test
VAR
    x : INT;
END_VAR
    x := Foo(1, B := 2);
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "Foo(1");
    let end =
        super::lsp_utils::offset_to_position(source, (source.find("Foo(1").unwrap() + 3) as u32);

    let diagnostic = tower_lsp::lsp_types::Diagnostic {
        range: tower_lsp::lsp_types::Range { start, end },
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "E205".to_string(),
        )),
        source: Some("truST".to_string()),
        message: "formal calls cannot mix positional arguments".to_string(),
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
    let has_convert_action = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
            code_action.title.contains("Convert")
        }
        _ => false,
    });
    assert!(has_convert_action, "expected call style conversion action");
}

#[test]
fn lsp_code_action_reorder_positional_first_call() {
    let source = r#"
FUNCTION Foo : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Foo := A + B;
END_FUNCTION

PROGRAM Test
VAR
    x : INT;
END_VAR
    x := Foo(A := 1, 2);
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "Foo(A");
    let end =
        super::lsp_utils::offset_to_position(source, (source.find("Foo(A").unwrap() + 3) as u32);

    let diagnostic = tower_lsp::lsp_types::Diagnostic {
        range: tower_lsp::lsp_types::Range { start, end },
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "E205".to_string(),
        )),
        source: Some("truST".to_string()),
        message: "positional arguments must precede formal arguments".to_string(),
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
    let has_reorder_action = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
            code_action
                .title
                .contains("Reorder to positional-first call")
                && code_action
                    .edit
                    .as_ref()
                    .and_then(|edit| edit.changes.as_ref())
                    .and_then(|changes| changes.values().next())
                    .and_then(|edits| edits.first())
                    .map(|edit| edit.new_text.contains("(2, A := 1)"))
                    .unwrap_or(false)
        }
        _ => false,
    });
    assert!(
        has_reorder_action,
        "expected positional-first reorder code action"
    );
}

#[test]
fn lsp_code_action_namespace_move() {
    let source = r#"
NAMESPACE LibA
END_NAMESPACE
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "LibA\nEND_NAMESPACE");
    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range { start, end: start },
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![tower_lsp::lsp_types::CodeActionKind::REFACTOR_REWRITE]),
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action(&state, params).expect("code actions");
    let has_move_action = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
            code_action.title.contains("Move namespace")
                && code_action
                    .command
                    .as_ref()
                    .map(|cmd| cmd.command == "editor.action.rename")
                    .unwrap_or(false)
        }
        _ => false,
    });
    assert!(has_move_action, "expected namespace move code action");
}
