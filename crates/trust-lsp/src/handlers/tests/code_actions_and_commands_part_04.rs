use super::*;

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
        return_edit: false,
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
                dependencies_offline: false,
                dependencies_locked: false,
                dependency_lockfile: PathBuf::from("trust-lsp.lock"),
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
