//! The connection plans of `act` (design §4.3 B7): exposing globals through a server, the `EtherCAT`
//! channel selection, an OPC UA client connection built from selected nodes, and one ADS connection
//! per responding port. Every expected value is written out, taken from the ported TypeScript
//! (`NC/exposeConfig.ts`, `NC/protocolActions.ts`, `W/opcuaClientModel.ts`, `NC/adsDiscoveryPorts.ts`
//! and their tests `opcua-client-model.test.ts` and `network-canvas.test.ts`) and the specification
//! 25 lines named in each test. A plan runs against the scripted fake runtime, so nothing reaches
//! the network.

use std::time::Instant;

use serde_json::{json, Map, Value};
use trust_runtime::control::offline_comm_schema_json;

use crate::actions::act;
use crate::actions::plans::{ads_device_connections, opcua_connection};
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::gather::{ProjectInputs, TargetReport};
use crate::request::{ActOp, ActRequest, HostFacts, RunMode, Token};
use crate::session::Sessions;
use crate::targets::TargetOrigin;
use crate::view::tests::fixtures as fx;
use crate::view::types::{DevicesView, Refresh, Runtime};

/// The token that the extension stored for a runtime's control endpoint.
const TOKEN: &str = "token-1";

/// A request that performs `action` with `payload`, no form values and no host facts.
fn request(action: &str, payload: Value) -> ActRequest {
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: action.to_owned(),
            payload,
        },
        values: Map::new(),
        project: None,
        host: HostFacts::default(),
    }
}

/// The project inputs of the tests: no configured topology, the tool's own catalog.
fn inputs(topology: Option<Value>) -> ProjectInputs {
    ProjectInputs {
        projects: Vec::new(),
        chosen: None,
        topology: topology.map(Ok),
        managed: Ok(Vec::new()),
        catalog: offline_comm_schema_json(None),
    }
}

/// A session that has seen the view of `topology` and `targets`, and that view (design §4.3 B7).
fn seen(topology: Option<Value>, targets: Vec<TargetReport>) -> (Sessions, DevicesView) {
    let gathered = fx::gathered(inputs(topology), targets);
    let view = crate::view::project(
        &gathered,
        &fx::request(fx::host(RunMode::Online, "")),
        None,
        fx::NOW_MS,
    );
    let mut panel = Sessions::default();
    panel
        .session("dc-test", Instant::now())
        .remember(&gathered, &view);
    (panel, view)
}

/// The runtime of `view` shown as `name`.
fn named<'a>(view: &'a DevicesView, name: &str) -> &'a Runtime {
    view.computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .find(|runtime| runtime.name == name)
        .unwrap_or_else(|| panic!("a runtime named {name}"))
}

/// The connection of the runtime `name` that has `protocol`, as its item ID.
fn connection_id(view: &DevicesView, name: &str, protocol: &str) -> String {
    named(view, name)
        .connections
        .iter()
        .find(|connection| connection.protocol == protocol)
        .unwrap_or_else(|| panic!("the {protocol} connection of {name}"))
        .id
        .as_str()
        .to_owned()
}

/// The `comm.apply` requests that arrived at `runtime`.
fn applied(runtime: &FakeRuntime) -> Vec<Value> {
    runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "comm.apply")
        .collect()
}

/// `value` as a JSON object.
fn object(value: &Value) -> Map<String, Value> {
    value.as_object().expect("a JSON object").clone()
}

/// A discovered ADS device: its address, AMS Net ID and responding ports.
fn ads_target(responding_ads_ports: &Value) -> Value {
    json!({
        "host": "10.0.0.9",
        "target_net_id": "5.6.7.8.1.1",
        "responding_ads_ports": responding_ads_ports.clone(),
        "local_net_id": "127.0.0.1.1.1",
    })
}

/// The parameters of the `opcua` server that the expose test re-applies
/// (`network-canvas.test.ts:2487-2499`).
fn expose_current() -> Value {
    json!({
        "enabled": true,
        "listen": "0.0.0.0",
        "ams_net_id": "127.0.0.1.1.1",
        "port": 48898,
        "expose": ["TankLevel"],
        "writable": [],
        "clients": [],
        "clients_count": 0,
        "clients_summary": ["127.0.0.1.1.100 (from 127.0.0.1)"],
        "username_set": true,
    })
}

/// The parameters of the `ethercat` bus that the channel test re-applies.
fn ethercat_current() -> Value {
    json!({
        "adapter": "eth0",
        "modules": [{ "slot": 1, "model": "EL1008" }],
        "selected_channels": ["ethercat.slot1.channel0"],
        "timeout_ms": 100,
    })
}

/// One endpoint of the `line` runtime.
fn endpoint(protocol: &str, name: &str, role: &str, params: &Value) -> Value {
    json!({
        "id": format!("endpoint:line:{protocol}"),
        "kind": "field", "protocol": protocol, "name": name, "role": role,
        "health": "connected", "detail": "Driver is healthy.", "owned": true,
        "supports_test": true, "source": "self", "params": params,
    })
}

/// A `fleet.topology` of the `line` runtime with its `opcua` server and its `ethercat` bus.
fn live_topology() -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": "host:line-pc", "hostname": "line-pc", "arch": "aarch64", "os": "linux",
            "ips": ["127.0.0.1"], "containers": [], "source": "self",
            "runtimes": [{
                "runtime_id": "line", "name": "line", "mode": "online", "cycle_ms": 10,
                "health": "connected", "detail": "Runtime answered fleet.topology.", "source": "self",
                "endpoints": [
                    endpoint("opcua", "OPC UA server", "server", &expose_current()),
                    endpoint("ethercat", "EtherCAT bus", "owned_driver", &ethercat_current()),
                ],
            }],
        }],
        "links": [], "shared": [], "external": [],
    })
}

/// A scripted runtime that answers the gather for `live_topology`.
fn script() -> Script {
    Script::running()
        .answer("status", Reply::Result(fx::status("running", "admin")))
        .answer("fleet.topology", Reply::Result(live_topology()))
}

/// The report of the project's own runtime `line`, answered at `runtime.endpoint`.
fn project_target(runtime: &FakeRuntime) -> TargetReport {
    fx::answered(
        TargetOrigin::Project,
        &runtime.endpoint,
        None,
        fx::status("running", "admin"),
        live_topology(),
    )
}

#[test]
fn expose_merges_into_the_owning_server_and_keeps_clients() {
    // Specification 25 §5.8 (its lines 525-528 in the design's checkout): re-applying a server's
    // configuration MUST remove topology-only and secret-presence evidence such as `clients_count`,
    // `clients_summary`, and `username_set`; it preserves configured exposure and adds reviewed
    // writable names only. The ported `buildExposeApplyParams` test is `network-canvas.test.ts:2486`
    // with its own numbers. A wrong implementation drops the server's existing `expose`, `writable`
    // or `clients`, keeps the derived evidence, exposes a name twice, or writes another server.
    let runtime = FakeRuntime::start(script());
    let (mut panel, view) = seen(None, vec![project_target(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let server = connection_id(&view, "line", "opcua");
    let mut expose = request(
        "connection.expose",
        json!({
            "pane": "browse:opcua",
            "protocol": "opcua",
            "runtime": item,
            "connection": server,
        }),
    );
    expose
        .values
        .insert("paths".to_owned(), json!(["global.Setpoint"]));
    expose.values.insert("writable".to_owned(), json!(true));
    expose
        .host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, expose);

    assert!(reply.ok);
    assert_eq!(reply.refresh, Refresh::Now);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some("browse:opcua")
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("OPC UA server: exposed 1 global. Restart to apply.")
    );
    let sent = applied(&runtime);
    assert_eq!(sent.len(), 1, "one comm.apply reached the runtime");
    assert_eq!(sent[0]["params"]["protocol"], json!("opcua"));
    assert_eq!(sent[0]["params"]["action"], json!("upsert"));
    assert_eq!(
        sent[0]["params"]["params"],
        json!({
            "enabled": true,
            "listen": "0.0.0.0",
            "ams_net_id": "127.0.0.1.1.1",
            "port": 48898,
            "expose": ["TankLevel", "Setpoint"],
            "writable": ["Setpoint"],
            "clients": [],
        })
    );
}

#[test]
fn ethercat_selection_sorted_and_unique() {
    // `NC/protocolActions.ts:266-268`: each chosen channel path is trimmed, an empty one is
    // dropped, a duplicate is kept once, and the result is sorted as today. A wrong implementation
    // keeps a duplicate, an empty path, an untrimmed path, or today's order unsorted.
    let runtime = FakeRuntime::start(script());
    let (mut panel, view) = seen(None, vec![project_target(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let bus = connection_id(&view, "line", "ethercat");
    let mut select = request(
        "connection.ethercat_channels",
        json!({
            "pane": "browse:ethercat",
            "runtime": item,
            "connection": bus,
        }),
    );
    select.values.insert(
        "paths".to_owned(),
        json!([
            "ethercat.slot3.channel0",
            "ethercat.slot1.channel0",
            "ethercat.slot2.channel1",
            "ethercat.slot1.channel0",
            "  ",
            " ethercat.slot1.channel0 ",
        ]),
    );
    select
        .host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, select);

    assert!(reply.ok);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("3 EtherCAT channels selected. Restart to apply.")
    );
    let sent = applied(&runtime);
    assert_eq!(sent.len(), 1, "one comm.apply reached the runtime");
    assert_eq!(sent[0]["params"]["protocol"], json!("ethercat"));
    assert_eq!(sent[0]["params"]["action"], json!("upsert"));
    assert_eq!(
        sent[0]["params"]["params"],
        json!({
            "adapter": "eth0",
            "modules": [{ "slot": 1, "model": "EL1008" }],
            "selected_channels": [
                "ethercat.slot1.channel0",
                "ethercat.slot2.channel1",
                "ethercat.slot3.channel0",
            ],
            "timeout_ms": 100,
        })
    );
}

#[test]
fn opcua_connection_from_selected_nodes() {
    // `opcua-client-model.test.ts:103-118` (`buildOpcuaConnection assembles connection + points
    // from target`): the connection carries exactly today's fields, and its one point carries the
    // **real** NodeId, the truST variable, the apply-ready type and the read-only access while the
    // user did not opt into writes. A wrong implementation drops a field, uses a default for
    // one that the target set, or writes `read_write` without the opt-in.
    let runtime = FakeRuntime::start(script());
    let (mut panel, view) = seen(None, vec![project_target(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let mut add = request(
        "connection.opcua_add",
        json!({
            "pane": "browse:opcua",
            "runtime": item,
            "label": "FreeOpcUa Python Server",
            "target": {
                "endpoint_url": "opc.tcp://127.0.0.1:4840/trust-test",
                "security_policy": "none",
                "security_mode": "none",
                "auth": "anonymous",
                "trust_server_certificate": true,
            },
        }),
    );
    add.values.insert(
        "nodes".to_owned(),
        json!([{
            "id": "opcua:node:ns_2_i_1",
            "node_id": "ns=2;i=1",
            "name": "Temperature",
            "path": "Temperature",
            "type": "i=11",
            "data_type": "double",
            "writable": true,
        }]),
    );
    add.values.insert("writable".to_owned(), json!(false));
    add.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, add);

    assert!(reply.ok);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some("browse:opcua")
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Added OPC UA client connection with 1 node(s). Restart the runtime to read it.")
    );
    let sent = applied(&runtime);
    assert_eq!(sent.len(), 1, "one comm.apply reached the runtime");
    assert_eq!(sent[0]["params"]["protocol"], json!("opcua_client"));
    assert_eq!(sent[0]["params"]["action"], json!("add"));
    assert_eq!(
        sent[0]["params"]["params"],
        json!({
            "enabled": true,
            "connections": [{
                "name": "freeopcua-python-server",
                "endpoint_url": "opc.tcp://127.0.0.1:4840/trust-test",
                "security_policy": "none",
                "security_mode": "none",
                "auth": "anonymous",
                "trust_server_certificate": true,
                "points": [{
                    "var": "global.Temperature",
                    "node_id": "ns=2;i=1",
                    "type": "double",
                    "access": "read",
                }],
            }],
        })
    );
}

#[test]
fn ads_device_gets_one_connection_per_responding_port() {
    // `NC/protocolActions.ts:71-84`: one connection per responding ADS port, each with today's
    // fields. A wrong implementation makes one connection for the whole device, drops a port,
    // renames without the `_port_<n>` suffix, or loses the AMS Net IDs.
    let runtime = FakeRuntime::start(script());
    let (mut panel, view) = seen(None, vec![project_target(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let mut add = request(
        "connection.ads_add",
        json!({
            "pane": "browse:ads",
            "runtime": item,
            "label": "PLC",
            "target": ads_target(&json!([851, 852])),
        }),
    );
    add.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, add);

    assert!(reply.ok);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Added 2 ADS connections. Restart to apply.")
    );
    let sent = applied(&runtime);
    assert_eq!(sent.len(), 1, "one comm.apply reached the runtime");
    assert_eq!(sent[0]["params"]["protocol"], json!("ads"));
    assert_eq!(sent[0]["params"]["action"], json!("add"));
    assert_eq!(
        sent[0]["params"]["params"],
        json!({
            "enabled": true,
            "connections": [
                {
                    "name": "PLC_port_851",
                    "target_net_id": "5.6.7.8.1.1",
                    "host": "10.0.0.9",
                    "ams_port": 851,
                    "local_net_id": "127.0.0.1.1.1",
                    "transport": "plain",
                    "insecure_transport": true,
                    "auto_add_route": false,
                    "points": [],
                },
                {
                    "name": "PLC_port_852",
                    "target_net_id": "5.6.7.8.1.1",
                    "host": "10.0.0.9",
                    "ams_port": 852,
                    "local_net_id": "127.0.0.1.1.1",
                    "transport": "plain",
                    "insecure_transport": true,
                    "auto_add_route": false,
                    "points": [],
                },
            ],
        })
    );
}

#[test]
fn opcua_point_keeps_the_raw_node_id_and_apply_ready_type() {
    // `opcua-client-model.test.ts:52-59`: the point keeps the real NodeId (not the sanitized id),
    // the truST variable, the resolved data type, and `read_write` while writable and opted in. A
    // wrong implementation stores the sanitized React id, or the raw `i=11` type.
    let node = json!({
        "id": "opcua:node:ns_2_i_1",
        "node_id": "ns=2;i=1",
        "name": "Temperature",
        "path": "Temperature",
        "type": "i=11",
        "data_type": "double",
        "writable": true,
    });
    let connection = opcua_connection(
        &object(&json!({ "endpoint_url": "opc.tcp://h:4840" })),
        "x",
        std::slice::from_ref(&node),
        true,
    )
    .expect("a connection");

    let point = &connection["points"].as_array().expect("points")[0];
    assert_eq!(point["node_id"], json!("ns=2;i=1"));
    assert_eq!(point["var"], json!("global.Temperature"));
    assert_eq!(point["type"], json!("double"));
    assert_eq!(point["access"], json!("read_write"));
}

#[test]
fn opcua_var_name_folds_non_identifier_characters() {
    // `opcua-client-model.test.ts:31-37`: the variable is `global.` plus the path with every run of
    // non-identifier characters folded to one `_`. A wrong implementation keeps the dots, folds a
    // run to several underscores, or uses the name instead of the path.
    let nodes = [
        json!({ "node_id": "ns=2;i=1", "name": "Temperature", "path": "Temperature" }),
        json!({ "node_id": "ns=2;i=2", "name": "X", "path": "Objects.Server.X" }),
    ];
    let connection = opcua_connection(
        &object(&json!({ "endpoint_url": "opc.tcp://h:4840" })),
        "x",
        &nodes,
        false,
    )
    .expect("a connection");

    let points = connection["points"].as_array().expect("points");
    assert_eq!(points[0]["var"], json!("global.Temperature"));
    assert_eq!(points[1]["var"], json!("global.Objects_Server_X"));
}

#[test]
fn opcua_display_type_prefers_the_resolved_data_type() {
    // `opcua-client-model.test.ts:39-43`: the resolved `data_type` wins, else the raw OPC UA type,
    // else empty (which the point writes as `string`). A wrong implementation always uses the raw
    // `i=11` type, or leaves an unknown type empty in the point.
    let nodes = [
        json!({ "node_id": "a", "type": "i=11", "data_type": "double" }),
        json!({ "node_id": "b", "type": "i=11" }),
        json!({ "node_id": "c" }),
    ];
    let connection = opcua_connection(
        &object(&json!({ "endpoint_url": "opc.tcp://h:4840" })),
        "x",
        &nodes,
        false,
    )
    .expect("a connection");

    let points = connection["points"].as_array().expect("points");
    assert_eq!(points[0]["type"], json!("double"));
    assert_eq!(points[1]["type"], json!("i=11"));
    assert_eq!(points[2]["type"], json!("string"));
}

#[test]
fn opcua_write_access_needs_a_writable_node_and_opt_in() {
    // `opcua-client-model.test.ts:45-50`: `read_write` needs the server to report the node writable
    // **and** the user to opt in; either alone is `read`, and a missing flag is `read`. A wrong
    // implementation writes whenever the user opted in, or whenever the server says writable.
    let target = object(&json!({ "endpoint_url": "opc.tcp://h:4840" }));
    let writable = json!({ "node_id": "a", "writable": true });
    let readonly = json!({ "node_id": "b", "writable": false });
    let no_flag = json!({ "node_id": "c" });

    let opted_in = opcua_connection(&target, "x", std::slice::from_ref(&writable), true)
        .expect("a connection");
    assert_eq!(opted_in["points"][0]["access"], json!("read_write"));

    let not_opted_in = opcua_connection(&target, "x", std::slice::from_ref(&writable), false)
        .expect("a connection");
    assert_eq!(not_opted_in["points"][0]["access"], json!("read"));

    let readonly_opted_in = opcua_connection(&target, "x", std::slice::from_ref(&readonly), true)
        .expect("a connection");
    assert_eq!(readonly_opted_in["points"][0]["access"], json!("read"));

    let no_flag_opted_in =
        opcua_connection(&target, "x", std::slice::from_ref(&no_flag), true).expect("a connection");
    assert_eq!(no_flag_opted_in["points"][0]["access"], json!("read"));
}

#[test]
fn opcua_leaf_without_a_raw_node_id_cannot_be_saved() {
    // `opcua-client-model.test.ts:61-64`: a leaf without the raw NodeId cannot be saved, and it is
    // dropped from a connection instead of being stored with a sanitized id. A wrong implementation
    // falls back to the React id or the path.
    let target = object(&json!({ "endpoint_url": "opc.tcp://h:4840" }));
    let without = json!({ "id": "opcua:node:x", "name": "Temperature", "path": "Temperature" });
    assert_eq!(
        opcua_connection(&target, "x", std::slice::from_ref(&without), true),
        None
    );

    let with = json!({ "node_id": "ns=2;i=1", "path": "Temperature" });
    let mixed = [without, with];
    let connection = opcua_connection(&target, "x", &mixed, true).expect("a connection");
    assert_eq!(
        connection["points"].as_array().expect("points").len(),
        1,
        "only the node with a raw NodeId is kept"
    );
}

#[test]
fn opcua_username_auth_carries_credentials_and_anonymous_does_not() {
    // `opcua-client-model.test.ts:126-133`: username authentication carries the username and
    // password; anonymous carries none even when the target holds them. A wrong implementation
    // sends the credentials on anonymous, or drops them on username authentication.
    let node = json!({ "node_id": "ns=2;i=1", "path": "Temperature" });
    let anonymous = object(&json!({
        "endpoint_url": "opc.tcp://h:4840", "auth": "anonymous",
        "username": "u", "password": "p",
    }));
    let connection = opcua_connection(&anonymous, "x", std::slice::from_ref(&node), false)
        .expect("a connection");
    assert_eq!(connection.get("username"), None);
    assert_eq!(connection.get("password"), None);

    let username = object(&json!({
        "endpoint_url": "opc.tcp://h:4840", "auth": "username",
        "username": "u", "password": "p",
    }));
    let connection =
        opcua_connection(&username, "x", std::slice::from_ref(&node), false).expect("a connection");
    assert_eq!(connection["username"], json!("u"));
    assert_eq!(connection["password"], json!("p"));
}

#[test]
fn opcua_connection_name_slugs_the_endpoint_when_the_label_is_the_protocol() {
    // `opcua-client-model.test.ts:135-140`: the raw protocol id as the label makes the name come
    // from the endpoint URL, slugged to `[a-z0-9-]`. A wrong implementation names it `opcua-client`
    // or keeps the scheme and dots.
    let node = json!({ "node_id": "ns=2;i=1", "path": "Temperature" });
    let target = object(&json!({ "endpoint_url": "opc.tcp://10.0.0.5:4840/Server" }));
    let connection = opcua_connection(&target, "opcua_client", std::slice::from_ref(&node), false)
        .expect("a connection");

    assert_eq!(connection["name"], json!("10-0-0-5-4840-server"));
}

#[test]
fn opcua_connection_needs_an_endpoint_and_usable_points() {
    // `opcua-client-model.test.ts:120-124`: a connection needs an endpoint URL and at least one
    // usable point; otherwise building it yields nothing. A wrong implementation writes a
    // connection with an empty endpoint URL or with no points.
    let node = json!({ "node_id": "ns=2;i=1", "path": "Temperature" });
    assert_eq!(
        opcua_connection(&object(&json!({})), "x", std::slice::from_ref(&node), true),
        None
    );

    let without = json!({ "name": "Temperature", "path": "Temperature" });
    assert_eq!(
        opcua_connection(
            &object(&json!({ "endpoint_url": "opc.tcp://h:4840" })),
            "x",
            std::slice::from_ref(&without),
            true,
        ),
        None
    );
}

#[test]
fn ads_responding_ports_are_unique_sorted_and_valid() {
    // `NC/adsDiscoveryPorts.ts:38-48`: only a whole number from 1 to 65535 is a port, each is kept
    // once, and the ports are sorted. A wrong implementation keeps a duplicate or an out-of-range
    // value, or returns them unsorted.
    let target = object(&json!({
        "host": "10.0.0.9", "target_net_id": "5.6.7.8.1.1",
        "responding_ads_ports": [852, 851, 851, 0, 65536, "x", 501],
        "local_net_id": "127.0.0.1.1.1",
    }));
    let connections = ads_device_connections(&target, "PLC").expect("connections");
    let ports: Vec<u16> = connections
        .iter()
        .map(|connection| {
            u16::try_from(connection["ams_port"].as_u64().expect("a port")).expect("a u16 port")
        })
        .collect();

    assert_eq!(ports, vec![501, 851, 852]);
}

#[test]
fn ads_connection_names_the_port_when_several_answer() {
    // `NC/adsDiscoveryPorts.ts:133-145`: one responding port keeps the device's own name; several
    // append `_port_<n>`, with an earlier `_port_<n>` suffix removed first. A wrong implementation
    // suffixes a single port, or repeats an existing suffix.
    let many = object(&ads_target(&json!([851, 852])));
    let connections = ads_device_connections(&many, "PLC").expect("connections");
    let names: Vec<&str> = connections
        .iter()
        .map(|connection| connection["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(names, ["PLC_port_851", "PLC_port_852"]);

    let one = object(&ads_target(&json!([851])));
    let connections = ads_device_connections(&one, "PLC").expect("connections");
    assert_eq!(connections[0]["name"], json!("PLC"));

    let suffixed = object(&ads_target(&json!([851, 852])));
    let connections = ads_device_connections(&suffixed, "cell1_port_851").expect("connections");
    assert_eq!(connections[0]["name"], json!("cell1_port_851"));
    assert_eq!(connections[1]["name"], json!("cell1_port_852"));
}

#[test]
fn ads_local_net_id_comes_from_the_route_plan() {
    // `NC/protocolActions.ts:714-733`: this computer's AMS Net ID comes from the configured
    // `local_net_id`, else from the route plan of the port's browse evidence. A wrong
    // implementation refuses a device whose AMS Net ID discovery reported only in its route plan.
    let target = object(&json!({
        "host": "10.0.0.9", "target_net_id": "5.6.7.8.1.1",
        "responding_ads_ports": [851],
        "ads_port_browse_results": [{
            "port": 851, "tree": [],
            "routePlan": { "local": { "ams_net_id": "127.0.0.1.1.1" } },
        }],
    }));
    let connections = ads_device_connections(&target, "PLC").expect("connections");

    assert_eq!(connections[0]["local_net_id"], json!("127.0.0.1.1.1"));
}

#[test]
fn ads_device_without_a_local_net_id_is_refused() {
    // `NC/protocolActions.ts:64-69`: a device whose discovery returned no AMS Net ID for this
    // computer cannot be added, and one that names no address, AMS Net ID or port cannot either.
    // A wrong implementation adds the connection with an empty local net ID anyway.
    let no_local = object(&json!({
        "host": "10.0.0.9", "target_net_id": "5.6.7.8.1.1",
        "responding_ads_ports": [851],
    }));
    assert_eq!(
        ads_device_connections(&no_local, "PLC"),
        Err(
            "Could not add the ADS device because discovery did not return this computer's AMS \
             Net ID. Scan it again and retry."
                .to_owned()
        )
    );

    let no_address = object(&json!({
        "target_net_id": "5.6.7.8.1.1", "responding_ads_ports": [851],
        "local_net_id": "127.0.0.1.1.1",
    }));
    assert_eq!(
        ads_device_connections(&no_address, "PLC"),
        Err(
            "Could not add the ADS device. Scan it again so its address, AMS Net ID, and \
             responding ports are available."
                .to_owned()
        )
    );
}
