//! The form and connection actions of `act` (design §4.3 B7, second part): the runtime each one
//! resolves from the session, the form it opens, and what it writes. Every expected value is
//! written out, taken from the runtime's own answers, the ported TypeScript (`AddDevicePanel.tsx`
//! and `configurationActions.ts`) and the specification 25 lines named in each test. A runtime
//! case runs against the scripted fake runtime, so nothing reaches the network.

use std::time::Instant;

use serde_json::{json, Map, Value};
use trust_runtime::control::offline_comm_schema_json;
use trust_runtime_client::TargetProbe;

use crate::actions::act;
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::gather::{LiveData, ProjectInputs, Reach, TargetReport};
use crate::request::{ActOp, ActRequest, HostFacts, RunMode, Token};
use crate::session::Sessions;
use crate::targets::{Target, TargetOrigin};
use crate::test_project::TempFolder;
use crate::view::tests::fixtures as fx;
use crate::view::types::{
    Connection, DevicesView, FieldError, HostEffect, ItemId, Op, Refresh, Runtime, Tone,
};

/// The project's runtime `line` on `host:line-pc`: `["runtime","host:line-pc",null,"line"]`.
const RUNTIME: &str = "trust-fleet-v1:WyJydW50aW1lIiwiaG9zdDpsaW5lLXBjIixudWxsLCJsaW5lIl0";
/// The `modbus_tcp:0` connection of that runtime: `["endpoint","host:line-pc",null,"line",
/// "endpoint:line:modbus_tcp"]`, the identity `comm.apply`'s `instance_id` maps to.
const CONNECTION: &str =
    "trust-fleet-v1:WyJlbmRwb2ludCIsImhvc3Q6bGluZS1wYyIsbnVsbCwibGluZSIsImVuZHBvaW50OmxpbmU6bW9kYnVzX3RjcCJd";
/// The form that a `modbus_tcp` connection on `RUNTIME` has (`forms::form_id`).
const FORM: &str = "modbus_tcp@trust-fleet-v1:WyJydW50aW1lIiwiaG9zdDpsaW5lLXBjIixudWxsLCJsaW5lIl0";
/// The pane that a result of that form is addressed to (specification 25 §5.13).
const PANE: &str =
    "form:modbus_tcp@trust-fleet-v1:WyJydW50aW1lIiwiaG9zdDpsaW5lLXBjIixudWxsLCJsaW5lIl0";
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

/// The `modbus_tcp` form values that a test submits.
fn values(request: &mut ActRequest) {
    request
        .values
        .insert("address".to_owned(), json!("10.0.0.9:502"));
    request.values.insert("unit_id".to_owned(), json!(1));
}

/// The `comm.apply` requests that arrived at `runtime`.
fn applied(runtime: &FakeRuntime) -> Vec<Value> {
    runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "comm.apply")
        .collect()
}

/// The project inputs of the tests: the tool's own catalog, and the configured topology.
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

/// The `modbus_tcp` connection of `runtime`.
fn modbus(runtime: &Runtime) -> &Connection {
    runtime
        .connections
        .iter()
        .find(|connection| connection.protocol == "modbus_tcp")
        .expect("the modbus_tcp connection of the runtime")
}

/// The project's configured topology: the runtime `line` on `host:line-pc`, stopped, with its
/// `modbus_tcp` connection (`10.0.0.5:502`, unit 2), disabled when `disabled`.
fn configured_topology(disabled: bool) -> Value {
    let (health, detail) = if disabled {
        (
            "disabled",
            "Disabled in io.toml; it will not run until enabled again.",
        )
    } else {
        (
            "configured_policy",
            "Configured in io.toml; runtime is not running.",
        )
    };
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": "host:line-pc", "hostname": "line-pc", "arch": "aarch64", "os": "linux",
            "ips": ["127.0.0.1"], "containers": [], "source": "config",
            "runtimes": [{
                "runtime_id": "line", "name": "line",
                "control_endpoint": "unix:///tmp/trust-runtime-line.sock", "mode": "stopped",
                "cycle_ms": 100, "health": "configured_policy",
                "detail": "Configured in project files; runtime is not running.",
                "source": "config",
                "endpoints": [{
                    "id": "endpoint:line:modbus_tcp", "kind": "field", "protocol": "modbus_tcp",
                    "name": "Modbus meter", "role": "client", "health": health, "detail": detail,
                    "owned": true, "supports_test": true, "source": "config",
                    "params": { "address": "10.0.0.5:502", "unit_id": 2 },
                }],
            }],
        }],
        "links": [], "shared": [], "external": [],
    })
}

/// A `fleet.topology` answer: the runtime `runtime_id` named `name` on `host`, with its
/// `modbus_tcp` connection.
fn topology_of(host: &str, runtime_id: &str, name: &str) -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": host, "hostname": host, "arch": "aarch64", "os": "linux",
            "ips": ["127.0.0.1"], "containers": [], "source": "self",
            "runtimes": [{
                "runtime_id": runtime_id, "name": name, "mode": "online", "cycle_ms": 10,
                "health": "connected", "detail": "Runtime answered fleet.topology.",
                "source": "self",
                "endpoints": [{
                    "id": format!("endpoint:{runtime_id}:modbus_tcp"), "kind": "field",
                    "protocol": "modbus_tcp", "name": "Modbus meter", "role": "client",
                    "health": "connected", "detail": "Driver is healthy.", "owned": true,
                    "supports_test": true, "source": "self",
                    "params": { "address": "10.0.0.5:502", "unit_id": 2 },
                }],
            }],
        }],
        "links": [], "shared": [], "external": [],
    })
}

/// The catalog that a runtime serves: one protocol with one field of its own.
fn served_catalog() -> Value {
    json!({
        "protocols": [{
            "id": "modbus_tcp",
            "title": "Modbus TCP (served)",
            "supports_test": true,
            "fields": [{ "id": "address", "type": "string", "label": "Address" }],
        }],
    })
}

/// A scripted runtime that answers the gather for `topology` and its `comm.apply`.
fn script(topology: Value) -> Script {
    Script::running()
        .answer("status", Reply::Result(fx::status("running", "admin")))
        .answer("fleet.topology", Reply::Result(topology))
}

/// The report of the project's own runtime, answered at `runtime.endpoint` (online mode).
fn project_target(runtime: &FakeRuntime) -> TargetReport {
    fx::answered(
        TargetOrigin::Project,
        &runtime.endpoint,
        None,
        fx::status("running", "admin"),
        topology_of("host:line-pc", "line", "line"),
    )
}

/// The report of a configured runtime that did not answer, labelled `label`.
fn silent_target(endpoint: &str, label: &str) -> TargetReport {
    TargetReport {
        target: Target {
            origin: TargetOrigin::Configured,
            endpoint: endpoint.to_owned(),
            enabled: true,
            token: None,
            label: Some(label.to_owned()),
            managed: None,
            events: false,
        },
        reach: Reach::Probe(TargetProbe::Unreachable),
        live: LiveData::default(),
    }
}

/// A `connection.save` request of the `modbus_tcp` form of `runtime` with the test's values.
fn save_of(runtime: &str) -> ActRequest {
    let mut save = request(
        "connection.save",
        json!({
            "form": format!("modbus_tcp@{runtime}"),
            "protocol": "modbus_tcp",
            "runtime": runtime,
        }),
    );
    values(&mut save);
    save
}

/// A live action (`connection.test`, `connection.apply_live`) of the `modbus_tcp` form of `runtime`
/// with the test's values.
fn live_of(runtime: &str, action: &str) -> ActRequest {
    let mut live = request(
        action,
        json!({
            "form": format!("modbus_tcp@{runtime}"),
            "protocol": "modbus_tcp",
            "runtime": runtime,
        }),
    );
    values(&mut live);
    live
}

#[test]
fn save_returns_the_written_instance_id_and_selects_it() {
    // Specification 25 §5.4 "Successful Save preserves its result and selects/focuses the saved
    // node"; design §4.3 B7, I41 (the id comes from `comm.apply`) and I42 (no re-read of the
    // topology). A wrong implementation re-reads the topology, or selects the wrong connection.
    let project = TempFolder::new("save-select");
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());
    let mut save = save_of(RUNTIME);
    save.project = Some(project.path.clone());

    let reply = act(session, save);

    assert!(reply.ok);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some(PANE)
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("I/O configuration saved. Restart the runtime to apply it.")
    );
    assert!(reply.close_pane);
    assert_eq!(reply.refresh, Refresh::Now);
    assert_eq!(
        reply.effects,
        vec![HostEffect::Select {
            item: ItemId(CONNECTION.to_owned()),
        }]
    );
    let io = std::fs::read_to_string(project.join("io.toml")).expect("the io.toml was written");
    assert!(io.contains("10.0.0.9:502"), "{io}");
}

#[test]
fn save_writes_to_the_runtime_chosen_in_the_form() {
    // Specification 25 §5.14 "`Save` writes to the runtime chosen in the form": the project's own
    // runtime in process, and a runtime that answers over its control channel (O16). A wrong
    // implementation always writes the project, or ignores the chosen runtime.
    let project = TempFolder::new("save-project");
    let stopped = TempFolder::new("save-project-untouched");
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());
    let mut to_project = save_of(RUNTIME);
    to_project.project = Some(project.path.clone());
    let written = act(session, to_project);
    assert!(written.ok);
    assert!(
        project.join("io.toml").is_file(),
        "the project's own runtime is written in process"
    );

    // The project's own runtime that answers: the same form writes over its control channel.
    let runtime = FakeRuntime::start(script(topology_of("host:line-pc", "line", "line")));
    let (mut panel, view) = seen(
        Some(configured_topology(false)),
        vec![project_target(&runtime)],
    );
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let mut to_runtime = save_of(&item);
    to_runtime.project = Some(stopped.path.clone());
    to_runtime
        .host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, to_runtime);

    assert!(reply.ok);
    assert_eq!(
        reply.effects,
        vec![HostEffect::Select {
            item: modbus(named(&view, "line")).id.clone(),
        }]
    );
    let sent = applied(&runtime);
    assert_eq!(sent.len(), 1, "one comm.apply reached the runtime");
    assert_eq!(sent[0]["params"]["protocol"], json!("modbus_tcp"));
    assert_eq!(sent[0]["params"]["action"], json!("add"));
    assert_eq!(
        sent[0]["params"]["params"],
        json!({ "address": "10.0.0.9:502", "unit_id": 1 })
    );
    assert!(
        sent[0]["params"].get("credential_channel").is_none(),
        "a remote runtime decides the credential channel itself"
    );
    assert!(
        !stopped.join("io.toml").exists(),
        "a runtime that answers is written over its control channel, not in process"
    );
}

#[test]
fn validate_reports_field_errors_without_writing() {
    // Design §3.3 I7: the runtime's own `comm.apply` `validate` replaces the panel's client-side
    // check (specification 25 §5.9); a refused field is a field error of that field and nothing is
    // written. A wrong implementation reports success, or writes the io.toml.
    let project = TempFolder::new("validate");
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());
    let mut check = request(
        "form.validate",
        json!({ "form": FORM, "protocol": "modbus_tcp", "runtime": RUNTIME }),
    );
    check.project = Some(project.path.clone());
    check
        .values
        .insert("address".to_owned(), json!("not-an-address"));
    check.values.insert("unit_id".to_owned(), json!(1));

    let reply = act(session, check);

    assert!(!reply.ok);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.tone),
        Some(Tone::Error)
    );
    assert_eq!(
        reply
            .notice
            .as_ref()
            .map(|notice| notice.field_errors.clone()),
        Some(vec![FieldError {
            field: "address".to_owned(),
            message: "Use an IP address and port, for example 127.0.0.1:502.".to_owned(),
        }])
    );
    assert!(
        !project.join("io.toml").exists(),
        "validation writes nothing"
    );
}

#[test]
fn an_unknown_protocol_opens_no_form() {
    // Design §3.2 J05: never a silent fallback to another protocol; the notice names the protocol
    // that was asked for, and no form is returned.
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());

    let reply = act(
        session,
        request(
            "form.open",
            json!({ "protocol": "mystery", "runtime": RUNTIME }),
        ),
    );

    assert!(!reply.ok);
    assert_eq!(reply.form, None);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("The catalog does not hold the protocol 'mystery'.")
    );
}

#[test]
fn remove_disable_and_enable_send_their_action_for_the_instance() {
    // Design §4.3 B7, R4: each names the connection's instance ID (`modbus_tcp:0`), taken from the
    // session's own entry, and its own `comm.apply` action; `Enable` writes the connection enabled
    // again. A wrong implementation sends the whole connection identity as the instance ID, or the
    // wrong action.
    let runtime = FakeRuntime::start(script(topology_of("host:line-pc", "line", "line")));
    let (mut panel, view) = seen(
        Some(configured_topology(false)),
        vec![project_target(&runtime)],
    );
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let connection = modbus(named(&view, "line")).id.as_str().to_owned();

    for (action, expected) in [
        ("connection.remove", "remove"),
        ("connection.disable", "disable"),
        ("connection.enable", "edit"),
    ] {
        let mut change = request(
            action,
            json!({
                "form": format!("modbus_tcp@{item}"),
                "protocol": "modbus_tcp",
                "runtime": item,
                "connection": connection,
            }),
        );
        change
            .host
            .tokens
            .insert(runtime.endpoint.clone(), Token::new(TOKEN));
        values(&mut change);

        let reply = act(session, change);

        assert!(reply.ok, "{action}");
        assert_eq!(reply.refresh, Refresh::Now, "{action}");
        let sent = applied(&runtime);
        let last = sent.last().expect("a comm.apply reached the runtime");
        assert_eq!(last["params"]["action"], json!(expected), "{action}");
        assert_eq!(
            last["params"]["instance_id"],
            json!("modbus_tcp:0"),
            "{action}"
        );
    }
}

#[test]
fn save_to_a_runtime_that_does_not_answer_writes_nothing() {
    // Design §4.3 B7, R1: a runtime that is not the open project's own and does not answer is
    // refused, and nothing is written. A wrong implementation writes the project's files instead,
    // or reaches the runtime anyway.
    let project = TempFolder::new("silent-runtime");
    let runtime = FakeRuntime::start(Script::running());
    let (mut panel, view) = seen(None, vec![silent_target(&runtime.endpoint, "cell1")]);
    let session = panel.session("dc-test", Instant::now());
    let mut save = save_of(named(&view, "cell1").id.as_str());
    save.project = Some(project.path.clone());

    let reply = act(session, save);

    assert!(!reply.ok);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("cell1 did not answer, and it is not this project's runtime, so nothing was saved.")
    );
    assert_eq!(
        applied(&runtime),
        Vec::<Value>::new(),
        "nothing reached the runtime"
    );
    assert!(
        !project.join("io.toml").exists(),
        "nothing was written to the project"
    );
}

#[test]
fn save_to_an_unknown_runtime_writes_nothing() {
    // Design §4.3 B7, R1: an ID the session does not know, and a session that has shown no view
    // yet, are refused. A wrong implementation falls back to the open project's files, which is
    // the wrong `io.toml` for a runtime it cannot resolve.
    let project = TempFolder::new("unknown-runtime");
    let runtime = FakeRuntime::start(Script::running());
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());
    let mut unknown = save_of("trust-fleet-v1:mystery");
    unknown.project = Some(project.path.clone());

    let not_shown = act(session, unknown);

    assert!(!not_shown.ok);
    assert_eq!(
        not_shown.notice.as_ref().map(|notice| notice.text.as_str()),
        Some(
            "Devices & Connections has not shown the runtime trust-fleet-v1:mystery. Open the \
             panel again and retry."
        )
    );
    assert!(
        !project.join("io.toml").exists(),
        "an unknown runtime writes nothing"
    );

    // A session that has shown no view at all.
    let mut blank = Sessions::default();
    let session = blank.session("dc-test", Instant::now());
    let mut fresh = save_of(RUNTIME);
    fresh.project = Some(project.path.clone());

    let no_view = act(session, fresh);

    let not_shown_text = format!(
        "Devices & Connections has not shown the runtime {RUNTIME}. Open the panel again and retry."
    );
    assert!(!no_view.ok);
    assert_eq!(
        no_view.notice.as_ref().map(|notice| notice.text.as_str()),
        Some(not_shown_text.as_str())
    );
    assert!(
        !project.join("io.toml").exists(),
        "a session without a view writes nothing"
    );
    assert_eq!(
        applied(&runtime),
        Vec::<Value>::new(),
        "nothing reached a runtime"
    );
}

#[test]
fn save_to_the_projects_stopped_runtime_writes_the_project_files() {
    // Design §4.3 B7, R1: the open project's own runtime is written in process while it does not
    // answer (specification 25 §5.14). A wrong implementation refuses it, or tries its control
    // endpoint.
    let project = TempFolder::new("stopped-project");
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());
    let mut save = save_of(RUNTIME);
    save.project = Some(project.path.clone());

    let reply = act(session, save);

    assert!(reply.ok);
    assert_eq!(
        reply.effects,
        vec![HostEffect::Select {
            item: ItemId(CONNECTION.to_owned()),
        }]
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("I/O configuration saved. Restart the runtime to apply it.")
    );
    let io = std::fs::read_to_string(project.join("io.toml")).expect("the io.toml was written");
    assert!(io.contains("10.0.0.9:502"), "{io}");
}

#[test]
fn save_to_a_running_runtime_uses_its_control_endpoint() {
    // Design §4.3 B7, R1 and O16: the project's own runtime that answers is written over its
    // control endpoint, not into the project's files. A wrong implementation writes the files.
    let project = TempFolder::new("running-runtime");
    let runtime = FakeRuntime::start(script(topology_of("host:line-pc", "line", "line")));
    let (mut panel, view) = seen(
        Some(configured_topology(false)),
        vec![project_target(&runtime)],
    );
    let session = panel.session("dc-test", Instant::now());
    let mut save = save_of(named(&view, "line").id.as_str());
    save.project = Some(project.path.clone());
    save.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, save);

    assert!(reply.ok);
    assert!(reply.close_pane);
    assert_eq!(reply.refresh, Refresh::Now);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("I/O configuration saved. Restart the runtime to apply it.")
    );
    let sent = applied(&runtime);
    assert_eq!(sent.len(), 1, "one comm.apply reached the runtime");
    assert_eq!(
        sent[0]["params"]["params"],
        json!({ "address": "10.0.0.9:502", "unit_id": 1 })
    );
    assert!(
        !project.join("io.toml").exists(),
        "a runtime that answers is not written in process"
    );
}

#[test]
fn form_open_for_an_edit_shows_the_connections_current_values() {
    // Specification 25 §5.4 and §5.14: `Edit` opens the form of the connection with its current
    // values, and a disabled connection shows that it is disabled. A wrong implementation shows
    // the field defaults or an empty prefill, and hides the disabled state.
    let (mut panel, _view) = seen(Some(configured_topology(true)), Vec::new());
    let session = panel.session("dc-test", Instant::now());

    let reply = act(
        session,
        request(
            "form.open",
            json!({
                "protocol": "modbus_tcp",
                "runtime": RUNTIME,
                "connection": CONNECTION,
            }),
        ),
    );

    assert!(reply.ok);
    let form = reply.form.expect("form.open answers the form");
    assert_eq!(form.id, format!("{FORM}#{CONNECTION}"));
    assert_eq!(form.title, "Modbus TCP");
    assert_eq!(form.owner_line, "on line");
    assert_eq!(
        form.buttons.first().map(|button| button.id.as_str()),
        Some("enable"),
        "a disabled connection offers Enable"
    );
    assert_eq!(
        form.note.as_deref(),
        Some(
            "This endpoint is disabled. Use Enable to turn it back on; restart the runtime to \
             apply the change."
        )
    );
    let field = |id: &str| {
        form.sections
            .iter()
            .flat_map(|section| &section.fields)
            .find(|field| field.id == id)
            .unwrap_or_else(|| panic!("the field {id}"))
    };
    assert_eq!(field("address").value, json!("10.0.0.5:502"));
    assert_eq!(field("unit_id").value, json!(2));
    let test = form
        .buttons
        .iter()
        .find(|button| button.id == "test")
        .expect("the Test button of a protocol that supports it");
    assert!(!test.enabled, "a stopped runtime cannot be tested");
    assert_eq!(
        test.reason.as_deref(),
        Some("Select a reachable online runtime before testing Communication setup.")
    );
}

#[test]
fn form_open_uses_the_catalog_that_the_runtime_serves() {
    // Specification 25 §5.14 "Its catalog always comes from Rust, from the runtime when one serves
    // it"; §5.13 "`Test` needs the connection's runtime to answer". A wrong implementation always
    // uses the tool's own catalog (six `modbus_tcp` fields), or marks a runtime that answers
    // unreachable.
    let peer = FakeRuntime::start(Script::running());
    let mut target = fx::answered(
        TargetOrigin::Configured,
        &peer.endpoint,
        Some("cell1"),
        fx::status("running", "admin"),
        topology_of("host:cell-node", "cell", "cell1"),
    );
    target.live.schema = Some(Ok(served_catalog()));
    let (mut panel, view) = seen(Some(configured_topology(false)), vec![target]);
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "cell1").id.as_str().to_owned();

    let reply = act(
        session,
        request(
            "form.open",
            json!({ "protocol": "modbus_tcp", "runtime": item }),
        ),
    );

    assert!(reply.ok);
    let form = reply.form.expect("form.open answers the form");
    assert_eq!(form.title, "Add Modbus TCP (served)");
    assert_eq!(form.owner_line, "on cell1");
    let fields: Vec<&str> = form
        .sections
        .iter()
        .flat_map(|section| &section.fields)
        .map(|field| field.id.as_str())
        .collect();
    assert_eq!(
        fields,
        ["address"],
        "the catalog that the runtime serves, not the tool's own"
    );
    let test = form
        .buttons
        .iter()
        .find(|button| button.id == "test")
        .expect("the Test button of a protocol that supports it");
    assert!(test.enabled, "the runtime answers");
    assert_eq!(test.reason, None);
}

#[test]
fn form_open_offers_the_runtimes_that_can_take_the_connection() {
    // Specification 25 §5.14: the form names the runtime it writes to and offers the runtimes a
    // new connection can be saved to. A wrong implementation offers a runtime that neither
    // answers nor owns the project, which would refuse the save.
    let peer = FakeRuntime::start(Script::running());
    let silent = FakeRuntime::start(Script::running());
    let targets = vec![
        fx::answered(
            TargetOrigin::Configured,
            &peer.endpoint,
            Some("cell1"),
            fx::status("running", "admin"),
            topology_of("host:cell-node", "cell", "cell1"),
        ),
        silent_target(&silent.endpoint, "cell2"),
    ];
    let (mut panel, _view) = seen(Some(configured_topology(false)), targets);
    let session = panel.session("dc-test", Instant::now());

    let reply = act(
        session,
        request(
            "form.open",
            json!({ "protocol": "modbus_tcp", "runtime": RUNTIME }),
        ),
    );

    assert!(reply.ok);
    let form = reply.form.expect("form.open answers the form");
    assert_eq!(form.owner_line, "on line");
    assert_eq!(
        form.runtime.as_ref().map(|runtime| runtime.id.as_str()),
        Some(RUNTIME)
    );
    let names: Vec<&str> = form
        .runtime_choices
        .iter()
        .map(|runtime| runtime.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["line", "cell1"],
        "the open project's own runtime and the runtime that answers; never cell2"
    );
}

#[test]
fn test_and_apply_live_need_a_reachable_runtime_with_reason() {
    // Specification 25 §5.13 "`Test` stays visible but disabled, with its reason, while the
    // connection's runtime does not answer" and §5.14 "`Test` and `Apply to running runtime` act on
    // that runtime". A wrong implementation tests anyway, falls back to the open project's files, or
    // answers with a reason of its own instead of the disabled button's.
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());
    let opened = act(
        session,
        request(
            "form.open",
            json!({ "protocol": "modbus_tcp", "runtime": RUNTIME }),
        ),
    )
    .form
    .expect("form.open answers the form");
    let reason = |id: &str| {
        opened
            .buttons
            .iter()
            .find(|button| button.id == id)
            .and_then(|button| button.reason.clone())
    };

    let test = act(session, live_of(RUNTIME, "connection.test"));
    assert!(!test.ok);
    assert_eq!(
        test.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some(PANE)
    );
    assert_eq!(
        test.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Select a reachable online runtime before testing Communication setup.")
    );
    assert_eq!(
        reason("test").as_deref(),
        test.notice.as_ref().map(|notice| notice.text.as_str()),
        "the refusal is the disabled `Test` button's own reason"
    );

    let apply = act(session, live_of(RUNTIME, "connection.apply_live"));
    assert!(!apply.ok);
    assert_eq!(
        apply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Select a reachable online runtime before applying Communication setup.")
    );
    assert_eq!(
        reason("apply_live").as_deref(),
        apply.notice.as_ref().map(|notice| notice.text.as_str()),
        "the refusal is the disabled `Apply to running runtime` button's own reason"
    );

    // A remote runtime that did not answer: refused with the same reason, and never reached.
    let silent = FakeRuntime::start(Script::running());
    let (mut panel, view) = seen(None, vec![silent_target(&silent.endpoint, "cell1")]);
    let session = panel.session("dc-test", Instant::now());
    let cell = named(&view, "cell1").id.as_str().to_owned();

    let refused = act(session, live_of(&cell, "connection.test"));

    assert!(!refused.ok);
    assert_eq!(
        refused.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Select a reachable online runtime before testing Communication setup.")
    );
    assert_eq!(
        applied(&silent),
        Vec::<Value>::new(),
        "a runtime that does not answer is not reached"
    );

    // A runtime the session does not know: refused by name.
    let unknown = act(
        session,
        live_of("trust-fleet-v1:mystery", "connection.test"),
    );

    assert!(!unknown.ok);
    assert_eq!(
        unknown.notice.as_ref().map(|notice| notice.text.as_str()),
        Some(
            "Devices & Connections has not shown the runtime trust-fleet-v1:mystery. Open the \
             panel again and retry."
        )
    );
}

#[test]
fn opcua_test_failures_have_recovery_text() {
    // Specification 25 §5.10 "An unreachable OPC UA client test reports that the OPC UA server is
    // not reachable and MUST NOT expose raw backend status tokens such as `BadNotConnected`", with
    // the recovery texts the extension's `commTestMessage` already gives. A wrong implementation
    // shows the runtime's raw backend detail, or the wrong recovery text.
    for (code, expected) in [
        (
            "endpoint_unreachable",
            "OPC UA server is not reachable. Check the endpoint URL, port, and that the server is running.",
        ),
        (
            "auth_required",
            "The OPC UA server needs valid credentials. Enter the username and password, then test again.",
        ),
        (
            "cert_untrusted",
            "The OPC UA server certificate is not trusted. Use Browse nodes and choose Trust certificate if this is the expected server.",
        ),
        (
            "unsupported_security_profile",
            "The OPC UA server does not offer the selected security policy and mode. Pick a supported endpoint and test again.",
        ),
        (
            "browse_denied",
            "The OPC UA server denied access. Check the selected credentials and permissions.",
        ),
    ] {
        let failure = "OPC UA endpoint handshake failed: BadNotConnected";
        let runtime = FakeRuntime::start(
            script(topology_of("host:line-pc", "line", "line")).answer(
                "comm.test",
                Reply::Result(json!({
                    "schema_version": 1,
                    "protocol": "opcua_client",
                    "supported": true,
                    "ok": false,
                    "detail": failure,
                    "error": { "code": code, "message": failure },
                    "field_errors": [],
                })),
            ),
        );
        let (mut panel, view) = seen(
            Some(configured_topology(false)),
            vec![project_target(&runtime)],
        );
        let session = panel.session("dc-test", Instant::now());
        let item = named(&view, "line").id.as_str().to_owned();
        let mut test = request(
            "connection.test",
            json!({
                "form": format!("opcua_client@{item}"),
                "protocol": "opcua_client",
                "runtime": item,
            }),
        );
        test.values.insert("enabled".to_owned(), json!(true));
        test.host
            .tokens
            .insert(runtime.endpoint.clone(), Token::new(TOKEN));

        let reply = act(session, test);

        assert!(!reply.ok, "{code} fails the test");
        assert_eq!(
            reply.notice.as_ref().map(|notice| notice.tone),
            Some(Tone::Error),
            "{code}"
        );
        assert_eq!(
            reply.notice.as_ref().map(|notice| notice.text.as_str()),
            Some(expected),
            "{code} has its own recovery text"
        );
        assert!(
            !reply
                .notice
                .as_ref()
                .expect("a notice")
                .text
                .contains("BadNotConnected"),
            "{code} never exposes the raw backend token"
        );
    }
}

#[test]
fn secret_over_plain_remote_tcp_is_refused_by_the_server() {
    // Specification 11 §6.9.7 and decision O16: a secret over a remote plain-TCP control channel
    // is refused by the runtime itself, which stamps the channel, and shown as the notice. A wrong
    // implementation sends the secret anyway, shows success, or refuses it on the client, which
    // would also refuse it on the trusted same-host channel.
    let refusal = "Secret fields cannot be sent over an untrusted runtime control channel.";

    // The runtime sees an untrusted remote plain-TCP client: both live actions are refused, and the
    // secret did reach the runtime, which refused it.
    let untrusted =
        FakeRuntime::start(script(topology_of("host:line-pc", "line", "line")).untrusted_channel());
    let (mut panel, view) = seen(
        Some(configured_topology(false)),
        vec![project_target(&untrusted)],
    );
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let secret = |action: &str| {
        let mut live = request(
            action,
            json!({
                "form": format!("mqtt@{item}"),
                "protocol": "mqtt",
                "runtime": item,
            }),
        );
        live.values
            .insert("broker".to_owned(), json!("10.0.0.7:1883"));
        live.values.insert("password".to_owned(), json!("s3cret"));
        live.host
            .tokens
            .insert(untrusted.endpoint.clone(), Token::new(TOKEN));
        live
    };

    for action in ["connection.apply_live", "connection.test"] {
        let reply = act(session, secret(action));

        assert!(!reply.ok, "{action}");
        assert_eq!(
            reply.notice.as_ref().map(|notice| notice.tone),
            Some(Tone::Error),
            "{action}"
        );
        assert_eq!(
            reply.notice.as_ref().map(|notice| notice.text.as_str()),
            Some(refusal),
            "{action}"
        );
        assert_eq!(
            reply
                .notice
                .as_ref()
                .map(|notice| notice.field_errors.clone()),
            Some(vec![FieldError {
                field: "password".to_owned(),
                message: refusal.to_owned(),
            }]),
            "{action} names the refused field"
        );
    }
    let applied_requests = applied(&untrusted);
    assert_eq!(applied_requests.len(), 1, "the apply reached the runtime");
    assert_eq!(
        applied_requests[0]["params"]["params"]["password"],
        json!("s3cret"),
        "the secret reached the runtime, which refused it"
    );
    let tested: Vec<Value> = untrusted
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "comm.test")
        .collect();
    assert_eq!(tested.len(), 1, "the test reached the runtime");
    assert_eq!(tested[0]["params"]["params"]["password"], json!("s3cret"));

    // The same secret over a trusted same-host channel is applied: the refusal is the server's
    // channel, not the panel's own rule.
    let trusted = FakeRuntime::start(script(topology_of("host:line-pc", "line", "line")));
    let (mut panel, view) = seen(
        Some(configured_topology(false)),
        vec![project_target(&trusted)],
    );
    let session = panel.session("dc-test", Instant::now());
    let item = named(&view, "line").id.as_str().to_owned();
    let mut live = request(
        "connection.apply_live",
        json!({
            "form": format!("mqtt@{item}"),
            "protocol": "mqtt",
            "runtime": item,
        }),
    );
    live.values
        .insert("broker".to_owned(), json!("10.0.0.7:1883"));
    live.values.insert("password".to_owned(), json!("s3cret"));
    live.host
        .tokens
        .insert(trusted.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, live);

    assert!(reply.ok, "a trusted same-host channel applies the secret");
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("I/O configuration saved. Restart the runtime to apply it.")
    );
}

#[test]
fn diagnose_answers_its_text() {
    // Specification 25 §5.13: an ADS connection offers `Test` and `Diagnose`, whose checks are the
    // ADS Doctor's and arrive in a later slice (inventory N28). Until then `connection.diagnose`
    // answers honestly that the diagnosis is not available, in the form's pane. A wrong
    // implementation claims a diagnosis, or succeeds silently.
    let (mut panel, _view) = seen(Some(configured_topology(false)), Vec::new());
    let session = panel.session("dc-test", Instant::now());

    let reply = act(
        session,
        request(
            "connection.diagnose",
            json!({ "form": FORM, "protocol": "modbus_tcp", "runtime": RUNTIME, "connection": CONNECTION }),
        ),
    );

    assert!(!reply.ok);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some(PANE)
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.tone),
        Some(Tone::Error)
    );
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some(
            "The connection's diagnosis is not available yet. The ADS Doctor checks arrive in a \
             later release."
        )
    );
}

#[test]
fn choosing_another_runtime_reopens_the_form_with_the_typed_values() {
    // Specification 25 §5.14: the form names the runtime it writes to and offers the runtimes a new
    // connection can be saved to; choosing one reopens the form on it with the values typed so far
    // as its prefill. A wrong implementation draws the choice but decides nothing, reopens on the
    // original runtime, or loses the typed values.
    let peer = FakeRuntime::start(Script::running());
    let (mut panel, view) = seen(
        Some(configured_topology(false)),
        vec![fx::answered(
            TargetOrigin::Configured,
            &peer.endpoint,
            Some("cell1"),
            fx::status("running", "admin"),
            topology_of("host:cell-node", "cell", "cell1"),
        )],
    );
    let session = panel.session("dc-test", Instant::now());
    let cell = named(&view, "cell1").id.as_str().to_owned();

    let opened = act(
        session,
        request(
            "form.open",
            json!({ "protocol": "modbus_tcp", "runtime": RUNTIME }),
        ),
    )
    .form
    .expect("form.open answers the form");
    assert_eq!(
        opened.owner_line, "on line",
        "the form starts on the runtime it names"
    );

    let choice = opened
        .runtime_choices
        .iter()
        .find(|choice| choice.action.id == cell)
        .unwrap_or_else(|| panic!("the choice of cell1"));
    assert_eq!(choice.name, "cell1", "the choice names the runtime");
    let Op::Rust { action, payload } = &choice.action.op else {
        panic!("a choice posts a Rust action");
    };
    assert_eq!(action, "form.open", "choosing reopens the form");
    assert_eq!(
        payload["runtime"],
        json!(cell),
        "the chosen runtime is the one it reopens on"
    );
    assert_eq!(
        payload["form"],
        json!(opened.id),
        "the action names the form it came from"
    );

    // The webview posts the chosen action with the values the user typed, which the reopened form
    // keeps as its values.
    let mut reopen = request("form.open", payload.clone());
    reopen
        .values
        .insert("address".to_owned(), json!("10.0.0.9:502"));
    reopen.values.insert("unit_id".to_owned(), json!(7));

    let reopened = act(session, reopen).form.expect("the form reopens");

    assert_eq!(
        reopened.runtime.as_ref().map(|runtime| runtime.id.as_str()),
        Some(cell.as_str()),
        "the reopened form writes to the chosen runtime"
    );
    assert_eq!(reopened.owner_line, "on cell1");
    assert_ne!(
        reopened.id, opened.id,
        "a form with different values is a different form"
    );
    let field = |id: &str| {
        reopened
            .sections
            .iter()
            .flat_map(|section| &section.fields)
            .find(|field| field.id == id)
            .unwrap_or_else(|| panic!("the field {id}"))
    };
    assert_eq!(field("address").value, json!("10.0.0.9:502"));
    assert_eq!(field("unit_id").value, json!(7));
    assert_eq!(
        reopened
            .runtime_choices
            .iter()
            .map(|choice| choice.name.as_str())
            .collect::<Vec<&str>>(),
        ["line", "cell1"],
        "the reopened form still offers the runtimes"
    );
}
