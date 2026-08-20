use super::*;

#[test]
pub(super) fn lsp_learner_diagnostics_include_syntax_habit_hints() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
    y : INT;
END_VAR
IF x == y THEN
    x = 1;
END_IF;
IF TRUE && FALSE THEN
    x := 2;
END_IF;
}
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///syntax-hints.st").unwrap();
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
    let hints: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .flat_map(|diag| {
            diag.related_information
                .iter()
                .flat_map(|items| items.iter().map(|item| item.message.clone()))
        })
        .collect();

    assert!(
        hints
            .iter()
            .any(|hint| hint.contains("use '=' for comparison")),
        "expected == guidance, got {hints:?}"
    );
    assert!(
        hints
            .iter()
            .any(|hint| hint.contains("assignments use ':='")),
        "expected assignment guidance, got {hints:?}"
    );
    assert!(
        hints.iter().any(|hint| hint.contains("AND instead of &&")),
        "expected && guidance, got {hints:?}"
    );
    assert!(
        hints
            .iter()
            .any(|hint| hint.contains("END_* keywords for block endings")),
        "expected brace guidance, got {hints:?}"
    );
}

#[test]
pub(super) fn lsp_learner_diagnostics_no_hint_noise_on_valid_code() {
    let source = r#"
PROGRAM Test
VAR
    x : INT;
    y : INT;
END_VAR
x := 1;
y := x + 1;
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///no-noise.st").unwrap();
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

    let hint_messages: Vec<String> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .flat_map(|diag| {
            diag.related_information
                .iter()
                .flat_map(|items| items.iter().map(|item| item.message.clone()))
        })
        .filter(|message| message.starts_with("Hint:"))
        .collect();
    assert!(
        hint_messages.is_empty(),
        "expected no learner hints on valid code, got {hint_messages:?}"
    );
}

#[test]
pub(super) fn lsp_config_diagnostics_report_library_dependency_issues() {
    let config = r#"
[project]
include_paths = ["src"]

[[libraries]]
name = "Core"
path = "libs/core"
version = "1.0"

[[libraries]]
name = "App"
path = "libs/app"
version = "1.0"
dependencies = [{ name = "Core", version = "2.0" }, { name = "Missing" }]
"#;
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri]);

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/trust-lsp.toml").unwrap();
    state.open_document(uri.clone(), 1, config.to_string());

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

    assert!(codes.contains(&"L001".to_string()));
    assert!(codes.contains(&"L002".to_string()));
}

#[test]
pub(super) fn lsp_config_diagnostics_report_dependency_cycle_issues() {
    let root = temp_dir("trustlsp-cycle-config");
    let dep_a = root.join("deps/lib-a");
    let dep_b = root.join("deps/lib-b");
    std::fs::create_dir_all(&dep_a).expect("create dep a");
    std::fs::create_dir_all(&dep_b).expect("create dep b");

    let config = r#"
[dependencies]
LibA = { path = "deps/lib-a" }
"#;
    std::fs::write(root.join("trust-lsp.toml"), config).expect("write root config");
    std::fs::write(
        dep_a.join("trust-lsp.toml"),
        r#"
[dependencies]
LibB = { path = "../lib-b" }
"#,
    )
    .expect("write dep a");
    std::fs::write(
        dep_b.join("trust-lsp.toml"),
        r#"
[dependencies]
LibA = { path = "../lib-a" }
"#,
    )
    .expect("write dep b");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_folders(vec![root_uri.clone()]);

    let uri =
        tower_lsp::lsp_types::Url::from_file_path(root.join("trust-lsp.toml")).expect("config uri");
    state.open_document(uri.clone(), 1, config.to_string());

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

    assert!(codes.contains(&"L004".to_string()));
    std::fs::remove_dir_all(root).ok();
}

#[test]
pub(super) fn lsp_config_diagnostics_report_malformed_configuration() {
    let config = "[project\nstdlib = \"iec\"";
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri]);

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/trust-lsp.toml").unwrap();
    state.open_document(uri.clone(), 1, config.to_string());
    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let report = document_diagnostic(&state, params);
    let tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
        tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
    ) = report
    else {
        panic!("expected full diagnostic report");
    };
    let messages: Vec<_> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    let codes: Vec<_> = full
        .full_document_diagnostic_report
        .items
        .iter()
        .filter_map(|diagnostic| diagnostic.code.as_ref())
        .collect();

    assert!(
        messages
            .iter()
            .any(|message| message.contains("failed to parse configuration")),
        "expected malformed-configuration diagnostic, got {messages:?}"
    );
    assert!(
        codes.iter().any(|code| {
            matches!(code, tower_lsp::lsp_types::NumberOrString::String(value) if value == "C001")
        }),
        "expected C001, got {codes:?}"
    );
}

#[test]
pub(super) fn lsp_config_diagnostics_report_invalid_normalized_values() {
    let config = r#"
[project]
stdlib = "vendor-mystery"

[indexing]
max_files = 0
max_ms = 0
memory_budget_mb = 0
evict_to_percent = 0
throttle_idle_ms = 60
throttle_active_ms = 8
throttle_max_ms = 50
throttle_active_window_ms = 0

[workspace]
visibility = "partners"

[telemetry]
flush_every = 0

[diagnostics.severity_overrides]
" " = "hint"
W999 = "fatal"
"#;
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri]);

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/trust-lsp.toml").unwrap();
    state.open_document(uri.clone(), 1, config.to_string());
    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let report = document_diagnostic(&state, params);
    let tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
        tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
    ) = report
    else {
        panic!("expected full diagnostic report");
    };
    let diagnostics = &full.full_document_diagnostic_report.items;
    for (expected_code, expected_message) in [
        ("C002", "unknown standard-library profile"),
        ("C003", "indexing.max_files"),
        ("C003", "indexing.max_ms"),
        ("C003", "indexing.memory_budget_mb"),
        ("C003", "indexing.evict_to_percent"),
        ("C003", "indexing throttle delays are incoherent"),
        ("C003", "indexing.throttle_active_window_ms"),
        ("C003", "telemetry.flush_every"),
        ("C004", "workspace.visibility"),
        ("C005", "override code must not be blank"),
        ("C005", "unknown diagnostic severity"),
    ] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(expected_message)
                    && matches!(
                        diagnostic.code.as_ref(),
                        Some(tower_lsp::lsp_types::NumberOrString::String(code))
                            if code == expected_code
                    )
            }),
            "expected {expected_code} diagnostic containing {expected_message:?}, got {diagnostics:?}"
        );
    }

    let max_files = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("indexing.max_files"))
        .expect("C003 diagnostic for indexing.max_files");
    let highlighted_line = config
        .lines()
        .nth(max_files.range.start.line as usize)
        .expect("diagnostic line must exist");
    let highlighted = &highlighted_line
        [max_files.range.start.character as usize..max_files.range.end.character as usize];
    assert_eq!(
        highlighted, "max_files",
        "C003 must highlight the invalid TOML key instead of falling back to the first line"
    );
}

#[test]
pub(super) fn lsp_config_diagnostics_report_upper_eviction_bound_normalization() {
    let config = r#"
[indexing]
evict_to_percent = 101
"#;
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_folders(vec![root_uri]);

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/trust-lsp.toml").unwrap();
    state.open_document(uri.clone(), 1, config.to_string());
    let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let report = document_diagnostic(&state, params);
    let tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
        tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
    ) = report
    else {
        panic!("expected full diagnostic report");
    };
    let diagnostics = &full.full_document_diagnostic_report.items;
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("indexing.evict_to_percent")
                && matches!(
                    diagnostic.code.as_ref(),
                    Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "C003"
                )
        }),
        "expected C003 for upper eviction bound normalization, got {diagnostics:?}"
    );
}

#[test]
pub(super) fn lsp_config_diagnostics_classify_full_eviction_integer_domain() {
    for raw in [-1, 256] {
        let config = format!("[indexing]\nevict_to_percent = {raw}\n");
        let state = ServerState::new();
        let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
        state.set_workspace_folders(vec![root_uri]);

        let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/trust-lsp.toml").unwrap();
        state.open_document(uri.clone(), 1, config.clone());
        let params = tower_lsp::lsp_types::DocumentDiagnosticParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let report = document_diagnostic(&state, params);
        let tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
            tower_lsp::lsp_types::DocumentDiagnosticReport::Full(full),
        ) = report
        else {
            panic!("expected full diagnostic report for {raw}");
        };
        let diagnostics = &full.full_document_diagnostic_report.items;
        assert!(
            !diagnostics.iter().any(|diagnostic| matches!(
                diagnostic.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "C001"
            )),
            "representable TOML integer {raw} must not degrade to whole-file C001: {diagnostics:?}"
        );
        let matching = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.message.contains("indexing.evict_to_percent")
                    && matches!(
                        diagnostic.code.as_ref(),
                        Some(tower_lsp::lsp_types::NumberOrString::String(code))
                            if code == "C003"
                    )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "expected one C003 for eviction value {raw}, got {diagnostics:?}"
        );
        assert!(matching[0].message.contains("1..=100"));
        let highlighted_line = config
            .lines()
            .nth(matching[0].range.start.line as usize)
            .expect("diagnostic line must exist");
        let highlighted = &highlighted_line
            [matching[0].range.start.character as usize..matching[0].range.end.character as usize];
        assert_eq!(highlighted, "evict_to_percent");
    }
}

#[test]
pub(super) fn lsp_workspace_config_diagnostics_report_read_failure() {
    let root = temp_dir("trustlsp-unreadable-config");
    let config_path = root.join("trust-lsp.toml");
    std::fs::write(&config_path, "").expect("write initial config");
    let config = ProjectConfig::load(&root);
    std::fs::remove_file(&config_path).expect("remove config file");
    std::fs::create_dir(&config_path).expect("replace config with unreadable directory");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).expect("root uri");
    state.set_workspace_config(root_uri, config);
    let params = tower_lsp::lsp_types::WorkspaceDiagnosticParams {
        identifier: None,
        previous_result_ids: Vec::new(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let report = workspace_diagnostic(&state, params).expect("workspace diagnostics");
    let tower_lsp::lsp_types::WorkspaceDiagnosticReportResult::Report(report) = report else {
        panic!("expected workspace diagnostic report");
    };
    let messages: Vec<_> = report
        .items
        .iter()
        .filter_map(|item| match item {
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) => {
                Some(&full.full_document_diagnostic_report.items)
            }
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(_) => None,
        })
        .flatten()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    let codes: Vec<_> = report
        .items
        .iter()
        .filter_map(|item| match item {
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) => {
                Some(&full.full_document_diagnostic_report.items)
            }
            tower_lsp::lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(_) => None,
        })
        .flatten()
        .filter_map(|diagnostic| diagnostic.code.as_ref())
        .collect();

    assert!(
        messages
            .iter()
            .any(|message| message.contains("failed to read configuration")),
        "expected configuration read diagnostic, got {messages:?}"
    );
    assert!(
        codes.iter().any(|code| {
            matches!(code, tower_lsp::lsp_types::NumberOrString::String(value) if value == "C001")
        }),
        "expected C001, got {codes:?}"
    );
    std::fs::remove_dir_all(root).ok();
}
