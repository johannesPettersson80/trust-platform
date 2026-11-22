//! Specification 11, "A mesh links only to the peers it names": a mesh connects to its `connect`
//! entries and accepts connections on its `listen` endpoint, and looks for no other mesh.
use super::security_tests::{config, mesh_on_a_free_port, STABLE_ID};
use crate::scheduler::{ResourceControl, StdClock};
use std::time::{Duration, Instant};
use zenoh::Wait;

#[test]
fn a_mesh_neither_scouts_the_network_nor_follows_its_peers_peers() {
    // Catches Zenoh's default scouting left on: by multicast the mesh linked to every mesh that
    // answered on the network (two runtimes with no `connect` entries linked, G244), and by gossip
    // to every peer a linked peer knew. A multicast answer cannot be relied on in a test, so the
    // setting is read from the configuration the runtime opens its session with.
    let built =
        super::startup_publish::build_zenoh_config(&config("127.0.0.1:0".into(), Vec::new(), None))
            .expect("the mesh's Zenoh configuration");
    for scouting in ["scouting/multicast/enabled", "scouting/gossip/enabled"] {
        assert_eq!(
            built.get_json(scouting).expect("the setting").as_str(),
            "false",
            "{scouting}"
        );
    }
}

#[test]
fn a_mesh_links_only_to_the_peers_its_connect_entries_name() {
    // A and C each name only B in `connect`. Catches gossip left on: B told A about C, and A linked
    // to C, which neither names (both listen on loopback, so no multicast is needed for this).
    let (resource, _commands) = ResourceControl::stub(StdClock::new());
    let (b, address) = mesh_on_a_free_port(|address| {
        super::start_mesh(
            &config(address.to_string(), Vec::new(), None),
            format!("peers-b-{}", std::process::id()).into(),
            STABLE_ID.into(),
            resource.clone(),
        )
        .expect("B's mesh")
        .expect("a mesh")
    });
    let peer_of_b = |name: &str| {
        let (resource, _commands) = ResourceControl::stub(StdClock::new());
        super::start_mesh(
            &config("127.0.0.1:0".into(), vec![address.to_string()], None),
            format!("peers-{name}-{}", std::process::id()).into(),
            STABLE_ID.into(),
            resource,
        )
        .expect("the peer's mesh")
        .expect("a mesh")
    };
    let a = peer_of_b("a");
    let c = peer_of_b("c");
    let zid = |mesh: &super::MeshService| {
        mesh.session
            .as_ref()
            .unwrap_or_else(|| panic!("a session: {:?}", mesh.degraded_reason()))
            .info()
            .zid()
            .wait()
            .to_string()
    };
    let linked = |mesh: &super::MeshService| -> Vec<String> {
        let mut peers: Vec<String> = mesh
            .session
            .as_ref()
            .expect("a session")
            .info()
            .peers_zid()
            .wait()
            .map(|id| id.to_string())
            .collect();
        peers.sort();
        peers
    };
    let (zid_a, zid_b, zid_c) = (zid(&a), zid(&b), zid(&c));

    let links_to_b = || {
        linked(&a).contains(&zid_b)
            && linked(&c).contains(&zid_b)
            && linked(&b).contains(&zid_a)
            && linked(&b).contains(&zid_c)
    };
    let until = Instant::now() + Duration::from_secs(10);
    while !links_to_b() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        links_to_b(),
        "A and C link to B, which accepts both on its listen endpoint: A {:?}, B {:?}, C {:?}",
        linked(&a),
        linked(&b),
        linked(&c)
    );
    // Gossip links within a second of the first links; watch three.
    let watched = Instant::now() + Duration::from_secs(3);
    while Instant::now() < watched {
        assert_eq!(
            linked(&a),
            std::slice::from_ref(&zid_b),
            "A links only to B"
        );
        assert_eq!(
            linked(&c),
            std::slice::from_ref(&zid_b),
            "C links only to B"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
