use super::*;

#[test]
pub(super) fn lsp_will_rename_files_updates_using_namespace() {
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
pub(super) fn lsp_workspace_symbols() {
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

#[tokio::test(flavor = "current_thread")]
pub(super) async fn lsp_references_partial_result_token_returns_empty_final_response() {
    use futures_util::StreamExt;

    let source = r#"
PROGRAM Main
VAR
    Speed : INT;
END_VAR
Speed := Speed + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let (client, mut service, mut socket) = crate::test_support::test_client_with_socket();
    crate::test_support::initialize_test_client(&mut service).await;
    let uri = tower_lsp::lsp_types::Url::parse("file:///partial-refs.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::ReferenceParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            position: position_at(source, "Speed : INT"),
        },
        context: tower_lsp::lsp_types::ReferenceContext {
            include_declaration: true,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: tower_lsp::lsp_types::PartialResultParams {
            partial_result_token: Some(tower_lsp::lsp_types::ProgressToken::String(
                "refs-partial".to_string(),
            )),
        },
    };

    let final_result = references_with_progress(&client, &state, params)
        .await
        .expect("references final result");
    assert!(
        final_result.is_empty(),
        "when partial results are streamed, the final references response must be empty, got {} items",
        final_result.len()
    );
    let notification = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("partial references notification timeout")
        .expect("partial references notification");
    assert_eq!(notification.method(), "$/progress");
    let params = notification.params().expect("partial references params");
    assert_eq!(params["token"], "refs-partial");
    assert!(
        params["value"].as_array().is_some_and(|items| !items.is_empty()),
        "partial references notification must carry the locations omitted from the final response: {params}"
    );
}

#[tokio::test(flavor = "current_thread")]
pub(super) async fn lsp_workspace_symbols_partial_result_token_returns_empty_final_response() {
    use futures_util::StreamExt;

    let source = r#"
FUNCTION_BLOCK Pump
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    pump : Pump;
END_VAR
END_PROGRAM
"#;
    let state = ServerState::new();
    let (client, mut service, mut socket) = crate::test_support::test_client_with_socket();
    crate::test_support::initialize_test_client(&mut service).await;
    let uri = tower_lsp::lsp_types::Url::parse("file:///partial-symbols.st").unwrap();
    state.open_document(uri, 1, source.to_string());

    let params = tower_lsp::lsp_types::WorkspaceSymbolParams {
        query: "".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: tower_lsp::lsp_types::PartialResultParams {
            partial_result_token: Some(tower_lsp::lsp_types::ProgressToken::String(
                "symbols-partial".to_string(),
            )),
        },
    };

    let final_result = workspace_symbol_with_progress(&client, &state, params)
        .await
        .expect("workspace symbol final result");
    assert!(
        final_result.is_empty(),
        "when partial results are streamed, the final workspace-symbol response must be empty, got {} items",
        final_result.len()
    );
    let notification = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("partial workspace-symbol notification timeout")
        .expect("partial workspace-symbol notification");
    assert_eq!(notification.method(), "$/progress");
    let params = notification
        .params()
        .expect("partial workspace-symbol params");
    assert_eq!(params["token"], "symbols-partial");
    assert!(
        params["value"].as_array().is_some_and(|items| !items.is_empty()),
        "partial workspace-symbol notification must carry the symbols omitted from the final response: {params}"
    );
}

#[test]
pub(super) fn lsp_workspace_symbols_respect_root_visibility_and_priority() {
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
pub(super) fn lsp_document_highlight_variable() {
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
pub(super) fn lsp_semantic_tokens_delta() {
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
pub(super) fn lsp_semantic_tokens_use_utf16_start_and_length_after_supplementary_scalar() {
    let source = "😀 (* 😀 *) speed\n";
    let comment_start = source.find("(*").expect("comment start");
    let comment_end = source.find("*)").expect("comment end") + "*)".len();
    let speed_start = source.find("speed").expect("speed start");

    let tokens = vec![
        trust_ide::SemanticToken::new(
            text_size::TextRange::new(
                text_size::TextSize::from(comment_start as u32),
                text_size::TextSize::from(comment_end as u32),
            ),
            trust_ide::SemanticTokenType::Comment,
        ),
        trust_ide::SemanticToken::new(
            text_size::TextRange::new(
                text_size::TextSize::from(speed_start as u32),
                text_size::TextSize::from((speed_start + "speed".len()) as u32),
            ),
            trust_ide::SemanticTokenType::Variable,
        ),
    ];

    let data = super::super::lsp_utils::semantic_tokens_to_lsp(source, tokens, 0, 0);
    assert_eq!(data.len(), 2);
    assert_eq!(data[0].delta_line, 0);
    assert_eq!(
        data[0].delta_start,
        utf16_position_at(source, "(*").character
    );
    assert_eq!(data[0].length, utf16_len("(* 😀 *)"));
    assert_eq!(data[1].delta_line, 0);
    assert_eq!(
        data[1].delta_start,
        utf16_position_at(source, "speed").character - utf16_position_at(source, "(*").character
    );
    assert_eq!(data[1].length, utf16_len("speed"));
}
