use super::*;

#[test]
fn lsp_completion_respects_stdlib_allowlist() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
    x := A
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
            stdlib: StdlibSettings {
                profile: Some("custom".to_string()),
                allow: Some(vec!["ABS".to_string()]),
            },
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

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::CompletionParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "A\nEND_PROGRAM"),
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

    let labels: Vec<String> = items.into_iter().map(|item| item.label).collect();
    assert!(labels.iter().any(|label| label.eq_ignore_ascii_case("ABS")));
    assert!(!labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("SQRT")));
}

#[test]
fn lsp_completion_respects_stdlib_profile_none() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
    x := A
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
            stdlib: StdlibSettings {
                profile: Some("none".to_string()),
                allow: None,
            },
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

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::CompletionParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "A\nEND_PROGRAM"),
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

    let labels: Vec<String> = items.into_iter().map(|item| item.label).collect();
    assert!(!labels.iter().any(|label| label.eq_ignore_ascii_case("ABS")));
}

#[test]
fn lsp_hover_member_method_and_property() {
    let interface = r#"
INTERFACE ICounter
    METHOD Next : DINT
    END_METHOD
    PROPERTY Value : DINT
        GET
        END_GET
    END_PROPERTY
END_INTERFACE
"#;
    let fb = r#"
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
    let main = r#"
PROGRAM Main
VAR
    counter : CounterFb;
    outVal : DINT;
END_VAR

outVal := counter.Next();
outVal := counter.Value;
END_PROGRAM
"#;

    let state = ServerState::new();
    let iface_uri = tower_lsp::lsp_types::Url::parse("file:///icounter.st").unwrap();
    let fb_uri = tower_lsp::lsp_types::Url::parse("file:///counterfb.st").unwrap();
    let main_uri = tower_lsp::lsp_types::Url::parse("file:///main.st").unwrap();
    state.open_document(iface_uri, 1, interface.to_string());
    state.open_document(fb_uri, 1, fb.to_string());
    state.open_document(main_uri.clone(), 1, main.to_string());

    let next_pos = position_at(main, "Next()");
    let next_params = tower_lsp::lsp_types::HoverParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: next_pos,
        },
        work_done_progress_params: Default::default(),
    };
    let next_hover = hover(&state, next_params).expect("hover next");
    let tower_lsp::lsp_types::HoverContents::Markup(next_markup) = next_hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(
        next_markup.value.contains("Next"),
        "expected hover to include Next"
    );

    let value_pos = position_at(main, "Value;");
    let value_params = tower_lsp::lsp_types::HoverParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: main_uri },
            position: value_pos,
        },
        work_done_progress_params: Default::default(),
    };
    let value_hover = hover(&state, value_params).expect("hover value");
    let tower_lsp::lsp_types::HoverContents::Markup(value_markup) = value_hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(
        value_markup.value.contains("Value"),
        "expected hover to include Value"
    );
}

#[test]
fn lsp_hover_respects_stdlib_filter() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
    x := ABS(1);
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
            stdlib: StdlibSettings {
                profile: Some("none".to_string()),
                allow: None,
            },
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

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::HoverParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "ABS(1"),
        },
        work_done_progress_params: Default::default(),
    };

    let hover = hover(&state, params);
    assert!(hover.is_none(), "expected stdlib hover to be filtered");
}
