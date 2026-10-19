//! The mesh peers of a runtime in a search (specification 11, "Sources" and O31). The peers are
//! real Zenoh sessions on 127.0.0.1 that connect to the searching session, so each identity query
//! crosses a real session. The fake peers answer literal wire answers. Every expected value is a
//! literal taken from those answers.

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use smol_str::SmolStr;
use zenoh::{Config, Session, Wait};

use crate::discovery::{DiscoveryEntry, DiscoveryState};
use crate::mesh::MeshTopologyEvidence;
use crate::search::{
    FoundDevice, Identity, RuntimeFacts, SearchHost, SearchRegistry, SearchRequest, SearchState,
    SearchStatus, SourceId,
};

/// Peer A's answer: an identity that sends its name, version and platform.
const ANSWER_A: &str = r#"{"api_version":"1.0","runtime_id":"peer-a","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"],"name":"Mixer-02","version":"0.24.75","platform":"linux-x86_64"}"#;
/// Peer B's answer: an identity only, as a runtime sends it without the optional fields.
const ANSWER_B: &str = r#"{"api_version":"1.0","runtime_id":"peer-b","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"]}"#;
/// The stable runtime ID of the bundled runtime `line-a`: sixteen random bytes in lowercase hex, as
/// `runtime_id` generates them.
const STABLE_LINE_A: &str = "3f9c2a1b7e4d5f60a18c2e4d6b9f0a13";
/// Line A's answer: its mesh name `line-a` keys its identity queryable, and its stable runtime ID is
/// the one that its mDNS announcement gives.
const ANSWER_LINE_A: &str = r#"{"api_version":"1.0","runtime_id":"line-a","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"],"name":"line-a","version":"0.24.75","platform":"linux-x86_64","stable_runtime_id":"3f9c2a1b7e4d5f60a18c2e4d6b9f0a13"}"#;
/// Ring 7's answer: an identity that sends neither a name nor a stable runtime ID.
const ANSWER_RING_7: &str = r#"{"api_version":"1.0","runtime_id":"ring-7","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"]}"#;
/// The stable runtime ID of peer X, a runtime other than the search runtime.
const STABLE_PEER_X: &str = "b7e1d4c9a2f85036e9c1b7d4a2f80e35";
/// Peer X's answer: its mesh name is `peer-x`, and it gives its stable runtime ID.
const ANSWER_X: &str = r#"{"api_version":"1.0","runtime_id":"peer-x","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"],"stable_runtime_id":"b7e1d4c9a2f85036e9c1b7d4a2f80e35"}"#;
/// The answer of the search runtime's own mesh peer: it gives the search runtime's ID (`rt-self`).
const ANSWER_SELF: &str = r#"{"api_version":"1.0","runtime_id":"rt-self-mesh","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"],"stable_runtime_id":"rt-self"}"#;

/// The control-endpoint warning of a mesh find (specification 11, "Sources").
const MESH_WARNING: &str =
    "Runtime found through the mesh without a control endpoint; adopt may require manual endpoint entry.";

/// A free loopback TCP port for a Zenoh endpoint.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a loopback port")
        .local_addr()
        .expect("the port's address")
        .port()
}

/// A Zenoh peer that listens on `port` and connects to the peers in `connect`. Multicast scouting
/// is off, so the test sessions see only each other.
fn open_peer(port: u16, connect: &[u16]) -> Session {
    let mut config = Config::default();
    config.insert_json5("mode", "\"peer\"").expect("peer mode");
    config
        .insert_json5(
            "listen/endpoints",
            &json!([format!("tcp/127.0.0.1:{port}")]).to_string(),
        )
        .expect("listen endpoint");
    let endpoints: Vec<String> = connect
        .iter()
        .map(|port| format!("tcp/127.0.0.1:{port}"))
        .collect();
    config
        .insert_json5("connect/endpoints", &json!(endpoints).to_string())
        .expect("connect endpoints");
    config
        .insert_json5("scouting/multicast/enabled", "false")
        .expect("multicast scouting off");
    zenoh::open(config).wait().expect("open a Zenoh session")
}

/// Declares the identity queryable of `runtime_id` on `session`, which answers `answer`.
fn answer_identity(session: &Session, runtime_id: &str, answer: &str) {
    let key = format!("truST/default-site/{runtime_id}/_meta/identity");
    let answer = answer.to_string();
    session
        .declare_queryable(key.as_str())
        .callback(move |query| {
            let _ = query.reply(query.key_expr().clone(), answer.clone()).wait();
        })
        .background()
        .wait()
        .expect("declare the identity queryable");
}

/// Records the key of every sample that `session` receives in the runtime data space. A write
/// from the search to a peer would arrive here.
fn record_writes(session: &Session) -> Arc<Mutex<Vec<String>>> {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    session
        .declare_subscriber("truST/**")
        .callback(move |sample| {
            recorded
                .lock()
                .expect("the write log")
                .push(sample.key_expr().to_string());
        })
        .background()
        .wait()
        .expect("declare the write recorder");
    writes
}

/// The facts of the runtime that runs the search: its mesh, and the discovery it is given.
fn runtime_facts(mesh: MeshTopologyEvidence, discovery: Arc<DiscoveryState>) -> RuntimeFacts {
    RuntimeFacts {
        project_root: None,
        discovery,
        discovery_enabled: true,
        discovery_interfaces: Vec::new(),
        runtime_id: SmolStr::new("rt-self"),
        owned_adapters: Vec::new(),
        mesh: Some(mesh),
    }
}

/// Starts a search of only the truST runtimes source inside a runtime, with `deadline_ms`.
fn start_trust_runtime_search(facts: RuntimeFacts, deadline_ms: u64) -> (SearchRegistry, String) {
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

/// Waits until `ready` holds for the search's status, or gives up after 20 s.
fn wait_until(
    registry: &SearchRegistry,
    search_id: &str,
    ready: impl Fn(&SearchStatus) -> bool,
) -> SearchStatus {
    let give_up = Instant::now() + Duration::from_secs(20);
    loop {
        let status = registry.status(search_id, None).expect("kept search");
        if ready(&status) || Instant::now() > give_up {
            return status;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// The device whose only runtime ID is `runtime_id`.
fn device_of<'a>(status: &'a SearchStatus, runtime_id: &str) -> Option<&'a FoundDevice> {
    status
        .devices
        .iter()
        .find(|device| device.runtime_ids == [runtime_id.to_string()])
}

/// Spec 11, "Sources" (O31): a mesh peer that answers its identity is a truST runtime find. The
/// answer names the runtime by its runtime ID, and gives its name, version and platform when it
/// sends them. An answer without them is still a find, named by its runtime ID alone. A peer that
/// does not answer is left out with no error. Wrong behaviours: a silent peer reported as a device,
/// an answer dropped for lacking the optional fields, or a warning for a silent peer.
#[test]
fn mesh_peers_answer_their_identity_query() {
    let search_port = free_port();
    let searcher = open_peer(search_port, &[]);
    let peer_a = open_peer(free_port(), &[search_port]);
    answer_identity(&peer_a, "peer-a", ANSWER_A);
    let peer_b = open_peer(free_port(), &[search_port]);
    answer_identity(&peer_b, "peer-b", ANSWER_B);
    // Peer C is a mesh peer that never answers: it declares no identity queryable.
    let _peer_c = open_peer(free_port(), &[search_port]);
    let mesh = MeshTopologyEvidence::for_test_with_session(
        searcher.clone(),
        &["peer-a", "peer-b", "peer-c"],
    );
    let (registry, search_id) =
        start_trust_runtime_search(runtime_facts(mesh, Arc::new(DiscoveryState::new())), 6_000);

    wait_until(&registry, &search_id, |status| status.devices.len() == 2);
    let status = wait_until(&registry, &search_id, |status| {
        status.state != SearchState::Running
    });
    assert_eq!(
        status.devices.len(),
        2,
        "peers A and B are found: {:?}",
        status.devices
    );

    let device_a = device_of(&status, "peer-a").expect("peer A is found");
    assert_eq!(device_a.finds.len(), 1);
    let find = &device_a.finds[0];
    assert_eq!(find.protocol, SourceId::TrustRuntime);
    assert_eq!(find.source, "mesh");
    let Identity::TrustRuntime(identity) = &find.identity else {
        panic!("expected a truST runtime identity, got {:?}", find.identity);
    };
    assert_eq!(identity.id, "peer-a");
    assert_eq!(identity.name, "Mixer-02");
    assert_eq!(identity.version.as_deref(), Some("0.24.75"));
    assert_eq!(identity.platform.as_deref(), Some("linux-x86_64"));
    assert_eq!(find.warnings, vec![MESH_WARNING.to_string()]);

    let device_b = device_of(&status, "peer-b")
        .expect("peer B is found although its answer has no optional fields");
    let Identity::TrustRuntime(identity) = &device_b.finds[0].identity else {
        panic!("expected a truST runtime identity");
    };
    assert_eq!(
        identity.name, "peer-b",
        "a runtime that sends no name is named by its runtime ID"
    );
    assert_eq!(identity.version, None);
    assert_eq!(identity.platform, None);

    assert!(
        device_of(&status, "peer-c").is_none(),
        "a silent peer is left out"
    );
    assert!(
        status.sources[0].warnings.is_empty(),
        "a silent peer gives no warning: {:?}",
        status.sources[0].warnings
    );

    registry.cancel(&search_id).ok();
}

/// Spec 11, "Sources" (O31): the search only queries the mesh peers and writes to none of them.
/// Every sample that the peer receives is recorded, and the search sends none. Wrong behaviour: a
/// search that publishes to a peer to probe it.
#[test]
fn mesh_peers_are_only_queried_never_written() {
    let search_port = free_port();
    let searcher = open_peer(search_port, &[]);
    let peer_a = open_peer(free_port(), &[search_port]);
    answer_identity(&peer_a, "peer-a", ANSWER_A);
    let writes = record_writes(&peer_a);
    let mesh = MeshTopologyEvidence::for_test_with_session(searcher.clone(), &["peer-a"]);
    let (registry, search_id) =
        start_trust_runtime_search(runtime_facts(mesh, Arc::new(DiscoveryState::new())), 6_000);

    let status = wait_until(&registry, &search_id, |status| status.devices.len() == 1);
    assert_eq!(status.devices.len(), 1, "peer A was queried and answered");
    let status = wait_until(&registry, &search_id, |status| {
        status.state != SearchState::Running
    });
    assert_eq!(status.state, SearchState::Done);
    let recorded = writes.lock().expect("the write log").clone();
    assert!(
        recorded.is_empty(),
        "the search wrote to a peer: {recorded:?}"
    );

    registry.cancel(&search_id).ok();
}

/// Spec 11, "Sources": a mesh find and the mDNS find of one runtime are merged into one device by
/// runtime ID. Each keeps its own protocol result, so the mesh answer does not replace the mDNS
/// find and its control endpoint. Wrong behaviour: the mesh answer overwrites the mDNS find.
#[test]
fn mesh_find_joins_the_mdns_find_of_its_runtime_id() {
    let search_port = free_port();
    let searcher = open_peer(search_port, &[]);
    let peer_a = open_peer(free_port(), &[search_port]);
    answer_identity(&peer_a, "peer-a", ANSWER_A);
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new("peer-a"),
        name: SmolStr::new("Mixer-02"),
        addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))],
        web_port: None,
        web_tls: false,
        mesh_port: Some(7443),
        control: Some(SmolStr::new("tls://192.168.1.10:7444")),
        host_group: None,
        last_seen_ns: 9_000_000_000_000_000_000,
    }]);
    let mesh = MeshTopologyEvidence::for_test_with_session(searcher.clone(), &["peer-a"]);
    let (registry, search_id) = start_trust_runtime_search(runtime_facts(mesh, discovery), 6_000);

    let status = wait_until(&registry, &search_id, |status| {
        status.devices.len() == 1 && status.devices[0].finds.len() == 2
    });
    assert_eq!(status.devices.len(), 1, "one device for one runtime ID");
    let device = &status.devices[0];
    let mut sources: Vec<&str> = device
        .finds
        .iter()
        .map(|find| find.source.as_str())
        .collect();
    sources.sort_unstable();
    assert_eq!(sources, vec!["mdns", "mesh"]);
    let mdns = device
        .finds
        .iter()
        .find(|find| find.source == "mdns")
        .expect("the mDNS find");
    let Identity::TrustRuntime(identity) = &mdns.identity else {
        panic!("expected a truST runtime identity");
    };
    assert_eq!(
        identity.control.as_deref(),
        Some("tls://192.168.1.10:7444"),
        "the mesh answer does not replace the mDNS find"
    );
    assert_eq!(identity.mesh_port, Some(7443));

    registry.cancel(&search_id).ok();
}

/// Spec 11, "Sources" (the mesh peers paragraph, R6 to R9): a bundled runtime's mesh answer gives the
/// stable runtime ID that its mDNS announcement gives, so the mesh find and the mDNS find are one
/// device with both sources, and both are named `line-a`. Wrong behaviour: the mesh find keyed by its
/// mesh name, which makes a second device, or named by the stable ID.
#[test]
fn bundled_runtime_mesh_find_merges_with_its_mdns_find() {
    let search_port = free_port();
    let searcher = open_peer(search_port, &[]);
    let line_a = open_peer(free_port(), &[search_port]);
    answer_identity(&line_a, "line-a", ANSWER_LINE_A);
    let discovery = Arc::new(DiscoveryState::new());
    discovery.replace_entries(vec![DiscoveryEntry {
        id: SmolStr::new(STABLE_LINE_A),
        name: SmolStr::new("line-a"),
        addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))],
        web_port: None,
        web_tls: false,
        mesh_port: Some(7443),
        control: Some(SmolStr::new("tls://192.168.1.20:7444")),
        host_group: None,
        last_seen_ns: 9_000_000_000_000_000_000,
    }]);
    let mesh = MeshTopologyEvidence::for_test_with_session(searcher.clone(), &["line-a"]);
    let (registry, search_id) = start_trust_runtime_search(runtime_facts(mesh, discovery), 6_000);

    let status = wait_until(&registry, &search_id, |status| {
        status.devices.len() == 1 && status.devices[0].finds.len() == 2
    });
    assert_eq!(
        status.devices.len(),
        1,
        "one device for one runtime: {:?}",
        status.devices
    );
    let device = &status.devices[0];
    assert_eq!(device.runtime_ids, vec![STABLE_LINE_A.to_string()]);
    let mut sources: Vec<&str> = device
        .finds
        .iter()
        .map(|find| find.source.as_str())
        .collect();
    sources.sort_unstable();
    assert_eq!(sources, vec!["mdns", "mesh"]);
    for find in &device.finds {
        let Identity::TrustRuntime(identity) = &find.identity else {
            panic!("expected a truST runtime identity, got {:?}", find.identity);
        };
        assert_eq!(
            identity.name, "line-a",
            "the {} find is named by the runtime's name",
            find.source
        );
        assert_eq!(
            identity.id, STABLE_LINE_A,
            "the {} find is keyed by the stable runtime ID",
            find.source
        );
    }

    registry.cancel(&search_id).ok();
}

/// Spec 11, "Sources" (the mesh peers paragraph): an answer that gives no stable runtime ID is named
/// by its mesh name. Wrong behaviour: the answer dropped for lacking the stable ID, or named by an
/// empty name or by a derived ID.
#[test]
fn mesh_answer_without_a_stable_id_is_named_by_its_mesh_name() {
    let search_port = free_port();
    let searcher = open_peer(search_port, &[]);
    let ring = open_peer(free_port(), &[search_port]);
    answer_identity(&ring, "ring-7", ANSWER_RING_7);
    let mesh = MeshTopologyEvidence::for_test_with_session(searcher.clone(), &["ring-7"]);
    let (registry, search_id) =
        start_trust_runtime_search(runtime_facts(mesh, Arc::new(DiscoveryState::new())), 6_000);

    let status = wait_until(&registry, &search_id, |status| status.devices.len() == 1);
    assert_eq!(
        status.devices.len(),
        1,
        "the answer is found: {:?}",
        status.devices
    );
    let device = &status.devices[0];
    assert_eq!(device.runtime_ids, vec!["ring-7".to_string()]);
    let Identity::TrustRuntime(identity) = &device.finds[0].identity else {
        panic!(
            "expected a truST runtime identity, got {:?}",
            device.finds[0].identity
        );
    };
    assert_eq!(identity.id, "ring-7");
    assert_eq!(identity.name, "ring-7");
    assert_eq!(identity.version, None);

    registry.cancel(&search_id).ok();
}

/// Spec 11, "One search, device identity, and presence": a runtime's search never reports the runtime
/// itself, so the mesh answer that gives the search runtime's own stable runtime ID (`rt-self`) is
/// left out. The other peer, which is not the runtime, is still found, so the test is not vacuous.
/// Wrong behaviour: the runtime finds itself through its own mesh session.
#[test]
fn mesh_answer_of_the_runtime_itself_is_left_out() {
    let search_port = free_port();
    let searcher = open_peer(search_port, &[]);
    let itself = open_peer(free_port(), &[search_port]);
    answer_identity(&itself, "rt-self-mesh", ANSWER_SELF);
    let peer_x = open_peer(free_port(), &[search_port]);
    answer_identity(&peer_x, "peer-x", ANSWER_X);
    let mesh =
        MeshTopologyEvidence::for_test_with_session(searcher.clone(), &["rt-self-mesh", "peer-x"]);
    let (registry, search_id) =
        start_trust_runtime_search(runtime_facts(mesh, Arc::new(DiscoveryState::new())), 6_000);

    let status = wait_until(&registry, &search_id, |status| {
        status.state != SearchState::Running
    });
    assert_eq!(
        status.devices.len(),
        1,
        "only peer X is found: {:?}",
        status.devices
    );
    assert_eq!(
        status.devices[0].runtime_ids,
        vec![STABLE_PEER_X.to_string()]
    );

    registry.cancel(&search_id).ok();
}
