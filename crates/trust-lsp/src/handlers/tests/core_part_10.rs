use super::*;

#[test]
pub(super) fn lsp_siemens_hash_prefixed_example_has_no_unexpected_diagnostics() {
    let source = include_str!("../../../../../examples/siemens_scl_v1/src/Main.st");
    let configuration = include_str!("../../../../../examples/siemens_scl_v1/src/Configuration.st");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").expect("workspace uri");
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: Some("siemens".to_string()),
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings {
                warn_unused: false,
                warn_unreachable: false,
                warn_missing_else: false,
                warn_implicit_conversion: false,
                warn_shadowed: false,
                warn_deprecated: false,
                warn_complexity: false,
                warn_nondeterminism: false,
                warn_numeric_hazards: false,
                severity_overrides: Default::default(),
            },
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri =
        tower_lsp::lsp_types::Url::parse("file:///workspace/examples/siemens_scl_v1/src/Main.st")
            .expect("siemens example uri");
    let configuration_uri = tower_lsp::lsp_types::Url::parse(
        "file:///workspace/examples/siemens_scl_v1/src/Configuration.st",
    )
    .expect("siemens configuration uri");
    state.open_document(configuration_uri, 1, configuration.to_string());
    state.open_document(uri.clone(), 1, source.to_string());

    let file_id = state.get_document(&uri).expect("example document").file_id;
    let ticket = state.begin_semantic_request();
    let diagnostics = super::diagnostics::collect_diagnostics_with_ticket_for_tests(
        &state, &uri, source, file_id, ticket,
    );

    let summary: Vec<String> = diagnostics
        .iter()
        .map(|diag| {
            let code = match diag.code.as_ref() {
                Some(tower_lsp::lsp_types::NumberOrString::String(value)) => value.clone(),
                Some(tower_lsp::lsp_types::NumberOrString::Number(value)) => value.to_string(),
                None => "NO_CODE".to_string(),
            };
            format!("{code}: {}", diag.message)
        })
        .collect();

    assert!(
        summary.is_empty(),
        "expected no diagnostics for Siemens SCL example, got {summary:?}"
    );
}
