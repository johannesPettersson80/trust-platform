use super::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_WORKSPACE_ID: AtomicUsize = AtomicUsize::new(0);

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new(label: &str) -> Self {
        let id = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-dev-agent-contract-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create test workspace");
        Self {
            root: root.canonicalize().expect("canonical test workspace"),
        }
    }

    fn server(&self) -> AgentServer {
        AgentServer::new(self.root.clone())
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(path, text).expect("write fixture");
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn request(id: Value, method: &str, params: Option<Value>) -> String {
    let mut value = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
    });
    if let Some(params) = params {
        value
            .as_object_mut()
            .expect("request object")
            .insert("params".to_string(), params);
    }
    serde_json::to_string(&value).expect("serialize request")
}

fn response_value(response: JsonRpcResponse) -> Value {
    serde_json::to_value(response).expect("serialize response")
}

fn success(response: JsonRpcResponse) -> Value {
    let value = response_value(response);
    assert_eq!(value["jsonrpc"], "2.0");
    assert!(value.get("error").is_none(), "unexpected error: {value}");
    assert!(value.get("result").is_some(), "missing result: {value}");
    value
}

fn failure(response: JsonRpcResponse, code: i64) -> Value {
    let value = response_value(response);
    assert_eq!(value["jsonrpc"], "2.0");
    assert!(value.get("result").is_none(), "unexpected result: {value}");
    assert_eq!(value["error"]["code"], code, "unexpected error: {value}");
    value
}

#[test]
fn parse_error_has_null_id_and_exclusive_error_member() {
    let workspace = TestWorkspace::new("parse-error");
    let mut server = workspace.server();

    let response = failure(server.handle_line("{"), -32700);

    assert_eq!(response["id"], Value::Null);
    assert!(response["error"]["message"]
        .as_str()
        .is_some_and(|message| message.starts_with("Parse error:")));
    assert!(response.get("result").is_none());
}

#[test]
fn non_object_json_is_invalid_envelope_not_parse_error() {
    let workspace = TestWorkspace::new("non-object-envelope");
    let mut server = workspace.server();

    for line in ["null", "true", "17", "[]", "\"request\""] {
        let response = failure(server.handle_line(line), -32600);
        assert_eq!(response["id"], Value::Null, "line: {line}");
    }
}

#[test]
fn missing_jsonrpc_is_invalid_envelope_not_parse_error() {
    let workspace = TestWorkspace::new("missing-jsonrpc");
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(r#"{"id":7,"method":"agent.describe"}"#),
        -32600,
    );

    assert_eq!(response["id"], 7);
}

#[test]
fn unsupported_jsonrpc_version_preserves_valid_request_id() {
    let workspace = TestWorkspace::new("wrong-version");
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(r#"{"jsonrpc":"1.0","id":"req-a","method":"agent.describe"}"#),
        -32600,
    );

    assert_eq!(response["id"], "req-a");
    assert!(response["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("Unsupported jsonrpc version")));
}

#[test]
fn missing_method_is_invalid_envelope_not_parse_error() {
    let workspace = TestWorkspace::new("missing-method");
    let mut server = workspace.server();

    let response = failure(server.handle_line(r#"{"jsonrpc":"2.0","id":8}"#), -32600);

    assert_eq!(response["id"], 8);
}

#[test]
fn empty_method_is_invalid_envelope() {
    let workspace = TestWorkspace::new("empty-method");
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(r#"{"jsonrpc":"2.0","id":9,"method":"  "}"#),
        -32600,
    );

    assert_eq!(response["id"], 9);
}

#[test]
fn missing_id_notification_is_rejected_as_unsupported_v1_envelope() {
    let workspace = TestWorkspace::new("notification");
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(r#"{"jsonrpc":"2.0","method":"agent.describe"}"#),
        -32600,
    );

    assert_eq!(response["id"], Value::Null);
}

#[test]
fn invalid_id_shapes_are_rejected_with_null_response_id() {
    let workspace = TestWorkspace::new("bad-id-shapes");
    let mut server = workspace.server();

    for id in [json!(null), json!(true), json!([]), json!({})] {
        let line = request(id, "agent.describe", None);
        let response = failure(server.handle_line(&line), -32600);
        assert_eq!(response["id"], Value::Null);
    }
}

#[test]
fn string_and_numeric_ids_are_preserved_exactly() {
    let workspace = TestWorkspace::new("valid-ids");
    let mut server = workspace.server();

    for id in [json!("request-17"), json!(17), json!(17.5)] {
        let line = request(id.clone(), "agent.describe", Some(json!({})));
        let response = success(server.handle_line(&line));
        assert_eq!(response["id"], id);
    }
}

#[test]
fn scalar_params_are_rejected_at_the_envelope_boundary() {
    let workspace = TestWorkspace::new("scalar-params");
    let mut server = workspace.server();

    for params in [json!(null), json!(true), json!(3), json!("bad")] {
        let line = request(json!(20), "agent.describe", Some(params));
        let response = failure(server.handle_line(&line), -32600);
        assert_eq!(response["id"], 20);
    }
}

#[test]
fn unknown_top_level_members_do_not_change_dispatch() {
    let workspace = TestWorkspace::new("unknown-envelope-members");
    let mut server = workspace.server();
    let line = r#"{"jsonrpc":"2.0","id":21,"method":"agent.describe","params":{},"future":true}"#;

    let response = success(server.handle_line(line));

    assert_eq!(response["id"], 21);
    assert_eq!(response["result"]["transport"], "stdio");
}

#[test]
fn unknown_method_is_correlated_and_has_stable_code() {
    let workspace = TestWorkspace::new("unknown-method");
    let mut server = workspace.server();
    let line = request(json!("missing-1"), "missing.method", Some(json!({})));

    let response = failure(server.handle_line(&line), -32601);

    assert_eq!(response["id"], "missing-1");
    assert_eq!(
        response["error"]["message"],
        "Method 'missing.method' is not available."
    );
}

#[test]
fn required_params_absence_and_wrong_shape_have_invalid_params_code() {
    let workspace = TestWorkspace::new("required-params");
    let mut server = workspace.server();

    let missing = failure(
        server.handle_line(&request(json!(30), "workspace.read", None)),
        -32602,
    );
    assert_eq!(missing["id"], 30);

    let wrong_shape = failure(
        server.handle_line(&request(
            json!(31),
            "workspace.read",
            Some(json!({"path": 7})),
        )),
        -32602,
    );
    assert_eq!(wrong_shape["id"], 31);
}

#[test]
fn describe_is_complete_duplicate_free_and_transport_bound() {
    let workspace = TestWorkspace::new("describe");
    let mut server = workspace.server();

    let response =
        success(server.handle_line(&request(json!(40), "agent.describe", Some(json!({})))));
    let result = &response["result"];
    assert_eq!(
        result["workspace_root"],
        workspace.root.display().to_string()
    );
    assert_eq!(result["transport"], "stdio");
    assert_eq!(result["framing"], "jsonl");

    let methods = result["methods"]
        .as_array()
        .expect("describe methods")
        .iter()
        .map(|value| value.as_str().expect("method string"))
        .collect::<Vec<_>>();
    let unique = methods
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), methods.len(), "duplicate method in describe");
    assert_eq!(
        unique,
        [
            "agent.describe",
            "harness.advance_time",
            "harness.cycle",
            "harness.execute",
            "harness.get_output",
            "harness.load",
            "harness.reload",
            "harness.run_until",
            "harness.set_input",
            "lsp.ast_canonicalize",
            "lsp.ast_similarity",
            "lsp.diagnostics",
            "lsp.format",
            "runtime.build",
            "runtime.compile_reload",
            "runtime.reload",
            "runtime.test",
            "runtime.validate",
            "workspace.project_info",
            "workspace.read",
            "workspace.write",
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn workspace_read_returns_normalized_relative_path_and_exact_text() {
    let workspace = TestWorkspace::new("read");
    workspace.write("src/main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let mut server = workspace.server();

    let response = success(server.handle_line(&request(
        json!(50),
        "workspace.read",
        Some(json!({"path": "./src/lib/../main.st"})),
    )));

    assert_eq!(response["result"]["path"], "src/main.st");
    assert_eq!(response["result"]["text"], "PROGRAM Main\nEND_PROGRAM\n");
}

#[test]
fn workspace_read_missing_file_has_io_code_and_normalized_path_data() {
    let workspace = TestWorkspace::new("read-missing");
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(&request(
            json!(51),
            "workspace.read",
            Some(json!({"path": "./src/../missing.st"})),
        )),
        -32002,
    );

    assert_eq!(response["error"]["data"]["path"], "missing.st");
    assert!(response["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("failed to read")));
}

#[test]
fn workspace_write_creates_parents_and_reports_utf8_bytes() {
    let workspace = TestWorkspace::new("write-utf8");
    let mut server = workspace.server();
    let text = "åäö PLC";

    let response = success(server.handle_line(&request(
        json!(52),
        "workspace.write",
        Some(json!({
            "path": "./generated/../src/main.st",
            "text": text,
            "create_parents": true
        })),
    )));

    assert_eq!(response["result"]["path"], "src/main.st");
    assert_eq!(response["result"]["bytes_written"], json!(text.len()));
    assert_eq!(
        fs::read_to_string(workspace.root.join("src/main.st")).unwrap(),
        text
    );
}

#[test]
fn workspace_write_without_parent_creation_is_fail_closed() {
    let workspace = TestWorkspace::new("write-no-parents");
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(&request(
            json!(53),
            "workspace.write",
            Some(json!({
                "path": "missing/parent/main.st",
                "text": "PROGRAM Main\nEND_PROGRAM\n",
                "create_parents": false
            })),
        )),
        -32002,
    );

    assert!(!workspace.root.join("missing").exists());
    assert_eq!(response["error"]["data"]["path"], "missing/parent/main.st");
}

#[test]
fn lexical_path_matrix_rejects_empty_absolute_and_above_root_paths() {
    for path in [
        "",
        " ",
        ".",
        "./",
        "..",
        "../main.st",
        "src/../../main.st",
        "/tmp/main.st",
    ] {
        let error = normalize_workspace_path(path).expect_err(path);
        assert!(
            error.code == -32602 || error.code == ERROR_PATH_OUTSIDE_WORKSPACE,
            "unexpected code {} for {path:?}",
            error.code
        );
    }
}

#[test]
fn platform_root_spellings_are_rejected_portably() {
    for path in [
        r"C:\Windows\system.ini",
        r"C:/Windows/system.ini",
        r"\\server\share\secret.st",
        r"//server/share/secret.st",
        r"\\?\C:\secret.st",
    ] {
        let error = normalize_workspace_path(path).expect_err(path);
        assert_eq!(error.code, ERROR_PATH_OUTSIDE_WORKSPACE, "{path}");
    }
}

#[test]
fn in_scope_parent_segments_normalize_without_losing_filename() {
    for (raw, expected) in [
        ("src/../main.st", "main.st"),
        ("a/b/../../main.st", "main.st"),
        ("a/./b/../c.st", "a/c.st"),
    ] {
        assert_eq!(
            normalize_workspace_path(raw).unwrap(),
            PathBuf::from(expected)
        );
    }
}

#[test]
fn resolved_project_must_be_an_existing_directory() {
    let workspace = TestWorkspace::new("project-directory");
    workspace.write("file-project", "not a directory");
    let server = workspace.server();

    let missing = server
        .resolve_project_root(Some("missing"))
        .expect_err("missing project");
    assert_eq!(missing.code, -32602);

    let file = server
        .resolve_project_root(Some("file-project"))
        .expect_err("file project");
    assert_eq!(file.code, -32602);
}

#[test]
fn project_subpath_uses_project_root_not_workspace_root() {
    let workspace = TestWorkspace::new("project-subpath");
    fs::create_dir_all(workspace.root.join("projects/plc/src")).unwrap();
    let server = workspace.server();
    let project = server.resolve_project_root(Some("projects/plc")).unwrap();

    let source_root = server.resolve_project_subpath(&project, "src").unwrap();

    assert_eq!(source_root, project.join("src"));
}

#[cfg(unix)]
#[test]
fn workspace_read_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("read-symlink-escape");
    let outside = TestWorkspace::new("read-symlink-outside");
    outside.write("secret.st", "SECRET");
    symlink(&outside.root, workspace.root.join("escape")).unwrap();
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(&request(
            json!(60),
            "workspace.read",
            Some(json!({"path": "escape/secret.st"})),
        )),
        -32001,
    );

    assert_eq!(response["error"]["data"]["kind"], "path_outside_workspace");
    assert_eq!(response["error"]["data"]["path"], "escape/secret.st");
}

#[cfg(unix)]
#[test]
fn workspace_write_rejects_symlink_escape_before_mutation() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("write-symlink-escape");
    let outside = TestWorkspace::new("write-symlink-outside");
    symlink(&outside.root, workspace.root.join("escape")).unwrap();
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(&request(
            json!(61),
            "workspace.write",
            Some(json!({
                "path": "escape/created/secret.st",
                "text": "MUST NOT ESCAPE",
                "create_parents": true
            })),
        )),
        -32001,
    );

    assert_eq!(response["error"]["data"]["kind"], "path_outside_workspace");
    assert!(!outside.root.join("created").exists());
}

#[cfg(unix)]
#[test]
fn project_root_rejects_symlinked_directory_outside_workspace() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("project-symlink-escape");
    let outside = TestWorkspace::new("project-symlink-outside");
    symlink(&outside.root, workspace.root.join("project")).unwrap();
    let server = workspace.server();

    let error = server
        .resolve_project_root(Some("project"))
        .expect_err("symlink project escape");

    assert_eq!(error.code, ERROR_PATH_OUTSIDE_WORKSPACE);
    assert_eq!(
        error.data.as_ref().unwrap()["kind"],
        "path_outside_workspace"
    );
}

#[cfg(unix)]
#[test]
fn project_subpath_rejects_symlink_escape_outside_project() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("subpath-symlink-escape");
    let outside = TestWorkspace::new("subpath-symlink-outside");
    let project = workspace.root.join("project");
    fs::create_dir_all(&project).unwrap();
    symlink(&outside.root, project.join("src")).unwrap();
    let server = workspace.server();

    let error = server
        .resolve_project_subpath(&project, "src")
        .expect_err("symlink source escape");

    assert_eq!(error.code, ERROR_PATH_OUTSIDE_WORKSPACE);
}

#[test]
fn workspace_containment_error_shape_is_stable() {
    let error = AgentCommandError::path_outside_workspace("../secret.st");
    let rpc_error: JsonRpcError = error.into();
    let value = serde_json::to_value(rpc_error).unwrap();

    assert_eq!(value["code"], -32001);
    assert_eq!(
        value["message"],
        "Path '../secret.st' resolves outside the workspace root."
    );
    assert_eq!(value["data"]["path"], "../secret.st");
    assert_eq!(value["data"]["kind"], "path_outside_workspace");
}

#[test]
fn harness_not_loaded_maps_to_stable_error_code_without_data() {
    let error: AgentCommandError = HarnessAutomationError::NotLoaded.into();
    let rpc_error: JsonRpcError = error.into();
    let value = serde_json::to_value(rpc_error).unwrap();

    assert_eq!(value["code"], -32003);
    assert_eq!(
        value["message"],
        "Harness is not loaded. Call harness.load first."
    );
    assert!(value.get("data").is_none());
}

#[test]
fn harness_invalid_argument_maps_to_json_rpc_invalid_params() {
    let error: AgentCommandError =
        HarnessAutomationError::InvalidArgument("bad cycle count".into()).into();
    let rpc_error: JsonRpcError = error.into();
    let value = serde_json::to_value(rpc_error).unwrap();

    assert_eq!(value["code"], -32602);
    assert_eq!(value["message"], "bad cycle count");
}

#[test]
fn harness_runtime_cycle_error_preserves_structured_errors() {
    let error: AgentCommandError = HarnessAutomationError::RuntimeCycle {
        message: "cycle failed".into(),
        errors: vec!["division by zero".into(), "output commit blocked".into()],
    }
    .into();
    let rpc_error: JsonRpcError = error.into();
    let value = serde_json::to_value(rpc_error).unwrap();

    assert_eq!(value["code"], -32002);
    assert_eq!(value["message"], "cycle failed");
    assert_eq!(
        value["data"]["errors"],
        json!(["division by zero", "output commit blocked"])
    );
}

#[test]
fn harness_run_until_timeout_preserves_name_budget_and_typed_expected_value() {
    let error: AgentCommandError = HarnessAutomationError::RunUntilTimeout {
        name: "q".into(),
        max_cycles: 7,
        expected: trust_runtime::value::Value::Bool(true),
    }
    .into();
    let rpc_error: JsonRpcError = error.into();
    let value = serde_json::to_value(rpc_error).unwrap();

    assert_eq!(value["code"], -32004);
    assert_eq!(value["data"]["name"], "q");
    assert_eq!(value["data"]["max_cycles"], 7);
    assert_eq!(
        value["data"]["expected"],
        json!({"type": "BOOL", "value": true})
    );
}
