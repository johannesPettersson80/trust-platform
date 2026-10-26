use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::Read;
use std::net::TcpListener;
use std::thread;

use super::*;

#[test]
fn modbus_discovery_reports_tcp_listener_as_port_reachable_only() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let handle = thread::spawn(move || {
        let _ = listener.accept().map(|(mut stream, _)| {
            let mut buffer = [0u8; 1];
            let _ = stream.read(&mut buffer);
        });
    });

    let value = discover_value(
        json!({
            "protocol": "modbus_tcp",
            "scope": { "host": addr.to_string(), "timeout_ms": 250 },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("discover value");
    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .expect("candidates");
    assert_eq!(candidates.len(), 1);
    let expected = addr.to_string();
    assert_eq!(
        candidates[0].get("confidence").and_then(Value::as_str),
        Some("port_reachable")
    );
    assert_ne!(
        candidates[0].get("confidence").and_then(Value::as_str),
        Some("confirmed")
    );
    assert_eq!(
        candidates[0]
            .get("params")
            .and_then(|params| params.get("address"))
            .and_then(Value::as_str),
        Some(expected.as_str())
    );
    assert_eq!(
        candidates[0].get("source").and_then(Value::as_str),
        Some("tcp_connect")
    );
    handle.join().expect("join listener");
}

#[test]
fn modbus_discovery_rejects_large_cidr() {
    let error = discover_value(
        json!({
            "protocol": "modbus_tcp",
            "scope": { "cidr": "10.0.0.0/16" },
            "origin": "this_host"
        }),
        None,
    )
    .expect_err("large scan must be rejected");
    assert!(error.contains("/24 or tighter"), "{error}");
}

#[test]
fn known_unimplemented_protocol_returns_warning_not_error() {
    let value = discover_value(
        json!({
            "protocol": "openot",
            "scope": {},
            "origin": "runtime"
        }),
        None,
    )
    .expect("known deferred protocol must not hard-error");
    assert_eq!(
        value
            .pointer("/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let warning = value
        .pointer("/warnings/0")
        .and_then(Value::as_str)
        .expect("warning");
    assert!(warning.contains("OpenOT discovery is not available yet"));
}

#[test]
fn ethercat_discovery_requires_runtime_origin() {
    let value = discover_value(
        json!({
            "protocol": "ethercat",
            "scope": { "adapter": "eth0" },
            "origin": "this_host"
        }),
        None,
    )
    .expect("runtime-only scan reports warning");
    assert_eq!(
        value
            .pointer("/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let warning = value
        .pointer("/warnings/0")
        .and_then(Value::as_str)
        .expect("warning");
    assert!(warning.contains("runtime host"));
}

#[test]
fn opcua_discovery_is_server_only_warning() {
    let value = discover_value(
        json!({
            "protocol": "opcua",
            "scope": { "host": "127.0.0.1:4840" },
            "origin": "this_host"
        }),
        None,
    )
    .expect("opcua server-only warning");
    assert_eq!(
        value
            .pointer("/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let warning = value
        .pointer("/warnings/0")
        .and_then(Value::as_str)
        .expect("warning");
    assert!(warning.contains("OPC UA server setup"));
}

#[test]
fn unknown_protocol_still_errors() {
    let error = discover_value(
        json!({
            "protocol": "made_up_protocol",
            "scope": {},
            "origin": "this_host"
        }),
        None,
    )
    .expect_err("unknown protocol should fail");
    assert!(
        error.contains("does not know protocol 'made_up_protocol'"),
        "{error}"
    );
}

#[test]
fn targeted_mqtt_discovery_reports_tcp_listener_as_port_reachable_only() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let handle = thread::spawn(move || {
        let _ = listener.accept().map(|(mut stream, _)| {
            let mut buffer = [0u8; 1];
            let _ = stream.read(&mut buffer);
        });
    });

    let value = discover_value(
        json!({
            "protocol": "mqtt",
            "scope": { "host": addr.to_string(), "timeout_ms": 250 },
            "origin": "this_host",
            "passive": true
        }),
        None,
    )
    .expect("discover value");
    let expected_broker = addr.to_string();
    assert_eq!(
        value
            .pointer("/candidates/0/params/broker")
            .and_then(Value::as_str),
        Some(expected_broker.as_str())
    );
    assert_eq!(
        value
            .pointer("/candidates/0/confidence")
            .and_then(Value::as_str),
        Some("port_reachable")
    );
    assert_ne!(
        value
            .pointer("/candidates/0/confidence")
            .and_then(Value::as_str),
        Some("confirmed")
    );
    assert_eq!(
        value
            .pointer("/candidates/0/source")
            .and_then(Value::as_str),
        Some("tcp_connect")
    );
    handle.join().expect("join listener");
}

#[test]
fn runtime_ethercat_discovery_reports_mock_bus_modules() {
    let value = discover_value(
        json!({
            "protocol": "ethercat",
            "scope": { "adapter": "mock" },
            "origin": "runtime",
            "passive": true
        }),
        None,
    )
    .expect("ethercat mock discovery");
    let candidates = value
        .pointer("/candidates")
        .and_then(Value::as_array)
        .expect("candidates");
    assert!(
        candidates.iter().any(|candidate| candidate
            .pointer("/params/model")
            .and_then(Value::as_str)
            == Some("EL1008")),
        "mock EtherCAT bus should expose configured/discovered modules: {value}"
    );
    assert_eq!(
        candidates
            .iter()
            .find(|candidate| {
                candidate.pointer("/params/model").and_then(Value::as_str) == Some("EL1008")
            })
            .and_then(|candidate| candidate.get("source"))
            .and_then(Value::as_str),
        Some("ethercat_bus")
    );
}

#[test]
fn discovery_window_defaults_to_two_seconds_and_probes_to_150_ms() {
    // Catches the truST runtime browse falling back on the 150 ms probe default, too short for an
    // mDNS query and its repeat one second later, and an explicit timeout escaping the 1 through
    // 2,000 ms clamp for either (spec 11 section 6.5, communication discovery scope).
    let unset = DiscoverScope::default();
    assert_eq!(mdns_browse_window(&unset), Duration::from_millis(2_000));
    assert_eq!(timeout(&unset), Duration::from_millis(150));
    for (requested, expected_ms) in [
        (0, 1),
        (1, 1),
        (150, 150),
        (500, 500),
        (2_000, 2_000),
        (2_001, 2_000),
        (u64::MAX, 2_000),
    ] {
        let scope = DiscoverScope {
            timeout_ms: Some(requested),
            ..DiscoverScope::default()
        };
        assert_eq!(
            mdns_browse_window(&scope),
            Duration::from_millis(expected_ms),
            "browse window for timeout_ms {requested}"
        );
        assert_eq!(
            timeout(&scope),
            Duration::from_millis(expected_ms),
            "probe timeout for timeout_ms {requested}"
        );
    }
}

#[test]
fn mdns_browse_keeps_listening_through_quiet_intervals() {
    // Catches the browse ending at the first receive that times out (spec 11 section 6.5), the
    // acceptance defect of 26 October 2026: every run ended after about 0.13 s and missed a
    // runtime that answered later.
    let channel = SimulatedBrowse::new(vec![
        (0, Arrival::OtherEvent),
        (350, Arrival::Resolved("c6-runtime")),
    ]);
    assert_eq!(channel.browse(2_000), ["c6-runtime"]);
}

#[test]
fn mdns_browse_ends_when_the_window_ends() {
    // Catches a browse that ends before its window while receives keep timing out, one that
    // listens past the window, and one that never ends (spec 11 section 6.5).
    for window_ms in [1, 150, 2_000] {
        let channel = SimulatedBrowse::new(Vec::new());
        assert!(channel.browse(window_ms).is_empty());
        assert_eq!(
            channel.now.get(),
            Duration::from_millis(window_ms),
            "the browse ends with its {window_ms} ms window"
        );
    }
}

#[test]
fn mdns_browse_ends_when_the_channel_closes() {
    // Catches a browse that goes on receiving from a closed channel until its window ends, and one
    // that drops the runtimes resolved before the close (spec 11 section 6.5).
    let channel = SimulatedBrowse::new(vec![
        (200, Arrival::Resolved("runtime-a")),
        (500, Arrival::Close),
    ]);
    assert_eq!(channel.browse(2_000), ["runtime-a"]);
    assert_eq!(channel.now.get(), Duration::from_millis(500));
}

#[test]
fn mdns_browse_collects_each_runtime_once() {
    // Catches a runtime listed twice when it answers the first query and the repeat one second
    // later (RFC 6762 §5.2), and a second runtime of the same name lost to the deduplication,
    // which goes by runtime id (spec 11 section 6.5, communication discovery scope).
    let channel = SimulatedBrowse::new(vec![
        (120, Arrival::Resolved("plc-1")),
        (1_120, Arrival::Resolved("plc-1")),
        (1_150, Arrival::Resolved("plc-2")),
    ]);
    assert_eq!(channel.browse(2_000), ["plc-1", "plc-2"]);
}

// What reaches a simulated mDNS browse channel at its scheduled time.
enum Arrival {
    Resolved(&'static str),
    OtherEvent,
    Close,
}

// An mDNS browse channel on a simulated clock: a receive returns the first arrival due within its
// wait, or times out after its full wait, as a receive with a timeout does on a real channel.
struct SimulatedBrowse {
    now: Cell<Duration>,
    arrivals: RefCell<VecDeque<(Duration, Arrival)>>,
    closed: Cell<bool>,
}

impl SimulatedBrowse {
    fn new(arrivals: Vec<(u64, Arrival)>) -> Self {
        Self {
            now: Cell::new(Duration::ZERO),
            arrivals: RefCell::new(
                arrivals
                    .into_iter()
                    .map(|(at_ms, arrival)| (Duration::from_millis(at_ms), arrival))
                    .collect(),
            ),
            closed: Cell::new(false),
        }
    }

    // Browses for the window and returns the ids of the runtimes collected.
    fn browse(&self, window_ms: u64) -> Vec<String> {
        let window = Duration::from_millis(window_ms);
        collect_resolved_runtimes(window, || self.now.get(), |wait| self.receive(window, wait))
            .into_iter()
            .map(|entry| entry.id.to_string())
            .collect()
    }

    fn receive(
        &self,
        window: Duration,
        wait: Duration,
    ) -> Result<Option<DiscoveryEntry>, RecvTimeoutError> {
        let now = self.now.get();
        assert!(
            !self.closed.get(),
            "the browse received again after the channel closed"
        );
        assert!(
            wait > Duration::ZERO && now + wait <= window,
            "the browse listened outside its {window:?} window: {wait:?} from {now:?}"
        );
        let mut arrivals = self.arrivals.borrow_mut();
        if !arrivals.front().is_some_and(|(at, _)| *at <= now + wait) {
            self.now.set(now + wait);
            return Err(RecvTimeoutError::Timeout);
        }
        let (at, arrival) = arrivals.pop_front().expect("an arrival is due");
        self.now.set(now.max(at));
        match arrival {
            Arrival::Resolved(id) => Ok(Some(runtime(id))),
            Arrival::OtherEvent => Ok(None),
            Arrival::Close => {
                self.closed.set(true);
                Err(RecvTimeoutError::Disconnected)
            }
        }
    }
}

// Every simulated runtime has the same name, so only its runtime id tells it apart.
fn runtime(id: &str) -> DiscoveryEntry {
    DiscoveryEntry {
        id: smol_str::SmolStr::new(id),
        name: smol_str::SmolStr::new("trust-runtime"),
        addresses: Vec::new(),
        web_port: None,
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        last_seen_ns: 0,
    }
}
