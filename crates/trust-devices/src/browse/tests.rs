//! The browse tests (design §4.3 B8; specification 25 §5.4 and §11.2): the browse runs at the
//! runtime that owns the named connection, a loading pane first and the tree in a later poll, an
//! older request never taking over, the raw protocol identity of a leaf, the structured failures,
//! the `Add` button's reasons, and the trust-certificate flag that only `Add` saves. Every
//! expected value is written out, taken from the ported TypeScript and the specification lines
//! named in each test. The browses run against the crate's scripted fake runtime, so nothing
//! reaches the network.

use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use trust_runtime::control::offline_comm_schema_json;

use super::open;
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::gather::{LiveData, ProjectInputs, Reach, TargetReport};
use crate::request::{ActOp, ActRequest, BrowseRequest, RunMode};
use crate::session::Sessions;
use crate::targets::{Target, TargetOrigin};
use crate::test_project::TempFolder;
use crate::view::tests::fixtures as fx;
use crate::view::types::{BrowseKind, BrowseView, Connection, DevicesView, Runtime, TreeNode};

/// The token that the extension stored for a runtime's control endpoint.
const TOKEN: &str = "token-1";

// ---- the tests' inputs (design §4.3 B8) ----

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

/// A session that has seen the view of `inputs` and `targets`, and that view.
fn seen(inputs: ProjectInputs, targets: Vec<TargetReport>) -> (Sessions, DevicesView) {
    let gathered = fx::gathered(inputs, targets);
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
fn runtime_named<'a>(view: &'a DevicesView, name: &str) -> &'a Runtime {
    view.computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .find(|runtime| runtime.name == name)
        .unwrap_or_else(|| panic!("a runtime named {name}"))
}

/// The connection of the runtime `name` with `protocol`.
fn connection_protocol<'a>(
    view: &'a DevicesView,
    assert_runtime: &str,
    protocol: &str,
) -> &'a Connection {
    runtime_named(view, assert_runtime)
        .connections
        .iter()
        .find(|connection| connection.protocol == protocol)
        .unwrap_or_else(|| panic!("the {protocol} connection of {assert_runtime}"))
}

/// The connection of the runtime `name` shown as `connection_name`.
fn connection_named<'a>(
    view: &'a DevicesView,
    assert_runtime: &str,
    connection_name: &str,
) -> &'a Connection {
    runtime_named(view, assert_runtime)
        .connections
        .iter()
        .find(|connection| connection.name == connection_name)
        .unwrap_or_else(|| panic!("the {connection_name} connection of {assert_runtime}"))
}

/// An endpoint of a `fleet.topology` report, with its parameters.
fn endpoint(
    runtime: &str,
    protocol: &str,
    index: Option<usize>,
    name: &str,
    params: &Value,
) -> Value {
    let id = match index {
        Some(index) => format!("endpoint:{runtime}:{protocol}:{index}"),
        None => format!("endpoint:{runtime}:{protocol}"),
    };
    json!({
        "id": id, "kind": "field", "protocol": protocol, "name": name, "role": "client",
        "health": "connected", "detail": "Driver is healthy.", "owned": true,
        "supports_test": true, "source": "self", "params": params,
    })
}

/// The configuration of an OPC UA client connection.
fn opcua_params(endpoint_url: &str) -> Value {
    json!({
        "name": "line1",
        "endpoint_url": endpoint_url,
        "security_policy": "none",
        "security_mode": "none",
        "auth": "anonymous",
        "trust_server_certificate": false,
        "points": [],
        "enabled": true,
    })
}

/// A `fleet.topology` report of one host with one runtime and its endpoints.
fn report(host_id: &str, hostname: &str, runtime: &Value) -> Value {
    fx::report(host_id, hostname, &["127.0.0.1"], vec![runtime.clone()])
}

/// A scripted runtime that answers `comm.browse_symbols` with `answer`.
fn script(answer: Value) -> Script {
    Script::running().answer("comm.browse_symbols", Reply::Result(answer))
}

/// A target that did not answer before the gather's deadline, so its runtime is browsed in the
/// open project's files instead (specification 25 §5.1 and §6.5).
fn unanswered_target(endpoint: &str) -> TargetReport {
    TargetReport {
        target: Target {
            origin: TargetOrigin::Project,
            endpoint: endpoint.to_owned(),
            enabled: true,
            token: None,
            label: None,
            managed: None,
            events: false,
        },
        reach: Reach::Unanswered,
        live: LiveData::default(),
    }
}

/// A `comm.browse_symbols` answer with `tree`.
fn answer(protocol: &str, kind: &str, tree: Value) -> Value {
    let mut value = json!({ "schema_version": 1, "protocol": protocol, "kind": kind });
    value["tree"] = tree;
    value
}

/// A `comm.browse_symbols` answer with a structured `error`.
fn failed_answer(code: &str) -> Value {
    json!({
        "schema_version": 1, "protocol": "opcua_client", "kind": "nodes", "tree": [],
        "error": { "code": code, "message": "OPC UA node browse failed: the server refused." },
    })
}

/// One node of a browse answer.
fn leaf(
    id: &str,
    node_id: Option<&str>,
    name: &str,
    path: &str,
    data_type: &str,
    writable: Option<bool>,
) -> Value {
    let mut node = json!({
        "id": id, "name": name, "path": path, "type": "i=11", "data_type": data_type,
    });
    if let Some(node_id) = node_id {
        node["node_id"] = json!(node_id);
    }
    if let Some(writable) = writable {
        node["writable"] = json!(writable);
    }
    node
}

// ---- opening a browse ----

/// A `trust-lsp.devicesBrowse` body that names the connection `connection`.
fn browse_body(connection: &str, name: &str, endpoint: &str, seq: u64, trust: bool) -> Value {
    let mut tokens = Map::new();
    tokens.insert(endpoint.to_owned(), json!(TOKEN));
    json!({
        "session": "dc-test",
        "seq": seq,
        "source": { "connection": connection, "connection_name": name },
        "kind": "symbols",
        "trust_server_certificate": trust,
        "host": { "tokens": Value::Object(tokens) },
    })
}

/// `body` decoded as a browse request.
fn browse_request(body: &Value) -> BrowseRequest {
    serde_json::from_value(body.clone()).expect("a browse request")
}

/// Waits until no browse run of the session is still out.
fn wait_for_browse(panel: &mut Sessions) {
    let until = Instant::now() + Duration::from_secs(10);
    while panel.session("dc-test", Instant::now()).browse_asking() {
        assert!(Instant::now() < until, "the browse answer arrives");
        thread::sleep(Duration::from_millis(10));
    }
}

/// A `browse.add` request that ticks `keys` of the browse answer of `body`.
fn add_request(body: &Value, pane: &str, keys: &[&str], writable: bool) -> ActRequest {
    let request = browse_request(body);
    let mut values = Map::new();
    values.insert("keys".to_owned(), json!(keys));
    values.insert("writable".to_owned(), json!(writable));
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: "browse.add".to_owned(),
            payload: json!({ "pane": pane, "request": body }),
        },
        values,
        project: None,
        host: request.host,
    }
}

/// The requests of type `kind` that arrived at `runtime`.
fn requests_of(runtime: &FakeRuntime, kind: &str) -> Vec<Value> {
    runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == kind)
        .collect()
}

/// Opens a live browse of a single-runtime project and returns the finished view.
fn open_live(protocol: &str, params: &Value, answer: Value) -> BrowseView {
    let runtime = FakeRuntime::start(script(answer));
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint("line", protocol, None, "line1", params)],
        ),
    );
    let (mut panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let connection = connection_protocol(&view, "line", protocol)
        .id
        .as_str()
        .to_owned();
    let body = browse_body(&connection, "line1", &runtime.endpoint, 1, false);
    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&body),
    );
    wait_for_browse(&mut panel);
    open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&body),
    )
}

#[test]
fn browse_runs_at_the_runtime_that_owns_the_connection() {
    // Design §4.3 B8 (fixes inventory 3 B29) and design line 258: the browse runs at the runtime
    // that owns the named connection. A wrong implementation browses every runtime, browses the
    // project's files instead, or reaches only the runtime that serves the live data.
    let alpha = FakeRuntime::start(script(answer(
        "opcua_client",
        "nodes",
        json!([leaf(
            "opcua:node:a",
            Some("ns=2;i=1"),
            "A",
            "A",
            "double",
            Some(false)
        )]),
    )));
    let beta = FakeRuntime::start(script(answer(
        "opcua_client",
        "nodes",
        json!([leaf(
            "opcua:node:b",
            Some("ns=2;i=2"),
            "B",
            "B",
            "double",
            Some(false)
        )]),
    )));
    let alpha_topology = report(
        "host:alpha",
        "alpha-pc",
        &fx::runtime(
            "alpha",
            "alpha",
            "connected",
            vec![endpoint(
                "alpha",
                "opcua_client",
                None,
                "alpha line",
                &opcua_params("opc.tcp://alpha:4840"),
            )],
        ),
    );
    let beta_topology = report(
        "host:beta",
        "beta-pc",
        &fx::runtime(
            "beta",
            "beta",
            "connected",
            vec![endpoint(
                "beta",
                "opcua_client",
                None,
                "beta line",
                &opcua_params("opc.tcp://beta:4840"),
            )],
        ),
    );
    let (mut panel, view) = seen(
        inputs(None),
        vec![
            fx::answered(
                TargetOrigin::Project,
                &alpha.endpoint,
                None,
                fx::status("running", "admin"),
                alpha_topology,
            ),
            fx::answered(
                TargetOrigin::Configured,
                &beta.endpoint,
                None,
                fx::status("running", "admin"),
                beta_topology,
            ),
        ],
    );
    let connection = connection_named(&view, "alpha", "alpha line")
        .id
        .as_str()
        .to_owned();
    let body = browse_body(&connection, "alpha line", &alpha.endpoint, 1, false);

    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&body),
    );
    wait_for_browse(&mut panel);

    assert_eq!(
        alpha.count("comm.browse_symbols"),
        1,
        "the owning runtime was browsed"
    );
    assert_eq!(
        beta.count("comm.browse_symbols"),
        0,
        "no runtime that does not own the connection was browsed"
    );
}

#[test]
fn browse_uses_the_named_connection() {
    // Design §3.3 J44 and inventory 2 E23: only the named connection is browsed, never the
    // connection array. A wrong implementation spreads `params.connections[0]`, so it browses the
    // first connection of the array instead of the named one, and it sends the array.
    let runtime = FakeRuntime::start(script(answer(
        "opcua_client",
        "nodes",
        json!([leaf(
            "opcua:node:a",
            Some("ns=2;i=1"),
            "A",
            "A",
            "double",
            Some(false)
        )]),
    )));
    let params = json!({
        "name": "first",
        "endpoint_url": "opc.tcp://first:4840",
        "security_policy": "none",
        "security_mode": "none",
        "auth": "anonymous",
        "connections": [{
            "name": "first", "endpoint_url": "opc.tcp://first:4840", "auth": "anonymous",
        }],
    });
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![
                endpoint("line", "opcua_client", None, "first", &params),
                endpoint(
                    "line",
                    "opcua_client",
                    Some(1),
                    "second",
                    &json!({ "name": "second", "endpoint_url": "opc.tcp://second:4840", "auth": "anonymous" }),
                ),
            ],
        ),
    );
    let (mut panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let connection = connection_named(&view, "line", "second")
        .id
        .as_str()
        .to_owned();
    let body = browse_body(&connection, "second", &runtime.endpoint, 1, false);

    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&body),
    );
    wait_for_browse(&mut panel);

    let sent = requests_of(&runtime, "comm.browse_symbols");
    assert_eq!(sent.len(), 1, "one browse request reached the runtime");
    assert_eq!(
        sent[0]["params"]["target"]["endpoint_url"],
        json!("opc.tcp://second:4840"),
        "the named connection's own target is browsed"
    );
    assert_eq!(
        sent[0]["params"]["target"].get("connections"),
        None,
        "the connection array is never sent"
    );
    assert_eq!(sent[0]["params"]["protocol"], json!("opcua_client"));
    assert_eq!(sent[0]["params"]["kind"], json!("nodes"));
}

#[test]
fn browse_falls_back_to_the_project_when_the_runtime_is_stopped() {
    // Design §4.3 B8 and §1.5: a runtime that does not answer is browsed in the open project's
    // own files (`offline_comm_browse_symbols_project_json`), so a stopped project still browses
    // its configured modules. A wrong implementation refuses a stopped project, or browses another
    // project's files.
    let project = TempFolder::new("browse-offline");
    project.write(
        "io.toml",
        r#"
[io]
safe_state = []

[[io.drivers]]
name = "ethercat"
params = { adapter = "mock", modules = [{ model = "EK1100", slot = 0, channels = 1 }, { model = "EL1008", slot = 1, channels = 8 }] }
"#,
    );
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "stopped",
            vec![endpoint(
                "line",
                "ethercat",
                None,
                "EtherCAT bus",
                &json!({}),
            )],
        ),
    );
    let mut inputs = inputs(Some(topology));
    inputs.projects = vec![project.path.clone()];
    inputs.chosen = Some(project.path.clone());
    let stopped = "unix:///tmp/trust-devices-browse.sock";
    let (mut panel, view) = seen(inputs, vec![unanswered_target(stopped)]);
    let connection = connection_protocol(&view, "line", "ethercat")
        .id
        .as_str()
        .to_owned();
    let body = browse_body(&connection, "EtherCAT bus", stopped, 1, false);

    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&body),
    );
    wait_for_browse(&mut panel);
    let view = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&body),
    );

    assert_eq!(view.kind, BrowseKind::Channels);
    assert_eq!(view.error, None);
    assert_eq!(view.tree.len(), 2, "the configured modules are browsed");
    let module = view
        .tree
        .iter()
        .find(|node| node.name == "EL1008 (slot 1)")
        .expect("the EL1008 module of the project");
    assert_eq!(module.children.len(), 8, "its eight channels are browsed");
}

#[test]
fn a_browse_answers_loading_first_and_the_tree_in_a_later_poll() {
    // Design §1.5, line 381: a loading browse view first, the tree in a later poll. Design §3.4
    // keeps the tick boxes in the webview, so the loading view is one whose tree is empty and
    // whose `Add` still waits. A wrong implementation blocks the request until the browse ends, or
    // answers the tree in the first poll.
    let runtime = FakeRuntime::start(script(answer(
        "opcua_client",
        "nodes",
        json!([leaf(
            "opcua:node:a",
            Some("ns=2;i=1"),
            "Temperature",
            "Objects.Temperature",
            "double",
            Some(false)
        )]),
    )));
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint(
                "line",
                "opcua_client",
                None,
                "line1",
                &opcua_params("opc.tcp://line:4840"),
            )],
        ),
    );
    let (mut panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let connection = connection_protocol(&view, "line", "opcua_client")
        .id
        .as_str()
        .to_owned();
    let body = browse_body(&connection, "line1", &runtime.endpoint, 1, false);
    let request = browse_request(&body);

    let first = open(panel.session("dc-test", Instant::now()), &request);
    assert_eq!(first.tree, Vec::<TreeNode>::new(), "the first poll loads");
    assert_eq!(first.error, None);
    assert!(!first.add.enabled);
    assert_eq!(
        first.add.reason.as_deref(),
        Some("Wait for browse results before adding tags.")
    );

    wait_for_browse(&mut panel);
    let second = open(panel.session("dc-test", Instant::now()), &request);

    assert_eq!(second.tree.len(), 1, "the later poll has the tree");
    assert_eq!(second.tree[0].key, "ns=2;i=1");
    assert_eq!(second.tree[0].name, "Temperature");
    assert!(second.add.enabled);
    assert_eq!(second.add.reason, None);
}

#[test]
fn an_older_seq_never_replaces_a_newer_browse() {
    // Design §3.3 J52: each request of a session has a larger number, and an answer whose number
    // is older than the cached one never replaces it. A wrong implementation starts a second
    // browse for the older request, or lets its answer overwrite the newer one.
    let runtime = FakeRuntime::start(script(answer(
        "opcua_client",
        "nodes",
        json!([leaf(
            "opcua:node:a",
            Some("ns=2;i=1"),
            "Temperature",
            "Objects.Temperature",
            "double",
            Some(false)
        )]),
    )));
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint(
                "line",
                "opcua_client",
                None,
                "line1",
                &opcua_params("opc.tcp://line:4840"),
            )],
        ),
    );
    let (mut panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let connection = connection_protocol(&view, "line", "opcua_client")
        .id
        .as_str()
        .to_owned();

    let newer = browse_body(&connection, "line1", &runtime.endpoint, 5, false);
    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&newer),
    );
    let older = browse_body(&connection, "line1", &runtime.endpoint, 3, false);
    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&older),
    );
    wait_for_browse(&mut panel);

    assert_eq!(
        runtime.count("comm.browse_symbols"),
        1,
        "the older request starts no second browse"
    );
    let cached = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&older),
    );
    assert_eq!(cached.tree.len(), 1, "the newer answer is kept");
    assert_eq!(cached.tree[0].key, "ns=2;i=1");
}

mod projection;
