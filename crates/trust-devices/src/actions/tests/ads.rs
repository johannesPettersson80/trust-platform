//! The ADS route action of `act` (design §4.3 B7; specification 25 §5.4, its lines 387-394): the
//! browse pane's `Create route` sends the `TwinCAT` administrator login once to the guarded
//! `ads.route_add` operation of the runtime the session resolves, and never stores or echoes it.
//!
//! Every expected value is written out: the notice comes from the ported text of
//! [`crate::texts::plans`], and the request shape from the runtime's own `ads.route_add` contract
//! (`crates/trust-runtime/src/control/ads_handlers/route.rs`). A route case runs against the
//! scripted fake runtime, so nothing reaches the network.

use std::time::Instant;

use serde_json::{json, Value};

use crate::actions::act;
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::gather::{ProjectInputs, TargetReport};
use crate::request::{ActOp, ActRequest, HostFacts, RunMode, Token};
use crate::session::Sessions;
use crate::targets::TargetOrigin;
use crate::view::tests::fixtures as fx;
use crate::view::types::{DevicesView, Refresh, Runtime};

/// The token that the extension stored for a runtime's control endpoint.
const TOKEN: &str = "token-1";
/// The `TwinCAT` administrator login the form collected. A distinctive value so that a leak in the
/// reply or in a log line is unmistakable, and never `Administrator`.
const USERNAME: &str = "route-user-4711";
/// The `TwinCAT` administrator password the form collected.
const PASSWORD: &str = "route-pw-9281";

/// A request that performs `action` with `payload`, no form values and no host facts.
fn request(action: &str, payload: Value) -> ActRequest {
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: action.to_owned(),
            payload,
        },
        values: serde_json::Map::new(),
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
        catalog: trust_runtime::control::offline_comm_schema_json(None),
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

/// A `fleet.topology` with one runtime of this project: `line`.
fn topology() -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": "host:line-pc", "hostname": "line-pc", "arch": "aarch64", "os": "linux",
            "ips": ["127.0.0.1"], "containers": [], "source": "self",
            "runtimes": [{
                "runtime_id": "line", "name": "line", "mode": "online", "cycle_ms": 10,
                "health": "connected", "detail": "Runtime answered fleet.topology.",
                "source": "self", "endpoints": [],
            }],
        }],
        "links": [], "shared": [], "external": [],
    })
}

/// A scripted runtime that answers the gather and creates a route.
fn script() -> Script {
    Script::running()
        .answer("status", Reply::Result(fx::status("running", "admin")))
        .answer("fleet.topology", Reply::Result(topology()))
        .answer(
            "ads.route_add",
            Reply::Result(json!({ "status": "route_added" })),
        )
}

/// The report of the project's own runtime `line`, answered at `runtime.endpoint`.
fn project_target(runtime: &FakeRuntime) -> TargetReport {
    fx::answered(
        TargetOrigin::Project,
        &runtime.endpoint,
        None,
        fx::status("running", "admin"),
        topology(),
    )
}

/// The `ads.route_add` requests that arrived at `runtime`.
fn route_adds(runtime: &FakeRuntime) -> Vec<Value> {
    runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "ads.route_add")
        .collect()
}

/// The `Create route` payload that the browse pane sends: its pane, the runtime, and the route plan
/// of the missing-route port.
fn payload(runtime_id: &str) -> Value {
    json!({
        "pane": "browse:ads",
        "runtime": runtime_id,
        "route_name": "trust-runtime-line-controller-1",
        "target": {
            "name": "CX-1234",
            "ip": "192.168.10.5",
            "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851,
            "tc_version": "3.1.4024",
        },
        "local": {
            "host_name": "line-controller-1",
            "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1",
            "nic": "eth0",
            "candidates": [],
            "classification": "lan",
        },
    })
}

#[test]
fn route_add_never_echoes_the_login() {
    // Specification 25:389-394: `Create route` asks once for the TwinCAT computer's administrator
    // login and sends it only to the guarded `ads.route_add` operation, which never stores or
    // echoes it. A wrong implementation keeps the login in the session, repeats it in a retry or a
    // log line, echoes it in the notice, or sends it more than once.
    let runtime = FakeRuntime::start(script());
    let (mut panel, view) = seen(None, vec![project_target(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let mut add = request("ads.route_add", payload(&item));
    add.values.insert("username".to_owned(), json!(USERNAME));
    add.values.insert("password".to_owned(), json!(PASSWORD));
    add.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, add);

    assert!(reply.ok, "the runtime created the route");
    assert_eq!(reply.refresh, Refresh::Now, "the browse pane retries");
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some("browse:ads")
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Created the TwinCAT route 'trust-runtime-line-controller-1'. Browse retries now.")
    );
    let sent = route_adds(&runtime);
    assert_eq!(sent.len(), 1, "the login reaches the runtime exactly once");
    assert_eq!(
        sent[0]["params"]["route_name"],
        json!("trust-runtime-line-controller-1")
    );
    assert_eq!(sent[0]["params"]["channel"], json!("trusted_same_host"));
    assert_eq!(
        sent[0]["params"]["credentials"],
        json!({ "username": USERNAME, "password": PASSWORD })
    );
    let reply_json = serde_json::to_string(&reply).expect("the reply serializes");
    assert!(
        !reply_json.contains(USERNAME),
        "the reply must not echo the login: {reply_json}"
    );
    assert!(
        !reply_json.contains(PASSWORD),
        "the reply must not echo the password: {reply_json}"
    );

    // A refused route: the runtime refuses the guarded operation, and neither the failure notice
    // nor the reply holds any part of the login (specification 25:385-394). A wrong implementation
    // echoes the login in the failure text, keeps it in the session for a retry, or sends it again.
    let refusing = FakeRuntime::start(
        Script::running()
            .answer("status", Reply::Result(fx::status("running", "admin")))
            .answer("fleet.topology", Reply::Result(topology()))
            .answer("ads.route_add", Reply::Refuse("route_refused")),
    );
    let (mut refused_panel, refused_view) = seen(None, vec![project_target(&refusing)]);
    let refused_session = refused_panel.session("dc-test", Instant::now());
    let refused_item = named(&refused_view, "line").id.as_str().to_owned();
    let mut refused = request("ads.route_add", payload(&refused_item));
    refused
        .values
        .insert("username".to_owned(), json!(USERNAME));
    refused
        .values
        .insert("password".to_owned(), json!(PASSWORD));
    refused
        .host
        .tokens
        .insert(refusing.endpoint.clone(), Token::new(TOKEN));

    let refused_reply = act(refused_session, refused);

    assert!(!refused_reply.ok, "the runtime refused the route");
    assert_eq!(refused_reply.refresh, Refresh::None);
    assert_eq!(
        refused_reply
            .notice
            .as_ref()
            .map(|notice| notice.pane.as_str()),
        Some("browse:ads")
    );
    assert_eq!(
        refused_reply
            .notice
            .as_ref()
            .map(|notice| notice.text.as_str()),
        Some("Could not create the TwinCAT route: route refused")
    );
    let refused_json = serde_json::to_string(&refused_reply).expect("the reply serializes");
    assert!(
        !refused_json.contains(USERNAME),
        "the refusal must not echo the login: {refused_json}"
    );
    assert!(
        !refused_json.contains(PASSWORD),
        "the refusal must not echo the password: {refused_json}"
    );
    let refused_sent = route_adds(&refusing);
    assert_eq!(refused_sent.len(), 1, "the login is sent exactly once");
}

#[test]
fn route_add_refuses_an_unknown_runtime_and_writes_nothing() {
    // Specification 25:385-394 and design §4.3 B7: an action resolves the runtime it writes to from
    // what the session last showed. A runtime the session never showed is refused, and no route is
    // created anywhere. A wrong implementation picks another runtime, or sends the login to an
    // endpoint the panel never learned.
    let runtime = FakeRuntime::start(script());
    let (mut panel, _view) = seen(None, vec![project_target(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let mut add = request("ads.route_add", payload("trust-fleet-v1:unknown-runtime"));
    add.values.insert("username".to_owned(), json!(USERNAME));
    add.values.insert("password".to_owned(), json!(PASSWORD));
    add.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, add);

    assert!(!reply.ok);
    assert_eq!(reply.refresh, Refresh::None);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some("browse:ads")
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some(
            "Devices & Connections has not shown the runtime trust-fleet-v1:unknown-runtime. Open \
             the panel again and retry."
        )
    );
    assert_eq!(
        route_adds(&runtime),
        Vec::<Value>::new(),
        "no route was created"
    );
    assert_eq!(
        runtime.requests(),
        Vec::<Value>::new(),
        "nothing reached the runtime, and no login left the panel"
    );
}
