use super::*;

#[test]
pub(super) fn lsp_inline_values_runtime_override_accepts_camel_case_client_settings() {
    let (endpoint, handle) = spawn_control_stub();
    let source = runtime_inline_values_source();
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
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_config(json!({
        "stLsp": {
            "runtime": {
                "inlineValuesEnabled": true,
                "controlEndpointEnabled": true,
                "controlEndpoint": endpoint,
            }
        }
    }));

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let params = runtime_inline_values_params(uri, source);
    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|text| text == " = DInt(11)"));
    assert!(texts.iter().any(|text| text == " = DInt(42)"));

    handle.join().expect("control stub thread");
}

#[test]
pub(super) fn lsp_inline_values_runtime_override_accepts_snake_case_client_settings() {
    let (endpoint, handle) = spawn_control_stub();
    let source = runtime_inline_values_source();
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
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_config(json!({
        "trust_lsp": {
            "runtime": {
                "inline_values_enabled": true,
                "control_endpoint_enabled": true,
                "control_endpoint": endpoint,
            }
        }
    }));

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let params = runtime_inline_values_params(uri, source);
    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|text| text == " = DInt(11)"));
    assert!(texts.iter().any(|text| text == " = DInt(42)"));

    handle.join().expect("control stub thread");
}

#[test]
pub(super) fn lsp_inline_values_runtime_override_prefers_camel_case_when_aliases_conflict() {
    let (endpoint, handle) = spawn_control_stub();
    let endpoint_addr = endpoint
        .strip_prefix("tcp://")
        .map(str::to_string)
        .expect("tcp endpoint");
    let source = runtime_inline_values_source();
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
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_config(json!({
        "stLsp": {
            "runtime": {
                "inlineValuesEnabled": false,
                "inline_values_enabled": true,
                "controlEndpointEnabled": false,
                "control_endpoint_enabled": true,
                "controlEndpoint": endpoint.clone(),
                "control_endpoint": endpoint,
            }
        }
    }));

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    let params = runtime_inline_values_params(uri, source);
    let values = inline_value(&state, params).expect("inline values");
    let texts: Vec<String> = values
        .iter()
        .filter_map(|value| match value {
            tower_lsp::lsp_types::InlineValue::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();

    assert!(
        texts.iter().all(|text| text != " = DInt(11)"),
        "camelCase control flag should disable runtime fetch"
    );
    assert!(
        texts.iter().all(|text| text != " = DInt(42)"),
        "camelCase control flag should disable runtime fetch"
    );

    let _ = std::net::TcpStream::connect(endpoint_addr);
    handle.join().expect("control stub thread");
}

#[test]
pub(super) fn lsp_inline_values_silent_runtime_endpoint_returns_bounded_empty_result() {
    let observation =
        silent_runtime_inline_value_observation().expect("silent runtime inline-value observation");
    assert_eq!(observation["completed_within_bound"], true);
    assert_eq!(observation["value_count"], 0);
}

pub(in crate::handlers::tests) fn silent_runtime_inline_value_observation() -> Result<Value, String>
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind silent endpoint");
    let addr = listener.local_addr().expect("silent endpoint addr");
    let endpoint = format!("tcp://{addr}");
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept silent endpoint");
        accepted_tx.send(()).expect("send accepted signal");
        let _stream = stream;
        let _ = release_rx.recv_timeout(std::time::Duration::from_secs(2));
    });

    let source = runtime_inline_values_source();
    let state = Arc::new(ServerState::new());
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
    let worker_state = Arc::clone(&state);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let params = runtime_inline_values_params(uri, source);
        let result = inline_value(&worker_state, params);
        done_tx.send(result).expect("send inline result");
    });

    accepted_rx
        .recv_timeout(std::time::Duration::from_millis(200))
        .map_err(|error| format!("silent endpoint did not accept connection: {error}"))?;
    let completed = done_rx.recv_timeout(std::time::Duration::from_millis(500));
    let _ = release_tx.send(());
    server
        .join()
        .map_err(|_| "silent endpoint thread panicked".to_string())?;
    worker
        .join()
        .map_err(|_| "inline worker thread panicked".to_string())?;

    let values = completed
        .map_err(|_| {
            "inlineValue exceeded the bounded timeout for a silent runtime endpoint".to_string()
        })?
        .ok_or_else(|| "inline values response missing".to_string())?;
    Ok(json!({
        "completed_within_bound": true,
        "value_count": values.len(),
    }))
}

#[test]
pub(super) fn lsp_inline_values_merge_instances_into_locals() {
    let (endpoint, handle) = spawn_control_stub_with_instances("TestProgram#1");
    let source = r#"
PROGRAM TestProgram
VAR
    x : DINT;
END_VAR
    x := x + 1;
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
