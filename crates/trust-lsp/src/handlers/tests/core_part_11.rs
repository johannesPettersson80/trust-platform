use super::*;

#[test]
pub(super) fn lsp_mitsubishi_gxworks3_example_has_no_unexpected_diagnostics() {
    let source = include_str!("../../../../../examples/mitsubishi_gxworks3_v1/src/Main.st");
    let configuration =
        include_str!("../../../../../examples/mitsubishi_gxworks3_v1/src/Configuration.st");

    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").expect("workspace uri");
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: Some("mitsubishi".to_string()),
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

    let uri = tower_lsp::lsp_types::Url::parse(
        "file:///workspace/examples/mitsubishi_gxworks3_v1/src/Main.st",
    )
    .expect("mitsubishi example uri");
    let configuration_uri = tower_lsp::lsp_types::Url::parse(
        "file:///workspace/examples/mitsubishi_gxworks3_v1/src/Configuration.st",
    )
    .expect("mitsubishi configuration uri");
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
        "expected no diagnostics for Mitsubishi GX Works3 example, got {summary:?}"
    );
}

pub(super) fn spawn_control_stub() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind control stub");
    let addr = listener.local_addr().expect("control stub addr");
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept control stub");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut writer = std::io::BufWriter::new(stream);

        for _ in 0..4 {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read line") == 0 {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let payload: serde_json::Value =
                serde_json::from_str(line.trim()).expect("parse payload");
            let id = payload
                .get("id")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let kind = payload
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let response = match kind {
                "debug.scopes" => json!({
                    "id": id,
                    "ok": true,
                    "result": {
                        "scopes": [
                            { "name": "Locals", "variablesReference": 1 },
                            { "name": "Globals", "variablesReference": 2 },
                            { "name": "Retain", "variablesReference": 3 },
                        ]
                    }
                }),
                "debug.variables" => {
                    let reference = payload
                        .get("params")
                        .and_then(|value| value.get("variables_reference"))
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0);
                    let variables = match reference {
                        1 => vec![json!({
                            "name": "x",
                            "value": "DInt(7)",
                            "variablesReference": 0
                        })],
                        2 => vec![json!({
                            "name": "g",
                            "value": "DInt(11)",
                            "variablesReference": 0
                        })],
                        3 => vec![json!({
                            "name": "r",
                            "value": "DInt(42)",
                            "variablesReference": 0
                        })],
                        _ => Vec::new(),
                    };
                    json!({
                        "id": id,
                        "ok": true,
                        "result": { "variables": variables }
                    })
                }
                _ => json!({ "id": id, "ok": false, "error": "unknown request" }),
            };
            writeln!(writer, "{response}").expect("write response");
            writer.flush().expect("flush response");
        }
    });

    (format!("tcp://{addr}"), handle)
}

pub(super) fn spawn_control_stub_with_instances(
    instance_name: &str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind control stub");
    let addr = listener.local_addr().expect("control stub addr");
    let instance_name = instance_name.to_string();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept control stub");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut writer = std::io::BufWriter::new(stream);

        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read line") == 0 {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let payload: serde_json::Value =
                serde_json::from_str(line.trim()).expect("parse payload");
            let id = payload
                .get("id")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let kind = payload
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let response = match kind {
                "debug.scopes" => json!({
                    "id": id,
                    "ok": true,
                    "result": {
                        "scopes": [
                            { "name": "Locals", "variablesReference": 1 },
                            { "name": "Instances", "variablesReference": 2 },
                        ]
                    }
                }),
                "debug.variables" => {
                    let reference = payload
                        .get("params")
                        .and_then(|value| value.get("variables_reference"))
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0);
                    let variables = match reference {
                        1 => vec![json!({
                            "name": "temp",
                            "value": "DInt(3)",
                            "variablesReference": 0
                        })],
                        2 => vec![json!({
                            "name": instance_name.clone(),
                            "value": "Instance(1)",
                            "variablesReference": 10
                        })],
                        10 => vec![json!({
                            "name": "x",
                            "value": "DInt(9)",
                            "variablesReference": 0
                        })],
                        _ => Vec::new(),
                    };
                    json!({
                        "id": id,
                        "ok": true,
                        "result": { "variables": variables }
                    })
                }
                _ => json!({ "id": id, "ok": false, "error": "unknown request" }),
            };
            writeln!(writer, "{response}").expect("write response");
            writer.flush().expect("flush response");
        }
    });

    (format!("tcp://{addr}"), handle)
}
