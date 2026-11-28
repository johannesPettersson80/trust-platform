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
        let _ = release_rx.recv_timeout(std::time::Duration::from_secs(30));
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
        .recv_timeout(std::time::Duration::from_secs(10))
        .map_err(|error| format!("silent endpoint did not accept connection: {error}"))?;
    // The request's own bound is the control client's 250 ms I/O timeout; the silent endpoint holds
    // the connection for 30 s, so a request that waited for it would take 30 s. Five seconds tells
    // the two apart and leaves a loaded runner a wide margin.
    let completed = done_rx.recv_timeout(std::time::Duration::from_secs(5));
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

/// Serves one control connection and sends the request it received to the returned channel. It
/// answers with a failure, so the inline-value fetch ends after that first request.
fn spawn_recording_control_stub() -> (String, std::sync::mpsc::Receiver<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind recording control stub");
    let addr = listener.local_addr().expect("recording control stub addr");
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept recording control stub");
        let mut reader = BufReader::new(stream.try_clone().expect("clone recording stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read recorded request");
        let request: Value =
            serde_json::from_str(line.trim_end()).expect("recorded request is JSON");
        request_tx.send(request).expect("send recorded request");
        writeln!(
            stream,
            "{}",
            json!({ "id": 1, "ok": false, "error": "recorded" })
        )
        .expect("write recorded response");
    });
    (format!("tcp://{addr}"), request_rx)
}

/// A workspace whose `trust-lsp.toml` runtime section is `repository`, with the editor's
/// `trust_lsp.runtime` settings set to `editor`.
fn inline_value_state(repository: RuntimeConfig, editor: Value) -> ServerState {
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
            runtime: repository,
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );
    state.set_config(json!({ "trust_lsp": { "runtime": editor } }));
    state
}

/// Runs one runtime inline-value request and returns the control request that the stub received.
/// The wait is bounded, so a request that is never sent fails the test instead of hanging it.
fn runtime_inline_request_sent(
    state: &ServerState,
    requests: &std::sync::mpsc::Receiver<Value>,
) -> Value {
    let source = runtime_inline_values_source();
    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/runtime.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());
    inline_value(state, runtime_inline_values_params(uri, source)).expect("inline values");
    requests
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the runtime endpoint received a control request")
}

#[test]
pub(super) fn lsp_inline_values_repository_endpoint_never_receives_the_editor_token() {
    let (endpoint, requests) = spawn_recording_control_stub();
    let state = inline_value_state(
        RuntimeConfig {
            control_endpoint: Some(endpoint),
            control_auth_token: None,
        },
        json!({
            "control_endpoint_enabled": true,
            "control_auth_token": "editor-token",
        }),
    );

    let request = runtime_inline_request_sent(&state, &requests);

    assert_eq!(request["type"], json!("debug.scopes"));
    assert!(
        request.get("auth").is_none(),
        "the editor token reached a repository endpoint: {request}"
    );
}

#[test]
pub(super) fn lsp_inline_values_repository_endpoint_uses_its_own_token() {
    let (endpoint, requests) = spawn_recording_control_stub();
    let state = inline_value_state(
        RuntimeConfig {
            control_endpoint: Some(endpoint),
            control_auth_token: Some("repository-token".to_string()),
        },
        json!({
            "control_endpoint_enabled": true,
            "control_auth_token": "editor-token",
        }),
    );

    let request = runtime_inline_request_sent(&state, &requests);

    assert_eq!(request["auth"], json!("repository-token"));
}

#[test]
pub(super) fn lsp_inline_values_editor_token_goes_to_the_editor_endpoint() {
    let (endpoint, requests) = spawn_recording_control_stub();
    let state = inline_value_state(
        RuntimeConfig::default(),
        json!({
            "control_endpoint_enabled": true,
            "control_endpoint": endpoint,
            "control_auth_token": "editor-token",
        }),
    );

    let request = runtime_inline_request_sent(&state, &requests);

    assert_eq!(request["auth"], json!("editor-token"));
}

/// A workspace folder's path on this platform, for a document URI that maps to a file path.
#[cfg(windows)]
const SIMULATOR_WORKSPACE: &str = r"C:\workspace";
/// A workspace folder's path on this platform, for a document URI that maps to a file path.
#[cfg(not(windows))]
const SIMULATOR_WORKSPACE: &str = "/workspace";

#[test]
pub(super) fn lsp_inline_values_reach_the_simulator_a_launch_started_for_the_workspace() {
    // Specification 14: with no endpoint in trust-lsp.toml or the editor's settings, a document's
    // inline values come from the simulator that a launch started for its workspace folder, with
    // the token the server named for it (specification 25 §6.8). Catches: inline values that reach
    // no debug session unless a pin names the session's endpoint (the examples' trust-debug.sock
    // pins reached none), and the editor's token sent to the simulator.
    let (endpoint, requests) = spawn_recording_control_stub();
    let state = ServerState::new();
    state.set_config(json!({
        "trust_lsp": { "runtime": { "control_auth_token": "editor-token" } }
    }));
    let folder = std::path::Path::new(SIMULATOR_WORKSPACE);
    state
        .simulator_controls()
        .name_and_record(Some(folder.to_path_buf()), |_| {
            crate::state::simulator_controls::SimulatorControl {
                endpoint,
                auth_token: "simulator-token".to_owned(),
            }
        });
    let source = runtime_inline_values_source();
    let uri =
        tower_lsp::lsp_types::Url::from_file_path(folder.join("runtime.st")).expect("a file URI");
    state.open_document(uri.clone(), 1, source.to_string());
    inline_value(&state, runtime_inline_values_params(uri, source)).expect("inline values");

    let request = requests
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the simulator received a control request");
    assert_eq!(request["type"], json!("debug.scopes"));
    assert_eq!(request["auth"], json!("simulator-token"));
}

/// A project folder whose `runtime.toml` names the control endpoint `endpoint` with the token
/// `token`, and that has no endpoint file, so that `ctl --project` finds that endpoint and token.
fn attached_project(endpoint: &str, token: &str) -> PathBuf {
    let folder = trust_test_support::unique_temp_path("trust-lsp-attached-inline");
    std::fs::create_dir_all(&folder).expect("the project folder");
    let runtime = trust_runtime::bundle_template::render_runtime_toml(&"Attached".into(), 100)
        .replacen(
            "[runtime.control]\n",
            &format!("[runtime.control]\nendpoint = \"{endpoint}\"\nauth_token = \"{token}\"\n"),
            1,
        );
    std::fs::write(folder.join("runtime.toml"), runtime).expect("runtime.toml");
    folder
}

/// The control request that a runtime inline-value request for a document of `folder` sent to the
/// recording stub, the editor's settings holding a token of their own.
fn attached_inline_request_sent(
    folder: &std::path::Path,
    attached: &str,
    requests: &std::sync::mpsc::Receiver<Value>,
) -> Value {
    let state = ServerState::new();
    state.set_config(json!({
        "trust_lsp": { "runtime": { "control_auth_token": "editor-token" } }
    }));
    // A simulator that a launch started for the folder earlier; the attach session comes first.
    state
        .simulator_controls()
        .name_and_record(Some(folder.to_path_buf()), |_| {
            crate::state::simulator_controls::SimulatorControl {
                endpoint: "tcp://127.0.0.1:9".to_owned(),
                auth_token: "simulator-token".to_owned(),
            }
        });
    state.simulator_controls().attached().record(
        Some(folder.to_path_buf()),
        "session-1",
        Some(attached.to_owned()),
    );
    let source = runtime_inline_values_source();
    let uri =
        tower_lsp::lsp_types::Url::from_file_path(folder.join("runtime.st")).expect("a file URI");
    state.open_document(uri.clone(), 1, source.to_string());
    inline_value(&state, runtime_inline_values_params(uri, source)).expect("inline values");
    requests
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the attached runtime received a control request")
}

#[test]
pub(super) fn lsp_inline_values_reach_the_runtime_a_session_attached_to_with_its_project_token() {
    // Specification 14 §7.1.2: with no endpoint in trust-lsp.toml or the editor's settings, the
    // runtime that a debug session of the document's folder attached to comes before the
    // simulator a launch started for the folder, and gets the token of the folder's own runtime,
    // here from the project's runtime.toml, which names the attached endpoint. Catches an attach
    // session whose inline values reach the simulator, and the editor's or the simulator's token
    // sent to the attached runtime.
    let (endpoint, requests) = spawn_recording_control_stub();
    let token = trust_runtime::control::endpoint::generate_control_token().expect("a token");
    let folder = attached_project(&endpoint, &token);

    let request = attached_inline_request_sent(&folder, &endpoint, &requests);

    assert_eq!(request["type"], json!("debug.scopes"));
    assert_eq!(request["auth"], json!(token));
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
pub(super) fn lsp_inline_values_send_no_token_to_an_attached_runtime_that_is_not_the_project_s() {
    // Specification 14 §7.1.2: the token of the folder's own runtime goes only to that runtime.
    // The project's runtime.toml names tcp://127.0.0.1:9 with a token, and the session attached
    // to another endpoint: the request carries no token. Catches the project's token, the
    // editor's or the simulator's sent to a runtime that is not the project's.
    let (endpoint, requests) = spawn_recording_control_stub();
    let token = trust_runtime::control::endpoint::generate_control_token().expect("a token");
    let folder = attached_project("tcp://127.0.0.1:9", &token);

    let request = attached_inline_request_sent(&folder, &endpoint, &requests);

    assert_eq!(request["type"], json!("debug.scopes"));
    assert_eq!(request.get("auth"), None, "{request}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
pub(super) fn lsp_inline_values_take_the_repository_endpoint_before_an_attach_session() {
    // Specification 14 §7.1.2: trust-lsp.toml's endpoint comes before the runtime that a debug
    // session of the folder attached to. With both, the request goes to the repository's endpoint
    // with its own token. Catches an attach record placed before the endpoints that a person
    // configured.
    let (endpoint, requests) = spawn_recording_control_stub();
    let state = inline_value_state(
        RuntimeConfig {
            control_endpoint: Some(endpoint),
            control_auth_token: Some("repository-token".to_string()),
        },
        json!({}),
    );
    state.simulator_controls().attached().record(
        Some(PathBuf::from("/workspace")),
        "session-1",
        Some("tcp://127.0.0.1:9".to_owned()),
    );

    let request = runtime_inline_request_sent(&state, &requests);

    assert_eq!(request["auth"], json!("repository-token"));
}
