//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

#[path = "support/runtime_process.rs"]
mod runtime_process;

#[allow(dead_code)]
#[path = "support/modbus.rs"]
mod modbus;

#[allow(dead_code)]
#[path = "support/mqtt.rs"]
mod mqtt;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

use modbus::{start_modbus_server, ModbusTestState};
use mqtt::MqttTestBroker;
use runtime_process::{ExampleRuntime, WAIT};

use serde_json::Value;
use smol_str::SmolStr;
use trust_runtime::bundle_template::render_runtime_toml;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    trust_test_support::unique_temp_path(&format!("trust-runtime-{prefix}"))
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

    // Most examples omit the endpoint, so Windows validates them with its own default: loopback TCP
    // with a generated token (spec 11, section 6.8.0a). Only an example that keeps an explicit Unix
    // socket, because its instructions name it, gets a loopback TCP endpoint here, since Windows has
    // no Unix sockets.
    let unix_endpoint = control
        .get("endpoint")
        .and_then(toml::Value::as_str)
        .is_some_and(|endpoint| endpoint.starts_with("unix://"));
    if !unix_endpoint {
        return;
    }
    control.insert(
        "endpoint".to_string(),
        toml::Value::String("tcp://127.0.0.1:18082".to_string()),
    );
    let has_auth_token = control
        .get("auth_token")
        .and_then(toml::Value::as_str)
        .is_some_and(|token| !token.is_empty());
    if !has_auth_token {
        control.insert(
            "auth_token".to_string(),
            toml::Value::String("trust-ci-token".to_string()),
        );
    }
    let normalized = toml::to_string(&doc).expect("serialize normalized runtime.toml");
    std::fs::write(&runtime_path, normalized).unwrap_or_else(|err| {
        panic!(
            "rewrite runtime endpoint for {}: {err}",
            runtime_path.display()
        )
    });
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

#[test]
fn the_modbus_tcp_example_carries_its_bit_through_the_server_and_writes_its_safe_state() {
    // The README of examples/communication/modbus_tcp (specification 11, the Modbus/TCP driver,
    // the image high byte first, and the safe state at a deliberate stop): the server's input
    // register 0 holds 256, bit 0 of its high byte, so %IX0.0 (DI0) reads TRUE; the program copies
    // it to DO0 at %QX0.0, which the runtime writes to holding register 0 as 256. At
    // `shutdown --exit` it writes io.toml's safe state, %QX0.0 FALSE, so holding register 0 goes
    // back to 0. Catches a driver that reads the low byte first or another register, an output that
    // does not reach the server, and a stop that leaves the output set.
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![256],
        vec![0],
    )));
    let server = start_modbus_server(state.clone(), 1_000_000);
    let address: &'static str = Box::leak(format!("address = \"{server}\"").into_boxed_str());
    let mut runtime = ExampleRuntime::new("communication/modbus_tcp")
        .edit_source("io.toml", "address = \"127.0.0.1:1502\"", address)
        .start();
    runtime.wait_for_snapshot(WAIT, "%IX0.0 and %QX0.0 to read TRUE", |s| {
        s.text("%IX0.0") == "Bool(true)" && s.text("%QX0.0") == "Bool(true)"
    });
    let holding = || {
        state
            .lock()
            .expect("the server's registers")
            .holding_registers[0]
    };
    runtime.wait_until(WAIT, "holding register 0 written as 256", || {
        holding() == 256
    });
    runtime.ctl_ok(&["shutdown", "--exit"]);
    let status = runtime.wait_for_exit(WAIT);
    assert!(
        status.success(),
        "the runtime ends with success after shutdown --exit: {status}"
    );
    assert_eq!(
        holding(),
        0,
        "the stop writes the safe state of %QX0.0 to holding register 0"
    );
}

#[test]
fn the_ethercat_example_maps_its_mock_input_to_its_output() {
    // The README of examples/communication/ethercat (specification 11, the EtherCAT driver's mock
    // adapter, and the safe state at a deliberate stop): the mock reports the input bytes
    // ["01", "00"], whose first sets %IX0.0 (DI0); the program copies it to DO0 at %QX0.0. The
    // resource runs without a fault, and `shutdown --exit` ends the process with success after the
    // safe state is written. Catches a mock that does not report its bytes, a chain mapped in
    // another layout, and a stop that fails on the chain.
    let mut runtime = ExampleRuntime::new("communication/ethercat").start();
    runtime.wait_for_snapshot(WAIT, "%IX0.0 and %QX0.0 to read TRUE", |s| {
        s.text("%IX0.0") == "Bool(true)" && s.text("%QX0.0") == "Bool(true)"
    });
    runtime.assert_running_without_fault();
    runtime.ctl_ok(&["shutdown", "--exit"]);
    let status = runtime.wait_for_exit(WAIT);
    assert!(
        status.success(),
        "the runtime ends with success after shutdown --exit: {status}"
    );
}

#[test]
fn the_multi_driver_example_runs_its_drive_from_modbus_and_its_start_from_mqtt() {
    // The README of examples/communication/multi_driver (specification 11, composed drivers, each
    // read in its order into the one input image; specification 32 §4 and §5, symbolic mappings,
    // which fill only their variables): the drive's discrete input 0 is TRUE, so DriveReady
    // (%IX0.0, the Modbus point) reads TRUE, while RemoteStart (%IX1.0, the MQTT mapping) stays
    // FALSE and so does Run (%QX0.0) = DriveReady AND RemoteStart, which goes to coil 0 and, as
    // `false`, to the running topic. A message `true` on the start topic sets RemoteStart, Run
    // follows, coil 0 is written TRUE and the running topic carries `true`; RemoteStart keeps the
    // message's value while no new message comes. At `shutdown --exit` the safe state reaches
    // both drivers: coil 0 FALSE and `false` on the running topic. Catches a composition in which
    // one driver overwrites the other's input (the example's raw MQTT topic overwrote the whole
    // image with the message `true` until 27 November 2026, so DriveReady read FALSE), a mapping
    // that does not reach its variable, and a stop that leaves the drive running.
    let modbus = Arc::new(Mutex::new(ModbusTestState {
        discrete_inputs: vec![true],
        coils: vec![false],
        ..ModbusTestState::default()
    }));
    let server = start_modbus_server(modbus.clone(), 1_000_000);
    let broker = MqttTestBroker::start();
    let address: &'static str = Box::leak(format!("address = \"{server}\"").into_boxed_str());
    let endpoint: &'static str =
        Box::leak(format!("broker = \"{}\"", broker.endpoint()).into_boxed_str());
    let mut runtime = ExampleRuntime::new("communication/multi_driver")
        .edit_source("io.toml", "address = \"127.0.0.1:1502\"", address)
        .edit_source("io.toml", "broker = \"127.0.0.1:1883\"", endpoint)
        .start();
    runtime.wait_for_snapshot(WAIT, "DriveReady TRUE, RemoteStart and Run FALSE", |s| {
        s.text("%IX0.0") == "Bool(true)"
            && s.text("%IX1.0") == "Bool(false)"
            && s.text("%QX0.0") == "Bool(false)"
    });
    runtime.wait_until(WAIT, "the subscription to the start topic", || {
        broker
            .subscriptions()
            .iter()
            .any(|topic| topic == "trust/examples/multi/start")
    });

    broker.deliver("trust/examples/multi/start", b"true");
    let started = runtime.wait_for_snapshot(WAIT, "DriveReady, RemoteStart and Run TRUE", |s| {
        s.text("%IX0.0") == "Bool(true)"
            && s.text("%IX1.0") == "Bool(true)"
            && s.text("%QX0.0") == "Bool(true)"
    });
    let coil = || modbus.lock().expect("the server's coils").coils[0];
    runtime.wait_until(WAIT, "coil 0 written TRUE", coil);
    let running = || broker.published("trust/examples/multi/running");
    runtime.wait_until(WAIT, "the running topic carrying true", || {
        running().last().is_some_and(|payload| payload == b"true")
    });
    let published = running();
    let first_true = published
        .iter()
        .position(|payload| payload == b"true")
        .expect("true on the running topic");
    assert!(
        published[..first_true]
            .iter()
            .any(|payload| payload == b"false"),
        "the running topic carries false before the remote start: {published:?}"
    );
    let later = started.scan() + 5;
    let kept =
        runtime.wait_for_snapshot(WAIT, "five scans after the message", |s| s.scan() >= later);
    assert_eq!(
        kept.text("%IX1.0"),
        "Bool(true)",
        "RemoteStart keeps the message's value while no new message comes"
    );

    runtime.ctl_ok(&["shutdown", "--exit"]);
    let status = runtime.wait_for_exit(WAIT);
    assert!(
        status.success(),
        "the runtime ends with success after shutdown --exit: {status}"
    );
    assert!(
        !coil(),
        "the stop writes the safe state of %QX0.0 to coil 0"
    );
    runtime.wait_until(WAIT, "the safe state, false, on the running topic", || {
        running().last().is_some_and(|payload| payload == b"false")
    });
}

#[test]
fn the_mqtt_example_carries_its_byte_through_the_broker_and_publishes_its_safe_state() {
    // The README of examples/communication/mqtt (specification 32 §4, the raw byte bridge, and §8,
    // a read without a new message keeps the input; specification 11, the safe state at a
    // deliberate stop): the runtime publishes its output image, one byte, to `topic_out` at every
    // cycle, 00 while DO0 is FALSE. The message 01 on `topic_in` fills the input image from its
    // first byte, so %IX0.0 (DI0) reads TRUE, the program copies it to DO0 at %QX0.0, and
    // `topic_out` carries 01; DI0 keeps the message's value while no new message comes. At
    // `shutdown --exit` the runtime publishes the safe state, 00. Catches a bridge that leaves the
    // input image unfilled or fills it from another bit, an output image that does not reach the
    // broker, and a stop that leaves the output set.
    let broker = MqttTestBroker::start();
    let endpoint: &'static str =
        Box::leak(format!("broker = \"{}\"", broker.endpoint()).into_boxed_str());
    let mut runtime = ExampleRuntime::new("communication/mqtt")
        .edit_source("io.toml", "broker = \"127.0.0.1:1883\"", endpoint)
        .start();
    runtime.wait_for_snapshot(WAIT, "%IX0.0 and %QX0.0 FALSE", |s| {
        s.text("%IX0.0") == "Bool(false)" && s.text("%QX0.0") == "Bool(false)"
    });
    let out = || broker.published("trust/examples/mqtt/out");
    runtime.wait_until(WAIT, "the byte 00 on topic_out", || {
        out()
            .last()
            .is_some_and(|payload| payload.as_slice() == [0x00])
    });
    runtime.wait_until(WAIT, "the subscription to topic_in", || {
        broker
            .subscriptions()
            .iter()
            .any(|topic| topic == "trust/examples/mqtt/in")
    });

    broker.deliver("trust/examples/mqtt/in", &[0x01]);
    let set = runtime.wait_for_snapshot(WAIT, "%IX0.0 and %QX0.0 TRUE", |s| {
        s.text("%IX0.0") == "Bool(true)" && s.text("%QX0.0") == "Bool(true)"
    });
    runtime.wait_until(WAIT, "the byte 01 on topic_out", || {
        out()
            .last()
            .is_some_and(|payload| payload.as_slice() == [0x01])
    });
    let later = set.scan() + 5;
    let kept =
        runtime.wait_for_snapshot(WAIT, "five scans after the message", |s| s.scan() >= later);
    assert_eq!(
        kept.text("%IX0.0"),
        "Bool(true)",
        "DI0 keeps the message's value while no new message comes"
    );

    runtime.ctl_ok(&["shutdown", "--exit"]);
    let status = runtime.wait_for_exit(WAIT);
    assert!(
        status.success(),
        "the runtime ends with success after shutdown --exit: {status}"
    );
    runtime.wait_until(WAIT, "the safe state, 00, on topic_out", || {
        out()
            .last()
            .is_some_and(|payload| payload.as_slice() == [0x00])
    });
}
