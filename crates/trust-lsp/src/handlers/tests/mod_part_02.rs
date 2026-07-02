use super::*;

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
