//! Native tests of the canvas's own items and of the panel engine's rules it now reaches
//! (specification 25 §2, §5.1, §5.4, §5.5, §5.6, §5.9, §5.12 and §5.13; specification 11,
//! "Identify"). Every expected value is worked out from the specification by hand; each test names
//! the wrong behaviour it catches.

use serde_json::{json, Value};

use super::comm::saved_node;
use super::tests::{
    act_json, answered, catalog, connection, endpoint, fleet_of, merged, report, result, runtime,
    runtimes,
};
use super::views::{browse_error, discovery_failure, discovery_label};
use super::FleetFault;
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::test_project::{runtime_toml, TempFolder};
use crate::texts::discover as discover_texts;
use crate::texts::graph as graph_texts;

/// A topology request with only the simulator facts `simulator` and the other fields `extra`.
fn local(simulator: Value, extra: Value) -> Value {
    let mut request = json!({ "run_mode": "simulate" });
    request["simulator"] = simulator;
    if let (Some(request), Value::Object(extra)) = (request.as_object_mut(), extra) {
        request.extend(extra);
    }
    request
}

/// The faults' IDs and targets.
fn faults(faults: &[FleetFault]) -> Vec<(&str, &str)> {
    faults
        .iter()
        .map(|fault| (fault.id.as_str(), fault.target_node_id.as_str()))
        .collect()
}

#[test]
fn the_drawn_simulator_shows_its_lifecycle_in_its_node_banner_and_summary() {
    // Specification 25 §5.6 and §5.10: while nothing reports the project's runtime, the canvas
    // draws the simulator on this computer: stopped until the user starts it, with the banner
    // `Simulator is stopped.` and `Start simulator`, and the summary `1 host · 1 runtime ·
    // Simulator stopped`; pending while it starts, with no banner; connected (`● Running`) while
    // it runs, with `Simulator running.` and `Stop simulator`, and the summary counting its
    // connections. A wrong implementation starts the simulator by drawing it, calls a stopped
    // simulator running, or counts connections of a simulator that does not run.
    let stopped = fleet_of(local(json!({}), json!({})));
    let simulator = runtime(&stopped, "Simulator");
    assert_eq!(simulator.health, "stopped");
    assert_eq!(simulator.detail, graph_texts::SIMULATOR_STOPPED_DETAIL);
    assert_eq!(stopped.hosts[0].hostname, "This computer");
    let banner = stopped.banner.as_ref().expect("a banner");
    assert_eq!(
        (banner.kind, banner.text.as_str()),
        ("info", "Simulator is stopped.")
    );
    assert_eq!(
        banner
            .actions
            .iter()
            .map(|action| (action.label, action.action))
            .collect::<Vec<_>>(),
        vec![("Start simulator", "startLocalSimulator")]
    );
    assert_eq!(stopped.summary, "1 host · 1 runtime · Simulator stopped");

    let starting = fleet_of(local(json!({ "starting": true }), json!({})));
    assert_eq!(runtime(&starting, "Simulator").health, "pending");
    assert_eq!(starting.banner, None);

    let running = fleet_of(local(
        json!({ "running": true, "runtime_state": "running" }),
        json!({}),
    ));
    let simulator = runtime(&running, "Simulator");
    assert_eq!(
        (simulator.health.as_str(), simulator.detail.as_str()),
        ("connected", "● Running")
    );
    let banner = running.banner.as_ref().expect("a banner");
    assert_eq!(
        (banner.text.as_str(), banner.actions[0].action),
        ("Simulator running.", "stopLocalSimulator")
    );
    assert_eq!(running.summary, "1 host · 1 runtime · 0 endpoints");
}

#[test]
fn a_start_failure_blocks_the_simulator_once_but_a_stopped_local_socket_does_not() {
    // Specification 25 §5.6: a start failure is the simulator's error with its message, one fault
    // `runtime:<kind>` on the simulator's node, and the banner offers `Retry` and `Open logs`. A
    // stale runtime that only says the local simulator is stopped is the neutral stopped state.
    // A wrong implementation draws the failure twice, keeps a neutral stop as an error, or drops
    // the recovery actions.
    let failed = fleet_of(local(
        json!({}),
        json!({ "failure": { "kind": "failed_spawn", "message": "trust-runtime is missing." } }),
    ));
    let simulator = runtime(&failed, "Simulator");
    assert_eq!(
        (simulator.health.as_str(), simulator.detail.as_str()),
        ("error", "trust-runtime is missing.")
    );
    assert_eq!(
        faults(&failed.faults),
        vec![("runtime:failed_spawn", "runtime:local")]
    );
    let banner = failed.banner.as_ref().expect("a banner");
    assert_eq!(banner.kind, "error");
    assert_eq!(
        banner
            .actions
            .iter()
            .map(|action| action.action)
            .collect::<Vec<_>>(),
        vec!["startLocalSimulator", "openRuntimeLogs"]
    );

    let neutral = fleet_of(local(
        json!({}),
        json!({ "failure": {
            "kind": "stale_runtime", "message": "Local simulator is stopped.",
        } }),
    ));
    assert_eq!(runtime(&neutral, "Simulator").health, "stopped");
    assert!(neutral.faults.is_empty(), "{:?}", neutral.faults);
}

#[test]
fn the_simulator_holds_the_catalogs_local_connections_connected_only_with_evidence() {
    // Specification 25 §5.5: the drawn simulator holds the local connections that the catalog
    // lists, each with the health its protocol reports, connected only while the simulator runs,
    // and simulated I/O only while the simulator reports I/O values. A wrong implementation calls
    // simulated I/O connected without values, or a Modbus connection connected while the
    // simulator does not run.
    let schema = json!({ "protocols": [
        { "id": "simulated", "title": "Simulated I/O", "instances": [{ "id": "simulated:0", "display_name": "sim" }] },
        { "id": "modbus_tcp", "title": "Modbus TCP", "instances": [{ "id": "modbus_tcp:0", "display_name": "Drive" }] },
    ] });
    let capabilities = json!({ "capabilities": [
        { "id": "simulated", "health": "connected" },
        { "id": "modbus_tcp", "health": "connected" },
    ] });
    let extra = json!({ "schema": schema, "capabilities": capabilities });
    let running = json!({ "running": true, "runtime_state": "running" });
    let without_values = fleet_of(local(running.clone(), extra.clone()));
    let simulator = runtime(&without_values, "Simulator");
    assert_eq!(connection(simulator, "Drive").health, "connected");
    assert_eq!(connection(simulator, "Simulated I/O").health, "pending");

    let mut with_values = running;
    with_values["io_values"] = json!(true);
    let with_values = fleet_of(local(with_values, extra.clone()));
    assert_eq!(
        connection(runtime(&with_values, "Simulator"), "Simulated I/O").health,
        "connected"
    );

    let stopped = fleet_of(local(json!({}), extra));
    assert_eq!(
        connection(runtime(&stopped, "Simulator"), "Drive").health,
        "pending"
    );
}

#[test]
fn the_add_form_draws_its_draft_and_its_refused_result_is_the_forms_own_fault() {
    // Specification 25 §5.9: while the add form of a protocol is open and the simulator has no
    // connection of it, the canvas draws its draft `draft:<protocol>` (`Draft setup`), and a
    // result the runtime refused is a fault on that draft that belongs to the form, so the canvas
    // hides it while the user edits the form. A wrong implementation draws no draft, targets the
    // fault elsewhere, or marks another fault as the form's.
    let fleet = fleet_of(local(
        json!({}),
        json!({ "setup": {
            "stage": "add_device", "active_protocol": "Modbus-TCP",
            "apply_result": {
                "protocol": "modbus_tcp", "lifecycle_effect": "blocked",
                "message": "Fix the highlighted fields and try again.",
            },
        } }),
    ));
    let draft = connection(runtime(&fleet, "Simulator"), "Modbus TCP");
    assert_eq!(
        (draft.id.as_str(), draft.detail.as_str()),
        ("draft:modbus_tcp", "Draft setup")
    );
    let fault = &fleet.faults[0];
    assert_eq!(
        (
            fault.id.as_str(),
            fault.target_node_id.as_str(),
            fault.label.as_str(),
            fault.form
        ),
        (
            "apply:modbus_tcp",
            "draft:modbus_tcp",
            "Fix the highlighted fields and try again.",
            true
        )
    );
}

#[test]
fn the_first_device_waits_for_io_values_before_it_is_connected() {
    // Specification 25 §5.5: the first device that the user asks for while the catalog lists none
    // waits for the simulator's I/O (`Waiting for runtime I/O`) and is connected only
    // while the simulator runs and reports values. A wrong implementation calls it connected
    // without I/O values.
    let waiting = fleet_of(local(
        json!({ "running": true, "runtime_state": "running" }),
        json!({ "setup": { "device_requested": true } }),
    ));
    let device = first_device(runtime(&waiting, "Simulator"));
    assert_eq!(
        (device.health.as_str(), device.detail.as_str()),
        ("pending", "Waiting for runtime I/O")
    );
    // Specification 25 §5.7: a local simulated connection is titled `Simulated I/O`.
    assert_eq!(device.name, "Simulated I/O");
    let live = fleet_of(local(
        json!({ "running": true, "runtime_state": "running", "io_values": true }),
        json!({ "setup": { "device_requested": true } }),
    ));
    assert_eq!(
        first_device(runtime(&live, "Simulator")).health,
        "connected"
    );
}

/// The first device that the canvas draws on the simulator.
fn first_device(simulator: &super::FleetRuntimeView) -> &super::FleetEndpointView {
    simulator
        .endpoints
        .iter()
        .find(|endpoint| endpoint.id == "first-run:simulated")
        .expect("the first device")
}

#[test]
fn managed_runtimes_join_this_computer_and_the_session_marks_its_runtimes() {
    // Specification 25 §5.1 and §6.5: a runtime that this computer manages is marked managed,
    // matched by control endpoint, with its lifecycle deciding its health; one that no runtime
    // shows joins this computer; the attached runtime and the selected run target are marked.
    // A wrong implementation draws the managed runtime twice, leaves a stopped managed runtime out,
    // or marks the run target on another runtime.
    let fleet = fleet_of(json!({
        "primary": answered("tcp://127.0.0.1:9901", "cell",
            report("host:pc", "127.0.0.1", "cell", "connected", &[])),
        "run_mode": "online",
        "managed": [
            { "name": "cell", "control_endpoint": "tcp://127.0.0.1:9901", "state": "running" },
            { "name": "lab", "control_endpoint": "", "state": "stopped" },
        ],
        "session": { "attached_endpoint": "tcp://127.0.0.1:9901", "run_target": "lab" },
    }));
    let cell = runtime(&fleet, "cell");
    assert!(cell.managed && cell.attached && !cell.run_target);
    assert_eq!(cell.lifecycle_state, Some("running"));
    assert_eq!(cell.detail, graph_texts::MANAGED_RUNNING_DETAIL);
    let lab = runtime(&fleet, "lab");
    assert_eq!(
        (lab.health.as_str(), lab.lifecycle_state, lab.run_target),
        ("stopped", Some("stopped"), true)
    );
    assert_eq!(runtimes(&fleet).len(), 2, "each runtime once");
}

#[test]
fn a_failed_connect_stays_on_the_runtime_it_was_for() {
    // Specification 25 §5.1: a failed Connect shows on the runtime it was for, as an error with
    // the failure's text, and its fault focuses that runtime; a runtime that answered with its
    // debugging disabled keeps its own health. A wrong implementation puts the failure on the
    // simulator, or marks the runtime that answered as an error.
    let peer = || {
        answered(
            "tcp://10.0.0.9:5510",
            "cell",
            report("host:cell", "10.0.0.9", "cell", "connected", &[]),
        )
    };
    let rejected = fleet_of(json!({
        "peers": [peer()],
        "run_mode": "online",
        "failure": { "kind": "stale_runtime", "message": "Auth token rejected.",
                     "endpoint": "tcp://10.0.0.9:5510" },
    }));
    let cell = runtime(&rejected, "cell");
    assert_eq!(
        (cell.health.as_str(), cell.detail.as_str()),
        ("error", "Auth token rejected.")
    );
    assert!(faults(&rejected.faults).contains(&("runtime:stale_runtime", cell.id.as_str())));

    let disabled = fleet_of(json!({
        "peers": [peer()],
        "run_mode": "online",
        "failure": { "kind": "debug_disabled", "message": "Debugging is disabled.",
                     "endpoint": "tcp://10.0.0.9:5510" },
    }));
    let cell = runtime(&disabled, "cell");
    assert_eq!(cell.health, "connected");
    assert!(faults(&disabled.faults).contains(&("runtime:debug_disabled", cell.id.as_str())));
}

#[test]
fn a_connect_refused_for_its_token_at_the_configured_runtime_offers_the_token() {
    // Specification 25 §5.1, §5.6 and §5.15: while the project runs in the simulator its
    // configured runtime is the simulator's node, `Simulator`, started and stopped; a Connect to
    // that runtime refused for its token makes it the runtime connected to, under its own name,
    // with Set auth token first, Connect as the retry and no Logs. A wrong implementation offers
    // Start, which cannot fix a refused token, drops the retry, or names the runtime `Simulator`.
    let request = |failure: Value| {
        let mut project = report("host:pc", "10.0.0.2", "Cpu", "configured_policy", &[]);
        project["hosts"][0]["runtimes"][0]["control_endpoint"] = json!("tcp://10.0.0.9:5510");
        json!({ "project": project, "run_mode": "simulate", "failure": failure })
    };
    let node = |fleet: &super::CanvasFleet| -> (String, Vec<(&'static str, &'static str)>) {
        let node = runtimes(fleet)
            .into_iter()
            .find(|node| node.control_endpoint.as_deref() == Some("tcp://10.0.0.9:5510"))
            .unwrap_or_else(|| panic!("the configured runtime in {fleet:?}"));
        let controls = node
            .controls
            .iter()
            .map(|control| (control.action, control.kind));
        (node.name.clone(), controls.collect())
    };
    let (name, controls) = node(&fleet_of(request(Value::Null)));
    assert_eq!(
        (name.as_str(), controls[0]),
        ("Simulator", ("startLocalSimulator", "primary"))
    );
    let refused = fleet_of(request(json!({
        "kind": "workspace_permission", "message": "Auth token missing.",
        "endpoint": "tcp://10.0.0.9:5510",
    })));
    assert_eq!(
        node(&refused),
        (
            "Cpu".to_owned(),
            vec![
                ("setAuthToken", "primary"),
                ("runtimeConnect", "secondary"),
                ("setAsRunTarget", "secondary"),
                ("openRuntimeSettings", "secondary"),
            ]
        )
    );
}

#[test]
fn identify_is_offered_on_a_runtime_that_answered_this_round() {
    // Specification 25 §5.12: `Identify` is shown for every runtime and can be asked of one that
    // answered; any other runtime is disabled with `Identify is unavailable until the runtime
    // answers.`. A wrong implementation offers it on a runtime that never answered.
    let fleet = fleet_of(json!({
        "peers": [
            answered("tcp://10.0.0.9:5510", "cell",
                report("host:cell", "10.0.0.9", "cell", "connected", &[])),
            { "endpoint": "tcp://10.0.0.8:5510", "label": "silent", "reach": "unreachable" },
        ],
        "run_mode": "online",
    }));
    assert!(runtime(&fleet, "cell").identify.enabled);
    let silent = &runtime(&fleet, "silent").identify;
    assert_eq!(
        (silent.enabled, silent.reason),
        (false, Some(graph_texts::IDENTIFY_UNREACHABLE))
    );
    assert!(!runtime(&fleet, "Simulator").identify.enabled);
}

#[test]
fn identify_asks_the_runtime_for_ten_seconds_and_shows_its_answer() {
    // Specification 25 §5.12 and specification 11, "Identify": the canvas sends `identify
    // {"duration_s": 10}` with the stored token and shows `Identifying <name>: its identify output
    // blinks for <seconds> seconds.` with the runtime's seconds; a refusal is the runtime's own
    // text; without a runtime that answers, nothing is sent. A wrong implementation sends another
    // duration, drops the token, or invents a success.
    let runtime = FakeRuntime::start(Script::running().answer(
        "identify",
        Reply::Result(json!({ "identifying": true, "duration_s": 10 })),
    ));
    let reply = act_json(json!({
        "op": "identify",
        "runtime": { "endpoint": runtime.endpoint, "token": "token-1" },
        "args": { "name": "line" },
    }));
    assert_eq!(
        reply.notice.map(|notice| (notice.tone, notice.text)),
        Some((
            "info",
            "Identifying line: its identify output blinks for 10 seconds.".to_owned()
        ))
    );
    let sent: Vec<Value> = runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "identify")
        .collect();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["params"], json!({ "duration_s": 10 }));
    assert_eq!(sent[0]["auth"], json!("token-1"));

    let refusing = FakeRuntime::start(
        Script::running().answer("identify", Reply::Refuse("identify_not_configured")),
    );
    let refused = act_json(json!({
        "op": "identify",
        "runtime": { "endpoint": refusing.endpoint },
        "args": { "name": "line" },
    }));
    assert_eq!(
        refused.notice.map(|notice| (notice.tone, notice.text)),
        Some(("warning", "identify not configured".to_owned()))
    );

    let unanswered = act_json(json!({ "op": "identify", "args": { "name": "line" } }));
    assert_eq!(
        unanswered.notice.map(|notice| notice.text),
        Some("line did not answer, so nothing was sent.".to_owned())
    );
}

#[test]
fn connections_say_when_they_need_attention_and_an_ads_client_sums_up_its_devices() {
    // Specification 25 §5.5 and §5.13: a connection needs attention unless its health is good or
    // quiet (`connected`, `configured`, `configured_policy`, `disabled`, `stopped`, `simulate`); an
    // ADS client groups its connections by address and AMS Net ID with the tags of each port, and
    // a configured client of a running runtime reads `Runtime running — ADS is configured.`.
    // A wrong implementation reports a configured connection as a problem, counts a device twice,
    // or calls the client of a running runtime stopped.
    let mut ads = endpoint("ads", "ads", "ADS", "configured_policy", "Configured.");
    ads["params"] = json!({ "connections": [
        { "host": "10.0.0.5", "target_net_id": "5.1.2.3.1.1", "ams_port": 851, "points": [{}, {}] },
        { "host": "10.0.0.5", "target_net_id": "5.1.2.3.1.1", "ams_port": 301, "points": [{}] },
    ] });
    let fleet = fleet_of(json!({
        "primary": answered("tcp://10.0.0.5:9901", "line", report(
            "host:plc", "10.0.0.5", "line", "connected",
            &[ads, endpoint("mqtt", "mqtt", "Broker", "degraded", "Slow.")],
        )),
        "run_mode": "online",
    }));
    let line = runtime(&fleet, "line");
    assert!(connection(line, "Broker").attention);
    let ads = line
        .endpoints
        .iter()
        .find(|endpoint| endpoint.protocol == "ads")
        .expect("the ADS client");
    assert!(!ads.attention);
    let summary = serde_json::to_value(ads.ads_summary.as_ref().expect("a summary"))
        .expect("a summary in JSON");
    assert_eq!(
        summary,
        json!({
            "devices": [{ "address": "10.0.0.5", "amsNetId": "5.1.2.3.1.1",
                          "ports": [{ "port": 301, "tagCount": 1 }, { "port": 851, "tagCount": 2 }] }],
            "status": "Runtime running — ADS is configured.",
            "statusKind": "neutral",
            "enabled": true,
            "configPath": "ads.toml",
            "updateIntervalMs": 20.0,
        })
    );
}

#[test]
fn a_failed_browse_names_its_one_recovery() {
    // Specification 25 §5.4 and §11.2: an OPC UA certificate the client does not trust offers
    // `trust`; credentials offer `credentials`; an ADS failure without a code is a failed symbol
    // upload offered again; an unavailable ADS port is offered again with its own title. A
    // discovered device's failed port browse carries the same recovery. A wrong implementation
    // offers trust for an ADS failure, or drops the server's message.
    let cert = browse_error("opcua_client", Some("cert_untrusted"), None);
    assert_eq!(
        (cert.action, cert.title.as_str()),
        ("trust", "Server certificate not trusted")
    );
    assert_eq!(
        browse_error("opcua_client", Some("auth_required"), None).action,
        "credentials"
    );
    let ads = browse_error("ads", None, Some("upload failed: 0x710"));
    assert_eq!(
        (
            ads.code.as_str(),
            ads.action,
            ads.title.as_str(),
            ads.detail.as_str()
        ),
        (
            "symbol_upload_failed",
            "retry",
            "ADS symbol browse failed",
            "upload failed: 0x710"
        )
    );
    assert_eq!(
        browse_error("ads", Some("ads_port_unavailable"), None).title,
        "ADS port unavailable"
    );

    let reply = act_json(json!({
        "op": "ads_finds",
        "args": { "candidates": [{ "id": "plc", "protocol": "ads", "params": {
            "responding_ads_ports": [851],
            "ads_port_browse_results": [{ "port": 852, "error": { "code": "empty_symbol_table" } }],
        } }] },
    }));
    assert_eq!(
        result(&reply)["candidates"][0]["params"]["ads_port_browse_results"][0]["error_view"]
            ["title"],
        json!("No compatible symbols")
    );
}

#[test]
fn a_discovery_names_its_protocol_and_words_its_failures() {
    // Specification 25 §2 (the old pane's words): a progress row names the protocol with `@
    // <host>` or the subnet; a failure at the selected runtime says whether the token was refused,
    // the request timed out, or it failed otherwise. A wrong implementation drops the address, or
    // words a timeout as an authentication failure.
    assert_eq!(
        discovery_label("modbus_tcp", Some("10.0.0.5"), None),
        "Modbus @ 10.0.0.5"
    );
    assert_eq!(
        discovery_label("ads", None, Some("10.0.0.0/24")),
        "ADS 10.0.0.0/24"
    );
    assert_eq!(discovery_label("opcua_client", None, None), "OPC UA server");
    assert_eq!(
        discovery_failure("mqtt", "request timed out"),
        "MQTT broker discovery timed out on the selected runtime. Reconnect or start the selected runtime, then scan again."
    );
    assert_eq!(
        discovery_failure("ads", "auth token rejected"),
        "The selected runtime rejected authentication for ADS discovery. Check its auth token. Reconnect or start the selected runtime, then scan again."
    );
    let reply = act_json(json!({
        "op": "discover_label",
        "args": { "protocol": "gpio", "host": " ", "cidr": "" },
    }));
    assert_eq!(result(&reply), &json!({ "label": "GPIO" }));
}

#[test]
fn a_result_whose_runtime_is_gone_or_whose_protocol_stopped_discovering_cannot_be_added() {
    // Specification 25 §2: a result found by a runtime that the canvas no longer draws cannot be
    // added (`The runtime that found this result is no longer available…`), nor one whose protocol
    // no longer offers discovery (`This discovery action is no longer available…`); the runtime's
    // absence is said first. A wrong implementation lets a stale result be added, or checks the
    // protocol before the runtime.
    let answer = merged(json!({
        "run_mode": "simulate",
        "schema": { "protocols": [
            { "id": "modbus_tcp", "actions": ["discover"] },
            { "id": "mqtt", "actions": [] },
        ] },
        "discovered": [
            { "id": "gone", "protocol": "mqtt", "originRuntimeId": "runtime:vanished" },
            { "id": "mqtt", "protocol": "mqtt" },
            { "id": "modbus", "protocol": "modbus_tcp", "originRuntimeId": "runtime:local" },
        ],
    }));
    let reasons: Vec<(&str, Option<&str>)> = answer
        .discovered
        .iter()
        .map(|found| (found.id.as_str(), found.disabled_reason))
        .collect();
    assert_eq!(
        reasons,
        vec![
            ("gone", Some(discover_texts::STALE_RUNTIME)),
            ("mqtt", Some(discover_texts::PROTOCOL_UNAVAILABLE)),
            ("modbus", None),
        ]
    );
}

#[test]
fn a_saved_connection_is_selected_on_the_node_the_canvas_draws() {
    // Specification 25 §5.9: a successful Save selects the saved node. The save answers the node
    // of the connection under the runtime the form saved to, which is the node the next round
    // draws: the owner-scoped identity under a configured runtime, and the catalog's ID on the
    // drawn simulator. A wrong implementation answers the project-local ID `gpio:0`, which no
    // node of a configured runtime carries, so nothing is selected.
    let project = TempFolder::new("canvas-save-node");
    project.write(
        "runtime.toml",
        &runtime_toml("line", "unix:///tmp/trust-canvas-save.sock", None),
    );
    let schema = catalog();
    let values: serde_json::Map<String, Value> = schema["protocols"]
        .as_array()
        .and_then(|protocols| protocols.iter().find(|protocol| protocol["id"] == "gpio"))
        .and_then(|gpio| gpio["fields"].as_array())
        .expect("the catalog's GPIO fields")
        .iter()
        .map(|field| {
            let text = match &field["default"] {
                Value::String(text) => text.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            (
                field["id"].as_str().unwrap_or_default().to_owned(),
                Value::String(text),
            )
        })
        .collect();
    let save = |runtime_id: Option<&str>| {
        act_json(json!({
            "op": "comm_save",
            "project": project.path,
            "args": {
                "protocol": "gpio", "action": "upsert",
                "values": values, "schema": schema,
                "runtime_id": runtime_id,
            },
        }))
    };
    let first = save(None);
    assert!(first.applied, "{first:?}");
    let drawn = || {
        let project_topology = trust_runtime::control::offline_fleet_topology_json(&project.path)
            .expect("the project's topology");
        fleet_of(json!({ "project": project_topology, "run_mode": "online" }))
    };
    let fleet = drawn();
    let line = &runtimes(&fleet)[0];
    let gpio = line
        .endpoints
        .iter()
        .find(|endpoint| endpoint.protocol == "gpio")
        .expect("the saved GPIO connection is drawn")
        .id
        .clone();
    let again = save(Some(&line.id));
    assert_eq!(result(&again)["node_id"], json!(gpio));
    assert_eq!(
        saved_node("runtime:local", "gpio:0").as_deref(),
        Some("gpio:0")
    );
}

#[test]
fn a_found_device_goes_to_the_runtime_that_found_it_and_a_found_runtime_becomes_a_host() {
    // Specification 25 §2 and §5.2: a device a search found is added to the runtime that found it
    // while the canvas draws that runtime, and to the first runtime drawn when the result names no
    // runtime; a result whose runtime is gone goes nowhere. A found runtime that announced a
    // control endpoint becomes a host with the result's label, else its name. A wrong
    // implementation adds the device to another runtime, invents a target for a stale result, or
    // adds a host without an endpoint.
    let answer = merged(json!({
        "run_mode": "simulate",
        "schema": { "protocols": [{ "id": "modbus_tcp", "actions": ["discover"] }] },
        "discovered": [
            { "id": "own", "protocol": "modbus_tcp", "originRuntimeId": "runtime:local" },
            { "id": "anywhere", "protocol": "modbus_tcp" },
            { "id": "gone", "protocol": "modbus_tcp", "originRuntimeId": "runtime:vanished" },
            { "id": "line", "protocol": "trust", "label": "  ",
              "params": { "control_endpoint": " tcp://10.0.0.7:5680 ", "name": "Line 7" } },
            { "id": "bare", "protocol": "trust", "params": { "name": "No endpoint" } },
        ],
    }));
    let found = |id: &str| {
        answer
            .discovered
            .iter()
            .find(|found| found.id == id)
            .expect("the result")
    };
    assert_eq!(
        found("own").draft_runtime_id.as_deref(),
        Some("runtime:local")
    );
    assert_eq!(
        found("anywhere").draft_runtime_id.as_deref(),
        Some("runtime:local")
    );
    assert_eq!(found("gone").draft_runtime_id, None);
    let host = found("line").host.as_ref().expect("a host");
    assert_eq!(
        (host.endpoint.as_str(), host.label.as_str()),
        ("tcp://10.0.0.7:5680", "Line 7")
    );
    assert_eq!(found("bare").host, None);
}

#[test]
fn a_search_starts_only_from_a_runtime_that_runs_or_is_attached() {
    // Specification 25 §2: a search for devices runs on a runtime, so it can start from a runtime
    // that runs or that the extension is attached to; any other runtime says what to do first. A
    // wrong implementation offers a scan from a stopped runtime, which can only fail.
    // The simulator of a project that runs in the simulator, not started: it does not run and no
    // session is attached, so the search cannot start from it.
    let answer = merged(json!({ "run_mode": "simulate" }));
    let fleet = answer.fleet.expect("the fleet view");
    let simulator = fleet
        .hosts
        .iter()
        .flat_map(|host| &host.runtimes)
        .find(|runtime| runtime.name == "Simulator")
        .expect("the simulator is drawn");
    assert!(!simulator.attached && simulator.health != "connected");
    assert!(!simulator.discovery.ready);
    assert_eq!(
        simulator.discovery.reason.as_deref(),
        Some("Start or connect Simulator before scanning from it.")
    );
}
