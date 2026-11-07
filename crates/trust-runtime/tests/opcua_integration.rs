#![cfg(feature = "opcua-wire")]
//! The OPC UA server against a reference client, serving the variables of a compiled program's
//! published snapshot.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::config::{validate_runtime_toml_text, RuntimeConfig};
use trust_runtime::debug::DebugControl;
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
    let root = trust_test_support::unique_temp_path(&format!(
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
    let snapshot = fixture_snapshot();
    Arc::new(move || Some(snapshot.clone()))
}

/// The snapshot the fixture program publishes at the end of its first scan.
fn fixture_snapshot() -> trust_runtime::debug::DebugSnapshot {
    let mut harness = TestHarness::from_source(include_str!("fixtures/opcua/program/main.st"))
        .expect("build opcua harness");
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    debug
        .snapshot()
        .expect("the scan publishes the program's state")
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

/// A client connection of this server's `counter` 42, a value that never changes, publishing every
/// `poll_interval_ms` with the stale timeout `timeout_ms`, whose worker has taken the first value.
fn steady_client(
    server: &trust_runtime::opcua::OpcUaWireServer,
    poll_interval_ms: u64,
    timeout_ms: u64,
) -> (
    trust_runtime::opcua::OpcUaClientBridge,
    trust_runtime::opcua::OpcUaClientWorker<trust_runtime::opcua::OpcUaWireClientTransport>,
) {
    use trust_runtime::opcua::{
        opcua_client_now_ms, OpcUaClientAuthConfig, OpcUaClientBinding, OpcUaClientBridge,
        OpcUaClientConnectionConfig, OpcUaClientPointAccess, OpcUaClientPointConfig,
        OpcUaWireClientTransport,
    };
    let node = server
        .exposed_nodes()
        .iter()
        .find(|node| node.name == "counter")
        .expect("the counter node")
        .clone();
    let point = OpcUaClientPointConfig {
        var: node.name.clone(),
        node_id: node.node_id.clone(),
        data_type: node.data_type,
        access: OpcUaClientPointAccess::Read,
    };
    let connection = OpcUaClientConnectionConfig {
        name: "steady".into(),
        endpoint_url: server.endpoint_url().to_string(),
        security: server.security_profile(),
        auth: OpcUaClientAuthConfig::Anonymous,
        trust_server_certificate: true,
        poll_interval_ms,
        timeout_ms,
        points: vec![point.clone()],
    };
    let binding = OpcUaClientBinding {
        point,
        target: trust_runtime::BoundGlobal(node.name.clone()),
    };
    let (bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        OpcUaWireClientTransport::new(),
        vec![binding],
    )
    .expect("the client bridge");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !counter_is_live(&bridge) {
        assert!(
            std::time::Instant::now() < deadline,
            "no value within 10 s: {}",
            bridge.snapshot().detail
        );
        let _ = worker.tick(opcua_client_now_ms());
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    (bridge, worker)
}

/// Whether the point `counter` is live with its value 42.
fn counter_is_live(bridge: &trust_runtime::opcua::OpcUaClientBridge) -> bool {
    let snapshot = bridge.snapshot();
    snapshot.point_statuses[0].state == trust_runtime::opcua::OpcUaClientConnectionState::Connected
        && snapshot.values.get("counter") == Some(&trust_runtime::value::Value::DInt(42))
}

/// Specification 11's OPC UA client against this server, whose values do not change: the
/// subscription's heartbeat, the server's current time, keeps the connection and its point live for
/// three seconds, past the stale bound of two seconds (the default timeout, more than three
/// publishing intervals of 50 ms), though the point reports its value 42 only once. Catches a client
/// that calls a steady subscription stale, and a heartbeat that the server never reports.
#[test]
fn a_steady_subscription_stays_live_through_its_heartbeat() {
    use trust_runtime::opcua::{opcua_client_now_ms, OpcUaClientConnectionState};
    let mut server = start_fixture_server("interop");
    let (bridge, mut worker) = steady_client(&server, 50, 2_000);
    let first = std::time::Instant::now();
    while first.elapsed() < std::time::Duration::from_secs(3) {
        worker.tick(opcua_client_now_ms()).expect("a tick");
        assert_eq!(
            bridge.state(),
            OpcUaClientConnectionState::Connected,
            "{} after {:?}",
            bridge.snapshot().detail,
            first.elapsed()
        );
        assert!(
            counter_is_live(&bridge),
            "the point after {:?}",
            first.elapsed()
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    drop(worker);
    server.stop();
}

/// Specification 11's OPC UA client: its subscription publishes at the connection's
/// `poll_interval_ms`, here 1,000 ms, so its heartbeat arrives about once a second. Over five
/// seconds at least three heartbeats arrive, and the median gap between them is at least 600 ms
/// (a busy computer only lengthens the gaps). Catches a subscription that publishes at a fixed rate
/// whatever the connection asks (it published every 250 ms), and a heartbeat that never arrives.
#[test]
fn the_subscription_publishes_at_the_poll_interval() {
    use trust_runtime::opcua::opcua_client_now_ms;
    let mut server = start_fixture_server("interop");
    let (bridge, mut worker) = steady_client(&server, 1_000, 60_000);
    let mut seen = Vec::new();
    let first = std::time::Instant::now();
    while first.elapsed() < std::time::Duration::from_secs(5) {
        worker.tick(opcua_client_now_ms()).expect("a tick");
        let last_seen = bridge.snapshot().last_seen_ms.expect("a notification time");
        if seen.last() != Some(&last_seen) {
            seen.push(last_seen);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut gaps: Vec<u64> = seen.windows(2).map(|pair| pair[1] - pair[0]).collect();
    gaps.sort_unstable();
    assert!(gaps.len() >= 3, "heartbeats at {seen:?}");
    assert!(gaps[gaps.len() / 2] >= 600, "gaps {gaps:?}");
    drop(worker);
    server.stop();
}

/// Specification 11's OPC UA client: a wire client's session ends cleanly while its server runs.
/// Twice, a client connection takes this server's `counter` 42, and its transport's disconnect
/// stops the session's run loop, which reconnects a session that it finds disconnected, and closes
/// the session within ten seconds. Catches a teardown that lets the run loop reconnect the session
/// it is closing, which aborted the process (opcua's session, dropped half reconnected, panicked in
/// its drop), or that waits for that loop forever.
#[test]
fn a_wire_client_disconnects_while_its_server_runs() {
    use trust_runtime::opcua::OpcUaClientTransport;
    let mut server = start_fixture_server("interop");
    for round in 1..=2 {
        let (bridge, mut worker) = steady_client(&server, 250, 2_000);
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(worker.transport_mut().disconnect());
        });
        let disconnected = finished
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("round {round}: no disconnect within 10 s"));
        assert_eq!(disconnected, Ok(()), "round {round}");
        drop(bridge);
    }
    server.stop();
}
