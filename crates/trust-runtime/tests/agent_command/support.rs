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

fn trust_dev_command() -> Command {
    Command::new(trust_dev_bin())
}

fn trust_runtime_command_with_dev_alias() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command.env("TRUST_DEV_BIN", trust_dev_bin());
    command
}

fn trust_dev_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_trust-dev") {
        return path.into();
    }
    if let Ok(path) = std::env::var("TRUST_DEV_BIN") {
        return path.into();
    }
    let exe = std::env::current_exe().expect("current test exe path");
    let debug_dir = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("target debug dir");
    debug_dir.join(format!("trust-dev{}", std::env::consts::EXE_SUFFIX))
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
