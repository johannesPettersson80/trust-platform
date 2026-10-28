//! The truST runtimes source in the one search (specification 11, "One search, device identity,
//! and presence"; design faults E11 and E12). Every expected value is a literal; the searches run
//! against an in-process [`DiscoveryState`](crate::discovery::DiscoveryState), never a real
//! network or mDNS daemon.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use mdns_sd::RecvTimeoutError;
use serde_json::json;
use smol_str::SmolStr;

use super::super::scope::{timeout, DiscoverScope};
use super::{collect_resolved_runtimes, mdns_browse_window};

use crate::discovery::{DiscoveryEntry, DiscoveryState};
use crate::search::model::DeviceKey;
use crate::search::{
    Identity, ProtocolFind, RuntimeFacts, SearchHost, SearchRegistry, SearchRequest, SearchState,
    SearchStatus, SourceId, SourceState,
};

/// The one mDNS announcement of the runtime that runs the search: it reports its own resource
/// name, so its ID is only in the literal entry of each test.
fn runtime_facts(discovery: &Arc<DiscoveryState>, runtime_id: &str) -> RuntimeFacts {
    RuntimeFacts {
        project_root: None,
        discovery: Arc::clone(discovery),
        discovery_enabled: true,
        discovery_interfaces: Vec::new(),
        runtime_id: SmolStr::new(runtime_id),
        owned_adapters: Vec::new(),
        mesh: None,
    }
}

/// Starts a search of only the truST runtimes source inside a runtime.
fn start_runtime_search(facts: RuntimeFacts, deadline_ms: u64) -> (SearchRegistry, String) {
    let registry = SearchRegistry::new();
    let request = SearchRequest::from_params(Some(json!({
        "only": ["trust_runtime"],
        "deadline_ms": deadline_ms,
    })))
    .expect("search request");
    let search_id = registry
        .start(request, SearchHost::Runtime(facts))
        .expect("start the search");
    (registry, search_id)
}

/// Waits for the first device, or gives up after 20 s.
fn wait_for_a_device(registry: &SearchRegistry, search_id: &str) -> SearchStatus {
    let give_up = Instant::now() + Duration::from_secs(20);
    loop {
        let status = registry.status(search_id, None).expect("kept search");
        if !status.devices.is_empty() || Instant::now() > give_up {
            return status;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Waits until the search ends, or gives up after 20 s.
fn wait_for_end(registry: &SearchRegistry, search_id: &str) -> SearchStatus {
    let give_up = Instant::now() + Duration::from_secs(20);
    loop {
        let status = registry.status(search_id, None).expect("kept search");
        if status.state != SearchState::Running || Instant::now() > give_up {
            return status;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// R2 (E12): a runtime's search never reports the runtime itself, and each find keeps its TLS web
/// address, mesh port, ID, address keys and last seen (specification 11, "One search, device
/// identity, and presence": a find is merged by address, net ID, USB serial or runtime ID, and the
/// runtime does not find itself).
#[test]
fn runtime_finds_keep_tls_mesh_id_addresses_last_seen_and_exclude_self() {
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![
        DiscoveryEntry {
            id: SmolStr::new("self-id"),
            name: SmolStr::new("self"),
            addresses: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
            web_port: None,
            web_tls: false,
            mesh_port: None,
            control: None,
            host_group: None,
            last_seen_ns: 1,
        },
        DiscoveryEntry {
            id: SmolStr::new("peer-id"),
            name: SmolStr::new("peer"),
            addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))],
            web_port: Some(8443),
            web_tls: true,
            mesh_port: Some(7443),
            control: Some(SmolStr::new("tls://192.168.1.10:7444")),
            host_group: Some(SmolStr::new("line-a")),
            last_seen_ns: 1_700_000_000_000_000_000,
        },
    ]);
    let (registry, search_id) = start_runtime_search(runtime_facts(&discovery, "self-id"), 1_000);

    let status = wait_for_a_device(&registry, &search_id);
    assert_eq!(
        status.devices.len(),
        1,
        "the runtime's own announcement must not be a find: {:?}",
        status.devices
    );
    let device = &status.devices[0];
    assert_eq!(device.id, "dev:ip:192.168.1.10");
    assert_eq!(device.runtime_ids, vec!["peer-id".to_string()]);
    assert_eq!(device.finds.len(), 1);
    let find = &device.finds[0];
    assert_eq!(find.protocol, SourceId::TrustRuntime);
    // `1_700_000_000_000_000_000` ns is `1_700_000_000_000` ms.
    assert_eq!(find.last_seen_ms, 1_700_000_000_000);
    assert!(
        find.keys
            .contains(&DeviceKey::Address("192.168.1.10".to_string())),
        "the find keeps its address key: {:?}",
        find.keys
    );
    assert!(
        find.keys
            .contains(&DeviceKey::RuntimeId("peer-id".to_string())),
        "the find keeps its runtime ID key: {:?}",
        find.keys
    );
    let Identity::TrustRuntime(identity) = &find.identity else {
        panic!("expected a truST runtime identity, got {:?}", find.identity);
    };
    assert_eq!(identity.id, "peer-id");
    assert_eq!(identity.name, "peer");
    assert_eq!(
        identity.web.as_deref(),
        Some("https://192.168.1.10:8443"),
        "a TLS runtime is found at its https address"
    );
    assert_eq!(identity.mesh_port, Some(7443));

    registry.cancel(&search_id).expect("cancel");
}

/// R3 (E11): the runtime source listens until the search ends and reports finds as they arrive; it
/// is not a one-shot 150 ms browse. An entry added after the search started is still found, and
/// the source returns only once the sink has ended (specification 11, "One search, device
/// identity, and presence").
#[test]
fn runtime_source_listens_until_the_search_deadline() {
    let discovery = Arc::new(DiscoveryState::new());
    let (registry, search_id) = start_runtime_search(runtime_facts(&discovery, "rt-self"), 1_000);
    let started = Instant::now();

    // The source has read the empty discovery once; the entry arrives afterwards.
    thread::sleep(Duration::from_millis(100));
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("late-id"),
        name: SmolStr::new("late"),
        addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))],
        web_port: Some(8080),
        web_tls: false,
        mesh_port: None,
        control: Some(SmolStr::new("tcp://10.0.0.5:5680")),
        host_group: None,
        last_seen_ns: 0,
    }]);

    let status = wait_for_end(&registry, &search_id);
    assert_eq!(status.state, SearchState::Done);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "the source listened until the search deadline, not a one-shot browse: {:?}",
        started.elapsed()
    );
    assert_eq!(
        status.sources[0].state,
        SourceState::TimedOut,
        "the source was still listening when the deadline passed"
    );
    assert_eq!(status.sources[0].count, Some(1));
    assert_eq!(status.devices.len(), 1);
    let Identity::TrustRuntime(identity) = &status.devices[0].finds[0].identity else {
        panic!("expected a truST runtime identity");
    };
    assert_eq!(
        identity.id, "late-id",
        "the entry added after the search started is still found"
    );

    registry.cancel(&search_id).ok();
}

/// A discovered runtime `id` at `10.1.0.<host>` with a control endpoint, so that no control warning
/// is due. Its mesh port and last-seen time are the test's own.
fn presence_entry(id: &str, host: u8, mesh_port: Option<u16>, last_seen_ns: u64) -> DiscoveryEntry {
    DiscoveryEntry {
        id: SmolStr::new(id),
        name: SmolStr::new(id),
        addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 1, 0, host))],
        web_port: None,
        web_tls: false,
        mesh_port,
        control: Some(SmolStr::new(format!("tcp://10.1.0.{host}:5680"))),
        host_group: None,
        last_seen_ns,
    }
}

/// The one find of the device whose only runtime ID is `runtime_id`.
fn find_of_runtime<'a>(status: &'a SearchStatus, runtime_id: &str) -> &'a ProtocolFind {
    let device = status
        .devices
        .iter()
        .find(|device| device.runtime_ids == [runtime_id.to_string()])
        .expect("the runtime is found");
    &device.finds[0]
}

/// R2: the runtime cloud's presence marks what discovery saw (specification 11, "Runtime cloud
/// presence"). A runtime without a mesh endpoint is stale, and partitioned when it was last seen 120
/// s or more ago. A runtime with a mesh endpoint is stale when it was last seen 30 s or more ago,
/// and a fresh one carries no mark. Each find's `last_seen` is its observation time. The times are
/// literal and far from the present: 2001 is more than 30 s old, and 2255 is after now. Wrong
/// behaviours: a missing or swapped mark, a runtime with a mesh endpoint marked partitioned, or a
/// find that keeps the announcement's time.
#[test]
fn cloud_presence_marks_stale_and_partitioned() {
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![
        presence_entry("old-partitioned", 1, None, 1_000_000_000_000_000_000),
        presence_entry("old-stale", 2, Some(7443), 1_000_000_000_000_000_000),
        presence_entry("fresh", 3, Some(7443), 9_000_000_000_000_000_000),
    ]);
    let (registry, search_id) = start_runtime_search(runtime_facts(&discovery, "rt-self"), 1_000);

    let status = wait_for_end(&registry, &search_id);
    assert_eq!(
        status.devices.len(),
        3,
        "each runtime is found: {:?}",
        status.devices
    );

    let partitioned = find_of_runtime(&status, "old-partitioned");
    assert_eq!(
        partitioned.warnings,
        vec!["stale".to_string(), "partitioned".to_string()]
    );
    assert_eq!(partitioned.last_seen_ms, 1_000_000_000_000);

    let stale = find_of_runtime(&status, "old-stale");
    assert_eq!(stale.warnings, vec!["stale".to_string()]);
    assert_eq!(stale.last_seen_ms, 1_000_000_000_000);

    let fresh = find_of_runtime(&status, "fresh");
    assert!(
        fresh.warnings.is_empty(),
        "a fresh runtime carries no mark: {:?}",
        fresh.warnings
    );
    assert_eq!(fresh.last_seen_ms, 9_000_000_000_000);

    registry.cancel(&search_id).ok();
}

/// R4: the presence probe connects to a runtime's web endpoint and writes nothing to it, and a
/// runtime whose web endpoint answers is seen now (specification 11, "Runtime cloud presence").
/// The listener counts the bytes of every connection that the probe makes, and a probe that sent a
/// request would show there. The announcement is 2001, so the probe is made; the answer makes the
/// runtime fresh, so its find has no mark and its `last_seen` is later than the announcement.
#[test]
fn cloud_presence_probe_only_connects_and_writes_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a web endpoint");
    let web_port = listener
        .local_addr()
        .expect("the endpoint's address")
        .port();
    let received = thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("a non-blocking listener");
        let until = Instant::now() + Duration::from_secs(2);
        let mut bytes = 0;
        while Instant::now() < until {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_nonblocking(false)
                        .expect("a blocking connection");
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .expect("a read timeout");
                    let mut buffer = Vec::new();
                    let _ = stream.read_to_end(&mut buffer);
                    bytes += buffer.len();
                }
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        }
        bytes
    });

    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("probed"),
        name: SmolStr::new("probed"),
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        web_port: Some(web_port),
        web_tls: false,
        mesh_port: Some(7443),
        control: Some(SmolStr::new("tcp://127.0.0.1:5680")),
        host_group: None,
        last_seen_ns: 1_000_000_000_000_000_000,
    }]);
    let (registry, search_id) = start_runtime_search(runtime_facts(&discovery, "rt-self"), 1_000);

    let status = wait_for_end(&registry, &search_id);
    let bytes = received.join().expect("the listener thread ends");
    assert_eq!(bytes, 0, "the probe writes nothing to the web endpoint");
    let find = find_of_runtime(&status, "probed");
    assert!(
        find.warnings.is_empty(),
        "the answering endpoint makes the runtime fresh: {:?}",
        find.warnings
    );
    assert!(
        find.last_seen_ms > 1_000_000_000_000,
        "the find is seen now, not at its announcement"
    );

    registry.cancel(&search_id).ok();
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
            Arrival::Resolved(id) => Ok(Some(browsed_runtime(id))),
            Arrival::OtherEvent => Ok(None),
            Arrival::Close => {
                self.closed.set(true);
                Err(RecvTimeoutError::Disconnected)
            }
        }
    }
}

// Every simulated runtime has the same name, so only its runtime id tells it apart.
fn browsed_runtime(id: &str) -> DiscoveryEntry {
    DiscoveryEntry {
        id: SmolStr::new(id),
        name: SmolStr::new("trust-runtime"),
        addresses: Vec::new(),
        web_port: None,
        web_tls: false,
        mesh_port: None,
        control: None,
        host_group: None,
        last_seen_ns: 0,
    }
}
