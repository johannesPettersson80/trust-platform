#![cfg(feature = "opcua-wire")]

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::config::{validate_runtime_toml_text, RuntimeConfig};
use trust_runtime::harness::TestHarness;
use trust_runtime::opcua::{
    start_wire_server, OpcUaClientIdentity, OpcUaClientOptions, OpcUaDataType, OpcUaVariant,
};

/// A free loopback port this process has not handed out before. The fixture servers bind their
/// port only after the reservation is released, so two tests running in parallel must never get
/// the same released port.
fn reserve_loopback_port() -> u16 {
    static RESERVED: Mutex<Vec<u16>> = Mutex::new(Vec::new());
    let mut reserved = RESERVED.lock().expect("reserved ports");
    loop {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
        let port = listener.local_addr().expect("local addr").port();
        if !reserved.contains(&port) {
            reserved.push(port);
            return port;
        }
    }
}

/// A fresh directory for each call. Clock resolution alone is not enough: macOS reports
/// microseconds, so two tests writing the same fixture in parallel could share a directory and one
/// could read the other's half-written `runtime.toml`.
fn temp_runtime_root(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-opcua-{name}-{}-{sequence}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temp runtime root");
    root
}

fn load_runtime_fixture(name: &str, port: u16) -> RuntimeConfig {
    let template = match name {
        "interop" => include_str!("fixtures/opcua/interop/runtime.toml"),
        "perf" => include_str!("fixtures/opcua/perf/runtime.toml"),
        "security" => include_str!("fixtures/opcua/security/runtime.toml"),
        other => panic!("unknown OPC UA integration fixture: {other}"),
    };
    let text = template.replace("__PORT__", &port.to_string());
    validate_runtime_toml_text(&text).expect("runtime fixture validates");
    let root = temp_runtime_root(name);
    let runtime_path = root.join("runtime.toml");
    std::fs::write(&runtime_path, text).expect("write runtime fixture");
    RuntimeConfig::load(runtime_path).expect("load runtime fixture")
}

fn snapshot_provider() -> Arc<dyn Fn() -> Option<trust_runtime::debug::DebugSnapshot> + Send + Sync>
{
    let mut harness = TestHarness::from_source(include_str!("fixtures/opcua/program/main.st"))
        .expect("build opcua harness");
    let _ = harness.runtime_mut().enable_debug();
    harness.cycle();
    let snapshot = trust_runtime::debug::DebugSnapshot {
        storage_abi: Default::default(),
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    };
    Arc::new(move || Some(snapshot.clone()))
}

fn fixture_snapshot() -> trust_runtime::debug::DebugSnapshot {
    let mut harness = TestHarness::from_source(include_str!("fixtures/opcua/program/main.st"))
        .expect("build opcua harness");
    let _ = harness.runtime_mut().enable_debug();
    harness.cycle();
    trust_runtime::debug::DebugSnapshot {
        storage_abi: Default::default(),
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    }
}

fn start_fixture_server(name: &str) -> trust_runtime::opcua::OpcUaWireServer {
    let port = reserve_loopback_port();
    let runtime = load_runtime_fixture(name, port);
    let runtime_root = temp_runtime_root(name);
    start_wire_server(
        runtime.resource_name.as_str(),
        &runtime.opcua,
        snapshot_provider(),
        Some(runtime_root.as_path()),
    )
    .expect("start opcua wire server")
    .expect("opcua enabled")
}

fn preferred_node_name(server: &trust_runtime::opcua::OpcUaWireServer) -> String {
    server
        .exposed_nodes()
        .iter()
        .find(|node| matches!(node.data_type, OpcUaDataType::Int32))
        .or_else(|| server.exposed_nodes().first())
        .map(|node| node.name.to_string())
        .expect("at least one exposed node")
}

#[test]
fn opcua_interop_reads_exposed_scalars_with_reference_client() {
    let mut server = start_fixture_server("interop");
    assert!(
        !server.exposed_nodes().is_empty(),
        "expected at least one exposed OPC UA node"
    );

    let node_name = preferred_node_name(&server);
    let value = server
        .probe_read(node_name.as_str(), OpcUaClientIdentity::Anonymous)
        .expect("read exposed node");
    assert!(matches!(
        value,
        OpcUaVariant::Boolean(_)
            | OpcUaVariant::Int16(_)
            | OpcUaVariant::Int32(_)
            | OpcUaVariant::Int64(_)
            | OpcUaVariant::UInt16(_)
            | OpcUaVariant::UInt32(_)
            | OpcUaVariant::UInt64(_)
            | OpcUaVariant::Float(_)
            | OpcUaVariant::Double(_)
            | OpcUaVariant::String(_)
    ));
    server.stop();
}

#[test]
fn opcua_server_start_refuses_a_listen_address_in_use() {
    let port = reserve_loopback_port();
    let occupant = TcpListener::bind(("127.0.0.1", port)).expect("occupy the fixture port");
    let runtime = load_runtime_fixture("interop", port);
    let runtime_root = temp_runtime_root("in-use");

    let error = match start_wire_server(
        runtime.resource_name.as_str(),
        &runtime.opcua,
        snapshot_provider(),
        Some(runtime_root.as_path()),
    ) {
        Ok(_) => panic!("a listen address in use must not start a server"),
        Err(error) => error.to_string(),
    };

    assert!(
        error.contains("OPC UA listen address") && error.contains("is unavailable"),
        "unexpected error: {error}"
    );
    assert_eq!(
        occupant.local_addr().expect("occupant address").port(),
        port,
        "the occupying listener keeps its port"
    );
}

#[test]
fn opcua_server_cold_starts_before_first_runtime_snapshot() {
    let port = reserve_loopback_port();
    let runtime = load_runtime_fixture("interop", port);
    let runtime_root = temp_runtime_root("cold-start");
    let snapshot_slot = Arc::new(Mutex::new(None));
    let provider_slot = snapshot_slot.clone();
    let provider: Arc<dyn Fn() -> Option<trust_runtime::debug::DebugSnapshot> + Send + Sync> =
        Arc::new(move || {
            provider_slot
                .lock()
                .expect("snapshot slot")
                .as_ref()
                .cloned()
        });

    let mut server = start_wire_server(
        runtime.resource_name.as_str(),
        &runtime.opcua,
        provider,
        Some(runtime_root.as_path()),
    )
    .expect("cold-start opcua wire server")
    .expect("opcua enabled");

    *snapshot_slot.lock().expect("snapshot slot") = Some(fixture_snapshot());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut last_error = None;
    while std::time::Instant::now() < deadline {
        match server.probe_read("counter", OpcUaClientIdentity::Anonymous) {
            Ok(value) => {
                assert_eq!(value, OpcUaVariant::Int32(42));
                server.stop();
                return;
            }
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
    panic!("counter was not published after snapshot: {last_error:?}");
}

#[test]
fn opcua_security_enforces_user_auth_and_certificate_trust() {
    let mut server = start_fixture_server("security");
    let node_name = preferred_node_name(&server);
    let anonymous = server.probe_read(node_name.as_str(), OpcUaClientIdentity::Anonymous);
    assert!(anonymous.is_err(), "anonymous read unexpectedly succeeded");

    let untrusted = server.probe_read_with_options(
        node_name.as_str(),
        OpcUaClientIdentity::UserName {
            username: "operator",
            password: "secret",
        },
        OpcUaClientOptions {
            trust_server_certificate: false,
        },
    );
    assert!(
        untrusted.is_err(),
        "read with certificate verification unexpectedly succeeded"
    );

    let trusted = server
        .probe_read(
            node_name.as_str(),
            OpcUaClientIdentity::UserName {
                username: "operator",
                password: "secret",
            },
        )
        .expect("authenticated read");
    assert!(matches!(
        trusted,
        OpcUaVariant::Boolean(_)
            | OpcUaVariant::Int16(_)
            | OpcUaVariant::Int32(_)
            | OpcUaVariant::Int64(_)
            | OpcUaVariant::UInt16(_)
            | OpcUaVariant::UInt32(_)
            | OpcUaVariant::UInt64(_)
            | OpcUaVariant::Float(_)
            | OpcUaVariant::Double(_)
            | OpcUaVariant::String(_)
    ));
    server.stop();
}

#[test]
fn opcua_load_fixture_rejects_writes_to_snapshot_backed_variables() {
    let mut server = start_fixture_server("perf");
    let node_name = preferred_node_name(&server);
    let report = server
        .run_load_fixture(
            node_name.as_str(),
            20,
            OpcUaClientIdentity::Anonymous,
            OpcUaClientOptions::default(),
        )
        .expect("load fixture");

    assert_eq!(report.iterations, 20);
    assert_eq!(report.browse_ok, 20);
    assert_eq!(report.read_ok, 20);
    assert_eq!(
        report.write_ok, 0,
        "snapshot-backed server variables must remain read-only"
    );
    server.stop();
}
