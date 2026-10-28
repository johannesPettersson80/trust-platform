//! The ADS tag browser's tests (design §4.3 B8 and §3.3 J40, J46, J54, J55, J57, J58 and J59;
//! specification 25 §5.4 and its lines 878-883): the loading pane, a found device's own target,
//! the typed wire errors and their human text, the missing-route help with `Create route`, the
//! port suggestions and their help, the port check, and the `added` flags that follow the
//! project's ADS configuration. Every expected value is written out, taken from the ported
//! TypeScript and the specification lines named in each test. Their inputs and helpers are in the
//! parent, which every one of these suites shares.

use std::time::Instant;

use serde_json::{json, Map, Value};

use super::{
    answer, browse_body, browse_request, connection_protocol, endpoint, inputs, report,
    requests_of, runtime_named, script, seen, wait_for_browse,
};
use crate::actions::act;
use crate::browse::open;
use crate::fake_runtime::FakeRuntime;
use crate::request::{ActOp, ActRequest, BrowseRequest, HostFacts};
use crate::session::Sessions;
use crate::targets::TargetOrigin;
use crate::view::tests::fixtures as fx;
use crate::view::types::{
    AdsPortCard, BrowseKind, BrowseView, FieldKind, FieldOption, HostEffect, Op,
};

/// A session that has seen one ADS connection of a single-runtime project, the browse request
/// that reads its port `port`, the owning runtime's item ID, and the scripted runtime itself.
fn ads_setup(
    params: &Value,
    answer: Value,
    port: Option<u16>,
) -> (Sessions, BrowseRequest, String, FakeRuntime) {
    let runtime = FakeRuntime::start(script(answer));
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint("line", "ads", None, "line1", params)],
        ),
    );
    let (panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let owner = runtime_named(&view, "line").id.as_str().to_owned();
    let connection = connection_protocol(&view, "line", "ads")
        .id
        .as_str()
        .to_owned();
    let mut body = browse_body(&connection, "line1", &runtime.endpoint, 1, false);
    if let Some(port) = port {
        body["port"] = json!(port);
    }
    (panel, browse_request(&body), owner, runtime)
}

/// Opens a live ADS browse of a single-runtime project and returns the finished view and the
/// owning runtime's item ID.
fn open_ads(params: &Value, answer: Value, port: Option<u16>) -> (BrowseView, String) {
    let (mut panel, request, owner, _runtime) = ads_setup(params, answer, port);
    let _ = open(panel.session("dc-test", Instant::now()), &request);
    wait_for_browse(&mut panel);
    (
        open(panel.session("dc-test", Instant::now()), &request),
        owner,
    )
}

/// The configuration of an ADS connection: its host, its ADS net id and its port.
fn ads_params(host: &str, net_id: &str, port: u16) -> Value {
    json!({
        "name": "line1", "host": host, "target_net_id": net_id, "ams_port": port, "points": [],
    })
}

/// One leaf of an ADS browse answer, as the runtime's symbol table gives it.
fn ads_leaf(id: &str, path: &str, data_type: &str, writable: Option<bool>) -> Value {
    let name = path.rsplit('.').next().unwrap_or(path);
    let mut node = json!({ "id": id, "name": name, "path": path, "type": data_type });
    if let Some(writable) = writable {
        node["writable"] = json!(writable);
    }
    node
}

/// A `comm.browse_symbols` answer of the ADS protocol with `tree`.
fn ads_answer(tree: Value) -> Value {
    answer("ads", "symbols", tree)
}

/// An ADS browse answer with a structured `error`.
fn ads_failed(code: &str) -> Value {
    json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols", "tree": [],
        "error": { "code": code, "message": "ADS port 851 symbol browse failed: the server refused." },
    })
}

/// An ADS browse answer whose route is missing, with `plan`.
fn route_missing(plan: &Value) -> Value {
    json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols", "tree": [],
        "route": {
            "status": "missing",
            "detail": "ADS port 851 route check failed: no route",
            "action": "ads.route_plan",
            "route_plan": plan,
        },
        "warnings": ["ADS route is not ready; create or fix the route before browsing symbols."],
    })
}

#[test]
fn a_browse_view_says_it_is_loading() {
    // Requirement R1; design §1.5, line 381: a browse view says it is loading from the first answer
    // until the origin has answered. A wrong implementation leaves `loading` false on the first
    // poll, so B8a's empty tree and its wait reason stand in, or keeps saying loading after the
    // answer arrived.
    let (mut panel, request, _owner, _runtime) = ads_setup(
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
        ads_answer(json!([ads_leaf(
            "ads:symbol:MAIN.x",
            "MAIN.x",
            "INT",
            Some(false)
        )])),
        None,
    );

    let first = open(panel.session("dc-test", Instant::now()), &request);
    assert!(first.loading, "the first poll loads");
    assert!(
        first.add.is_none(),
        "an ADS pane has no Add button: each tag is ticked on its own"
    );
    assert_eq!(first.ports[0].state_text, "Loading…");
    assert_eq!(
        first.ports[0].empty_text.as_deref(),
        Some("Loading tags…"),
        "the loading card says its tags are loading, in Rust's words"
    );

    wait_for_browse(&mut panel);
    let second = open(panel.session("dc-test", Instant::now()), &request);
    assert!(!second.loading, "the later poll has the answer");
    assert_eq!(second.ports[0].tags.len(), 1);
}

#[test]
fn a_found_device_is_browsed_at_its_own_target() {
    // Requirement R2; design §4.3 B8, line 261, and specification 11, "Symbol browsing": a found
    // device is browsed at its own address, ADS net id and port. A wrong implementation browses
    // the project's runtime (B8a sent an empty target) or the project's own ADS connection.
    let runtime = FakeRuntime::start(script(ads_answer(json!([ads_leaf(
        "ads:symbol:MAIN.x",
        "MAIN.x",
        "INT",
        Some(false)
    )]))));
    let project = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint(
                "line",
                "ads",
                None,
                "project plc",
                &json!({
                    "name": "project plc", "host": "10.0.0.9", "target_net_id": "1.2.3.4.5.6",
                    "ams_port": 851,
                }),
            )],
        ),
    );
    let (mut panel, _view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            project,
        )],
    );
    let body = json!({
        "session": "dc-test",
        "seq": 1,
        "source": {
            "found": "card:ads",
            "protocol": "ads",
            "target": {
                "name": "CX-1234",
                "host": "192.168.10.5",
                "ams_net_id": "5.23.91.12.1.1",
                "ams_port": 852,
                "responding_ads_ports": [851, 852],
                "ads_port_browse_results": [{ "port": 851, "tree": [], "routeMissing": false }],
            },
        },
        "kind": "symbols",
        "host": {},
    });
    let request = browse_request(&body);

    let _ = open(panel.session("dc-test", Instant::now()), &request);
    wait_for_browse(&mut panel);
    let view = open(panel.session("dc-test", Instant::now()), &request);

    let sent = requests_of(&runtime, "comm.browse_symbols");
    assert_eq!(sent.len(), 1, "one ADS browse reached the runtime");
    assert_eq!(sent[0]["params"]["protocol"], json!("ads"));
    assert_eq!(sent[0]["params"]["kind"], json!("symbols"));
    assert_eq!(
        sent[0]["params"]["target"],
        json!({
            "ip": "192.168.10.5", "ams_net_id": "5.23.91.12.1.1", "name": "CX-1234", "ams_port": 852,
        }),
        "the found device's own target is browsed"
    );
    assert_eq!(
        view.ports.iter().map(|card| card.port).collect::<Vec<_>>(),
        vec![851, 852],
        "one card per port, ascending"
    );
    assert_eq!(
        view.ports[1].tags.len(),
        1,
        "the device's port 852 has its tag"
    );
    assert_eq!(view.ports[1].tags[0].key, "852:ads:symbol:MAIN.x");
}

#[test]
fn ads_error_boxes_from_typed_wire_errors() {
    // Design §3.3 J46 and `classifyBrowseError` (`W/browseErrorModel.ts:13-55`): each typed ADS
    // wire error is one human title and detail, never its raw code. A wrong implementation shows
    // the code, or falls back to the generic text for a known code.
    let cases = [
        (
            "ads_port_unavailable",
            "ADS port unavailable",
            "The selected ADS server did not answer. Check the port and that this server is \
             running, then browse again.",
        ),
        (
            "symbol_upload_unsupported",
            "Symbol Upload unsupported",
            "This ADS server is reachable but does not expose Beckhoff Symbol Upload. Enable \
             symbol generation for that server or choose another ADS port.",
        ),
        (
            "empty_symbol_table",
            "No compatible symbols",
            "The ADS server returned an empty symbol table or no symbols that truST can import.",
        ),
    ];
    for (code, title, detail) in cases {
        let (view, _) = open_ads(
            &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
            ads_failed(code),
            None,
        );
        assert_eq!(view.error, None, "{code} belongs to the port, not the pane");
        let error = view.ports[0]
            .error
            .clone()
            .unwrap_or_else(|| panic!("{code} answers a structured error"));
        assert_eq!(error.title, title, "{code} has a human title");
        assert_eq!(error.detail, detail, "{code} has human help");
        assert!(!error.title.contains(code), "{code} is never shown raw");
        assert!(!error.detail.contains(code), "{code} is never shown raw");
    }

    let (view, _) = open_ads(
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
        ads_failed("symbol_upload_failed"),
        None,
    );
    let error = view.ports[0].error.clone().expect("the generic failure");
    assert_eq!(error.title, "ADS symbol browse failed");
    assert_eq!(
        error.detail, "ADS port 851 symbol browse failed: the server refused.",
        "the origin's own message is kept"
    );
}

#[test]
fn route_missing_port_card_has_help_and_create_route() {
    // Specification 25:387-394, design §3.3 J54 and N25/N26: a port whose route is missing shows
    // the route help and its generated files, and `Create route` opens the login form with the
    // pane, the runtime, the route name and the plan's own target and local, which the form's
    // `Create route` then posts to `ads.route_add` (`create_route_asks_for_the_login_once_and_
    // posts_route_add`). A wrong implementation drops the runtime or the target, posts another
    // action, or offers no help.
    let plan = json!({
        "route_name": "trust-runtime-line-controller-1",
        "target": {
            "name": "CX-1234", "ip": "192.168.10.5", "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851, "tc_version": "3.1.4024",
        },
        "local": {
            "host_name": "line-controller-1", "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1", "nic": "eth0", "candidates": [],
            "classification": "lan",
        },
        "channel": "trusted_same_host",
        "automatic_route": "available",
        "artifacts": [{
            "kind": "powershell", "label": "Download PowerShell",
            "filename": "add-ads-route-x.ps1", "content_type": "text/x-powershell",
            "content": "# generated",
        }],
    });
    let (view, owner) = open_ads(
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
        route_missing(&plan),
        None,
    );

    let route = view.ports[0]
        .route
        .clone()
        .expect("the missing route's help");
    assert_eq!(
        route.text,
        "An ADS route is required before tags can be read from port 851."
    );
    assert_eq!(
        route.help,
        vec![
            "Route needs TwinCAT administrator access. Run the generated route script on the \
             TwinCAT computer, then reopen Browse."
                .to_owned()
        ]
    );
    assert_eq!(route.artifacts.len(), 1);
    assert_eq!(route.artifacts[0].label, "Copy Download PowerShell");
    match &route.artifacts[0].op {
        Op::Host {
            effect: HostEffect::Copy { text },
        } => assert_eq!(text, "# generated"),
        other => panic!("a copy action, got {other:?}"),
    }

    let create = &route.create_route;
    assert_eq!(create.label, "Create route");
    assert!(create.enabled);
    match &create.op {
        Op::Rust { action, payload } => {
            assert_eq!(action, "form.open");
            assert_eq!(
                payload["form"],
                json!(format!("ads.route@{owner}#trust-runtime-line-controller-1")),
                "the login form is identified by its runtime and route name"
            );
            assert_eq!(payload["route"]["pane"], json!("browse:ads"));
            assert_eq!(payload["route"]["runtime"], json!(owner));
            assert_eq!(
                payload["route"]["route_name"],
                json!("trust-runtime-line-controller-1")
            );
            assert_eq!(payload["route"]["target"], plan["target"]);
            assert_eq!(payload["route"]["local"], plan["local"]);
        }
        other => panic!("a Rust form.open, got {other:?}"),
    }
}

#[test]
fn create_route_asks_for_the_login_once_and_posts_route_add() {
    // Specification 25:387-394 and design N26: `Create route` opens a form that asks **once** for
    // the TwinCAT computer's administrator user name and password, and that form's `Create route`
    // posts `ads.route_add` with the pane, the runtime, the route name and the plan's own target
    // and local. A wrong implementation sends `ads.route_add` straight from the pane (asking for
    // nothing), asks for the login twice or not as a secret, or posts another action.
    let plan = json!({
        "route_name": "trust-runtime-line-controller-1",
        "target": {
            "name": "CX-1234", "ip": "192.168.10.5", "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851, "tc_version": "3.1.4024",
        },
        "local": {
            "host_name": "line-controller-1", "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1", "nic": "eth0", "candidates": [],
            "classification": "lan",
        },
        "artifacts": [],
    });
    let (mut panel, body, owner, _runtime) = ads_setup(
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
        route_missing(&plan),
        None,
    );
    let _ = open(panel.session("dc-test", Instant::now()), &body);
    wait_for_browse(&mut panel);
    let view = open(panel.session("dc-test", Instant::now()), &body);
    let create = view.ports[0]
        .route
        .clone()
        .expect("the missing route's help")
        .create_route;
    let Op::Rust { action, payload } = &create.op else {
        panic!("Create route opens a form");
    };
    assert_eq!(action, "form.open");

    let opened = act(
        panel.session("dc-test", Instant::now()),
        ActRequest {
            session: "dc-test".to_owned(),
            op: ActOp {
                action: action.clone(),
                payload: payload.clone(),
            },
            values: Map::new(),
            project: None,
            host: HostFacts::default(),
        },
    );

    let form = opened.form.expect("the login form");
    assert_eq!(form.title, "Create route");
    let id = format!("ads.route@{owner}#trust-runtime-line-controller-1");
    assert_eq!(form.id, id);
    let fields: Vec<(&str, FieldKind, bool)> = form.sections[0]
        .fields
        .iter()
        .map(|field| (field.id.as_str(), field.kind, field.required))
        .collect();
    assert_eq!(
        fields,
        vec![
            ("username", FieldKind::Text, true),
            ("password", FieldKind::Secret, true),
        ],
        "the form asks once for the login, and the password is a secret"
    );
    let submit = form
        .buttons
        .iter()
        .find(|button| button.id == "create_route")
        .expect("the Create route button");
    assert_eq!(submit.label, "Create route");
    match &submit.op {
        Op::Rust { action, payload } => {
            assert_eq!(action, "ads.route_add");
            assert_eq!(payload["pane"], json!(format!("form:{id}")));
            assert_eq!(payload["runtime"], json!(owner));
            assert_eq!(
                payload["route_name"],
                json!("trust-runtime-line-controller-1")
            );
            assert_eq!(payload["target"], plan["target"]);
            assert_eq!(payload["local"], plan["local"]);
        }
        other => panic!("a Rust ads.route_add, got {other:?}"),
    }
}

#[test]
fn port_suggestions_and_help() {
    // Specification 25:880-883, design §3.3 J55 and N23: the ADS browser shows the four port
    // suggestions with their names and their help. A wrong implementation drops them, orders them
    // differently, or mislabels a port.
    let (view, _) = open_ads(
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
        ads_answer(json!([])),
        None,
    );
    assert_eq!(
        view.port_suggestions,
        vec![
            FieldOption {
                value: "301".to_owned(),
                label: "I/O".to_owned(),
            },
            FieldOption {
                value: "501".to_owned(),
                label: "Motion".to_owned(),
            },
            FieldOption {
                value: "851".to_owned(),
                label: "PLC runtime 1".to_owned(),
            },
            FieldOption {
                value: "852".to_owned(),
                label: "PLC runtime 2".to_owned(),
            },
        ]
    );
    assert_eq!(
        view.port_help,
        "Each ADS port is a separate server and symbol namespace. The server must support Symbol \
         Upload."
    );
}

#[test]
fn another_ads_port_is_checked() {
    // Design §3.3 J59 and `W/adsTargetPort.ts:1-65`: the chosen port is browsed as the target's
    // port, and a port outside 1 through 65,535 is refused with its reason before any browse runs.
    // A wrong implementation browses the default port anyway, or sends a browse for port 0.
    let (view, _) = open_ads(
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
        ads_answer(json!([ads_leaf(
            "ads:symbol:MAIN.x",
            "MAIN.x",
            "INT",
            Some(false)
        )])),
        Some(25_000),
    );
    assert_eq!(
        view.ports.iter().map(|card| card.port).collect::<Vec<_>>(),
        vec![851, 25_000]
    );
    assert_eq!(view.ports[1].tags.len(), 1, "the chosen port is browsed");

    let (refused, _) = open_ads(
        &ads_params("192.168.10.5", "5.23.91.12.1.1", 851),
        ads_answer(json!([])),
        Some(0),
    );
    assert_eq!(
        refused.error.expect("the refused port").detail,
        "ADS port must be between 1 and 65535."
    );
    assert_eq!(
        refused.ports,
        Vec::<AdsPortCard>::new(),
        "no port is browsed"
    );
    assert_eq!(
        refused.kind,
        BrowseKind::AdsTags,
        "the failure pane is still the ADS browser"
    );
}

#[test]
fn added_flags_follow_ads_toml_points() {
    // Design §3.3 J40 and J57, `W/useDiscoverPane.ts:132-146` and `NC/adsTagBatch.ts:69-87`: a tag
    // the project's ADS configuration already has is marked added, from the connection's
    // `[runtime.ads]` points and from the selections the panel merged in, and the tags come in
    // name order. A wrong implementation marks every tag, none, or ignores the port.
    let params = json!({
        "name": "line1", "host": "192.168.10.5", "target_net_id": "5.23.91.12.1.1",
        "ams_port": 851,
        "points": [{ "symbol": "MAIN.added" }, { "path": "MAIN.via_path" }],
        "imported_ads_symbols": [{ "port": 851, "paths": ["MAIN.imported"] }],
    });
    let tree = json!([
        ads_leaf(
            "ads:symbol:MAIN.via_path",
            "MAIN.via_path",
            "INT",
            Some(true)
        ),
        ads_leaf("ads:symbol:MAIN.fresh", "MAIN.fresh", "INT", Some(true)),
        ads_leaf(
            "ads:symbol:MAIN.imported",
            "MAIN.imported",
            "INT",
            Some(true)
        ),
        ads_leaf("ads:symbol:MAIN.added", "MAIN.added", "INT", Some(true)),
    ]);
    let (view, _) = open_ads(&params, ads_answer(tree), None);

    let tags = &view.ports[0].tags;
    assert_eq!(
        tags.iter().map(|tag| tag.path.as_str()).collect::<Vec<_>>(),
        vec!["MAIN.added", "MAIN.fresh", "MAIN.imported", "MAIN.via_path"],
        "the tags are sorted by name"
    );
    assert_eq!(
        tags.iter().map(|tag| tag.added).collect::<Vec<_>>(),
        vec![true, false, true, true],
        "the project's points and merged selections are marked added"
    );
}
