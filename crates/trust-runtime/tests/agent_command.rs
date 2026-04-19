use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value as JsonValue};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn canonicalize_for_assert(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn normalize_path_text(text: &str) -> String {
    let without_verbatim = if let Some(stripped) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{stripped}")
    } else if let Some(stripped) = text.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        text.to_string()
    };
    without_verbatim.replace('\\', "/")
}

fn assert_json_relative_path_eq(actual: &JsonValue, expected: &str) {
    assert_eq!(
        normalize_path_text(actual.as_str().expect("relative path string")),
        normalize_path_text(expected)
    );
}

fn assert_json_absolute_path_eq(actual: &JsonValue, expected: &Path) {
    let expected_display = canonicalize_for_assert(expected).display().to_string();
    assert_eq!(
        normalize_path_text(actual.as_str().expect("absolute path string")),
        normalize_path_text(&expected_display)
    );
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    for _ in 0..64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let seq = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        match fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("create temp dir {}: {err}", dir.display()),
        }
    }
    panic!("failed to allocate unique temp dir for '{prefix}'")
}

fn fixture_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ci")
        .join(name)
}

fn copy_file_with_retry(src: &Path, dst: &Path) {
    for attempt in 0..5 {
        match fs::copy(src, dst) {
            Ok(_) => return,
            Err(err) if cfg!(windows) && err.raw_os_error() == Some(32) && attempt < 4 => {
                std::thread::sleep(Duration::from_millis(20 * (attempt + 1)));
            }
            Err(err) => panic!(
                "copy fixture file {} -> {}: {err}",
                src.display(),
                dst.display()
            ),
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create destination directory");
    for entry in fs::read_dir(src).expect("read fixture directory") {
        let entry = entry.expect("read fixture entry");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            copy_file_with_retry(&src_path, &dst_path);
        }
    }
}

fn copy_fixture(name: &str) -> PathBuf {
    let target = unique_temp_dir(&format!("agent-{name}"));
    copy_dir_recursive(&fixture_root(name), &target);
    target
}

fn read_response(reader: &mut BufReader<std::process::ChildStdout>) -> JsonValue {
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).expect("read agent response");
    assert!(bytes > 0, "expected agent response line");
    serde_json::from_str::<JsonValue>(line.trim()).expect("decode JSON-RPC response")
}

fn write_request(stdin: &mut std::process::ChildStdin, request: JsonValue) {
    writeln!(stdin, "{request}").expect("write agent request");
}

fn allocate_loopback_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind temporary loopback port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn patch_control_endpoint(project: &Path, port: u16) {
    let runtime_toml = project.join("runtime.toml");
    let text = fs::read_to_string(&runtime_toml).expect("read runtime.toml");
    let updated = text.replace(
        "tcp://127.0.0.1:0",
        format!("tcp://127.0.0.1:{port}").as_str(),
    );
    fs::write(runtime_toml, updated).expect("write runtime.toml");
}

fn try_control_request(
    endpoint: &str,
    token: &str,
    request_type: &str,
    params: Option<JsonValue>,
) -> std::io::Result<JsonValue> {
    let mut stream = TcpStream::connect(endpoint)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    writeln!(
        stream,
        "{}",
        json!({
            "id": 1,
            "type": request_type,
            "auth": token,
            "params": params,
        })
    )?;
    stream.flush()?;
    let mut line = String::new();
    reader.read_line(&mut line)?;
    serde_json::from_str::<JsonValue>(line.trim()).map_err(std::io::Error::other)
}

fn control_request(
    endpoint: &str,
    token: &str,
    request_type: &str,
    params: Option<JsonValue>,
) -> JsonValue {
    try_control_request(endpoint, token, request_type, params).expect("control request")
}

fn control_ready(endpoint: &str, token: &str, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Ok(response) = try_control_request(endpoint, token, "status", None) {
            if response["ok"] == json!(true) {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    false
}

fn spawn_runtime_with_retry(project: &Path, token: &str) -> (std::process::Child, String) {
    for _ in 0..5 {
        let port = allocate_loopback_port();
        patch_control_endpoint(project, port);
        let endpoint = format!("127.0.0.1:{port}");
        let mut runtime = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
            .args(["run", "--project"])
            .arg(project)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn trust-runtime run");
        if control_ready(&endpoint, token, Duration::from_secs(5)) {
            return (runtime, endpoint);
        }
        let _ = runtime.kill();
        let _ = runtime.wait();
    }
    panic!(
        "timed out waiting for runtime control endpoint after retrying fresh ports for {}",
        project.display()
    );
}

fn read_output_bool(endpoint: &str, token: &str, address: &str) -> Option<bool> {
    let response = try_control_request(endpoint, token, "io.read", None).ok()?;
    response["result"]["snapshot"]["outputs"]
        .as_array()
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                (entry.get("address") == Some(&json!(address))).then(|| {
                    entry
                        .get("value")
                        .and_then(JsonValue::as_str)
                        .map(|value| value.contains("true"))
                })?
            })
        })
}

fn wait_for_output_bool(
    endpoint: &str,
    token: &str,
    address: &str,
    expected: bool,
    timeout: Duration,
) {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if read_output_bool(endpoint, token, address) == Some(expected) {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {address}={expected} at {endpoint}");
}

#[test]
fn agent_serve_supports_describe_write_and_read_roundtrip() {
    let project = unique_temp_dir("agent-workspace");
    let source_text = "PROGRAM Main\nEND_PROGRAM\n";
    fs::create_dir_all(&project).expect("create workspace root");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "agent.describe",
        })
    )
    .expect("write describe request");
    let describe = read_response(&mut reader);
    assert_eq!(describe["result"]["framing"], json!("jsonl"));
    assert_json_absolute_path_eq(&describe["result"]["workspace_root"], &project);

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "workspace.write",
            "params": {
                "path": "src/main.st",
                "text": source_text,
            },
        })
    )
    .expect("write workspace.write request");
    let write_response = read_response(&mut reader);
    assert_json_relative_path_eq(&write_response["result"]["path"], "src/main.st");
    assert_eq!(
        write_response["result"]["bytes_written"],
        json!(source_text.len())
    );

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "workspace.read",
            "params": {
                "path": "src/main.st",
            },
        })
    )
    .expect("write workspace.read request");
    let read_response = read_response(&mut reader);
    assert_json_relative_path_eq(&read_response["result"]["path"], "src/main.st");
    assert_eq!(read_response["result"]["text"], json!(source_text));

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_reports_method_and_path_errors_with_stable_codes() {
    let project = unique_temp_dir("agent-errors");
    fs::create_dir_all(&project).expect("create workspace root");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "workspace.missing",
        })
    )
    .expect("write missing-method request");
    let missing_method = read_response(&mut reader);
    assert_eq!(missing_method["error"]["code"], json!(-32601));

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "workspace.write",
            "params": {
                "path": "../escape.st",
                "text": "PROGRAM Main\nEND_PROGRAM\n",
            },
        })
    )
    .expect("write path-escape request");
    let path_escape = read_response(&mut reader);
    assert_eq!(path_escape["error"]["code"], json!(-32001));
    assert_eq!(
        path_escape["error"]["data"]["kind"],
        json!("path_outside_workspace")
    );

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "workspace.read",
        })
    )
    .expect("write missing-params request");
    let invalid_params = read_response(&mut reader);
    assert_eq!(invalid_params["error"]["code"], json!(-32602));

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_supports_runtime_project_commands_and_harness_loop() {
    let project = copy_fixture("green");
    let harness_program = r#"
PROGRAM Main
VAR
    in1 : BOOL;
    ton_fb : TON;
    q : BOOL;
    et : TIME;
END_VAR
ton_fb(IN := in1, PT := T#30MS, Q => q, ET => et);
END_PROGRAM
"#;

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "runtime.build",
        }),
    );
    let build = read_response(&mut reader);
    assert_eq!(build["result"]["status"], json!("ok"));
    assert_eq!(build["result"]["command"], json!("build"));
    assert_eq!(build["result"]["source_count"], json!(2));
    assert!(
        project.join("program.stbc").is_file(),
        "build should write program.stbc"
    );

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "runtime.validate",
        }),
    );
    let validate = read_response(&mut reader);
    assert_eq!(validate["result"]["status"], json!("ok"));
    assert_eq!(validate["result"]["command"], json!("validate"));
    assert_json_absolute_path_eq(&validate["result"]["project"], &project);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 23,
            "method": "runtime.test",
        }),
    );
    let tests = read_response(&mut reader);
    assert_eq!(tests["result"]["summary"]["passed"], json!(2));
    assert_eq!(tests["result"]["summary"]["failed"], json!(0));
    assert_eq!(tests["result"]["summary"]["errors"], json!(0));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 24,
            "method": "harness.load",
            "params": {
                "inline_sources": [
                    { "text": harness_program }
                ]
            },
        }),
    );
    let load = read_response(&mut reader);
    assert_eq!(load["result"]["source_count"], json!(1));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 25,
            "method": "harness.set_input",
            "params": {
                "name": "in1",
                "value": { "type": "BOOL", "value": true }
            },
        }),
    );
    let set_input = read_response(&mut reader);
    assert_eq!(set_input["result"]["status"], json!("ok"));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 26,
            "method": "harness.run_until",
            "params": {
                "name": "q",
                "equals": { "type": "BOOL", "value": true },
                "dt_ms": 10,
                "max_cycles": 5,
                "watch": ["q", "et"]
            },
        }),
    );
    let run_until = read_response(&mut reader);
    assert_eq!(run_until["result"]["name"], json!("q"));
    assert_eq!(run_until["result"]["cycles_ran"], json!(3));
    assert_eq!(
        run_until["result"]["matched_value"],
        json!({"type": "BOOL", "value": true})
    );
    assert_eq!(
        run_until["result"]["values"]["et"],
        json!({"type": "TIME", "nanos": 30_000_000})
    );

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 27,
            "method": "harness.get_output",
            "params": {
                "name": "q"
            },
        }),
    );
    let get_output = read_response(&mut reader);
    assert_eq!(
        get_output["result"]["value"],
        json!({"type": "BOOL", "value": true})
    );

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_supports_lsp_diagnostics_and_format_preview() {
    let project = unique_temp_dir("agent-lsp");
    fs::create_dir_all(project.join("src")).expect("create src directory");
    fs::write(
        project.join("src").join("main.st"),
        "PROGRAM Main\nEND_PROGRAM\n",
    )
    .expect("write seed source");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 28,
            "method": "lsp.diagnostics",
            "params": {
                "path": "src/main.st",
                "content": "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCounter := UnknownSymbol + 1;\nEND_PROGRAM\n",
            },
        }),
    );
    let diagnostics = read_response(&mut reader);
    assert_json_absolute_path_eq(
        &diagnostics["result"]["target"],
        &project.join("src/main.st"),
    );
    assert!(
        diagnostics["result"]["errors"]
            .as_u64()
            .expect("errors count")
            >= 1
    );
    assert_eq!(
        normalize_path_text(
            diagnostics["result"]["issues"][0]["path"]
                .as_str()
                .expect("diagnostic path")
        ),
        "src/main.st"
    );
    assert!(
        diagnostics["result"]["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .any(|item| item["severity"] == json!("error")),
        "expected at least one error diagnostic"
    );
    assert!(
        diagnostics["result"]["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .any(|item| {
                item["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("UnknownSymbol"))
            }),
        "expected unresolved symbol diagnostic"
    );

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 29,
            "method": "lsp.format",
            "params": {
                "path": "src/main.st",
                "content": "PROGRAM Main\nVAR\nCounter:INT;\nEND_VAR\nIF Counter > 0 THEN\nCounter:=Counter+1;\nELSE\nCounter:=0;\nEND_IF\nEND_PROGRAM\n",
            },
        }),
    );
    let format = read_response(&mut reader);
    assert_json_relative_path_eq(&format["result"]["path"], "src/main.st");
    assert_eq!(format["result"]["changed"], json!(true));
    assert_eq!(
        format["result"]["content"],
        json!(
            "PROGRAM Main\n  VAR\n    Counter:INT;\n  END_VAR\n  IF Counter > 0 THEN\n    Counter:=Counter+1;\n  ELSE\n    Counter:=0;\n  END_IF\nEND_PROGRAM\n"
        )
    );

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_reports_workspace_project_info() {
    let project = unique_temp_dir("agent-project-info");
    let dependency = project.join("deps").join("lib-a");
    fs::create_dir_all(project.join("src")).expect("create src directory");
    fs::create_dir_all(dependency.join("src")).expect("create dependency src directory");
    fs::write(
        project.join("src").join("main.st"),
        "PROGRAM Main\nEND_PROGRAM\n",
    )
    .expect("write project source");
    fs::write(
        dependency.join("src").join("lib.st"),
        "FUNCTION DepDouble : INT\nVAR_INPUT x : INT; END_VAR\nDepDouble := x * 2;\nEND_FUNCTION\n",
    )
    .expect("write dependency source");
    fs::write(
        project.join("runtime.toml"),
        r#"[bundle]
version = 1

[resource]
name = "Res"
cycle_interval_ms = 100

[runtime.control]
endpoint = "tcp://127.0.0.1:0"
auth_token = "secret"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#,
    )
    .expect("write runtime.toml");
    fs::write(
        project.join("io.toml"),
        "[io]\ndriver = \"simulated\"\nparams = {}\n",
    )
    .expect("write io.toml");
    fs::write(
        project.join("trust-lsp.toml"),
        r#"[project]
vendor_profile = "codesys"

[dependencies]
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
    )
    .expect("write trust-lsp.toml");
    fs::write(
        dependency.join("trust-lsp.toml"),
        "[package]\nversion = \"1.0.0\"\n",
    )
    .expect("write dependency manifest");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "workspace.project_info",
        }),
    );
    let project_info = read_response(&mut reader);
    assert_json_absolute_path_eq(&project_info["result"]["project"], &project);
    assert_eq!(project_info["result"]["sourceCount"], json!(2));
    assert_eq!(
        project_info["result"]["resolvedDependencies"],
        json!(["LibA"])
    );
    assert_eq!(
        project_info["result"]["lsp"]["vendorProfile"],
        json!("codesys")
    );
    assert_eq!(
        project_info["result"]["runtime"]["controlEndpoint"],
        json!("tcp://127.0.0.1:0")
    );
    assert_eq!(
        project_info["result"]["runtime"]["hasControlToken"],
        json!(true)
    );
    assert_eq!(
        project_info["result"]["io"]["drivers"],
        json!(["simulated"])
    );
    assert_eq!(
        project_info["result"]["files"]["trustLspToml"]["exists"],
        json!(true)
    );
    assert!(
        project_info["result"]["sources"]
            .as_array()
            .expect("sources array")
            .iter()
            .any(|value| value == "src/main.st"),
        "expected project source in sources list"
    );

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_reports_run_until_timeout_with_stable_code() {
    let project = unique_temp_dir("agent-run-until-timeout");
    let harness_program = r#"
PROGRAM Main
VAR
    flag : BOOL;
END_VAR
END_PROGRAM
"#;

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "harness.load",
            "params": {
                "inline_sources": [
                    { "text": harness_program }
                ]
            },
        }),
    );
    let load = read_response(&mut reader);
    assert_eq!(load["result"]["source_count"], json!(1));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "harness.run_until",
            "params": {
                "name": "flag",
                "equals": { "type": "BOOL", "value": true },
                "max_cycles": 2
            },
        }),
    );
    let timeout = read_response(&mut reader);
    assert_eq!(timeout["error"]["code"], json!(-32004));
    assert_eq!(timeout["error"]["data"]["name"], json!("flag"));
    assert_eq!(timeout["error"]["data"]["max_cycles"], json!(2));

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_runtime_reload_rebuilds_and_reloads_a_live_runtime() {
    let project = copy_fixture("green");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock control listener");
    let port = listener.local_addr().expect("listener addr").port();
    let (request_tx, request_rx) = mpsc::sync_channel(1);
    let source_false = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#;
    let source_true = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := TRUE;
END_PROGRAM
"#;

    patch_control_endpoint(&project, port);
    fs::write(project.join("src").join("main.st"), source_false).expect("write initial source");

    let build_output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["build", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-runtime build");
    assert!(
        build_output.status.success(),
        "initial build failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept control connection");
        let mut reader = BufReader::new(stream.try_clone().expect("clone control stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read control request");
        let request = serde_json::from_str::<JsonValue>(line.trim()).expect("decode request");
        request_tx.send(request).expect("send request to test");
        writeln!(
            stream,
            "{}",
            json!({
                "id": 1,
                "ok": true,
                "result": { "status": "reloaded" }
            })
        )
        .expect("write control response");
        stream.flush().expect("flush control response");
    });

    fs::write(project.join("src").join("main.st"), source_true).expect("write updated source");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);
    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 41,
            "method": "runtime.reload",
        }),
    );
    let reload = read_response(&mut reader);
    assert_eq!(reload["result"]["build"]["status"], json!("ok"));
    assert_eq!(
        reload["result"]["reload"]["result"]["status"],
        json!("reloaded")
    );
    let request = request_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("receive control request");
    assert_eq!(request["type"], json!("bytecode.reload"));
    assert!(
        request["params"]["bytes"]
            .as_str()
            .is_some_and(|bytes| !bytes.is_empty()),
        "expected bytecode payload in control request"
    );
    drop(stdin);
    let agent_output = child.wait_with_output().expect("wait for agent process");
    assert!(
        agent_output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&agent_output.stdout),
        String::from_utf8_lossy(&agent_output.stderr)
    );
    server.join().expect("join mock control server");
    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_runtime_compile_reload_blocks_on_diagnostics() {
    let project = copy_fixture("green");
    fs::write(
        project.join("src").join("main.st"),
        "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCounter := UnknownSymbol + 1;\nEND_PROGRAM\n",
    )
    .expect("write broken source");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 51,
            "method": "runtime.compile_reload",
        }),
    );
    let compile_reload = read_response(&mut reader);
    assert_eq!(compile_reload["result"]["runtimeStatus"], json!("skipped"));
    assert_eq!(
        compile_reload["result"]["runtimeMessage"],
        json!("Build and reload blocked by diagnostics.")
    );
    assert!(
        compile_reload["result"]["errors"]
            .as_u64()
            .expect("errors count")
            >= 1
    );
    assert_eq!(compile_reload["result"]["build"], JsonValue::Null);
    assert_eq!(compile_reload["result"]["reload"], JsonValue::Null);
    assert!(
        compile_reload["result"]["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .any(|item| {
                item["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("UnknownSymbol"))
            }),
        "expected unresolved symbol diagnostic"
    );

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_runtime_compile_reload_reports_reload_failure() {
    let project = copy_fixture("green");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 52,
            "method": "runtime.compile_reload",
            "params": {
                "endpoint": "tcp://127.0.0.1:1"
            },
        }),
    );
    let compile_reload = read_response(&mut reader);
    assert_eq!(compile_reload["result"]["runtimeStatus"], json!("error"));
    assert!(
        compile_reload["result"]["runtimeMessage"]
            .as_str()
            .is_some_and(|message| message.contains("Reload failed")),
        "expected reload failure message"
    );
    assert_eq!(compile_reload["result"]["errors"], json!(0));
    assert_eq!(compile_reload["result"]["build"]["status"], json!("ok"));
    assert_eq!(compile_reload["result"]["reload"], JsonValue::Null);

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_runtime_compile_reload_rebuilds_and_reloads_a_live_runtime() {
    let project = copy_fixture("green");
    let token = "trust-ci-token";
    let source_false = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#;
    let source_true = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := TRUE;
END_PROGRAM
"#;

    fs::write(project.join("src").join("main.st"), source_false).expect("write initial source");

    let build_output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["build", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-runtime build");
    assert!(
        build_output.status.success(),
        "initial build failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let (mut runtime, control_endpoint) = spawn_runtime_with_retry(&project, token);
    wait_for_output_bool(
        &control_endpoint,
        token,
        "%QX0.0",
        false,
        Duration::from_secs(20),
    );

    fs::write(project.join("src").join("main.st"), source_true).expect("write updated source");

    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);
    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 53,
            "method": "runtime.compile_reload",
        }),
    );
    let compile_reload = read_response(&mut reader);
    assert_eq!(compile_reload["result"]["runtimeStatus"], json!("ok"));
    assert_eq!(
        compile_reload["result"]["runtimeMessage"],
        json!("Runtime reload succeeded.")
    );
    assert_eq!(compile_reload["result"]["build"]["status"], json!("ok"));
    assert_eq!(
        compile_reload["result"]["reload"]["result"]["status"],
        json!("reloaded")
    );
    drop(stdin);
    let agent_output = child.wait_with_output().expect("wait for agent process");
    assert!(
        agent_output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&agent_output.stdout),
        String::from_utf8_lossy(&agent_output.stderr)
    );

    wait_for_output_bool(
        &control_endpoint,
        token,
        "%QX0.0",
        true,
        Duration::from_secs(20),
    );

    let _ = control_request(&control_endpoint, token, "shutdown", None);
    let _ = runtime.kill();
    let _ = runtime.wait();
    let _ = fs::remove_dir_all(project);
}
