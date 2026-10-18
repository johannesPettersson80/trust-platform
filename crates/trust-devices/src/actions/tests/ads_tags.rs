//! The ADS tag action of `act` (design §4.3 B7 and §3.3 J56, J58, I19, I21, I22, I23, I29, I72 and
//! N36; specification 25 §5.4): ticking a tag of one ADS connection and port imports it, unticking
//! removes it; a running runtime is written over its control endpoint, a stopped project in its own
//! files. Every expected value is written out, taken from the ported TypeScript
//! (`NC/protocolActions.ts`, `NC/adsTagBatch.ts`, `NC/adsImportUx.ts`) and the specification lines
//! named in each test. A live case runs against the scripted fake runtime, so nothing reaches the
//! network; the offline case writes a project folder of its own.

use std::fs;
use std::time::Instant;

use serde_json::{json, Map, Value};
use trust_runtime::control::offline_comm_schema_json;

use crate::actions::act;
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::gather::{LiveData, ProjectInputs, Reach, TargetReport};
use crate::request::{ActOp, ActRequest, HostFacts, RunMode, Token};
use crate::session::Sessions;
use crate::targets::{Target, TargetOrigin};
use crate::test_project::TempFolder;
use crate::view::tests::fixtures as fx;
use crate::view::types::{DevicesView, HostEffect, Op, Refresh, Runtime};

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

/// The parameters of an ADS connection: its name, its host, its ADS net id and its port.
fn ads_params(host: &str, net_id: &str, port: u16) -> Value {
    json!({
        "name": "line1", "host": host, "target_net_id": net_id, "ams_port": port, "points": [],
    })
}

/// The parameters of an ADS connection whose browse reported this host's identity for `port`, as
/// the offline import reads it (`adsPortBrowseEvidence`, `NC/adsDiscoveryPorts.ts:50-77`).
fn ads_params_with_local(host: &str, net_id: &str, port: u16) -> Value {
    let mut params = ads_params(host, net_id, port);
    params["ads_port_browse_results"] = json!([{
        "port": port,
        "tree": [],
        "routePlan": {
            "local": {
                "host_name": "line-controller-1", "chosen_ip": "192.168.10.20",
                "ams_net_id": "192.168.10.20.1.1", "nic": "eth0", "candidates": [],
                "classification": "lan",
            },
        },
    }]);
    params
}

/// A `fleet.topology` of one host with the runtime `line`, in `health`, and its `ads` connection of
/// `params`.
fn ads_topology(health: &str, params: &Value) -> Value {
    fx::report(
        "host:line-pc",
        "line-pc",
        &["127.0.0.1"],
        vec![fx::runtime(
            "line",
            "line",
            health,
            vec![json!({
                "id": "endpoint:line:ads", "kind": "field", "protocol": "ads", "name": "line1",
                "role": "client", "health": "connected", "detail": "Driver is healthy.",
                "owned": true, "supports_test": true, "source": "self", "params": params,
            })],
        )],
    )
}

/// The report of the project's own runtime `line`, answered at `runtime.endpoint`.
fn answered_target(runtime: &FakeRuntime, topology: Value) -> TargetReport {
    fx::answered(
        TargetOrigin::Project,
        &runtime.endpoint,
        None,
        fx::status("running", "admin"),
        topology,
    )
}

/// The report of the project's own runtime `line`, which did not answer before the gather's
/// deadline, so its connections are read and written in the open project's files.
fn unanswered_project(endpoint: &str) -> TargetReport {
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

/// The requests of type `kind` that arrived at `runtime`.
fn requests_of(runtime: &FakeRuntime, kind: &str) -> Vec<Value> {
    runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == kind)
        .collect()
}

/// A cached ADS symbol snapshot of the route `route_name` with two read symbols, as the pane's own
/// browse carries it (`SymbolSnapshot`, `crates/trust-ads-core/src/symbols.rs`).
fn snapshot(route_name: &str) -> Value {
    json!({
        "schema_version": 1,
        "route_name": route_name,
        "symbols": [
            {
                "name": "MAIN.Temperature",
                "data_type": { "source_name": "REAL", "iec_type": "REAL" },
                "index_group": 16_416,
                "index_offset": 8,
                "byte_size": 4,
                "flags": ["read"],
            },
            {
                "name": "GVL.Setpoint",
                "data_type": { "source_name": "REAL", "iec_type": "REAL" },
                "index_group": 16_416,
                "index_offset": 4,
                "byte_size": 4,
                "flags": ["read"],
            },
        ],
    })
}

#[test]
fn ticking_imports_and_unticking_removes_at_the_owning_runtime() {
    // Specification 25:399-401 and design §3.3 J56, I19, I21 and I23: at a running runtime that
    // owns the connection, the ticked tags are imported with `ads.import_symbols.apply` and the
    // unticked ones removed with `ads.import_symbols.remove`, both over its own control endpoint.
    // A wrong implementation writes the project's files while a runtime answers, sends the import
    // to another runtime, drops the unticked tag, or forgets to open the generated ST.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer(
                "ads.import_symbols.apply",
                Reply::Result(json!({
                    "schema_version": 1, "applied": true, "lifecycle_effect": "restart_required",
                    "connection_name": "line1", "selected_count": 2, "candidate_count": 5,
                    "generated_path": "/project/src/generated/ads_generated.st",
                    "message": "Selected ADS symbols were written to ads.toml, cached snapshot, and generated ST.",
                })),
            )
            .answer(
                "ads.import_symbols.remove",
                Reply::Result(json!({
                    "schema_version": 1, "applied": true, "matched": true, "removed_count": 1,
                    "restart_required": true, "files_written": ["ads.toml"],
                    "message": "Removed 1 ADS tag.",
                })),
            ),
    );
    let topology = ads_topology(
        "connected",
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
    );
    let (mut panel, view) = seen(inputs(None), vec![answered_target(&runtime, topology)]);
    let session = panel.session("dc-test", Instant::now());
    let owner = named(&view, "line").id.as_str().to_owned();
    let connection = connection_id(&view, "line", "ads");
    let mut set = request(
        "ads.tags_set",
        json!({ "pane": "browse:ads", "runtime": owner, "connection": connection, "port": 851 }),
    );
    set.values.insert(
        "ticked".to_owned(),
        json!(["MAIN.Temperature", "GVL.Setpoint"]),
    );
    set.values.insert("unticked".to_owned(), json!(["GVL.Old"]));
    set.values.insert("writable".to_owned(), json!(false));
    set.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, set);

    assert!(reply.ok);
    assert_eq!(reply.refresh, Refresh::Now, "the canvas refreshes");
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some("browse:ads")
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Added 2 ADS tags and removed 1 ADS tag from ADS port 851.")
    );
    assert_eq!(
        reply.effects,
        vec![HostEffect::OpenDocument {
            path: "/project/src/generated/ads_generated.st".to_owned(),
        }],
        "the generated Structured Text is opened so diagnostics refresh"
    );
    let applied = requests_of(&runtime, "ads.import_symbols.apply");
    assert_eq!(applied.len(), 1, "one import reached the owning runtime");
    assert_eq!(applied[0]["params"]["connection_name"], json!("line1"));
    assert_eq!(
        applied[0]["params"]["symbols"],
        json!(["MAIN.Temperature", "GVL.Setpoint"])
    );
    assert_eq!(
        applied[0]["params"]["target"],
        json!({
            "name": "line1",
            "ip": "192.168.10.5",
            "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851,
        })
    );
    assert_eq!(applied[0]["params"]["write_acknowledged"], json!(false));
    let removed = requests_of(&runtime, "ads.import_symbols.remove");
    assert_eq!(removed.len(), 1, "one removal reached the owning runtime");
    assert_eq!(
        removed[0]["params"],
        json!({
            "ams_port": 851,
            "host": "192.168.10.5",
            "target_net_id": "5.23.91.12.1.1",
            "paths": ["GVL.Old"],
        })
    );
}

#[test]
fn a_stopped_project_imports_and_removes_offline_and_enables_ads() {
    // Specification 25:399-401 and design §3.3 I19, I23 and I29: a stopped project imports the
    // ticked tags and removes the unticked ones in its own files, with the same Rust functions the
    // `trust-runtime ads` commands use, and the import enables `[runtime.ads]`. A wrong
    // implementation refuses a stopped project, needs a running runtime, or writes `ads.toml`
    // without enabling the ADS client.
    let project = TempFolder::new("ads-tags-offline");
    project.write("runtime.toml", "[bundle]\nversion = 1\n");
    let params = ads_params_with_local("192.168.10.5", "5.23.91.12.1.1", 851);
    let mut project_inputs = inputs(Some(ads_topology("stopped", &params)));
    project_inputs.projects = vec![project.path.clone()];
    project_inputs.chosen = Some(project.path.clone());
    let stopped = "unix:///tmp/trust-devices-ads-tags.sock";
    let (mut panel, view) = seen(project_inputs, vec![unanswered_project(stopped)]);
    let session = panel.session("dc-test", Instant::now());
    let owner = named(&view, "line").id.as_str().to_owned();
    let connection = connection_id(&view, "line", "ads");

    let mut add = request(
        "ads.tags_set",
        json!({ "pane": "browse:ads", "runtime": owner, "connection": connection, "port": 851 }),
    );
    add.project = Some(project.path.clone());
    add.values.insert(
        "ticked".to_owned(),
        json!(["MAIN.Temperature", "GVL.Setpoint"]),
    );
    add.values.insert("writable".to_owned(), json!(false));
    add.values.insert("snapshot".to_owned(), snapshot("line1"));

    let reply = act(session, add);

    assert!(reply.ok, "the stopped project imports offline");
    assert_eq!(reply.refresh, Refresh::Now);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Added 2 ADS tags from ADS port 851.")
    );
    let actions = &reply.notice.as_ref().expect("the result notice").actions;
    assert_eq!(actions.len(), 1, "an offline add offers Start runtime");
    assert_eq!(actions[0].label, "Start runtime");
    assert_eq!(
        actions[0].op,
        Op::Host {
            effect: HostEffect::StartSimulator
        }
    );
    let ads_toml = fs::read_to_string(project.join("ads.toml")).expect("read ads.toml");
    assert!(
        ads_toml.contains("symbol = \"MAIN.Temperature\""),
        "the ticked tag reached ads.toml: {ads_toml}"
    );
    let generated = fs::read_to_string(project.join("src/generated/ads_generated.st"))
        .expect("read the generated ST");
    assert!(
        generated.contains("main_temperature : REAL;"),
        "the generated Structured Text has the tag: {generated}"
    );
    let runtime_toml = fs::read_to_string(project.join("runtime.toml")).expect("read runtime.toml");
    assert!(
        runtime_toml.contains("[runtime.ads]\n") && runtime_toml.contains("\nenabled = true\n"),
        "the import enabled [runtime.ads]: {runtime_toml}"
    );

    let mut remove = request(
        "ads.tags_set",
        json!({ "pane": "browse:ads", "runtime": owner, "connection": connection, "port": 851 }),
    );
    remove.project = Some(project.path.clone());
    remove
        .values
        .insert("unticked".to_owned(), json!(["MAIN.Temperature"]));

    let removed = act(session, remove);

    assert!(removed.ok, "the stopped project removes offline");
    assert_eq!(removed.refresh, Refresh::Now);
    assert_eq!(
        removed.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Removed 1 ADS tag from ADS port 851.")
    );
    let ads_toml = fs::read_to_string(project.join("ads.toml")).expect("read ads.toml");
    assert!(
        !ads_toml.contains("MAIN.Temperature"),
        "the unticked tag left ads.toml: {ads_toml}"
    );
    assert!(
        ads_toml.contains("symbol = \"GVL.Setpoint\""),
        "the other tag stays: {ads_toml}"
    );
}

#[test]
fn an_offline_add_offers_start_runtime() {
    // Specification 25:399-401 and design §3.3 I22 and I72: an offline add's notice carries a
    // `Start runtime` action, so the user can start the stopped project's runtime to use the
    // generated ST. A wrong implementation offers no action, or offers one that starts nothing.
    let project = TempFolder::new("ads-tags-start-runtime");
    project.write("runtime.toml", "[bundle]\nversion = 1\n");
    let mut project_inputs = inputs(Some(ads_topology(
        "stopped",
        &ads_params_with_local("192.168.10.5", "5.23.91.12.1.1", 851),
    )));
    project_inputs.projects = vec![project.path.clone()];
    project_inputs.chosen = Some(project.path.clone());
    let (mut panel, view) = seen(
        project_inputs,
        vec![unanswered_project(
            "unix:///tmp/trust-devices-ads-tags.sock",
        )],
    );
    let session = panel.session("dc-test", Instant::now());
    let owner = named(&view, "line").id.as_str().to_owned();
    let connection = connection_id(&view, "line", "ads");
    let mut add = request(
        "ads.tags_set",
        json!({ "pane": "browse:ads", "runtime": owner, "connection": connection, "port": 851 }),
    );
    add.project = Some(project.path.clone());
    add.values
        .insert("ticked".to_owned(), json!(["MAIN.Temperature"]));
    add.values.insert("snapshot".to_owned(), snapshot("line1"));

    let reply = act(session, add);

    assert!(reply.ok, "the offline add succeeded");
    let notice = reply.notice.expect("the result notice");
    let start = notice
        .actions
        .iter()
        .find(|action| action.id == "start_runtime")
        .expect("a Start runtime action");
    assert_eq!(start.label, "Start runtime");
    assert_eq!(
        start.op,
        Op::Host {
            effect: HostEffect::StartSimulator
        }
    );
}

#[test]
fn tags_set_refuses_another_connection_and_writes_nothing() {
    // Specification 25 §5.4 and design §4.3 B7: `ads.tags_set` works on the runtime's own ADS
    // connection. Another protocol's connection of that runtime is refused, and neither the runtime
    // nor the project is touched. A wrong implementation imports tags through any connection, or
    // writes to a runtime the session named for another connection.
    let runtime = FakeRuntime::start(Script::running());
    let topology = fx::report(
        "host:line-pc",
        "line-pc",
        &["127.0.0.1"],
        vec![fx::runtime(
            "line",
            "line",
            "connected",
            vec![
                json!({
                    "id": "endpoint:line:ads", "kind": "field", "protocol": "ads",
                    "name": "line1", "role": "client", "health": "connected",
                    "detail": "Driver is healthy.", "owned": true, "supports_test": true,
                    "source": "self",
                    "params": ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
                }),
                json!({
                    "id": "endpoint:line:opcua_client", "kind": "field", "protocol": "opcua_client",
                    "name": "server", "role": "client", "health": "connected",
                    "detail": "Driver is healthy.", "owned": true, "supports_test": true,
                    "source": "self",
                    "params": { "name": "server", "endpoint_url": "op.tcp://h:4840" },
                }),
            ],
        )],
    );
    let (mut panel, view) = seen(inputs(None), vec![answered_target(&runtime, topology)]);
    let session = panel.session("dc-test", Instant::now());
    let owner = named(&view, "line").id.as_str().to_owned();
    let other = connection_id(&view, "line", "opcua_client");
    let mut set = request(
        "ads.tags_set",
        json!({ "pane": "browse:ads", "runtime": owner, "connection": other, "port": 851 }),
    );
    set.values
        .insert("ticked".to_owned(), json!(["MAIN.Temperature"]));
    set.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, set);

    assert!(!reply.ok, "another protocol's connection is refused");
    assert_eq!(reply.refresh, Refresh::None);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some("browse:ads")
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("A OPC UA client connection cannot be browsed here yet.")
    );
    assert_eq!(
        runtime.requests(),
        Vec::<Value>::new(),
        "nothing reached the runtime"
    );
}
