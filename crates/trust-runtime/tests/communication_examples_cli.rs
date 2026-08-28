use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use smol_str::SmolStr;
use trust_runtime::bundle_template::render_runtime_toml;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

fn communication_project_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/communication")
        .join(name)
}

fn communication_examples_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/communication")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst)
        .unwrap_or_else(|err| panic!("create directory {}: {err}", dst.display()));
    let entries = std::fs::read_dir(src)
        .unwrap_or_else(|err| panic!("read directory {}: {err}", src.display()));
    for entry in entries {
        let entry = entry.expect("directory entry");
        let source_path = entry.path();
        let dest_path = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("query file type {}: {err}", source_path.display()));
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &dest_path);
        } else if file_type.is_file() {
            std::fs::copy(&source_path, &dest_path).unwrap_or_else(|err| {
                panic!(
                    "copy file {} -> {}: {err}",
                    source_path.display(),
                    dest_path.display()
                )
            });
        } else {
            panic!(
                "unsupported non-file/non-directory entry {} in communication example fixture",
                source_path.display()
            );
        }
    }
}

fn successful_json(output: Output, command: &str) -> Value {
    assert!(
        output.status.success(),
        "expected {command} success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected {command} to emit JSON: {error}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn assert_cli_failure(output: Output, command: &str, expected_stderr: &str) {
    assert!(
        !output.status.success(),
        "expected {command} failure.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_stderr),
        "expected {command} stderr to contain {expected_stderr:?}.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn normalize_runtime_endpoint_for_platform(_project: &Path) {}

#[cfg(not(unix))]
fn normalize_runtime_endpoint_for_platform(project: &Path) {
    let runtime_path = project.join("runtime.toml");
    let Ok(raw) = std::fs::read_to_string(&runtime_path) else {
        return;
    };

    let Ok(mut doc) = toml::from_str::<toml::Value>(&raw) else {
        return;
    };
    let Some(control) = doc
        .get_mut("runtime")
        .and_then(toml::Value::as_table_mut)
        .and_then(|runtime| runtime.get_mut("control"))
        .and_then(toml::Value::as_table_mut)
    else {
        return;
    };

    let mut changed = false;
    if let Some(endpoint) = control.get("endpoint").and_then(toml::Value::as_str) {
        if endpoint.starts_with("unix://") {
            control.insert(
                "endpoint".to_string(),
                toml::Value::String("tcp://127.0.0.1:18082".to_string()),
            );
            changed = true;
        }
    }

    let has_auth_token = control
        .get("auth_token")
        .and_then(toml::Value::as_str)
        .map(|token| !token.is_empty())
        .unwrap_or(false);
    if !has_auth_token {
        control.insert(
            "auth_token".to_string(),
            toml::Value::String("trust-ci-token".to_string()),
        );
        changed = true;
    }

    if changed {
        let normalized = toml::to_string(&doc).expect("serialize normalized runtime.toml");
        std::fs::write(&runtime_path, normalized).unwrap_or_else(|err| {
            panic!(
                "rewrite runtime endpoint for {}: {err}",
                runtime_path.display()
            )
        });
    }
}

#[test]
fn communication_examples_build_and_validate() {
    let root = communication_examples_root();
    let mut examples: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("read communication examples root {}: {err}", root.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            entry.file_name().to_str().map(|s| s.to_string())
        })
        .collect();
    examples.sort();

    assert!(
        !examples.is_empty(),
        "no communication example directories found under {}",
        root.display()
    );

    for name in examples {
        let fixture = communication_project_path(&name);
        assert!(
            fixture.is_dir(),
            "missing communication example fixture: {}",
            fixture.display()
        );
        let required_files = [
            "io.toml",
            "runtime.toml",
            "trust-lsp.toml",
            "src/main.st",
            "src/config.st",
        ];
        for file in required_files {
            let path = fixture.join(file);
            assert!(
                path.is_file(),
                "communication example {} is missing required file {}",
                fixture.display(),
                path.display()
            );
        }

        let temp_root = unique_temp_dir(&format!("communication-example-{name}"));
        let project = temp_root.join(&name);
        copy_dir_recursive(&fixture, &project);
        normalize_runtime_endpoint_for_platform(&project);

        let build =
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .args(["build", "--project"])
            .arg(&project)
            .args(["--sources", "src"])
            .output()
            .expect("run trust-runtime build");
        assert!(
            build.status.success(),
            "expected build success for {name} example.\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );

        let validate =
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .args(["validate", "--project"])
            .arg(&project)
            .output()
            .expect("run trust-runtime validate");
        assert!(
            validate.status.success(),
            "expected validate success for {name} example.\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&validate.stdout),
            String::from_utf8_lossy(&validate.stderr)
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}

#[test]
fn comm_cli_runs_schema_apply_topology_and_cached_browse_offline() {
    let temp_root = unique_temp_dir("comm-cli-offline");
    std::fs::create_dir_all(&temp_root).expect("create comm CLI project");
    std::fs::write(
        temp_root.join("runtime.toml"),
        render_runtime_toml(&SmolStr::new("comm-cli"), 10),
    )
    .expect("write runtime.toml");

    let schema =
        successful_json(
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .args(["comm", "schema", "--protocol", "modbus-tcp", "--json"])
            .output()
            .expect("run comm schema"),
            "comm schema",
        );
    assert_eq!(
        schema.get("schema_version").and_then(Value::as_u64),
        Some(4)
    );
    let protocols = schema
        .get("protocols")
        .and_then(Value::as_array)
        .expect("comm schema protocols");
    assert_eq!(protocols.len(), 1);
    assert_eq!(
        protocols[0].get("id").and_then(Value::as_str),
        Some("modbus_tcp")
    );

    let params = serde_json::json!({
        "address": "127.0.0.1:502",
        "unit_id": 1,
        "input_start": 0,
        "output_start": 0,
        "timeout_ms": 500,
        "on_error": "warn"
    })
    .to_string();
    let apply =
        successful_json(
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .args(["comm", "apply", "--project"])
            .arg(&temp_root)
            .args([
                "--protocol",
                "modbus-tcp",
                "--params",
                params.as_str(),
                "--action",
                "add",
                "--json",
            ])
            .output()
            .expect("run comm apply"),
            "comm apply",
        );
    assert_eq!(apply.get("applied").and_then(Value::as_bool), Some(true));
    assert_eq!(
        apply.get("lifecycle_effect").and_then(Value::as_str),
        Some("restart_required")
    );
    assert!(temp_root.join("io.toml").is_file());

    let topology =
        successful_json(
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .args(["comm", "topology", "--project"])
            .arg(&temp_root)
            .arg("--json")
            .output()
            .expect("run comm topology"),
            "comm topology",
        );
    assert_eq!(
        topology.get("schema_version").and_then(Value::as_u64),
        Some(4)
    );
    let endpoints = topology
        .pointer("/hosts/0/runtimes/0/endpoints")
        .and_then(Value::as_array)
        .expect("topology endpoints");
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.get("protocol").and_then(Value::as_str) == Some("modbus_tcp")
            && endpoint.get("health").and_then(Value::as_str) == Some("configured_policy")
    }));

    let snapshot = communication_project_path("ads_line1").join("ads/snapshots/line1.symbols.json");
    let browse =
        successful_json(
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .args(["comm", "browse-symbols", "--protocol", "ads"])
            .arg("--snapshot-file")
            .arg(&snapshot)
            .args(["--connection-name", "line1", "--json"])
            .output()
            .expect("run comm browse-symbols"),
            "comm browse-symbols",
        );
    assert_eq!(
        browse.get("schema_version").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(browse.get("protocol").and_then(Value::as_str), Some("ads"));
    assert!(browse
        .get("tree")
        .and_then(Value::as_array)
        .is_some_and(|tree| !tree.is_empty()));
    assert_eq!(
        browse
            .pointer("/ads_import/snapshot/route_name")
            .and_then(Value::as_str),
        Some("line1")
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn comm_cli_validates_payloads_and_manages_the_selected_opcua_trust_store() {
    let invalid = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args([
        "comm",
        "apply",
        "--project",
        ".",
        "--protocol",
        "modbus-tcp",
        "--params",
        "[]",
        "--json",
    ])
    .output()
    .expect("run comm apply with invalid params");
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("--params must be a JSON object"));

    let discovery =
        successful_json(
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .args([
                "comm",
                "discover",
                "--protocol",
                "opcua",
                "--origin",
                "runtime",
                "--passive",
                "false",
                "--json",
            ])
            .output()
            .expect("run comm discover"),
            "comm discover",
        );
    assert_eq!(
        discovery.get("origin").and_then(Value::as_str),
        Some("runtime")
    );
    let warnings = discovery
        .get("warnings")
        .and_then(Value::as_array)
        .expect("discovery warnings");
    assert!(warnings.iter().any(|warning| {
        warning
            .as_str()
            .is_some_and(|text| text.contains("Active write probes are not supported"))
    }));

    let pki_root = unique_temp_dir("comm-cli-opcua-pki");
    let trusted = pki_root.join("trusted/certs");
    std::fs::create_dir_all(&trusted).expect("create OPC UA trusted directory");
    std::fs::write(trusted.join("server.der"), b"certificate").expect("write trusted certificate");

    let list =
        successful_json(
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .env("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &pki_root)
            .args(["comm", "opcua-trust", "list", "--json"])
            .output()
            .expect("run comm opcua-trust list"),
            "comm opcua-trust list",
        );
    assert_eq!(
        list.get("protocol").and_then(Value::as_str),
        Some("opcua_client")
    );
    assert_eq!(
        list.pointer("/trusted/0/file_name").and_then(Value::as_str),
        Some("server.der")
    );

    let clear =
        successful_json(
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .env("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &pki_root)
            .args(["comm", "opcua-trust", "clear", "--json"])
            .output()
            .expect("run comm opcua-trust clear"),
            "comm opcua-trust clear",
        );
    assert_eq!(clear.get("cleared").and_then(Value::as_u64), Some(1));
    assert!(!trusted.join("server.der").exists());

    let _ = std::fs::remove_dir_all(&pki_root);
}

#[test]
fn comm_cli_rejects_malformed_local_authoring_inputs() {
    let malformed_params = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args([
        "comm",
        "apply",
        "--project",
        ".",
        "--protocol",
        "modbus-tcp",
        "--params",
        "{",
        "--json",
    ])
    .output()
    .expect("run comm apply with malformed params");
    assert_cli_failure(
        malformed_params,
        "comm apply with malformed params",
        "failed to parse --params as a JSON object",
    );

    let non_object_target = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args([
        "comm",
        "browse-symbols",
        "--protocol",
        "ads",
        "--target",
        "[]",
        "--json",
    ])
    .output()
    .expect("run comm browse-symbols with non-object target");
    assert_cli_failure(
        non_object_target,
        "comm browse-symbols with non-object target",
        "--params must be a JSON object",
    );

    let temp_root = unique_temp_dir("comm-cli-malformed-input");
    std::fs::create_dir_all(&temp_root).expect("create malformed comm input directory");
    let missing_snapshot = temp_root.join("missing.json");
    let unreadable = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["comm", "browse-symbols", "--protocol", "ads"])
    .arg("--snapshot-file")
    .arg(&missing_snapshot)
    .arg("--json")
    .output()
    .expect("run comm browse-symbols with missing snapshot");
    assert_cli_failure(
        unreadable,
        "comm browse-symbols with missing snapshot",
        "failed to read snapshot file",
    );

    let malformed_snapshot = temp_root.join("malformed.json");
    std::fs::write(&malformed_snapshot, b"{").expect("write malformed snapshot");
    let malformed = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["comm", "browse-symbols", "--protocol", "ads"])
    .arg("--snapshot-file")
    .arg(&malformed_snapshot)
    .arg("--json")
    .output()
    .expect("run comm browse-symbols with malformed snapshot");
    assert_cli_failure(
        malformed,
        "comm browse-symbols with malformed snapshot",
        "failed to parse snapshot file",
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}
