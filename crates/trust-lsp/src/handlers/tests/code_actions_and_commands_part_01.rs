use super::*;

#[test]
fn lsp_code_action_missing_else() {
    let source = r#"
PROGRAM Test
    VAR
        x : INT;
    END_VAR

    CASE x OF
        1: x := 1;
    END_CASE
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "CASE x OF");
    let end_offset = source
        .find("END_CASE")
        .map(|idx| idx + "END_CASE".len())
        .expect("END_CASE");
    let end = super::lsp_utils::offset_to_position(source, end_offset as u32);

    let diagnostic = tower_lsp::lsp_types::Diagnostic {
        range: tower_lsp::lsp_types::Range { start, end },
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "W004".to_string(),
        )),
        source: Some("truST".to_string()),
        message: "CASE statement has no ELSE branch".to_string(),
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
    let has_else_action = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
            code_action.title.contains("ELSE")
        }
        _ => false,
    });
    assert!(has_else_action, "expected ELSE code action");
}

#[test]
fn lsp_code_action_create_var() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
    foo := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "foo");
    let end =
        super::lsp_utils::offset_to_position(source, (source.find("foo").unwrap() + 3) as u32);

    let diagnostic = tower_lsp::lsp_types::Diagnostic {
        range: tower_lsp::lsp_types::Range { start, end },
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "E101".to_string(),
        )),
        source: Some("truST".to_string()),
        message: "undefined identifier 'foo'".to_string(),
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
    let has_var_action = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
            code_action.title.contains("VAR")
                && code_action
                    .edit
                    .as_ref()
                    .and_then(|edit| edit.changes.as_ref())
                    .and_then(|changes| changes.values().next())
                    .and_then(|edits| edits.first())
                    .map(|edit| edit.new_text.contains("foo"))
                    .unwrap_or(false)
        }
        _ => false,
    });
    assert!(has_var_action, "expected VAR creation code action");
}

#[test]
fn lsp_code_action_create_type() {
    let source = r#"
PROGRAM Test
VAR
    x : MissingType;
END_VAR
    x := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "MissingType");
    let end = super::lsp_utils::offset_to_position(
        source,
        (source.find("MissingType").unwrap() + "MissingType".len()) as u32,
    );

    let diagnostic = tower_lsp::lsp_types::Diagnostic {
        range: tower_lsp::lsp_types::Range { start, end },
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "E102".to_string(),
        )),
        source: Some("truST".to_string()),
        message: "cannot resolve type 'MissingType'".to_string(),
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
    let has_type_action = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action) => {
            code_action.title.contains("TYPE")
                && code_action
                    .edit
                    .as_ref()
                    .and_then(|edit| edit.changes.as_ref())
                    .and_then(|changes| changes.values().next())
                    .and_then(|edits| edits.first())
                    .map(|edit| edit.new_text.contains("TYPE MissingType"))
                    .unwrap_or(false)
        }
        _ => false,
    });
    assert!(has_type_action, "expected TYPE creation code action");
}

#[test]
fn lsp_code_action_implicit_conversion() {
    let source = r#"
PROGRAM Test
VAR
    x : REAL;
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
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "W005".to_string(),
        )),
        source: Some("truST".to_string()),
        message: "implicit conversion from 'INT' to 'REAL'".to_string(),
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
                    .map(|edit| edit.new_text.contains("INT_TO_REAL"))
                    .unwrap_or(false)
        }
        _ => false,
    });
    assert!(has_conversion_action, "expected conversion code action");
}
