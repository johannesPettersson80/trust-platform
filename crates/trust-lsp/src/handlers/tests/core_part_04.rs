use super::*;

#[test]
pub(super) fn lsp_workspace_symbols_include_dependency_sources() {
    let root = temp_dir("trustlsp-dependency-symbols");
    let dep = root.join("deps/vendor");
    std::fs::create_dir_all(root.join("sources")).expect("create sources");
    std::fs::create_dir_all(dep.join("sources")).expect("create dependency sources");
    std::fs::write(
        root.join("trust-lsp.toml"),
        r#"
[project]
include_paths = ["sources"]

[dependencies]
Vendor = { path = "deps/vendor", version = "1.0.0" }
"#,
    )
    .expect("write root config");
    std::fs::write(
        dep.join("trust-lsp.toml"),
        r#"
[package]
version = "1.0.0"
"#,
    )
    .expect("write dependency config");
    std::fs::write(
        root.join("sources/main.st"),
        r#"
PROGRAM Main
VAR
    out : INT;
END_VAR
out := VendorDouble(2);
END_PROGRAM
"#,
    )
    .expect("write root source");
    std::fs::write(
        dep.join("sources/vendor.st"),
        r#"
FUNCTION VendorDouble : INT
VAR_INPUT
    x : INT;
END_VAR
VendorDouble := x * 2;
END_FUNCTION
"#,
    )
    .expect("write dependency source");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_folders(vec![root_uri]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        let client = test_client();
        index_workspace(&client, &state).await;
    });

    let dep_source = dep
        .join("sources/vendor.st")
        .canonicalize()
        .expect("dep source");
    let dep_source_norm = normalize_path_for_assert(&dep_source);
    let mut found_dependency_symbol = false;
    for _ in 0..40 {
        let symbols = workspace_symbol(
            &state,
            tower_lsp::lsp_types::WorkspaceSymbolParams {
                query: "VendorDouble".to_string(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .expect("workspace symbols");
        found_dependency_symbol = symbols.iter().any(|symbol| {
            if symbol.name != "VendorDouble" {
                return false;
            }
            let Some(path) = symbol.location.uri.to_file_path().ok() else {
                return false;
            };
            let path = path.canonicalize().unwrap_or(path);
            normalize_path_for_assert(&path) == dep_source_norm
        });
        if found_dependency_symbol {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(
        found_dependency_symbol,
        "expected dependency symbol to be indexed"
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
pub(super) fn lsp_external_diagnostics_provide_quick_fixes() {
    let root = temp_dir("trustlsp-external-diag");
    let lint_path = root.join("lint.json");
    std::fs::write(
        &lint_path,
        r#"
[
  {
    "path": "main.st",
    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 7 } },
    "severity": "warning",
    "code": "X001",
    "message": "External issue",
    "fix": {
      "title": "Fix external issue",
      "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 7 } },
      "new_text": "PROGRAM"
    }
  }
]
"#,
    )
    .expect("write lint json");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).unwrap();
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri.clone(),
        ProjectConfig {
            root: root.clone(),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: vec![lint_path.clone()],
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::from_file_path(root.join("main.st")).unwrap();
    let source = "PROGRAM Main\nEND_PROGRAM\n";
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
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
    let external = diagnostics
        .iter()
        .find(|diag| {
            diag.code.as_ref().is_some_and(|code| {
            matches!(code, tower_lsp::lsp_types::NumberOrString::String(value) if value == "X001")
        })
        })
        .expect("external diagnostic");

    let params = tower_lsp::lsp_types::CodeActionParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: external.range,
        context: tower_lsp::lsp_types::CodeActionContext {
            diagnostics: vec![external.clone()],
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let actions = code_action(&state, params).expect("code actions");
    let has_external = actions.iter().any(|action| match action {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(action) => {
            action.title.contains("Fix external issue")
        }
        _ => false,
    });
    assert!(has_external, "expected external quick fix");

    std::fs::remove_dir_all(root).ok();
}

#[test]
pub(super) fn lsp_document_symbols_include_configuration_hierarchy() {
    let source = r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION

PROGRAM Main
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///config.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentSymbolParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let response = document_symbol(&state, params).expect("document symbols");
    let tower_lsp::lsp_types::DocumentSymbolResponse::Nested(outline) = response else {
        panic!("expected the outline as a tree (specification 14 §6.3.3)")
    };
    let configuration = outline
        .iter()
        .find(|entry| entry.name.starts_with("Conf"))
        .expect("the configuration at the top");
    let resource = configuration
        .children
        .iter()
        .flatten()
        .find(|entry| entry.name.starts_with("R "))
        .expect("the resource under its configuration");
    let names: Vec<&str> = resource
        .children
        .iter()
        .flatten()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(names, ["Fast (TASK)", "P1 (PROGRAM)"]);
    assert!(outline.iter().any(|entry| entry.name.starts_with("Main")));
}
