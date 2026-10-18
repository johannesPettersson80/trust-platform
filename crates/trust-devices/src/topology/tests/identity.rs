//! Owner-scoped identity (specification 25 §5.6), ported from
//! `network-canvas-fleet-identity.test.ts`.

use serde_json::{json, Value};

use super::fixtures::{
    answered, distinct, endpoint_ids, external_ids, link_ids, runtime, runtime_ids, snapshot, Mesh,
};
use crate::topology::identity::{decode, encode, normalize};
use crate::topology::merge::merge_snapshots;
use crate::topology::{
    self, ManagedRuntime, ManagedState, Runtime, RuntimeReport, Topology, TopologyInputs,
};

/// A mesh report whose two containers both hold `RESOURCE` with the same mesh endpoint, beside a
/// bare runtime `AUX` with its own OPC UA link: the ownership test of the TypeScript suite.
fn ambiguous_report() -> Value {
    let mut report = Mesh::new(
        4,
        "host:ambiguous",
        "RESOURCE",
        "tcp/192.168.77.42:7447",
        "degraded",
        "ambiguous peer",
    )
    .report();
    let original = report["hosts"][0]["runtimes"][0].clone();
    let mut duplicate = original.clone();
    duplicate["name"] = json!("Second RESOURCE");
    report["hosts"][0]["runtimes"] = json!([{
        "runtime_id": "AUX", "name": "Auxiliary runtime", "mode": "online", "cycle_ms": 10,
        "health": "connected", "detail": "Running.", "source": "self",
        "endpoints": [{
            "id": "endpoint:AUX:opcua", "kind": "service", "protocol": "opcua_client",
            "name": "OPC UA client", "role": "client", "health": "connected",
            "detail": "Auxiliary OPC UA link ready.", "owned": true, "supports_test": false,
            "source": "self",
        }],
    }]);
    report["hosts"][0]["containers"] = json!([
        {"container_id": "Cell A", "name": "Cell A", "image": "runtime:latest",
         "status": "running", "runtimes": [original], "source": "self"},
        {"container_id": "Cell B", "name": "Cell B", "image": "runtime:latest",
         "status": "running", "runtimes": [duplicate], "source": "self"},
    ]);
    report["links"].as_array_mut().expect("links").push(json!({
        "id": "link:opcua:aux", "from": "endpoint:AUX:opcua", "to": "external:opcua:aux",
        "protocol": "opcua_client", "role": "client", "direction": "outbound",
        "same_host": false, "status": "connected", "secure": true,
        "detail": "Auxiliary OPC UA peer ready.",
    }));
    report["external"]
        .as_array_mut()
        .expect("external")
        .push(json!({
            "id": "external:opcua:aux", "kind": "server", "name": "opc.tcp://aux.example:4840",
            "via_protocol": ["opcua_client"], "direction": "outbound", "source": "config",
        }));
    report["shared"][0]["used_by"]
        .as_array_mut()
        .expect("used_by")
        .push(json!("AUX"));
    report
}

/// Every runtime, endpoint, container, external system and link of `merged` keeps its own
/// identity, every link names an endpoint and an external that appear, and nothing loses its
/// source.
fn assert_owners_stay_distinct(label: &str, merged: &Topology) {
    let endpoints = endpoint_ids(merged);
    let externals = external_ids(merged);
    let containers: Vec<String> = merged
        .hosts
        .iter()
        .flat_map(|host| host.containers.iter().map(|container| container.id.clone()))
        .collect();
    assert_eq!(runtime_ids(merged).len(), 2, "{label}");
    assert_eq!(distinct(&runtime_ids(merged)), 2, "{label}");
    assert_eq!(distinct(&endpoints), 2, "{label}");
    if !containers.is_empty() {
        assert_eq!(containers.len(), 2, "{label}");
        assert_eq!(distinct(&containers), 2, "{label}");
    }
    assert_eq!(externals.len(), 2, "{label}");
    assert_eq!(distinct(&externals), 2, "{label}");
    assert_eq!(merged.links.len(), 2, "{label}");
    assert_eq!(distinct(&link_ids(merged)), 2, "{label}");
    assert!(
        merged
            .links
            .iter()
            .all(|link| endpoints.contains(&link.from)),
        "{label}"
    );
    assert!(
        merged.links.iter().all(|link| externals.contains(&link.to)),
        "{label}"
    );
    assert!(merged
        .hosts
        .iter()
        .all(|host| host.source.as_deref() == Some("self")));
    assert!(merged
        .runtimes()
        .all(|runtime| runtime.source.as_deref() == Some("self")));
    assert!(merged
        .runtimes()
        .flat_map(|runtime| &runtime.endpoints)
        .all(|endpoint| endpoint.source.as_deref() == Some("self")));
    assert!(merged
        .external
        .iter()
        .all(|external| external.source.as_deref() == Some("config")));
    let mut used_by = merged.shared[0].used_by.clone();
    let mut runtimes = runtime_ids(merged);
    used_by.sort();
    runtimes.sort();
    assert_eq!(used_by, runtimes, "{label}");
}

#[test]
fn owner_tuple_encoding_is_injective() {
    // Tuples that a joined string, display sanitization, case folding or a truncated hash would
    // collapse into one another.
    let long = "x".repeat(4096);
    let longer = format!("{long}y");
    let tuples: [&[Option<&str>]; 16] = [
        &[Some("runtime"), Some("host:a"), None, Some("b")],
        &[Some("runtime"), Some("host"), None, Some("a:b")],
        &[Some("runtime"), Some("host:a"), Some("b"), Some("c")],
        &[Some("runtime"), Some("host:a"), Some("null"), Some("b")],
        &[Some("runtime"), Some("host:a"), Some(""), Some("b")],
        &[Some("runtime"), Some("host:a"), None, Some("Line_A")],
        &[Some("runtime"), Some("host:a"), None, Some("line-a")],
        &[Some("runtime"), Some("host:a"), None, Some("LINE_A")],
        &[Some("runtime"), Some("host:a"), None, Some("a\"b")],
        &[Some("runtime"), Some("host:a"), None, Some("a\\\"b")],
        &[Some("runtime"), Some("host:a"), None, Some("\u{e9}")],
        &[Some("runtime"), Some("host:a"), None, Some("e\u{301}")],
        &[Some("runtime"), Some("host:a"), None, Some("tab\there")],
        &[Some("endpoint"), Some("host:a"), None, Some("b"), Some("c")],
        &[Some("runtime"), Some("host:a"), None, Some(&long)],
        &[Some("runtime"), Some("host:a"), None, Some(&longer)],
    ];
    let encoded: Vec<String> = tuples.iter().map(|parts| encode(parts)).collect();
    assert_eq!(distinct(&encoded), tuples.len(), "{encoded:?}");
    for (parts, identity) in tuples.iter().zip(&encoded) {
        let body = identity
            .strip_prefix("trust-fleet-v1:")
            .expect("the prefix");
        assert!(
            body.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'),
            "{identity}"
        );
        let expected: Vec<Option<String>> =
            parts.iter().map(|part| part.map(str::to_owned)).collect();
        assert_eq!(decode(identity), Some(expected), "{identity}");
    }

    // The scheme of the panel before it moved to Rust, so canvas positions keep their keys.
    assert_eq!(
        encode(&[Some("runtime"), Some("pi"), None, Some("resource")]),
        "trust-fleet-v1:WyJydW50aW1lIiwicGkiLG51bGwsInJlc291cmNlIl0"
    );
    assert_eq!(
        encode(&[Some("container"), Some("host:x"), Some("Cell_A")]),
        "trust-fleet-v1:WyJjb250YWluZXIiLCJob3N0OngiLCJDZWxsX0EiXQ"
    );
    assert_eq!(
        decode("trust-fleet-v1:WyJob3N0IiwicGkiXQ"),
        Some(vec![Some("host".to_owned()), Some("pi".to_owned())])
    );

    // Values that are not identities: raw IDs, an empty or foreign body, JSON that is not an
    // array of strings and nulls, and a non-canonical encoding.
    for value in [
        "RESOURCE",
        "endpoint:RESOURCE:mesh",
        "trust-fleet-v1:",
        "trust-fleet-v1:!!",
        "trust-fleet-v1:e30",
        "trust-fleet-v1:WzFd",
        "trust-fleet-v1:e31",
        "trust-fleet-v2:WyJob3N0IiwicGkiXQ",
    ] {
        assert_eq!(decode(value), None, "{value}");
    }
}

/// The three scenarios of the TypeScript suite whose raw IDs collide under different owners.
fn same_raw_ids() -> [(&'static str, [Mesh; 2]); 3] {
    [
        (
            "same host, runtime IDs that collide after display sanitization",
            [
                Mesh::new(
                    4,
                    "host:line",
                    "Line_A",
                    "tcp/192.168.77.21:7447",
                    "degraded",
                    "line A degraded",
                ),
                Mesh::new(
                    4,
                    "host:line",
                    "line-a",
                    "tcp/192.168.77.22:7447",
                    "error",
                    "line B failed",
                ),
            ],
        ),
        (
            "same host, separate containers with the same runtime ID",
            [
                Mesh::new(
                    4,
                    "host:containers",
                    "RESOURCE",
                    "tcp/192.168.77.31:7447",
                    "degraded",
                    "container A degraded",
                )
                .in_container("Cell_A"),
                Mesh::new(
                    4,
                    "host:containers",
                    "RESOURCE",
                    "tcp/192.168.77.32:7447",
                    "error",
                    "container B failed",
                )
                .in_container("cell-a"),
            ],
        ),
        (
            "different hosts with the same container and runtime IDs",
            [
                Mesh::new(
                    4,
                    "host:container-a",
                    "RESOURCE",
                    "tcp/192.168.77.33:7447",
                    "degraded",
                    "host A container degraded",
                )
                .in_container("Cell"),
                Mesh::new(
                    4,
                    "host:container-b",
                    "RESOURCE",
                    "tcp/192.168.77.34:7447",
                    "error",
                    "host B container failed",
                )
                .in_container("Cell"),
            ],
        ),
    ]
}

#[test]
fn equal_raw_ids_under_different_owners_stay_distinct() {
    for (label, meshes) in &same_raw_ids() {
        let snapshots: Vec<Topology> = meshes.iter().map(|mesh| snapshot(&mesh.report())).collect();
        assert_owners_stay_distinct(label, &merge_snapshots(&snapshots));
    }
    several_runtimes_on_one_computer();
}

/// Several runtimes on one computer (owner, 17 October 2026; specification 34 ARCH-15): one host
/// holds each of them under its own runtime ID, reached through its own endpoint, and the
/// managed runtimes and the simulator join the same host.
fn several_runtimes_on_one_computer() {
    let inputs = TopologyInputs {
        peers: vec![
            answered(
                "tcp://192.168.10.31:5680",
                Mesh::new(
                    4,
                    "host:pi",
                    "LINE1",
                    "tcp/192.168.10.41:7447",
                    "connected",
                    "line 1",
                )
                .report(),
            ),
            answered(
                "tcp://192.168.10.31:5681",
                Mesh::new(
                    4,
                    "host:pi",
                    "LINE2",
                    "tcp/192.168.10.42:7447",
                    "connected",
                    "line 2",
                )
                .report(),
            ),
        ],
        managed: vec![ManagedRuntime {
            name: "line3".to_owned(),
            control_endpoint: Some("tcp://127.0.0.1:5682".to_owned()),
            state: ManagedState::Stopped,
        }],
        this_computer: Some("host:pi".to_owned()),
        ..TopologyInputs::default()
    };
    let merged = topology::merge(&inputs);
    assert_eq!(merged.hosts.len(), 1, "one computer, one host");
    let names: Vec<&str> = merged
        .runtimes()
        .map(|runtime| runtime.name.as_str())
        .collect();
    assert_eq!(names, ["Simulator", "LINE1", "LINE2", "line3"]);
    assert_eq!(distinct(&runtime_ids(&merged)), 4);
    assert_eq!(distinct(&endpoint_ids(&merged)), 2);
    assert_eq!(distinct(&external_ids(&merged)), 2);
    for (name, endpoint) in [
        ("LINE1", "tcp://192.168.10.31:5680"),
        ("LINE2", "tcp://192.168.10.31:5681"),
        ("line3", "tcp://127.0.0.1:5682"),
    ] {
        assert_eq!(
            runtime(&merged, name).control_endpoint.as_deref(),
            Some(endpoint)
        );
    }
}

/// The peer of the idempotence test: one runtime with one mesh link.
fn singleton_report() -> Value {
    Mesh::new(
        4,
        "host:singleton",
        "RESOURCE",
        "tcp/192.168.77.40:7447",
        "degraded",
        "singleton peer degraded",
    )
    .report()
}

#[test]
fn normalization_is_idempotent_and_leaves_its_input_unchanged() {
    let input = snapshot(&singleton_report());
    let before = input.clone();
    let once = merge_snapshots(std::slice::from_ref(&input));
    let twice = merge_snapshots(std::slice::from_ref(&once));
    assert_eq!(input, before, "normalization mutated its input");
    assert_eq!(twice, once, "normalization must be idempotent");
    let only = once.runtimes().next().expect("one runtime");
    assert_ne!(only.id, "RESOURCE");
    assert_ne!(only.endpoints[0].id, "endpoint:RESOURCE:mesh");
    assert_ne!(once.external[0].id, "external:mesh:0");
    assert_eq!(once.external[0].name, "tcp/192.168.77.40:7447");
    assert_eq!(
        once.links[0].detail.as_deref(),
        Some("singleton peer degraded")
    );
    assert!(once.links[0].secure);

    // The same holds for each schema version, and with ambiguous references.
    for schema in 2..=4 {
        let mesh = Mesh::new(
            schema,
            "host:a",
            "RESOURCE",
            "tcp/peer:7447",
            "connected",
            "ok",
        );
        let normalized = normalize(&snapshot(&mesh.report()));
        assert_eq!(normalize(&normalized), normalized, "schema {schema}");
    }
    let normalized = normalize(&snapshot(&ambiguous_report()));
    assert_eq!(normalize(&normalized), normalized);
    adding_a_peer_changes_no_identity();
}

/// A lone report gets the identities it gets beside another report: adding a peer changes no
/// identity. Its own runtime is told apart by the endpoint it was reached through.
fn adding_a_peer_changes_no_identity() {
    let endpoint = "tcp://192.168.77.40:5680";
    let other = Mesh::new(
        4,
        "host:other",
        "RESOURCE",
        "tcp/192.168.77.50:7447",
        "connected",
        "other peer ready",
    )
    .report();
    let alone = topology::merge(&TopologyInputs {
        peers: vec![answered(endpoint, singleton_report())],
        ..TopologyInputs::default()
    });
    let together = topology::merge(&TopologyInputs {
        peers: vec![
            answered("tcp://192.168.77.50:5680", other),
            answered(endpoint, singleton_report()),
        ],
        ..TopologyInputs::default()
    });
    let singleton = |merged: &Topology| {
        let host = merged
            .hosts
            .iter()
            .find(|host| host.id == "host:singleton")
            .cloned()
            .expect("the reported host");
        let external = merged
            .external
            .iter()
            .find(|external| external.name == "tcp/192.168.77.40:7447")
            .cloned()
            .expect("the reported mesh peer");
        let link = merged
            .links
            .iter()
            .find(|link| link.detail.as_deref() == Some("singleton peer degraded"))
            .cloned()
            .expect("the reported mesh link");
        (host, external, link)
    };
    assert_eq!(singleton(&alone), singleton(&together));
    let parts = decode(&singleton(&alone).0.runtimes[0].id).expect("a runtime identity");
    let owned = |part: &str| Some(part.to_owned());
    assert_eq!(
        parts,
        [
            owned("runtime"),
            owned("host:singleton"),
            None,
            owned("RESOURCE"),
            owned(endpoint),
        ]
    );
}

/// Two runtimes on one computer that report one resource name (specification 34 ARCH-15; lead
/// decision O10): a stable runtime ID tells them apart when they report one, and the endpoint
/// each was reached through when they do not, which is every runtime today. Snapshots of the same
/// runtime still deduplicate.
#[test]
fn two_runtimes_with_one_resource_name_on_one_computer_stay_distinct() {
    let (first, second) = ("tcp://192.168.10.31:5680", "tcp://192.168.10.31:5681");
    let reached = |endpoint: &str, stable_id: Option<&str>, peer: &'static str| RuntimeReport {
        stable_id: stable_id.map(str::to_owned),
        ..answered(
            endpoint,
            Mesh::new(4, "host:pi", "RESOURCE", peer, "connected", "ready").report(),
        )
    };
    let round = |peers: Vec<RuntimeReport>| {
        topology::merge(&TopologyInputs {
            peers,
            this_computer: Some("host:pi".to_owned()),
            ..TopologyInputs::default()
        })
    };
    let (peer_a, peer_b) = ("tcp/192.168.10.41:7447", "tcp/192.168.10.42:7447");

    // By the endpoint: two endpoints, two runtimes; one endpoint twice, one runtime.
    let two = round(vec![
        reached(first, None, peer_a),
        reached(second, None, peer_b),
    ]);
    assert_two_resources(&two, [first, second], [first, second]);
    let one = round(vec![
        reached(first, None, peer_a),
        reached(first, None, peer_a),
    ]);
    assert_eq!(
        resources(&one).len(),
        1,
        "the same runtime twice is one runtime"
    );

    // By the stable ID: two IDs, two runtimes; one ID through two endpoints, one runtime.
    let two = round(vec![
        reached(first, Some("line-1"), peer_a),
        reached(second, Some("line-2"), peer_b),
    ]);
    assert_two_resources(&two, [first, second], ["line-1", "line-2"]);
    let one = round(vec![
        reached(first, Some("line-1"), peer_a),
        reached(second, Some("line-1"), peer_a),
    ]);
    assert_eq!(resources(&one).len(), 1, "one stable ID is one runtime");
}

fn resources(merged: &Topology) -> Vec<&Runtime> {
    merged
        .runtimes()
        .filter(|runtime| runtime.name == "RESOURCE")
        .collect()
}

/// Two runtimes named `RESOURCE` on one host, reached through `endpoints` and told apart by
/// `instances`, each with its own identity, endpoint, mesh peer and link, and both in the shared
/// broker's `used_by`.
fn assert_two_resources(merged: &Topology, endpoints: [&str; 2], instances: [&str; 2]) {
    assert_eq!(merged.hosts.len(), 1, "one computer, one host");
    let runtimes = resources(merged);
    assert_eq!(runtimes.len(), 2, "two runtimes, not one");
    assert_ne!(runtimes[0].id, runtimes[1].id);
    let reached: Vec<Option<&str>> = runtimes
        .iter()
        .map(|runtime| runtime.control_endpoint.as_deref())
        .collect();
    assert_eq!(reached, endpoints.map(Some));
    let told_apart: Vec<Option<&str>> = runtimes
        .iter()
        .map(|runtime| runtime.instance.as_deref())
        .collect();
    assert_eq!(told_apart, instances.map(Some));
    assert_eq!(distinct(&endpoint_ids(merged)), 2);
    assert_eq!(distinct(&external_ids(merged)), 2);
    assert_eq!(distinct(&link_ids(merged)), 2);
    let mut used_by = merged.shared[0].used_by.clone();
    let mut ids: Vec<String> = runtimes.iter().map(|runtime| runtime.id.clone()).collect();
    used_by.sort();
    ids.sort();
    assert_eq!(used_by, ids);
}

#[test]
fn link_ids_are_scoped_for_schema_2_3_and_4() {
    for schema in 2..=4 {
        let first = Mesh::new(
            schema,
            "host:line-a",
            "RESOURCE",
            "tcp/192.168.77.11:7447",
            "degraded",
            "peer A degraded",
        );
        let second = Mesh::new(
            schema,
            "host:line-b",
            "RESOURCE",
            "tcp/192.168.77.12:7447",
            "error",
            "peer B failed",
        );
        let merged = merge_snapshots(&[snapshot(&first.report()), snapshot(&second.report())]);
        let endpoints = endpoint_ids(&merged);
        let externals = external_ids(&merged);
        assert_eq!(merged.hosts.len(), 2, "schema {schema}");
        assert_eq!(distinct(&runtime_ids(&merged)), 2, "schema {schema}");
        assert_eq!(distinct(&endpoints), 2, "schema {schema}");
        assert_eq!(distinct(&externals), 2, "schema {schema}");
        assert_eq!(merged.links.len(), 2, "schema {schema}");
        assert_eq!(distinct(&link_ids(&merged)), 2, "schema {schema}");
        assert!(merged
            .links
            .iter()
            .all(|link| endpoints.contains(&link.from)));
        assert!(merged.links.iter().all(|link| externals.contains(&link.to)));
        let names: Vec<&str> = merged
            .runtimes()
            .map(|runtime| runtime.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["RESOURCE", "RESOURCE"],
            "schema {schema} changed runtime names"
        );
        let mut peers: Vec<&str> = merged
            .external
            .iter()
            .map(|external| external.name.as_str())
            .collect();
        peers.sort_unstable();
        assert_eq!(peers, ["tcp/192.168.77.11:7447", "tcp/192.168.77.12:7447"]);
        let mut links: Vec<(&str, Option<&str>, bool)> = merged
            .links
            .iter()
            .map(|link| (link.status.as_str(), link.detail.as_deref(), link.secure))
            .collect();
        links.sort_by_key(|link| link.1);
        assert_eq!(
            links,
            [
                ("degraded", Some("peer A degraded"), true),
                ("error", Some("peer B failed"), true)
            ]
        );

        // The scoped identity keeps the raw wire identity: version 2 links have no ID.
        let parts =
            decode(merged.links[0].id.as_deref().unwrap_or_default()).expect("a link identity");
        assert_eq!(parts[0].as_deref(), Some("link"));
        let wire_id =
            (schema >= 3).then_some("link:mesh:peer:endpoint-resource-mesh:external-mesh-0");
        assert_eq!(parts[1].as_deref(), wire_id, "schema {schema}");
        assert_eq!(parts[2].as_deref(), Some("endpoint:RESOURCE:mesh"));
        assert_eq!(parts[3].as_deref(), Some("external:mesh:0"));
    }
    cross_runtime_link_scopes_each_end();
}

/// A link between two runtimes resolves and scopes each end on its own.
fn cross_runtime_link_scopes_each_end() {
    let left = Mesh::new(
        4,
        "host:cross-runtime",
        "LEFT",
        "tcp/192.168.77.51:7447",
        "connected",
        "left ready",
    )
    .report();
    let right = Mesh::new(
        4,
        "host:cross-runtime",
        "RIGHT",
        "tcp/192.168.77.52:7447",
        "connected",
        "right ready",
    )
    .report();
    let raw_from = left["hosts"][0]["runtimes"][0]["endpoints"][0]["id"].clone();
    let raw_to = right["hosts"][0]["runtimes"][0]["endpoints"][0]["id"].clone();
    let mut host = left["hosts"][0].clone();
    host["runtimes"] = json!([
        left["hosts"][0]["runtimes"][0],
        right["hosts"][0]["runtimes"][0]
    ]);
    let report = json!({
        "schema_version": 4, "hosts": [host], "shared": [], "external": [],
        "links": [{
            "id": "link:cross-runtime", "from": raw_from, "to": raw_to, "protocol": "mesh",
            "role": "peer", "direction": "bidirectional", "same_host": true,
            "status": "connected", "secure": true, "detail": "Cross-runtime mesh route ready.",
        }],
    });
    let merged = merge_snapshots(&[snapshot(&report)]);
    let endpoints = endpoint_ids(&merged);
    assert_eq!(merged.links.len(), 1);
    let link = &merged.links[0];
    assert!(endpoints.contains(&link.from) && endpoints.contains(&link.to));
    assert_ne!(link.from, raw_from.as_str().unwrap_or_default());
    assert_ne!(link.to, raw_to.as_str().unwrap_or_default());
    assert_ne!(link.id.as_deref(), Some("link:cross-runtime"));
}

#[test]
fn ambiguous_reference_drops_only_its_link() {
    let input = snapshot(&ambiguous_report());
    let before = input.clone();
    let merged = merge_snapshots(std::slice::from_ref(&input));
    assert_eq!(input, before, "fail-closed normalization mutated its input");
    assert_eq!(runtime_ids(&merged).len(), 3);
    assert_eq!(distinct(&runtime_ids(&merged)), 3);
    assert_eq!(
        merged.links.len(),
        1,
        "the uniquely owned AUX link must survive"
    );
    assert_eq!(merged.links[0].protocol, "opcua_client");
    assert_eq!(
        merged.external.len(),
        1,
        "the unrelated AUX external must survive"
    );
    assert_eq!(merged.external[0].name, "opc.tcp://aux.example:4840");
    assert_eq!(
        merged.shared.len(),
        1,
        "the global shared system remains visible"
    );
    let aux = runtime(&merged, "Auxiliary runtime");
    assert_eq!(merged.shared[0].used_by, std::slice::from_ref(&aux.id));
}

#[test]
fn mesh_external_binds_to_one_owner_or_is_omitted() {
    // One owner: the configured peer is scoped by the runtime whose mesh endpoint links to it.
    let mesh = Mesh::new(
        4,
        "host:line-a",
        "RESOURCE",
        "tcp/192.168.77.11:7447",
        "degraded",
        "peer A",
    );
    let merged = merge_snapshots(&[snapshot(&mesh.report())]);
    assert_eq!(
        merged.external[0].id,
        "trust-fleet-v1:WyJtZXNoLWV4dGVybmFsIiwiaG9zdDpsaW5lLWEiLG51bGwsIlJFU09VUkNFIiwiZXh0ZXJuYWw6bWVzaDowIl0"
    );
    assert_eq!(merged.links[0].to, merged.external[0].id);

    // A peer that an endpoint of more than one possible owner links to is omitted, while the
    // unrelated external system stays global.
    let merged = merge_snapshots(&[snapshot(&ambiguous_report())]);
    assert!(!merged
        .external
        .iter()
        .any(|external| external.name == "tcp/192.168.77.42:7447"));
    assert_eq!(merged.external[0].id, "external:opcua:aux");

    // A peer that two runtimes link to is omitted together with its links.
    let left = Mesh::new(
        4,
        "host:pair",
        "LEFT",
        "tcp/192.168.77.60:7447",
        "connected",
        "left",
    )
    .report();
    let right = Mesh::new(
        4,
        "host:pair",
        "RIGHT",
        "tcp/192.168.77.60:7447",
        "connected",
        "right",
    )
    .report();
    let mut report = left.clone();
    report["hosts"][0]["runtimes"] = json!([
        left["hosts"][0]["runtimes"][0],
        right["hosts"][0]["runtimes"][0]
    ]);
    report["links"] = json!([left["links"][0], right["links"][0]]);
    let merged = merge_snapshots(&[snapshot(&report)]);
    assert_eq!(
        merged.external,
        Vec::<trust_runtime::fleet_topology::FleetExternal>::new()
    );
    assert_eq!(
        merged.links,
        Vec::<trust_runtime::fleet_topology::FleetLink>::new()
    );
    assert_eq!(runtime_ids(&merged).len(), 2);

    // A peer that no link pairs with an endpoint keeps its global identity.
    let mut report = mesh.report();
    report["links"] = json!([]);
    let merged = merge_snapshots(&[snapshot(&report)]);
    assert_eq!(merged.external[0].id, "external:mesh:0");
}

#[test]
fn used_by_rewritten_to_owner_tuples() {
    let bare = |id: &str| {
        json!({
            "runtime_id": id, "name": id, "mode": "online", "cycle_ms": 10, "health": "connected",
            "detail": "", "endpoints": [],
        })
    };
    let report_of = |runtimes: Value, used_by: Value| {
        json!({
            "schema_version": 3, "links": [], "external": [],
            "hosts": [{"host_id": "H", "hostname": "H", "arch": "x", "os": "linux", "ips": [],
                       "containers": [], "runtimes": runtimes}],
            "shared": [{"id": "S1", "kind": "broker", "name": "b", "address": "x",
                        "used_by": used_by}],
        })
    };
    let merged = merge_snapshots(&[
        snapshot(&report_of(json!([bare("A")]), json!(["A"]))),
        snapshot(&report_of(json!([bare("B")]), json!(["B"]))),
    ]);
    let id = |name: &str| encode(&[Some("runtime"), Some("H"), None, Some(name)]);
    assert_eq!(merged.shared[0].used_by, [id("A"), id("B")]);

    // A reference with two possible owners and an unknown reference leave the list; a repeated
    // one appears once.
    let mut ambiguous = report_of(json!([bare("A")]), json!(["R", "ghost", "A", "A"]));
    ambiguous["hosts"][0]["containers"] = json!([
        {"container_id": "c1", "name": "c1", "image": "i", "status": "running",
         "runtimes": [bare("R")]},
        {"container_id": "c2", "name": "c2", "image": "i", "status": "running",
         "runtimes": [bare("R")]},
    ]);
    let merged = merge_snapshots(&[snapshot(&ambiguous)]);
    assert_eq!(merged.shared[0].used_by, [id("A")]);
}
