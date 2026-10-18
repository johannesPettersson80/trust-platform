use serde_json::{json, Value};

use super::{
    FleetContainer, FleetDiscovered, FleetEndpoint, FleetEndpointChild, FleetExternal, FleetHost,
    FleetLink, FleetRuntime, FleetShared, FleetTopology, TopologyDecodeError,
    FLEET_TOPOLOGY_SCHEMA_VERSION,
};

const SEEN_MS: u64 = 1_760_692_130_112;

/// The report of `legacyMeshSnapshot` in `network-canvas-fleet-identity.test.ts`, with that
/// test's host, runtime, peer and texts: what a runtime of each schema version sends. Version 3
/// adds the link's `id` and `role`; version 4 adds `source` everywhere and the endpoint's `role`.
fn legacy_mesh_report(schema_version: u64, container: Option<&str>) -> Value {
    let mut endpoint = json!({
        "id": "endpoint:RESOURCE:mesh", "kind": "peer", "protocol": "mesh", "name": "Mesh / Zenoh",
        "health": "degraded", "detail": "peer A degraded", "owned": true, "supports_test": false,
    });
    let mut runtime = json!({
        "runtime_id": "RESOURCE", "name": "RESOURCE", "mode": "online", "cycle_ms": 10,
        "health": "connected", "detail": "Running.",
    });
    let mut link = json!({
        "from": "endpoint:RESOURCE:mesh", "to": "external:mesh:0", "protocol": "mesh",
        "direction": "outbound", "same_host": false, "status": "degraded", "secure": true,
        "detail": "peer A degraded",
    });
    let mut external = json!({
        "id": "external:mesh:0", "kind": "peer", "name": "tcp/192.168.77.11:7447",
        "via_protocol": ["mesh"], "direction": "outbound",
    });
    let mut host = json!({
        "host_id": "host:line-a", "hostname": "host:line-a", "arch": "x86_64", "os": "linux",
        "ips": [], "containers": [], "runtimes": [],
    });
    let v4 = schema_version == 4;
    if schema_version >= 3 {
        link["id"] = json!("link:mesh:peer:endpoint-resource-mesh:external-mesh-0");
        link["role"] = json!("peer");
    }
    if v4 {
        endpoint["role"] = json!("peer");
        endpoint["source"] = json!("self");
        runtime["source"] = json!("self");
        host["source"] = json!("self");
        external["source"] = json!("config");
    }
    runtime["endpoints"] = json!([endpoint]);
    match container {
        Some(container_id) => {
            let mut container = json!({
                "container_id": container_id, "name": container_id, "image": "runtime:latest",
                "status": "running", "runtimes": [runtime],
            });
            if v4 {
                container["source"] = json!("self");
            }
            host["containers"] = json!([container]);
        }
        None => host["runtimes"] = json!([runtime]),
    }
    json!({
        "schema_version": schema_version,
        "hosts": [host],
        "links": [link],
        "shared": [{
            "id": "shared:mqtt:broker", "kind": "broker", "name": "MQTT broker",
            "address": "127.0.0.1:1883", "used_by": ["RESOURCE"],
        }],
        "external": [external],
    })
}

/// A runtime whose optional fields are all absent, with one fieldbus device whose optional
/// fields are absent too.
fn minimal_runtime() -> FleetRuntime {
    FleetRuntime {
        runtime_id: "RESOURCE".into(),
        name: "RESOURCE".into(),
        control_endpoint: None,
        web_listen: None,
        mode: "stopped".into(),
        cycle_ms: 10,
        load: None,
        health: "configured_policy".into(),
        detail: "Configured in project files; runtime is not running.".into(),
        endpoints: vec![FleetEndpoint {
            id: "endpoint:RESOURCE:ethercat:1".into(),
            kind: "field".into(),
            protocol: "ethercat".into(),
            name: "EtherCAT 2".into(),
            address: None,
            role: None,
            health: "configured_policy".into(),
            detail: "Configured in io.toml; runtime is not running.".into(),
            live: None,
            params: None,
            children: vec![FleetEndpointChild {
                id: "ethercat:slot:2".into(),
                kind: "field_slave".into(),
                name: "slot 2".into(),
                slot: None,
                model: None,
                channels: None,
                health: "configured_policy".into(),
                detail: "Configured in io.toml; runtime is not running.".into(),
                source: None,
            }],
            owned: true,
            supports_test: false,
            source: None,
        }],
        source: None,
        last_seen_ms: None,
    }
}

fn minimal_runtime_json() -> Value {
    json!({
        "runtime_id": "RESOURCE", "name": "RESOURCE", "mode": "stopped", "cycle_ms": 10,
        "health": "configured_policy",
        "detail": "Configured in project files; runtime is not running.",
        "endpoints": [{
            "id": "endpoint:RESOURCE:ethercat:1", "kind": "field", "protocol": "ethercat",
            "name": "EtherCAT 2", "health": "configured_policy",
            "detail": "Configured in io.toml; runtime is not running.",
            "children": [{
                "id": "ethercat:slot:2", "kind": "field_slave", "name": "slot 2",
                "health": "configured_policy",
                "detail": "Configured in io.toml; runtime is not running.",
            }],
            "owned": true, "supports_test": false,
        }],
    })
}

/// A report with every field of every kind of node present.
fn full_topology() -> FleetTopology {
    FleetTopology {
        schema_version: FLEET_TOPOLOGY_SCHEMA_VERSION,
        hosts: vec![FleetHost {
            host_id: "host:line-a".into(),
            hostname: "line-a".into(),
            board: Some("Raspberry Pi 5".into()),
            arch: "aarch64".into(),
            os: "linux".into(),
            ips: vec!["192.168.10.31".into()],
            temp_c: Some(48.5),
            uptime_s: Some(3_600),
            load: Some(0.5),
            containers: vec![FleetContainer {
                container_id: "Cell_A".into(),
                name: "cell-a".into(),
                image: "runtime:latest".into(),
                status: "running".into(),
                runtimes: vec![minimal_runtime()],
                restart_policy: Some("always".into()),
                cpu_limit: Some("2".into()),
                mem_limit: Some("512m".into()),
                mounts: Some(vec!["/data".into()]),
                source: Some("self".into()),
            }],
            runtimes: vec![FleetRuntime {
                runtime_id: "RESOURCE".into(),
                name: "RESOURCE".into(),
                control_endpoint: Some("tcp://127.0.0.1:9901".into()),
                web_listen: Some("0.0.0.0:8080".into()),
                mode: "online".into(),
                cycle_ms: 10,
                load: Some(0.25),
                health: "connected".into(),
                detail: "Running.".into(),
                endpoints: vec![FleetEndpoint {
                    id: "endpoint:RESOURCE:ethercat".into(),
                    kind: "field".into(),
                    protocol: "ethercat".into(),
                    name: "EtherCAT".into(),
                    address: Some("eth0".into()),
                    role: Some("master".into()),
                    health: "connected".into(),
                    detail: "Running.".into(),
                    live: Some(json!({"rtt_ms": 3})),
                    params: Some(json!({"adapter": "eth0"})),
                    children: vec![FleetEndpointChild {
                        id: "ethercat:slot:1".into(),
                        kind: "field_slave".into(),
                        name: "EL1008 (slot 1)".into(),
                        slot: Some(1),
                        model: Some("EL1008".into()),
                        channels: Some(8),
                        health: "configured_policy".into(),
                        detail: "Configured in io.toml.".into(),
                        source: Some("config".into()),
                    }],
                    owned: true,
                    supports_test: false,
                    source: Some("self".into()),
                }],
                source: Some("self".into()),
                last_seen_ms: Some(SEEN_MS),
            }],
            source: Some("self".into()),
            last_seen_ms: Some(SEEN_MS),
        }],
        links: vec![FleetLink {
            id: Some("link:mesh:peer:endpoint-resource-mesh:external-mesh-0".into()),
            from: "endpoint:RESOURCE:mesh".into(),
            to: "external:mesh:0".into(),
            protocol: "mesh".into(),
            role: Some("peer".into()),
            direction: "outbound".into(),
            same_host: false,
            status: "degraded".into(),
            secure: true,
            detail: Some("peer A degraded".into()),
        }],
        shared: vec![FleetShared {
            id: "shared:mqtt:broker".into(),
            kind: "broker".into(),
            name: "MQTT broker".into(),
            address: "127.0.0.1:1883".into(),
            used_by: vec!["RESOURCE".into()],
        }],
        external: vec![FleetExternal {
            id: "external:mesh:0".into(),
            kind: "peer".into(),
            name: "tcp/192.168.77.11:7447".into(),
            via_protocol: vec!["mesh".into()],
            direction: "outbound".into(),
            source: Some("config".into()),
        }],
        discovered: vec![FleetDiscovered {
            id: "external:discovery:peer".into(),
            kind: "runtime".into(),
            name: "peer".into(),
            addresses: vec!["192.0.2.10".into()],
            via_protocol: vec!["discovery".into()],
            direction: "observed".into(),
            adopted: false,
            control: Some("tcp://192.0.2.10:5680".into()),
            web_port: Some(8080),
            web_tls: Some(false),
            mesh_port: Some(7447),
            host_group: Some("line".into()),
            last_seen_ms: Some(10),
            source: Some("discovery".into()),
        }],
    }
}

/// A report with every optional field absent and every omissible array empty, except
/// `discovered`, whose entry shows its own omissions.
fn minimal_topology() -> FleetTopology {
    FleetTopology {
        schema_version: FLEET_TOPOLOGY_SCHEMA_VERSION,
        hosts: vec![FleetHost {
            host_id: "host:line-a".into(),
            hostname: "line-a".into(),
            board: None,
            arch: "aarch64".into(),
            os: "linux".into(),
            ips: Vec::new(),
            temp_c: None,
            uptime_s: None,
            load: None,
            containers: vec![FleetContainer {
                container_id: "Cell_A".into(),
                name: "cell-a".into(),
                image: "unknown".into(),
                status: "running".into(),
                runtimes: Vec::new(),
                restart_policy: None,
                cpu_limit: None,
                mem_limit: None,
                mounts: None,
                source: None,
            }],
            runtimes: vec![minimal_runtime()],
            source: None,
            last_seen_ms: None,
        }],
        links: vec![FleetLink {
            id: None,
            from: "endpoint:RESOURCE:mesh".into(),
            to: "external:mesh:0".into(),
            protocol: "mesh".into(),
            role: None,
            direction: "outbound".into(),
            same_host: false,
            status: "degraded".into(),
            secure: true,
            detail: None,
        }],
        shared: Vec::new(),
        external: vec![FleetExternal {
            id: "external:mesh:0".into(),
            kind: "peer".into(),
            name: "tcp/192.168.77.11:7447".into(),
            via_protocol: Vec::new(),
            direction: "outbound".into(),
            source: None,
        }],
        discovered: vec![FleetDiscovered {
            id: "external:discovery:peer".into(),
            kind: "runtime".into(),
            name: "peer".into(),
            addresses: Vec::new(),
            via_protocol: Vec::new(),
            direction: "observed".into(),
            adopted: false,
            control: None,
            web_port: None,
            web_tls: None,
            mesh_port: None,
            host_group: None,
            last_seen_ms: None,
            source: None,
        }],
    }
}

/// Collects the JSON pointer of every value below the root.
fn collect_paths(value: &Value, at: &str, paths: &mut Vec<String>) {
    let children: Vec<(String, &Value)> = match value {
        Value::Object(map) => map
            .iter()
            .map(|(key, child)| (format!("{at}/{key}"), child))
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, child)| (format!("{at}/{index}"), child))
            .collect(),
        _ => Vec::new(),
    };
    for (path, child) in children {
        collect_paths(child, &path, paths);
        paths.push(path);
    }
}

/// `value` without the field or array element at `path`.
fn without(value: &Value, path: &str) -> Value {
    let mut copy = value.clone();
    if let Some((parent, last)) = path.rsplit_once('/') {
        match copy.pointer_mut(parent) {
            Some(Value::Object(map)) => {
                map.remove(last);
            }
            Some(Value::Array(items)) => {
                if let Ok(index) = last.parse::<usize>()
                    && index < items.len()
                {
                    items.remove(index);
                }
            }
            _ => {}
        }
    }
    copy
}

#[test]
fn topology_decodes_schema_2_3_and_4() {
    for version in 2..=4 {
        let topology = FleetTopology::decode(&legacy_mesh_report(version, None))
            .unwrap_or_else(|error| panic!("schema version {version}: {error}"));
        assert_eq!(u64::from(topology.schema_version), version);
        let host = &topology.hosts[0];
        let runtime = &host.runtimes[0];
        let endpoint = &runtime.endpoints[0];
        let link = &topology.links[0];
        assert_eq!(host.host_id, "host:line-a");
        assert_eq!(runtime.runtime_id, "RESOURCE");
        assert_eq!(endpoint.id, "endpoint:RESOURCE:mesh");
        assert_eq!(endpoint.health, "degraded");
        assert_eq!(link.from, "endpoint:RESOURCE:mesh");
        assert_eq!(link.to, "external:mesh:0");
        assert_eq!(link.detail.as_deref(), Some("peer A degraded"));
        assert_eq!(topology.shared[0].used_by, ["RESOURCE"]);
        assert_eq!(topology.external[0].name, "tcp/192.168.77.11:7447");
        assert!(topology.discovered.is_empty());

        // Version 2 links have no id and no role; versions 3 and 4 carry both.
        let scoped = version >= 3;
        assert_eq!(
            link.id.as_deref(),
            scoped.then_some("link:mesh:peer:endpoint-resource-mesh:external-mesh-0"),
            "version {version}"
        );
        assert_eq!(
            link.role.as_deref(),
            scoped.then_some("peer"),
            "version {version}"
        );

        // Only version 4 says where each fact comes from, and the endpoint's role.
        let v4 = version == 4;
        assert_eq!(host.source.as_deref(), v4.then_some("self"));
        assert_eq!(runtime.source.as_deref(), v4.then_some("self"));
        assert_eq!(endpoint.source.as_deref(), v4.then_some("self"));
        assert_eq!(endpoint.role.as_deref(), v4.then_some("peer"));
        assert_eq!(
            topology.external[0].source.as_deref(),
            v4.then_some("config")
        );
    }

    // A runtime in a container.
    let topology =
        FleetTopology::decode(&legacy_mesh_report(4, Some("Cell_A"))).expect("container report");
    let container = &topology.hosts[0].containers[0];
    assert_eq!(container.container_id, "Cell_A");
    assert_eq!(container.source.as_deref(), Some("self"));
    assert_eq!(container.runtimes[0].runtime_id, "RESOURCE");
    assert!(topology.hosts[0].runtimes.is_empty());

    // What this runtime writes reads back unchanged.
    for topology in [full_topology(), minimal_topology()] {
        let value = serde_json::to_value(&topology).expect("serialize the report");
        assert_eq!(FleetTopology::decode(&value), Ok(topology));
    }

    // Arrays that a report leaves out read as empty, and unknown fields are ignored.
    let mut sparse = legacy_mesh_report(2, None);
    for key in ["links", "shared", "external"] {
        sparse.as_object_mut().expect("report object").remove(key);
    }
    sparse["hosts"][0]
        .as_object_mut()
        .expect("host object")
        .remove("containers");
    let endpoint = &mut sparse["hosts"][0]["runtimes"][0]["endpoints"][0];
    endpoint["display_name"] = json!("Mesh");
    endpoint["connector"] = json!({"connector_id": "mesh", "state": "ready"});
    sparse["category"] = json!("future");
    let topology = FleetTopology::decode(&sparse).expect("sparse report");
    assert!(topology.links.is_empty());
    assert!(topology.shared.is_empty());
    assert!(topology.external.is_empty());
    assert!(topology.hosts[0].containers.is_empty());
    assert_eq!(
        topology.hosts[0].runtimes[0].endpoints[0].name,
        "Mesh / Zenoh"
    );

    // Versions before 2 and after 4 are not read.
    for version in [0, 1, 5] {
        assert_eq!(
            FleetTopology::decode(&legacy_mesh_report(version, None)),
            Err(TopologyDecodeError::UnsupportedSchemaVersion(version))
        );
    }
}

#[test]
fn topology_decoder_never_panics_on_malformed_json() {
    for value in [
        json!(null),
        json!(true),
        json!(4),
        json!("fleet"),
        json!([]),
        json!([{"schema_version": 4}]),
    ] {
        assert_eq!(
            FleetTopology::decode(&value),
            Err(TopologyDecodeError::NotAnObject),
            "{value}"
        );
    }
    for version in [
        json!(null),
        json!("4"),
        json!(-4),
        json!(4.5),
        json!([4]),
        json!({}),
    ] {
        assert_eq!(
            FleetTopology::decode(&json!({"schema_version": version, "hosts": []})),
            Err(TopologyDecodeError::NoSchemaVersion)
        );
    }
    assert_eq!(
        FleetTopology::decode(&json!({"hosts": []})),
        Err(TopologyDecodeError::NoSchemaVersion)
    );
    assert_eq!(
        FleetTopology::decode(&json!({"schema_version": u64::MAX})),
        Err(TopologyDecodeError::UnsupportedSchemaVersion(u64::MAX))
    );

    // Every value of a full report, removed or replaced by a value of another type.
    let report = serde_json::to_value(full_topology()).expect("serialize the report");
    let replacements = [
        json!(null),
        json!(true),
        json!(-1),
        json!(1.5),
        json!(70_000),
        json!(u64::MAX),
        json!("text"),
        json!([]),
        json!({}),
        json!([[[[1]]]]),
    ];
    let mut paths = Vec::new();
    collect_paths(&report, "", &mut paths);
    let (mut accepted, mut rejected) = (0_usize, 0_usize);
    for path in &paths {
        let mut variants = vec![without(&report, path)];
        for replacement in &replacements {
            let mut variant = report.clone();
            if let Some(slot) = variant.pointer_mut(path) {
                *slot = replacement.clone();
            }
            variants.push(variant);
        }
        for variant in &variants {
            match FleetTopology::decode(variant) {
                Ok(_) => accepted += 1,
                Err(error) => {
                    assert!(!error.to_string().is_empty(), "{path}");
                    rejected += 1;
                }
            }
        }
    }
    assert!(
        paths.len() > 100 && accepted > 0 && rejected > 0,
        "{} paths: {accepted} accepted, {rejected} rejected",
        paths.len()
    );

    // An optional field may be absent; a required one may not, nor may a value of the wrong type.
    assert!(FleetTopology::decode(&without(&report, "/hosts/0/board")).is_ok());
    assert!(matches!(
        FleetTopology::decode(&without(&report, "/hosts/0/host_id")),
        Err(TopologyDecodeError::Malformed(_))
    ));
    let mut wrong = report.clone();
    wrong["hosts"][0]["runtimes"][0]["cycle_ms"] = json!("10 ms");
    assert!(matches!(
        FleetTopology::decode(&wrong),
        Err(TopologyDecodeError::Malformed(_))
    ));
    let mut wide = report.clone();
    wide["hosts"][0]["runtimes"][0]["endpoints"][0]["children"][0]["slot"] = json!(70_000);
    assert!(matches!(
        FleetTopology::decode(&wide),
        Err(TopologyDecodeError::Malformed(_))
    ));

    // Deeply nested parameters stay one value.
    let mut deep = json!(1);
    for _ in 0..100 {
        deep = json!([deep]);
    }
    let mut nested = report.clone();
    nested["hosts"][0]["runtimes"][0]["endpoints"][0]["params"] = deep.clone();
    let topology = FleetTopology::decode(&nested).expect("deeply nested parameters");
    assert_eq!(
        topology.hosts[0].runtimes[0].endpoints[0].params,
        Some(deep)
    );
}

/// The producer's wire output keeps every field name and every omission of the types that moved
/// here from `control/fleet_handlers/types.rs`. The control response is a JSON value with sorted
/// keys, so equal values are equal bytes.
#[test]
fn topology_wire_output_keeps_field_names_and_omissions() {
    assert_eq!(
        serde_json::to_value(full_topology()).expect("serialize the report"),
        json!({
            "schema_version": 4,
            "hosts": [{
                "host_id": "host:line-a", "hostname": "line-a", "board": "Raspberry Pi 5",
                "arch": "aarch64", "os": "linux", "ips": ["192.168.10.31"], "temp_c": 48.5,
                "uptime_s": 3600, "load": 0.5,
                "containers": [{
                    "container_id": "Cell_A", "name": "cell-a", "image": "runtime:latest",
                    "status": "running", "runtimes": [minimal_runtime_json()],
                    "restart_policy": "always", "cpu_limit": "2", "mem_limit": "512m",
                    "mounts": ["/data"], "source": "self",
                }],
                "runtimes": [{
                    "runtime_id": "RESOURCE", "name": "RESOURCE",
                    "control_endpoint": "tcp://127.0.0.1:9901", "web_listen": "0.0.0.0:8080",
                    "mode": "online", "cycle_ms": 10, "load": 0.25, "health": "connected",
                    "detail": "Running.",
                    "endpoints": [{
                        "id": "endpoint:RESOURCE:ethercat", "kind": "field",
                        "protocol": "ethercat", "name": "EtherCAT", "address": "eth0",
                        "role": "master", "health": "connected", "detail": "Running.",
                        "live": {"rtt_ms": 3}, "params": {"adapter": "eth0"},
                        "children": [{
                            "id": "ethercat:slot:1", "kind": "field_slave",
                            "name": "EL1008 (slot 1)", "slot": 1, "model": "EL1008",
                            "channels": 8, "health": "configured_policy",
                            "detail": "Configured in io.toml.", "source": "config",
                        }],
                        "owned": true, "supports_test": false, "source": "self",
                    }],
                    "source": "self", "last_seen_ms": SEEN_MS,
                }],
                "source": "self", "last_seen_ms": SEEN_MS,
            }],
            "links": [{
                "id": "link:mesh:peer:endpoint-resource-mesh:external-mesh-0",
                "from": "endpoint:RESOURCE:mesh", "to": "external:mesh:0", "protocol": "mesh",
                "role": "peer", "direction": "outbound", "same_host": false,
                "status": "degraded", "secure": true, "detail": "peer A degraded",
            }],
            "shared": [{
                "id": "shared:mqtt:broker", "kind": "broker", "name": "MQTT broker",
                "address": "127.0.0.1:1883", "used_by": ["RESOURCE"],
            }],
            "external": [{
                "id": "external:mesh:0", "kind": "peer", "name": "tcp/192.168.77.11:7447",
                "via_protocol": ["mesh"], "direction": "outbound", "source": "config",
            }],
            "discovered": [{
                "id": "external:discovery:peer", "kind": "runtime", "name": "peer",
                "addresses": ["192.0.2.10"], "via_protocol": ["discovery"],
                "direction": "observed", "adopted": false, "control": "tcp://192.0.2.10:5680",
                "web_port": 8080, "web_tls": false, "mesh_port": 7447, "host_group": "line",
                "last_seen_ms": 10, "source": "discovery",
            }],
        })
    );
    assert_eq!(
        serde_json::to_value(minimal_topology()).expect("serialize the report"),
        json!({
            "schema_version": 4,
            "hosts": [{
                "host_id": "host:line-a", "hostname": "line-a", "arch": "aarch64", "os": "linux",
                "ips": [],
                "containers": [{
                    "container_id": "Cell_A", "name": "cell-a", "image": "unknown",
                    "status": "running", "runtimes": [],
                }],
                "runtimes": [minimal_runtime_json()],
            }],
            "links": [{
                "from": "endpoint:RESOURCE:mesh", "to": "external:mesh:0", "protocol": "mesh",
                "direction": "outbound", "same_host": false, "status": "degraded",
                "secure": true,
            }],
            "shared": [],
            "external": [{
                "id": "external:mesh:0", "kind": "peer", "name": "tcp/192.168.77.11:7447",
                "via_protocol": [], "direction": "outbound",
            }],
            "discovered": [{
                "id": "external:discovery:peer", "kind": "runtime", "name": "peer",
                "addresses": [], "via_protocol": [], "direction": "observed", "adopted": false,
            }],
        })
    );
}
