//! The truST runtimes source in the one search (specification 11, "One search, device identity,
//! and presence"; design faults E11 and E12). Every expected value is a literal; the searches run
//! against an in-process [`DiscoveryState`](crate::discovery::DiscoveryState), never a real
//! network or mDNS daemon.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use smol_str::SmolStr;

use crate::discovery::{DiscoveryEntry, DiscoveryState};
use crate::search::model::DeviceKey;
use crate::search::{
    Identity, RuntimeFacts, SearchHost, SearchRegistry, SearchRequest, SearchState, SearchStatus,
    SourceId, SourceState,
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
