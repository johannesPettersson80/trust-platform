//! `Identify` of a runtime card (specification 25 §5.12; specification 11, "Identify"): the
//! request the panel sends, the notice it shows, and the runtimes it does not send to. A runtime
//! case runs against the scripted fake runtime, so nothing reaches the network. Every expected
//! value is written out from the specifications named in each test.

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
use crate::view::tests::fixtures as fx;
use crate::view::types::{DevicesView, Refresh, Runtime, Tone};

/// The token that the extension stored for a runtime's control endpoint.
const TOKEN: &str = "token-1";

/// A request that performs `runtime.identify` with `payload`, as the runtime's `Identify` button
/// sends it.
fn identify_request(payload: Value) -> ActRequest {
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: "runtime.identify".to_owned(),
            payload,
        },
        values: Map::new(),
        project: None,
        host: HostFacts::default(),
    }
}

/// A `fleet.topology` answer: the runtime `line` on `host:line-pc`, without connections.
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

/// A scripted runtime that answers the gather and `identify` as `identify` says.
fn script(identify: Reply) -> Script {
    Script::running()
        .answer("status", Reply::Result(fx::status("running", "operator")))
        .answer("fleet.topology", Reply::Result(topology()))
        .answer("identify", identify)
}

/// The project's own runtime, answered at `runtime.endpoint`.
fn answering(runtime: &FakeRuntime) -> TargetReport {
    fx::answered(
        TargetOrigin::Project,
        &runtime.endpoint,
        None,
        fx::status("running", "operator"),
        topology(),
    )
}

/// A configured runtime at `endpoint` that did not answer.
fn silent(endpoint: &str) -> TargetReport {
    TargetReport {
        target: Target {
            origin: TargetOrigin::Configured,
            endpoint: endpoint.to_owned(),
            enabled: true,
            token: None,
            label: Some("line".to_owned()),
            managed: None,
            events: false,
        },
        reach: Reach::Probe(TargetProbe::Unreachable),
        live: LiveData::default(),
    }
}

/// A session that has seen the view of `targets`, and that view.
fn seen(targets: Vec<TargetReport>) -> (Sessions, DevicesView) {
    let inputs = ProjectInputs {
        projects: Vec::new(),
        chosen: None,
        topology: None,
        managed: Ok(Vec::new()),
        catalog: offline_comm_schema_json(None),
    };
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

/// The runtime of `view` named `line`.
fn line(view: &DevicesView) -> &Runtime {
    view.computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .find(|runtime| runtime.name == "line")
        .expect("a runtime named line")
}

/// The `identify` requests that arrived at `runtime`.
fn identifies(runtime: &FakeRuntime) -> Vec<Value> {
    runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "identify")
        .collect()
}

#[test]
fn identify_on_a_runtime_card_sends_identify_for_ten_seconds() {
    // Specification 25 §5.12: `Identify` sends `identify {"duration_s": 10}` to the runtime,
    // with the token the extension stored for its endpoint, and shows the runtime's answer as the
    // notice of the runtime's inspector: `Identifying line: its identify output blinks for 10
    // seconds.`, with the seconds the runtime reports. The panel needs no refresh. Wrong
    // behaviour: another duration, another request type, a request without the token (the runtime
    // would refuse it), a notice that does not come from the answer, or a notice in another pane.
    let runtime = FakeRuntime::start(script(Reply::Result(
        json!({"identifying": true, "duration_s": 10}),
    )));
    let (mut panel, view) = seen(vec![answering(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let item = line(&view).id.as_str().to_owned();
    let mut ask = identify_request(json!({ "runtime": item }));
    ask.host
        .tokens
        .insert(runtime.endpoint.clone(), Token::new(TOKEN));

    let reply = act(session, ask);

    assert!(reply.ok);
    let notice = reply.notice.expect("the runtime's answer is a notice");
    assert_eq!(notice.pane, format!("inspector:{item}"));
    assert_eq!(notice.tone, Tone::Ok);
    assert_eq!(
        notice.text,
        "Identifying line: its identify output blinks for 10 seconds."
    );
    assert_eq!(reply.refresh, Refresh::None);
    let sent = identifies(&runtime);
    assert_eq!(sent.len(), 1, "one identify reached the runtime");
    assert_eq!(sent[0]["params"], json!({"duration_s": 10}));
    assert_eq!(sent[0]["auth"], json!(TOKEN));
}

#[test]
fn identify_shows_the_refusal_of_a_runtime_without_an_identify_output() {
    // Specification 25 §5.12 and specification 11, "Identify": a runtime that refuses `identify`
    // (here `identify_not_configured`, as a runtime without `[runtime.identify] output` and a chip
    // do) gives the reason, and the panel shows the runtime's own refusal text as an error notice
    // of the inspector, whatever the text is. Wrong behaviour: a success notice for a refused
    // request, or a refusal replaced by a generic text.
    let runtime = FakeRuntime::start(script(Reply::Refuse("identify_not_configured")));
    let (mut panel, view) = seen(vec![answering(&runtime)]);
    let session = panel.session("dc-test", Instant::now());
    let item = line(&view).id.as_str().to_owned();
    let ask = identify_request(json!({ "runtime": item }));

    let reply = act(session, ask);

    assert!(!reply.ok);
    let notice = reply.notice.expect("the refusal is a notice");
    assert_eq!(notice.pane, format!("inspector:{item}"));
    assert_eq!(notice.tone, Tone::Error);
    assert_eq!(notice.text, "identify not configured");
    assert_eq!(identifies(&runtime).len(), 1);
}

#[test]
fn identify_sends_nothing_to_a_runtime_that_did_not_answer() {
    // Specification 25 §5.12: a runtime that did not answer the last round is refused by name, and
    // nothing is sent to it. Wrong behaviour: a request sent to an address that the view showed as
    // silent.
    let runtime = FakeRuntime::start(script(Reply::Result(
        json!({"identifying": true, "duration_s": 10}),
    )));
    let (mut panel, view) = seen(vec![silent(&runtime.endpoint)]);
    let session = panel.session("dc-test", Instant::now());
    let item = line(&view).id.as_str().to_owned();
    let ask = identify_request(json!({ "runtime": item }));

    let reply = act(session, ask);

    assert!(!reply.ok);
    let notice = reply.notice.expect("the refusal is a notice");
    assert_eq!(notice.tone, Tone::Error);
    assert_eq!(notice.pane, format!("inspector:{item}"));
    assert_eq!(notice.text, "line did not answer, so nothing was sent.");
    assert_eq!(identifies(&runtime).len(), 0, "nothing was sent");
}
