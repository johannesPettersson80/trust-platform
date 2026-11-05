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

/// Specification 20 §7: `page` and `page_size` come together, and both count from 1. Catches a
/// request whose page cannot be cut being answered with every issue, with none, or with a guessed
/// page.
#[test]
fn page_and_page_size_come_together_and_count_from_one() {
    let workspace = TestWorkspace::new("diagnostics-page-params");
    workspace.write("src/main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let mut server = workspace.server();

    for (params, message) in [
        (
            json!({"page": 2}),
            "'page' and 'page_size' must be given together.",
        ),
        (
            json!({"page_size": 10}),
            "'page' and 'page_size' must be given together.",
        ),
        (
            json!({"page": 0, "page_size": 10}),
            "'page' and 'page_size' must both be at least 1 when present.",
        ),
        (
            json!({"page": 1, "page_size": 0}),
            "'page' and 'page_size' must both be at least 1 when present.",
        ),
    ] {
        let response = failure(
            server.handle_line(&request(json!(60), "lsp.diagnostics", Some(params.clone()))),
            -32602,
        );
        assert_eq!(response["error"]["message"], message, "{params}");
    }
}

/// The listed issues of a response as (path, line, code).
fn listed_issues(response: &Value) -> Vec<(String, u64, String)> {
    response["result"]["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .map(|issue| {
            (
                issue["path"].as_str().expect("issue path").to_owned(),
                issue["line"].as_u64().expect("issue line"),
                issue["code"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// Specification 20 §7: a library elsewhere in the workspace is listed by its full path, and the
/// issues are sorted by file before a page is cut. Catches a listing filtered by the project instead
/// of the workspace, a full path written with Windows' `\\?\` prefix or with backslashes, and a page
/// cut in the language server's order, which sorts by the issue's path text and so puts the full
/// path first.
#[test]
fn a_library_elsewhere_in_the_workspace_is_listed_by_its_full_path_in_file_order() {
    let workspace = TestWorkspace::new("diagnostics-workspace-library");
    workspace.write(
        "proj/trust-lsp.toml",
        "[project]\ninclude_paths = [\"src\"]\nvendor_profile = \"codesys\"\nlibrary_paths = [\"../zlib\"]\n",
    );
    workspace.write(
        "proj/src/main.st",
        "PROGRAM Main\nVAR\n    fb : fbLib;\n    s : SINT;\n    b : BYTE;\nEND_VAR\nfb(x := 1);\nb := s;\nEND_PROGRAM\n",
    );
    workspace.write(
        "zlib/fb_lib.st",
        "FUNCTION_BLOCK fbLib\nVAR_INPUT\n    x : INT;\nEND_VAR\nVAR\n    s : SINT;\n    b : BYTE;\nEND_VAR\nb := s;\nEND_FUNCTION_BLOCK\n",
    );
    let library = workspace.root.join("zlib").join("fb_lib.st");
    let library = library
        .display()
        .to_string()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    let issue = |path: &str, line: u64, code: &str| (path.to_owned(), line, code.to_owned());
    let mut server = workspace.server();
    let mut diagnostics = |params: Value| {
        success(server.handle_line(&request(json!(61), "lsp.diagnostics", Some(params))))
    };

    // The program that no configuration runs (W009, line 1) and its narrowing store (E203 under
    // `codesys`, line 8), then the library's narrowing store (line 9): `.../proj/src/main.st`
    // sorts before `.../zlib/fb_lib.st`. A library is API surface, so its unused input is not
    // reported (#180).
    let all = diagnostics(json!({"project": "proj"}));
    assert_eq!(
        listed_issues(&all),
        vec![
            issue("src/main.st", 1, "W009"),
            issue("src/main.st", 8, "E203"),
            issue(&library, 9, "E203"),
        ],
        "{all:#}"
    );
    let first = diagnostics(json!({"project": "proj", "page": 1, "page_size": 1}));
    assert_eq!(listed_issues(&first), vec![issue("src/main.st", 1, "W009")]);
    let last = diagnostics(json!({"project": "proj", "page": 3, "page_size": 1}));
    assert_eq!(listed_issues(&last), vec![issue(&library, 9, "E203")]);
}

/// Canonical contract, "Workspace filesystem containment": a `path` that a symbolic link takes out of
/// the workspace, or out of the project into the rest of the workspace, is `-32001` before anything
/// is analyzed. Catches the override that replaces a library's text elsewhere and then lists none of
/// its findings: a clean answer for a file the agent may not see.
#[cfg(unix)]
#[test]
fn a_diagnostics_path_linked_out_of_the_project_is_refused() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("diagnostics-path-link");
    let outside = TestWorkspace::new("diagnostics-path-link-outside");
    outside.write("fb_lib.st", "FUNCTION_BLOCK fbLib\nEND_FUNCTION_BLOCK\n");
    workspace.write(
        "proj/trust-lsp.toml",
        &format!(
            "[project]\ninclude_paths = [\"src\"]\nlibrary_paths = [{:?}, \"../shared\"]\n",
            outside.root.display().to_string()
        ),
    );
    workspace.write("proj/src/main.st", "PROGRAM Main\nEND_PROGRAM\n");
    workspace.write(
        "shared/other.st",
        "FUNCTION_BLOCK fbOther\nEND_FUNCTION_BLOCK\n",
    );
    symlink(
        outside.root.join("fb_lib.st"),
        workspace.root.join("proj/src/lib_alias.st"),
    )
    .unwrap();
    symlink(
        workspace.root.join("shared/other.st"),
        workspace.root.join("proj/src/other_alias.st"),
    )
    .unwrap();
    let mut server = workspace.server();

    for alias in ["src/lib_alias.st", "src/other_alias.st"] {
        let params = json!({
            "project": "proj",
            "path": alias,
            "content": "FUNCTION_BLOCK fbLib\nv := ;\nEND_FUNCTION_BLOCK\n",
        });
        let response = failure(
            server.handle_line(&request(json!(62), "lsp.diagnostics", Some(params))),
            -32001,
        );
        assert_eq!(
            response["error"]["data"]["kind"], "path_outside_workspace",
            "{alias}"
        );
        assert_eq!(response["error"]["data"]["path"], alias);
    }
}

/// Canonical contract: an I/O failure never answers as an empty project. A `sources_root` that does
/// not exist is "sources directory not found", and one that holds none of the project's source files
/// (a folder outside the include paths) is refused too, while the project has an error. Catches the
/// typo `scr` answering `errors: 0`, and a check that refuses the project's own source folder.
#[test]
fn a_sources_root_without_the_projects_sources_is_an_error() {
    let workspace = TestWorkspace::new("diagnostics-sources-root");
    workspace.write("trust-lsp.toml", "[project]\ninclude_paths = [\"src\"]\n");
    workspace.write(
        "src/main.st",
        "PROGRAM Main\nVAR\n    v : INT;\nEND_VAR\nv := Missing;\nEND_PROGRAM\n",
    );
    workspace.write("alt/other.st", "PROGRAM Other\nEND_PROGRAM\n");
    let mut server = workspace.server();

    for (sources_root, message) in [
        ("scr", "sources directory not found"),
        ("alt", "holds none of the project's source files"),
    ] {
        let params = json!({"sources_root": sources_root});
        let response = failure(
            server.handle_line(&request(json!(63), "lsp.diagnostics", Some(params))),
            -32002,
        );
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|text| text.contains(message)),
            "{sources_root}: {response:#}"
        );
    }
    let params = json!({"sources_root": "src"});
    let listed = success(server.handle_line(&request(json!(64), "lsp.diagnostics", Some(params))));
    assert_eq!(listed["result"]["errors"], 1, "{listed:#}");
}

/// A refused request says why: a file outside the include paths is "not a source file of the
/// project". Catches an answer that keeps only "collect diagnostics" and drops the reason.
#[test]
fn a_file_outside_the_include_paths_is_refused_with_its_reason() {
    let workspace = TestWorkspace::new("diagnostics-path-reason");
    workspace.write("trust-lsp.toml", "[project]\ninclude_paths = [\"src\"]\n");
    workspace.write("src/main.st", "PROGRAM Main\nEND_PROGRAM\n");
    workspace.write("notes/draft.st", "PROGRAM Draft\nEND_PROGRAM\n");
    let mut server = workspace.server();

    let params = json!({"path": "notes/draft.st"});
    let response = failure(
        server.handle_line(&request(json!(65), "lsp.diagnostics", Some(params))),
        -32002,
    );
    let message = response["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("notes/draft.st") && message.contains("not a source file of the project"),
        "{message}"
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

// --- Opt-in issues_limit cap on the runtime transaction methods
// --- (specification 20 §7).

fn invalid_agent_program(name: &str, symbol: &str) -> String {
    format!("PROGRAM {name}\nVAR\n  value : INT;\nEND_VAR\nvalue := {symbol};\nEND_PROGRAM\n")
}

#[test]
fn compile_reload_issues_limit_caps_and_flags_only_when_exceeded() {
    let workspace = TestWorkspace::new("compile-reload-cap");
    workspace.write("src/a.st", &invalid_agent_program("Alpha", "MissingA"));
    let mut server = workspace.server();

    let whole = success(server.handle_line(&request(
        json!(1),
        "runtime.compile_reload",
        Some(json!({})),
    )));
    let total_issues = whole["result"]["issues"]
        .as_array()
        .expect("issues array")
        .len();
    assert!(
        total_issues >= 1,
        "blocked diagnose carries issues: {whole:#}"
    );
    assert!(
        whole.get("issuesTruncated").is_none(),
        "uncapped response carries no truncation fields: {whole:#}"
    );

    let capped = success(server.handle_line(&request(
        json!(2),
        "runtime.compile_reload",
        Some(json!({"issues_limit": 1})),
    )));
    assert_eq!(
        capped["result"]["issues"]
            .as_array()
            .expect("issues array")
            .len(),
        1
    );
    assert_eq!(capped["result"]["issuesTruncated"], true);
    assert_eq!(capped["result"]["totalIssues"], total_issues);

    let generous = success(server.handle_line(&request(
        json!(3),
        "runtime.compile_reload",
        Some(json!({"issues_limit": 9999})),
    )));
    assert_eq!(
        generous["result"]["issues"]
            .as_array()
            .expect("issues array")
            .len(),
        total_issues
    );
    assert!(
        generous.get("issuesTruncated").is_none(),
        "an unexceeded limit leaves the response unchanged: {generous:#}"
    );
}

#[test]
fn compile_reload_rejects_a_zero_issues_limit_as_invalid_params() {
    let workspace = TestWorkspace::new("compile-reload-cap-zero");
    let mut server = workspace.server();

    let response = failure(
        server.handle_line(&request(
            json!(1),
            "runtime.compile_reload",
            Some(json!({"issues_limit": 0})),
        )),
        -32602,
    );
    assert!(
        response["error"]["message"].as_str().is_some(),
        "{response:#}"
    );
}

#[test]
fn runtime_build_issues_limit_bounds_a_failed_builds_error_text() {
    let workspace = TestWorkspace::new("runtime-build-cap-error-text");
    workspace.write("src/a.st", &invalid_agent_program("Alpha", "MissingA"));
    workspace.write("src/b.st", &invalid_agent_program("Beta", "MissingB"));
    workspace.write("src/c.st", &invalid_agent_program("Gamma", "MissingC"));
    let mut server = workspace.server();

    let uncapped = failure(
        server.handle_line(&request(json!(1), "runtime.build", Some(json!({})))),
        -32002,
    );
    let uncapped_lines = uncapped["error"]["message"]
        .as_str()
        .unwrap()
        .lines()
        .count();

    let capped = failure(
        server.handle_line(&request(
            json!(2),
            "runtime.build",
            Some(json!({"issues_limit": 1})),
        )),
        -32002,
    );
    let capped_message = capped["error"]["message"].as_str().unwrap();
    let capped_lines: Vec<&str> = capped_message.lines().collect();
    assert_eq!(
        capped_lines.len(),
        2,
        "first limit line plus one closing marker: {capped_message}"
    );
    assert!(
        capped_lines[0].contains("error[E"),
        "the first diagnostic line survives: {capped_message}"
    );
    assert!(
        capped_lines[1].contains("more diagnostic line(s) omitted by 'issues_limit'"),
        "the closing line names the applied cap: {capped_message}"
    );
    assert!(
        uncapped_lines > capped_lines.len(),
        "the cap actually dropped dump lines: {uncapped_lines} vs {}",
        capped_lines.len()
    );

    let generous = failure(
        server.handle_line(&request(
            json!(3),
            "runtime.build",
            Some(json!({"issues_limit": 9999})),
        )),
        -32002,
    );
    assert_eq!(
        generous["error"]["message"]
            .as_str()
            .unwrap()
            .lines()
            .count(),
        uncapped_lines,
        "a generous limit leaves the error text unchanged: {generous:#}"
    );
}
