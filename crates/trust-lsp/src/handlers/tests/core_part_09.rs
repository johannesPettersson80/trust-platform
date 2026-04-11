use super::*;

#[test]
pub(super) fn lsp_inline_values_merge_instances_with_namespace() {
    let (endpoint, handle) = spawn_control_stub_with_instances("Ns.TestProgram#1");
    let source = r#"
NAMESPACE Ns
PROGRAM TestProgram
VAR
    x : DINT;
END_VAR
    x := x + 1;
END_PROGRAM
END_NAMESPACE
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
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig {
                control_endpoint: Some(endpoint),
                control_auth_token: None,
            },
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::InlineValueParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        range: tower_lsp::lsp_types::Range {
            start: position_at(source, "x := x"),
            end: position_at(source, "END_PROGRAM"),
        },
        context: tower_lsp::lsp_types::InlineValueContext {
            frame_id: 1,
            stopped_location: tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position::new(0, 0),
                end: tower_lsp::lsp_types::Position::new(0, 0),
            },
        },
        work_done_progress_params: Default::default(),
    };

    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();

    assert!(texts.iter().any(|text| text == " = DInt(9)"));

    handle.join().expect("control stub thread");
}

#[test]
pub(super) fn lsp_tutorial_examples_no_unexpected_diagnostics_snapshot() {
    let tutorials = [
        (
            "01_hello_counter.st",
            include_str!("../../../../../examples/tutorials/01_hello_counter.st"),
        ),
        (
            "02_blinker.st",
            include_str!("../../../../../examples/tutorials/02_blinker.st"),
        ),
        (
            "03_traffic_light.st",
            include_str!("../../../../../examples/tutorials/03_traffic_light.st"),
        ),
        (
            "04_tank_level.st",
            include_str!("../../../../../examples/tutorials/04_tank_level.st"),
        ),
        (
            "05_motor_starter.st",
            include_str!("../../../../../examples/tutorials/05_motor_starter.st"),
        ),
        (
            "06_recipe_manager.st",
            include_str!("../../../../../examples/tutorials/06_recipe_manager.st"),
        ),
        (
            "07_pid_loop.st",
            include_str!("../../../../../examples/tutorials/07_pid_loop.st"),
        ),
        (
            "08_conveyor_system.st",
            include_str!("../../../../../examples/tutorials/08_conveyor_system.st"),
        ),
        (
            "09_simulation_coupling.st",
            include_str!("../../../../../examples/tutorials/09_simulation_coupling.st"),
        ),
    ];

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").expect("workspace uri");
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
    let mut output = serde_json::Map::new();

    for (name, source) in tutorials {
        let uri = tower_lsp::lsp_types::Url::parse(&format!(
            "file:///workspace/examples/tutorials/{name}"
        ))
        .expect("tutorial uri");
        state.open_document(uri.clone(), 1, source.to_string());

        let file_id = state.get_document(&uri).expect("tutorial document").file_id;
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
                let severity = match diag.severity {
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR) => "error",
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING) => "warning",
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION) => "info",
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT) => "hint",
                    _ => "none",
                };
                format!(
                    "{code}|{severity}|{}:{}-{}:{}|{}",
                    diag.range.start.line,
                    diag.range.start.character,
                    diag.range.end.line,
                    diag.range.end.character,
                    diag.message
                )
            })
            .collect();

        assert!(
            summary.is_empty(),
            "expected no diagnostics for {name}, got {summary:?}"
        );
        output.insert(
            name.to_string(),
            serde_json::to_value(summary).expect("serialize diagnostics"),
        );
    }

    let rendered =
        serde_json::to_string_pretty(&Value::Object(output)).expect("serialize diagnostics");
    insta::with_settings!({ snapshot_path => "../snapshots" }, {
        insta::assert_snapshot!("lsp_tutorial_examples_diagnostics", rendered);
    });
}
