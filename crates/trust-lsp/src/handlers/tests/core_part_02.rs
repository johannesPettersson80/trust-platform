use super::*;

#[test]
pub(super) fn lsp_supports_virtual_document_uris() {
    let state = ServerState::new();
    let uri =
        tower_lsp::lsp_types::Url::parse("vscode-notebook-cell:/workspace/notebook#cell1").unwrap();
    state.open_document(uri.clone(), 1, "PROGRAM Test END_PROGRAM".to_string());

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
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
    assert!(
        matches!(
            report,
            tower_lsp::lsp_types::DocumentDiagnosticReport::Full(_)
        ),
        "expected full diagnostic report"
    );
}

#[test]
pub(super) fn lsp_diagnostics_respect_config_toggles() {
    let mut body = String::new();
    for _ in 0..15 {
        body.push_str("    IF TRUE THEN x := x + 1; END_IF;\n");
    }
    let source = format!(
        r#"
PROGRAM Test
    VAR
        x : INT;
        y : REAL;
    END_VAR
    CASE x OF
        1: x := 1;
    END_CASE;
{body}
    y := 1;
END_PROGRAM
"#
    );
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
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings {
                warn_unused: true,
                warn_unreachable: true,
                warn_missing_else: false,
                warn_implicit_conversion: false,
                warn_shadowed: true,
                warn_deprecated: true,
                warn_complexity: false,
                warn_nondeterminism: true,
                warn_numeric_hazards: true,
                severity_overrides: Default::default(),
            },
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/diag.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
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
    let codes: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .filter_map(|diag| diag.code.as_ref())
        .map(|code| match code {
            tower_lsp::lsp_types::NumberOrString::String(value) => value.clone(),
            tower_lsp::lsp_types::NumberOrString::Number(value) => value.to_string(),
        })
        .collect();

    assert!(
        !codes.iter().any(|code| code == "W004"),
        "expected MissingElse warning to be filtered"
    );
    assert!(
        !codes.iter().any(|code| code == "W005"),
        "expected ImplicitConversion warning to be filtered"
    );
    assert!(
        !codes.iter().any(|code| code == "W008"),
        "expected HighComplexity warning to be filtered"
    );
}

#[test]
pub(super) fn lsp_learner_diagnostics_include_did_you_mean_and_conversion_guidance() {
    let source = r#"
TYPE MotorConfig : STRUCT
    speed : INT;
END_STRUCT
END_TYPE

PROGRAM Test
VAR
    speedValue : INT;
    cfg : MotroConfig;
    flag : BOOL;
END_VAR
    speadValue := 1;
    flag := 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///learner-hints.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
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

    let undefined_identifier = diagnostics
        .iter()
        .find(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E101"
            )
        })
        .expect("expected E101 diagnostic");
    let id_hints: Vec<&str> = undefined_identifier
        .related_information
        .as_ref()
        .map(|items| items.iter().map(|item| item.message.as_str()).collect())
        .unwrap_or_default();
    assert!(
        id_hints
            .iter()
            .any(|hint| hint.contains("Did you mean 'speedValue'?")),
        "expected did-you-mean hint for E101, got {id_hints:?}"
    );

    let undefined_type = diagnostics
        .iter()
        .find(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E102"
            )
        })
        .expect("expected E102 diagnostic");
    let type_hints: Vec<&str> = undefined_type
        .related_information
        .as_ref()
        .map(|items| items.iter().map(|item| item.message.as_str()).collect())
        .unwrap_or_default();
    assert!(
        type_hints
            .iter()
            .any(|hint| hint.contains("Did you mean 'MotorConfig'?")),
        "expected did-you-mean hint for E102, got {type_hints:?}"
    );

    let incompatible_assignment = diagnostics
        .iter()
        .find(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "E203"
            )
        })
        .expect("expected E203 diagnostic");
    let conversion_hints: Vec<&str> = incompatible_assignment
        .related_information
        .as_ref()
        .map(|items| items.iter().map(|item| item.message.as_str()).collect())
        .unwrap_or_default();
    assert!(
        conversion_hints
            .iter()
            .any(|hint| hint.contains("_TO_BOOL(<expr>)")),
        "expected explicit conversion hint, got {conversion_hints:?}"
    );
}
