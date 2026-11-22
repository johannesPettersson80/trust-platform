//! Specification 11, "Runtime Cloud sites", on the mesh: a runtime's keys are under its own site,
//! and it names its peers relative to that site, by their resource names in its own site and by
//! `<site>/<resource>` in another.

use std::time::{Duration, Instant};

use super::mapping::{liveliness_peer, peer_data_key};
use super::security_tests::{config, mesh_on_a_free_port, STABLE_ID};
use crate::config::MeshConfig;
use crate::scheduler::{ResourceControl, StdClock};

/// Catches a peer of another site named by its bare resource name, which a runtime of the own site
/// with that name would shadow, and a key of another shape taken for a peer.
#[test]
fn a_liveliness_key_names_its_runtime_relative_to_the_own_site() {
    let key = "truST/site-b/runtime_b/svc/liveliness/runtime_b";
    assert_eq!(
        liveliness_peer(key, "default-site").as_deref(),
        Some("site-b/runtime_b")
    );
    assert_eq!(liveliness_peer(key, "site-b").as_deref(), Some("runtime_b"));
    assert_eq!(
        liveliness_peer(
            "truST/default-site/runtime_a/svc/liveliness/runtime_a",
            "site-b"
        )
        .as_deref(),
        Some("default-site/runtime_a")
    );
    assert_eq!(
        liveliness_peer("truST/site-b/runtime_b/svc/other/runtime_b", "site-b"),
        None
    );
    assert_eq!(
        liveliness_peer("other/site-b/runtime_b/svc/liveliness/runtime_b", "site-b"),
        None
    );
}

/// Catches a subscription that reads its peer's key in the subscriber's site: `peer:key` reads the
/// key under the site the peer's name gives, the subscriber's own for a bare name.
#[test]
fn a_subscription_reads_the_key_under_the_site_its_peer_names() {
    assert_eq!(
        peer_data_key("default-site", "site-b/runtime_b", "Pressure"),
        "truST/site-b/runtime_b/mesh/data/Pressure"
    );
    assert_eq!(
        peer_data_key("site-b", "runtime_b", "Pressure"),
        "truST/site-b/runtime_b/mesh/data/Pressure"
    );
    assert_eq!(
        peer_data_key("site-b", "default-site/runtime_a", "Level"),
        "truST/default-site/runtime_a/mesh/data/Level"
    );
}

/// The peers `mesh` names, waiting up to ten seconds until `peer` is among them.
fn peers_once_seen(mesh: &super::MeshService, peer: &str) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let peers = mesh.liveliness_snapshot().peers;
        if peers.iter().any(|seen| seen == peer) || Instant::now() >= deadline {
            return peers;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Catches a mesh that keys itself in the default site whatever its site: B of `site-b` and A of
/// the default site, linked, see each other by the names their sites give, and A's identity query
/// of `site-b/<B>` reaches B's queryable under `truST/site-b/<B>`, whose answer names `site-b`.
#[test]
fn meshes_of_two_sites_key_themselves_under_their_sites_and_name_each_other_by_them() {
    let b_name = format!("site_b_runtime{}", std::process::id());
    let a_name = format!("default_runtime{}", std::process::id());
    let (b_resource, _b_commands) = ResourceControl::stub(StdClock::new());
    let (b, address) = mesh_on_a_free_port(|address| {
        super::start_mesh(
            &MeshConfig {
                site: Some("site-b".into()),
                ..config(address.to_string(), Vec::new(), None)
            },
            b_name.as_str().into(),
            STABLE_ID.into(),
            b_resource.clone(),
        )
        .expect("B's mesh")
        .expect("a mesh")
    });
    let (a_resource, _a_commands) = ResourceControl::stub(StdClock::new());
    let a = super::start_mesh(
        &config("127.0.0.1:0".into(), vec![address.to_string()], None),
        a_name.as_str().into(),
        STABLE_ID.into(),
        a_resource,
    )
    .expect("A's mesh")
    .expect("a mesh");
    assert_eq!(b.site(), "site-b");
    assert_eq!(a.site(), "default-site");

    let b_as_a_names_it = format!("site-b/{b_name}");
    let seen_by_a = peers_once_seen(&a, &b_as_a_names_it);
    assert!(seen_by_a.contains(&b_as_a_names_it), "{seen_by_a:?}");
    assert!(!seen_by_a.contains(&b_name), "{seen_by_a:?}");
    let a_as_b_names_it = format!("default-site/{a_name}");
    let seen_by_b = peers_once_seen(&b, &a_as_b_names_it);
    assert!(seen_by_b.contains(&a_as_b_names_it), "{seen_by_b:?}");

    let session = a.session.as_ref().expect("A's session");
    let answers = super::query_identities(
        session,
        a.site(),
        std::slice::from_ref(&b_as_a_names_it),
        Duration::from_secs(5),
    );
    let (_, answer) = answers
        .into_iter()
        .next()
        .expect("B answers A's identity query");
    let answer = answer.expect("the answer is B's identity");
    assert_eq!(answer.identity.runtime_id, b_name);
    assert_eq!(answer.identity.site, "site-b");
}
