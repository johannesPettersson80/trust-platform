use super::*;

#[test]
fn lsp_code_action_adds_openot_logging_by_declared_type() {
    let source = r#"
TYPE E_Step : (Idle := 0, Filling := 1) END_TYPE

PROGRAM Main
VAR
    Step : E_Step;
    Level : REAL;
    BatchCount : DINT;
    LongLevel : LREAL;
    LongCount : ULINT;
    HighPhAlarm : BOOL;
END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///openot-actions.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let step_edits = openot_action_edits(&state, &uri, source, "Step : E_Step;");
    assert!(
        step_edits.iter().any(|(title, text)| {
            title == "Add OpenOT logging"
                && text.contains("{attribute 'oot' := 'state', 'category' := 'process'}")
        }),
        "{step_edits:?}"
    );

    let level_edits = openot_action_edits(&state, &uri, source, "Level :");
    assert!(
        level_edits.iter().any(|(title, text)| {
            title == "Add OpenOT logging" && text.contains("{attribute 'oot' := 'value'}")
        }),
        "{level_edits:?}"
    );

    let count_edits = openot_action_edits(&state, &uri, source, "BatchCount :");
    assert!(
        count_edits.iter().any(|(title, text)| {
            title == "Add OpenOT logging"
                && text.contains("{attribute 'oot' := 'value'}")
                && !text.contains("unit")
                && !text.contains("deadband")
        }),
        "{count_edits:?}"
    );

    let long_level_edits = openot_action_edits(&state, &uri, source, "LongLevel :");
    assert!(
        long_level_edits
            .iter()
            .any(|(title, text)| title == "Add OpenOT logging" && text.contains("'value'")),
        "{long_level_edits:?}"
    );

    let long_count_edits = openot_action_edits(&state, &uri, source, "LongCount :");
    assert!(
        long_count_edits
            .iter()
            .any(|(title, text)| title == "Add OpenOT logging" && text.contains("'value'")),
        "{long_count_edits:?}"
    );

    let bool_edits = openot_action_edits(&state, &uri, source, "HighPhAlarm :");
    assert!(
        bool_edits.iter().any(|(title, text)| {
            title == "Add OpenOT logging as alarm"
                && text.contains("{attribute 'oot' := 'alarm'}")
                && !text.contains("class")
                && !text.contains("severity")
        }),
        "{bool_edits:?}"
    );
    assert!(
        bool_edits.iter().any(|(title, text)| {
            title == "Add OpenOT logging as message"
                && text.contains("{attribute 'oot' := 'message'}")
                && !text.contains("template")
        }),
        "{bool_edits:?}"
    );
}

fn openot_action_edits(
    state: &ServerState,
    uri: &tower_lsp::lsp_types::Url,
    source: &str,
    needle: &str,
) -> Vec<(String, String)> {
    let position = position_at(source, needle);
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

    let actions = code_action(state, params).expect("code actions");
    actions
        .into_iter()
        .filter_map(|action| match action {
            tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(action) => {
                let edit_text = action
                    .edit
                    .and_then(|edit| edit.changes)
                    .into_iter()
                    .flat_map(|changes| changes.into_values())
                    .flatten()
                    .map(|edit| edit.new_text)
                    .collect::<Vec<_>>()
                    .join("");
                Some((action.title, edit_text))
            }
            tower_lsp::lsp_types::CodeActionOrCommand::Command(_) => None,
        })
        .collect()
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
        source: Some("truST".to_string()),
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
        source: Some("truST".to_string()),
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
