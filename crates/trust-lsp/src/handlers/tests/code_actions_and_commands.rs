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
        source: Some("trust-lsp".to_string()),
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
        source: Some("trust-lsp".to_string()),
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
        source: Some("trust-lsp".to_string()),
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
        source: Some("trust-lsp".to_string()),
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
        source: Some("trust-lsp".to_string()),
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
        source: Some("trust-lsp".to_string()),
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
        source: Some("trust-lsp".to_string()),
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

#[test]
fn lsp_code_action_generate_interface_stubs() {
    let source = r#"
INTERFACE IControl
    METHOD Start
    END_METHOD
END_INTERFACE

CLASS Pump IMPLEMENTS IControl
END_CLASS
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let position = position_at(source, "IMPLEMENTS IControl");
    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range {
            start: position,
            end: position,
        },
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: Vec::new(),
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action(&state, params).expect("code actions");
    let stub_action = actions.iter().find_map(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action)
            if code_action.title.contains("interface stubs") =>
        {
            Some(code_action)
        }
        _ => None,
    });
    let stub_action = stub_action.expect("stub action");
    let edits = stub_action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("stub edits");
    assert!(edits
        .iter()
        .any(|edit| edit.new_text.contains("METHOD PUBLIC Start")));
}

#[test]
fn lsp_code_action_inline_variable() {
    let source = r#"
PROGRAM Test
    VAR
        x : INT := 1 + 2;
    END_VAR
    y := x;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let position = position_at(source, "x;");
    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range {
            start: position,
            end: position,
        },
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: Vec::new(),
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action(&state, params).expect("code actions");
    let inline_action = actions.iter().find_map(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action)
            if code_action.title.contains("Inline variable") =>
        {
            Some(code_action)
        }
        _ => None,
    });
    let inline_action = inline_action.expect("inline action");
    let edits = inline_action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("inline edits");
    assert!(edits.iter().any(|edit| edit.new_text.contains("1 + 2")));
}

#[test]
fn lsp_code_action_extract_method() {
    let source = r#"
CLASS Controller
    METHOD Run
        x := 1;
        y := 2;
    END_METHOD
END_CLASS
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start_offset = source.find("x := 1;").expect("start");
    let end_offset = source.find("y := 2;").expect("end") + "y := 2;".len();
    let range = tower_lsp::lsp_types::Range {
        start: super::lsp_utils::offset_to_position(source, start_offset as u32),
        end: super::lsp_utils::offset_to_position(source, end_offset as u32),
    };
    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range,
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![tower_lsp::lsp_types::CodeActionKind::REFACTOR_EXTRACT]),
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action(&state, params).expect("code actions");
    let extract_action = actions.iter().find_map(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action)
            if code_action.title.contains("Extract method") =>
        {
            Some(code_action)
        }
        _ => None,
    });
    let extract_action = extract_action.expect("extract action");
    let edits = extract_action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("extract edits");
    assert!(edits
        .iter()
        .any(|edit| edit.new_text.contains("METHOD ExtractedMethod")));
}

#[test]
fn lsp_code_action_convert_function_to_function_block() {
    let source = r#"
FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION

PROGRAM Main
    Foo();
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let position = position_at(source, "FUNCTION Foo");
    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range {
            start: position,
            end: position,
        },
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![tower_lsp::lsp_types::CodeActionKind::REFACTOR_REWRITE]),
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action(&state, params).expect("code actions");
    let convert_action = actions.iter().find_map(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action)
            if code_action
                .title
                .contains("Convert FUNCTION to FUNCTION_BLOCK") =>
        {
            Some(code_action)
        }
        _ => None,
    });
    let convert_action = convert_action.expect("convert action");
    let edits = convert_action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("convert edits");
    assert!(edits
        .iter()
        .any(|edit| edit.new_text.contains("FUNCTION_BLOCK")));
    assert!(edits
        .iter()
        .any(|edit| edit.new_text.contains("FooInstance")));
}

#[test]
fn lsp_code_action_convert_function_block_to_function() {
    let source = r#"
FUNCTION_BLOCK Fb
    VAR_OUTPUT
        result : INT;
    END_VAR
    result := 1;
END_FUNCTION_BLOCK
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let position = position_at(source, "FUNCTION_BLOCK Fb");
    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range {
            start: position,
            end: position,
        },
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![tower_lsp::lsp_types::CodeActionKind::REFACTOR_REWRITE]),
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = code_action(&state, params).expect("code actions");
    let convert_action = actions.iter().find_map(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(code_action)
            if code_action
                .title
                .contains("Convert FUNCTION_BLOCK to FUNCTION") =>
        {
            Some(code_action)
        }
        _ => None,
    });
    let convert_action = convert_action.expect("convert action");
    let edits = convert_action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("convert edits");
    assert!(edits.iter().any(|edit| edit.new_text.contains("FUNCTION")));
    assert!(edits.iter().any(|edit| edit.new_text.contains(": INT")));
}

#[test]
fn lsp_execute_command_namespace_move_workspace_edit() {
    let source = r#"
NAMESPACE LibA
TYPE Foo : INT;
END_TYPE
FUNCTION FooFunc : INT
END_FUNCTION
END_NAMESPACE
"#;
    let main_source = r#"
PROGRAM Main
    USING LibA;
    VAR
        x : LibA.Foo;
    END_VAR
    x := LibA.FooFunc();
END_PROGRAM
"#;
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);

    let namespace_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/liba.st").unwrap();
    state.open_document(namespace_uri.clone(), 1, source.to_string());

    let main_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/main.st").unwrap();
    state.open_document(main_uri.clone(), 1, main_source.to_string());

    let args = super::commands::MoveNamespaceCommandArgs {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
            uri: namespace_uri.clone(),
        },
        position: position_at(source, "LibA\nTYPE"),
        new_path: "Company.LibA".to_string(),
        target_uri: None,
    };

    let edit = namespace_move_workspace_edit(&state, args).expect("workspace edit");
    let document_changes = edit.document_changes.expect("document changes");
    let document_changes = match document_changes {
        tower_lsp::lsp_types::DocumentChanges::Operations(ops) => ops,
        _ => panic!("expected document change operations"),
    };

    let target_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/Company/LibA.st").unwrap();
    assert!(
        document_changes.iter().any(|change| {
            matches!(
                change,
                tower_lsp::lsp_types::DocumentChangeOperation::Op(
                    tower_lsp::lsp_types::ResourceOp::Create(create)
                ) if create.uri == target_uri
            )
        }),
        "expected create file for target namespace"
    );

    assert!(
        document_changes.iter().any(|change| {
            matches!(
                change,
                tower_lsp::lsp_types::DocumentChangeOperation::Op(
                    tower_lsp::lsp_types::ResourceOp::Delete(delete)
                ) if delete.uri == namespace_uri
            )
        }),
        "expected delete file for source namespace"
    );

    let target_edit = document_changes.iter().find_map(|change| match change {
        tower_lsp::lsp_types::DocumentChangeOperation::Edit(edit) => {
            if edit.text_document.uri == target_uri {
                Some(edit)
            } else {
                None
            }
        }
        _ => None,
    });
    let target_edit = target_edit.expect("target edit");
    let has_namespace_text = target_edit.edits.iter().any(|edit| match edit {
        tower_lsp::lsp_types::OneOf::Left(edit) => edit.new_text.contains("NAMESPACE Company.LibA"),
        _ => false,
    });
    assert!(has_namespace_text, "expected updated namespace text");

    let main_edit = document_changes.iter().find_map(|change| match change {
        tower_lsp::lsp_types::DocumentChangeOperation::Edit(edit) => {
            if edit.text_document.uri == main_uri {
                Some(edit)
            } else {
                None
            }
        }
        _ => None,
    });
    let main_edit = main_edit.expect("main edit");
    let has_using_update = main_edit.edits.iter().any(|edit| match edit {
        tower_lsp::lsp_types::OneOf::Left(edit) => edit.new_text.contains("Company.LibA"),
        _ => false,
    });
    assert!(has_using_update, "expected USING update");
}

#[test]
fn lsp_project_info_exposes_build_and_targets() {
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri.clone(),
        ProjectConfig {
            root: PathBuf::from("/workspace"),
            config_path: Some(PathBuf::from("/workspace/trust-lsp.toml")),
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: StdlibSettings::default(),
            libraries: vec![LibrarySpec {
                name: "Core".to_string(),
                path: PathBuf::from("/workspace/libs/core"),
                version: Some("1.0".to_string()),
                dependencies: vec![LibraryDependency {
                    name: "Utils".to_string(),
                    version: None,
                }],
                docs: Vec::new(),
            }],
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig {
                target: Some("x86_64".to_string()),
                profile: Some("release".to_string()),
                flags: vec!["-O2".to_string()],
                defines: vec!["SIM=1".to_string()],
            },
            targets: vec![TargetProfile {
                name: "sim".to_string(),
                profile: Some("debug".to_string()),
                flags: vec!["-g".to_string()],
                defines: vec!["TRACE=1".to_string()],
            }],
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let info = super::commands::project_info_value(&state, Vec::new()).expect("project info");
    let projects = info
        .get("projects")
        .and_then(|value| value.as_array())
        .expect("projects array");
    assert_eq!(projects.len(), 1);
    let project = &projects[0];
    let build = project.get("build").expect("build");
    assert_eq!(build.get("target").and_then(|v| v.as_str()), Some("x86_64"));
    assert_eq!(
        build.get("profile").and_then(|v| v.as_str()),
        Some("release")
    );
    let targets = project
        .get("targets")
        .and_then(|value| value.as_array())
        .expect("targets");
    assert!(targets.iter().any(|target| {
        target.get("name").and_then(|v| v.as_str()) == Some("sim")
            && target.get("profile").and_then(|v| v.as_str()) == Some("debug")
    }));
    let libraries = project
        .get("libraries")
        .and_then(|value| value.as_array())
        .expect("libraries");
    assert!(libraries.iter().any(|lib| {
        lib.get("name").and_then(|v| v.as_str()) == Some("Core")
            && lib.get("version").and_then(|v| v.as_str()) == Some("1.0")
    }));
}

#[test]
fn lsp_code_action_namespace_disambiguation() {
    let source = r#"
NAMESPACE LibA
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE

NAMESPACE LibB
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
    USING LibA;
    USING LibB;
    VAR
        x : INT;
    END_VAR
    x := Foo();
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "Foo()");
    let end =
        super::lsp_utils::offset_to_position(source, (source.find("Foo()").unwrap() + 3) as u32);

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

#[test]
fn lsp_code_action_namespace_disambiguation_project_using() {
    let lib_a = r#"
NAMESPACE LibA
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE
"#;
    let lib_b = r#"
NAMESPACE LibB
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE
"#;
    let main = r#"
USING LibA;
USING LibB;

PROGRAM Main
    VAR
        x : INT;
    END_VAR
    x := Foo();
END_PROGRAM
"#;
    let state = ServerState::new();
    let lib_a_uri = tower_lsp::lsp_types::Url::parse("file:///liba.st").unwrap();
    let lib_b_uri = tower_lsp::lsp_types::Url::parse("file:///libb.st").unwrap();
    let main_uri = tower_lsp::lsp_types::Url::parse("file:///main.st").unwrap();
    state.open_document(lib_a_uri, 1, lib_a.to_string());
    state.open_document(lib_b_uri, 1, lib_b.to_string());
    state.open_document(main_uri.clone(), 1, main.to_string());

    let start = position_at(main, "Foo()");
    let end = super::lsp_utils::offset_to_position(main, (main.find("Foo()").unwrap() + 3) as u32);

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
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: main_uri },
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
