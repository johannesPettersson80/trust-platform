//! Native tests of the canvas's rules (specification 25 §5.1 to §5.10 and §5.15; specification 11,
//! "Communication discovery scope and symbol browsing"). Every expected value is worked out from the
//! specification by hand; each test names the wrong behaviour it catches.

use serde_json::{json, Map, Value};

use std::path::Path;

use super::ads::typed_port;
use super::ads_tags::{batch_summary, offline_import_params, removal, remove_offline, Removal};
use super::comm::{apply_action, endpoints_of, tested_params, unambiguous};
use super::plans::endpoint_params;
use super::{act, topology, CanvasActRequest, CanvasFleet, CanvasReply, CanvasTopologyRequest};
use super::{FleetEndpointView, FleetRuntimeView};
use crate::test_project::TempFolder;
use crate::texts::forms as form_texts;
use crate::texts::graph as graph_texts;
use crate::texts::plans as plan_texts;
use crate::topology::configured_runtime_id;

// ---- fixtures ----

/// A `fleet.topology` report of one computer `host` (with the address `ip`) running one runtime
/// `name` with the connections `endpoints`.
pub(super) fn report(host: &str, ip: &str, name: &str, health: &str, endpoints: &[Value]) -> Value {
    json!({
        "schema_version": 3,
        "hosts": [{
            "host_id": host, "hostname": host, "arch": "x86_64", "os": "linux", "ips": [ip],
            "containers": [],
            "runtimes": [{
                "runtime_id": "RESOURCE", "name": name, "mode": "online", "cycle_ms": 10,
                "health": health, "detail": "Running.", "source": "self",
                "endpoints": endpoints,
            }],
        }],
        "links": [], "shared": [], "external": [],
    })
}

/// A connection of a report.
pub(super) fn endpoint(id: &str, protocol: &str, name: &str, health: &str, detail: &str) -> Value {
    json!({
        "id": id, "kind": "field", "protocol": protocol, "name": name, "health": health,
        "detail": detail, "owned": true, "supports_test": false,
    })
}

/// A connection of a report with its configuration.
fn endpoint_with(id: &str, protocol: &str, params: Value) -> Value {
    let mut endpoint = endpoint(id, protocol, protocol, "connected", "Connected.");
    endpoint["params"] = params;
    endpoint
}

/// A target that answered with `topology`.
pub(super) fn answered(endpoint: &str, label: &str, topology: Value) -> Value {
    let mut target = json!({
        "endpoint": endpoint, "label": label, "reach": "reachable",
        "connectors": { "ok": { "connectors": [] } },
    });
    target["topology"]["ok"] = topology;
    target
}

pub(super) fn merged(request: Value) -> super::CanvasTopology {
    let request: CanvasTopologyRequest = serde_json::from_value(request).expect("a request");
    topology(&request)
}

pub(super) fn fleet_of(request: Value) -> CanvasFleet {
    merged(request).fleet.expect("a fleet view")
}

pub(super) fn runtimes(fleet: &CanvasFleet) -> Vec<&FleetRuntimeView> {
    fleet
        .hosts
        .iter()
        .flat_map(|host| {
            host.runtimes.iter().chain(
                host.containers
                    .iter()
                    .flat_map(|container| &container.runtimes),
            )
        })
        .collect()
}

pub(super) fn runtime<'a>(fleet: &'a CanvasFleet, name: &str) -> &'a FleetRuntimeView {
    runtimes(fleet)
        .into_iter()
        .find(|runtime| runtime.name == name)
        .unwrap_or_else(|| panic!("a runtime named {name} in {fleet:?}"))
}

pub(super) fn connection<'a>(runtime: &'a FleetRuntimeView, name: &str) -> &'a FleetEndpointView {
    runtime
        .endpoints
        .iter()
        .find(|endpoint| endpoint.name == name)
        .unwrap_or_else(|| panic!("a connection named {name} in {runtime:?}"))
}

pub(super) fn act_json(request: Value) -> CanvasReply {
    let request: CanvasActRequest = serde_json::from_value(request).expect("an action");
    act(&request).expect("an action of the canvas")
}

pub(super) fn result(reply: &CanvasReply) -> &Value {
    reply.result.as_ref().expect("a result")
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(object) => object,
        other => panic!("an object, not {other}"),
    }
}

/// The comm catalog that `trust-runtime comm schema` answers.
pub(super) fn catalog() -> Value {
    trust_runtime::control::offline_comm_schema_json(None).expect("the tool's comm catalog")
}

// ---- trust-lsp.devicesTopology (specification 25 §5.1, §5.5, §5.6 and §5.10) ----

#[test]
fn the_runtime_that_answered_gets_the_endpoint_it_answered_at() {
    // Specification 25 §5.1: a runtime does not know the address it was reached at, so the graph
    // gives its live node the endpoint that answered; its node then offers Connect and `Set as run
    // target` for that endpoint. A wrong implementation leaves the node without an endpoint, or
    // keeps the endpoint off the merged `local` report that the canvas's actions read.
    let request = json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1",
            report("host:plc-01", "10.0.0.5", "line", "connected", &[])),
        "run_mode": "online",
    });
    let fleet = fleet_of(request.clone());
    assert_eq!(
        runtime(&fleet, "line").control_endpoint.as_deref(),
        Some("tcp://10.0.0.5:9901")
    );
    let local = merged(request).local.expect("the project's own runtime");
    assert_eq!(
        local["hosts"][0]["runtimes"][0]["control_endpoint"],
        json!("tcp://10.0.0.5:9901")
    );
}

#[test]
fn the_extensions_own_simulator_shows_no_endpoint_and_is_named_simulator() {
    // Specification 25 §5.1 and §5.6: the extension starts and stops its own simulator, so its node
    // names no control endpoint (it keeps Start and Stop), and it is named `Simulator` whatever its
    // Structured Text resource is called. A wrong implementation stamps the simulator's socket on
    // the node, which then offers Connect, or shows the resource name `line`.
    let fleet = fleet_of(json!({
        "primary": answered("unix:///tmp/trust-sim.sock", "Simulator",
            report("host:pc", "10.0.0.2", "line", "connected", &[])),
        "simulator_endpoint": "unix:///tmp/trust-sim.sock",
    }));
    let simulator = runtime(&fleet, "Simulator");
    assert_eq!(simulator.control_endpoint, None);
    assert!(runtimes(&fleet)
        .iter()
        .all(|runtime| runtime.name != "line"));
}

#[test]
fn a_running_simulator_keeps_its_own_state_beside_connected_io() {
    // Specification 25 §5.5: without an error or a degraded word, the runtime's own state wins, so
    // a simulator that reports `simulate` with its simulated I/O `connected` stays `simulate` (drawn
    // `Running`). A wrong implementation lets the connected connection outrank the runtime's own
    // state and shows `connected`, as the canvas did before 26 October 2026.
    let fleet = fleet_of(json!({
        "primary": answered("unix:///tmp/trust-sim.sock", "Simulator",
            report("host:pc", "10.0.0.2", "line", "simulate", &[
                endpoint("endpoint:RESOURCE:simulated:0", "simulated", "sim", "connected", "Up."),
            ])),
        "simulator_endpoint": "unix:///tmp/trust-sim.sock",
    }));
    assert_eq!(runtime(&fleet, "Simulator").health, "simulate");
}

#[test]
fn runtime_health_rolls_up_its_connections_and_the_computer_shows_reachability() {
    // Specification 25 §5.5: runtime health rolls up the raw health of its connections; a host's
    // health is the machine's reachability and never inherits a connection's degraded state. A
    // connected runtime with one degraded connection is `degraded`; its computer, which reported
    // its architecture, is `connected`. The degraded connection is a warning fault named
    // `<name>: <detail>`. A wrong implementation keeps the runtime `connected`, marks the
    // computer degraded, or drops the fault.
    let fleet = fleet_of(json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1", report(
            "host:plc-01", "10.0.0.5", "line", "connected",
            &[
                endpoint("endpoint:RESOURCE:modbus_tcp:0", "modbus_tcp", "Drive", "connected", "Polling."),
                endpoint("endpoint:RESOURCE:mqtt:0", "mqtt", "Broker", "degraded", "Broker is slow."),
            ],
        )),
        "run_mode": "online",
    }));
    let line = runtime(&fleet, "line");
    assert_eq!(line.health, "degraded");
    assert_eq!(fleet.hosts[0].health, "connected");
    let broker = connection(line, "Broker");
    assert_eq!(
        fleet
            .faults
            .iter()
            .map(|fault| (
                fault.label.as_str(),
                fault.severity,
                fault.target_node_id.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![("Broker: Broker is slow.", "warning", broker.id.as_str())]
    );
    assert_eq!(fleet.summary, "1 host · 1 runtime · 2 endpoints");
}

#[test]
fn an_unreachable_peer_is_unknown_under_its_configured_label() {
    // Specification 25 §5.5 and §5.6: a configured peer that does not answer is drawn `unknown`,
    // never green and never `stopped`, under its configured label (never `This computer`), and
    // keeps its control endpoint for the inspector's actions. Its computer is not reachable. With
    // nothing that reports the project's runtime, the simulator stands for it on this computer, as
    // a second computer. A wrong implementation drops the peer, labels it `This computer`, or
    // shows it stopped.
    let fleet = fleet_of(json!({
        "peers": [{ "endpoint": "tcp://10.0.0.9:9901", "label": "Line 2", "reach": "unreachable" }],
    }));
    assert_eq!(
        fleet
            .hosts
            .iter()
            .map(|host| host.hostname.as_str())
            .collect::<Vec<_>>(),
        vec!["This computer", "Line 2"]
    );
    let host = &fleet.hosts[1];
    assert_eq!(host.health, "runtime_unreachable");
    let peer = runtime(&fleet, "Line 2");
    assert_eq!(peer.health, "unknown");
    assert_eq!(peer.detail, graph_texts::UNREACHABLE_PEER_DETAIL);
    assert_eq!(
        peer.control_endpoint.as_deref(),
        Some("tcp://10.0.0.9:9901")
    );
    assert_eq!(peer.id, configured_runtime_id("tcp://10.0.0.9:9901"));
}

#[test]
fn a_peer_that_refused_the_token_is_an_error_naming_the_recovery() {
    // Specification 25 §5.6: an authentication failure is an error with distinct missing-token and
    // rejected-token recovery text. A wrong implementation shows both as `unknown`, or gives the
    // missing token the rejected token's text.
    let fleet = fleet_of(json!({
        "peers": [
            { "endpoint": "tcp://10.0.0.9:9901", "label": "Missing", "reach": "auth_missing" },
            { "endpoint": "tcp://10.0.0.8:9901", "label": "Rejected", "reach": "auth_rejected" },
        ],
    }));
    let missing = runtime(&fleet, "Missing");
    assert_eq!(missing.health, "error");
    assert_eq!(missing.detail, graph_texts::AUTH_MISSING_DETAIL);
    let rejected = runtime(&fleet, "Rejected");
    assert_eq!(rejected.health, "error");
    assert_eq!(rejected.detail, graph_texts::AUTH_REJECTED_DETAIL);
}

#[test]
fn reports_that_cannot_be_read_are_the_banner_the_projects_own_first() {
    // Specification 25 §5.10: the banner shows the reports that cannot be read, the project's own
    // first, then the other runtimes' after `Peer topology degraded:`, and the local view stays
    // visible. The project's runtime failed with `refused`, the peer `Line 2` with `timed out`:
    // `Fleet topology unavailable: refused; Peer topology degraded: Line 2 topology: timed out`.
    // A wrong implementation hides the local view, or orders or words the banner differently.
    let answer = merged(json!({
        "primary": {
            "endpoint": "tcp://10.0.0.5:9901", "label": "Line 1", "reach": "reachable",
            "topology": { "error": "refused" },
        },
        "peers": [{
            "endpoint": "tcp://10.0.0.9:9901", "label": "Line 2", "reach": "reachable",
            "topology": { "error": "timed out" },
        }],
        "project": report("host:pc", "10.0.0.2", "project", "configured_policy", &[]),
    }));
    assert_eq!(
        answer.topology_error.as_deref(),
        Some("Fleet topology unavailable: refused; Peer topology degraded: Line 2 topology: timed out")
    );
    assert!(
        answer.fleet.is_some(),
        "the project's configuration stays drawn"
    );
}

#[test]
fn the_search_dims_connections_it_does_not_match_without_hiding_their_problems() {
    // Specification 25 §5.5: search is non-destructive. A search for `modbus` dims the MQTT
    // connection, whose words do not contain it, and leaves the Modbus one; the dimmed connection's
    // degraded health still rolls up into its runtime and stays a fault. Matching ignores case and
    // the search's surrounding blanks. A wrong implementation hides the dimmed connection's
    // problem, dims everything, or matches case-sensitively.
    let fleet = fleet_of(json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1", report(
            "host:plc-01", "10.0.0.5", "line", "connected",
            &[
                endpoint("endpoint:RESOURCE:modbus_tcp:0", "modbus_tcp", "Drive", "connected", "Polling."),
                endpoint("endpoint:RESOURCE:mqtt:0", "mqtt", "Broker", "degraded", "Broker is slow."),
            ],
        )),
        "run_mode": "online",
        "search_query": "  MODBUS ",
    }));
    let line = runtime(&fleet, "line");
    assert!(!connection(line, "Drive").dimmed);
    assert!(connection(line, "Broker").dimmed);
    assert_eq!(line.health, "degraded");
    assert_eq!(fleet.faults.len(), 1);

    let unsearched = fleet_of(json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1", report(
            "host:plc-01", "10.0.0.5", "line", "connected",
            &[endpoint("endpoint:RESOURCE:mqtt:0", "mqtt", "Broker", "connected", "Up.")],
        )),
    }));
    assert!(
        !connection(runtime(&unsearched, "line"), "Broker").dimmed,
        "no search dims nothing"
    );
}

#[test]
fn local_io_connections_take_their_product_names() {
    // Specification 25 §5.7: local simulated and loopback endpoint titles are `Simulated I/O` and
    // `Loopback I/O`; any other connection keeps its own name. A wrong implementation shows the raw
    // driver names `sim` and `loop`.
    let fleet = fleet_of(json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1", report(
            "host:plc-01", "10.0.0.5", "line", "connected",
            &[
                endpoint("endpoint:RESOURCE:simulated:0", "simulated", "sim", "connected", "Up."),
                endpoint("endpoint:RESOURCE:loopback:1", "loopback", "loop", "connected", "Up."),
                endpoint("endpoint:RESOURCE:modbus_tcp:2", "modbus_tcp", "Drive", "connected", "Up."),
            ],
        )),
        "run_mode": "online",
    }));
    let names: Vec<&str> = runtime(&fleet, "line")
        .endpoints
        .iter()
        .map(|endpoint| endpoint.name.as_str())
        .collect();
    assert_eq!(names, vec!["Simulated I/O", "Loopback I/O", "Drive"]);
}

/// The configured connections of one ADS client endpoint: the device `5.1.2.3.1.1` on ports 851
/// and 852, each with one imported tag, and another device.
fn ads_connections() -> Value {
    json!({
        "enabled": true,
        "connections": [
            { "name": "plc", "target_net_id": "5.1.2.3.1.1", "host": "10.0.0.2", "ams_port": 851,
              "points": [{ "var": "a", "symbol": "MAIN.a" }] },
            { "name": "plc_port_852", "target_net_id": "5.1.2.3.1.1", "host": "10.0.0.2",
              "ams_port": 852, "points": [{ "var": "b", "path": "MAIN.b" }] },
            { "name": "other", "target_net_id": "5.9.9.9.1.1", "host": "10.0.0.9", "ams_port": 851,
              "points": [{ "var": "x", "symbol": "X" }] },
        ],
    })
}

#[test]
fn an_ads_client_browses_its_first_connection_with_the_devices_ports_and_tags() {
    // Specification 25 §5.4: ADS browse receives one configured client connection, not the whole
    // connection array. The first connection's device (`5.1.2.3.1.1` at 10.0.0.2) answers on its
    // connections' ports 851 and 852 and has `MAIN.a` on 851 and `MAIN.b` on 852 imported; the
    // other device's port and tag are not its own. A wrong implementation browses the whole
    // parameter object, or mixes the other device's tag `X` in.
    let fleet = fleet_of(json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1", report(
            "host:plc-01", "10.0.0.5", "line", "connected",
            &[endpoint_with("endpoint:RESOURCE:ads", "ads", ads_connections())],
        )),
        "run_mode": "online",
    }));
    let target = runtime(&fleet, "line").endpoints[0]
        .browse_target
        .clone()
        .expect("a browse target");
    assert_eq!(target["name"], json!("plc"));
    assert_eq!(target["ams_port"], json!(851));
    assert_eq!(target["responding_ads_ports"], json!([851, 852]));
    assert_eq!(
        target["imported_ads_symbols"],
        json!([
            { "port": 851, "paths": ["MAIN.a"] },
            { "port": 852, "paths": ["MAIN.b"] },
        ])
    );
    assert_eq!(target["connections"], ads_connections()["connections"]);
}

#[test]
fn a_client_connection_browses_its_first_connection_and_others_their_parameters() {
    // Specification 25 §5.4: OPC UA browse receives the first configured client connection; a
    // connection without client connections browses with its own parameters (no browse target).
    // An ADS connection without a port is browsed at 851 (specification 11). A wrong
    // implementation answers the whole parameter object for OPC UA, or gives Modbus a target.
    let fleet = fleet_of(json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1", report(
            "host:plc-01", "10.0.0.5", "line", "connected",
            &[
                endpoint_with("endpoint:RESOURCE:opcua_client", "opcua_client", json!({
                    "enabled": true,
                    "connections": [
                        { "name": "a", "endpoint_url": "opc.tcp://a:4840" },
                        { "name": "b", "endpoint_url": "opc.tcp://b:4840" },
                    ],
                })),
                endpoint_with("endpoint:RESOURCE:modbus_tcp:0", "modbus_tcp", json!({ "address": "10.0.0.7:502" })),
                endpoint_with("endpoint:RESOURCE:ads", "ads", json!({
                    "connections": [{ "name": "plc", "host": "10.0.0.2" }],
                })),
            ],
        )),
        "run_mode": "online",
    }));
    let line = runtime(&fleet, "line");
    assert_eq!(
        line.endpoints[0].browse_target,
        Some(json!({ "name": "a", "endpoint_url": "opc.tcp://a:4840" }))
    );
    assert_eq!(line.endpoints[1].browse_target, None);
    assert_eq!(
        line.endpoints[2]
            .browse_target
            .as_ref()
            .map(|target| target["ams_port"].clone()),
        Some(json!(851))
    );
}

#[test]
fn a_discovered_ads_device_says_whether_the_project_uses_it() {
    // A search's ADS find is `On canvas` when a configured ADS connection serves the same device
    // (its AMS Net ID and host do not contradict the find's and it shares one of them), with the
    // tags already imported per port; any other find is not, and a find of another protocol is never
    // on the canvas and has no tags. A wrong implementation matches on the host alone when the AMS
    // Net IDs differ, or answers the other device's tags.
    let answer = merged(json!({
        "primary": answered("tcp://10.0.0.5:9901", "Line 1", report(
            "host:plc-01", "10.0.0.5", "line", "connected",
            &[endpoint_with("endpoint:RESOURCE:ads", "ads", ads_connections())],
        )),
        "run_mode": "online",
        "discovered": [
            { "id": "ads:5.1.2.3.1.1", "protocol": "ads",
              "params": { "ams_net_id": "5.1.2.3.1.1", "host": "10.0.0.2" } },
            { "id": "ads:5.7.7.7.1.1", "protocol": "ads",
              "params": { "ams_net_id": "5.7.7.7.1.1", "host": "10.0.0.2" } },
            { "id": "modbus:10.0.0.7", "protocol": "modbus_tcp", "params": { "host": "10.0.0.7" } },
        ],
    }));
    let facts: Vec<(String, bool, Value)> = answer
        .discovered
        .iter()
        .map(|facts| {
            (
                facts.id.clone(),
                facts.on_canvas,
                serde_json::to_value(&facts.imported_ads_symbols).expect("selections"),
            )
        })
        .collect();
    assert_eq!(
        facts,
        vec![
            (
                "ads:5.1.2.3.1.1".to_owned(),
                true,
                json!([{ "port": 851, "paths": ["MAIN.a"] }, { "port": 852, "paths": ["MAIN.b"] }])
            ),
            ("ads:5.7.7.7.1.1".to_owned(), false, json!([])),
            ("modbus:10.0.0.7".to_owned(), false, json!([])),
        ]
    );
}

// ---- trust-lsp.devicesCanvasAct: forms (specification 25 §5.4, §5.9 and §5.10) ----

#[test]
fn a_form_needs_a_runtime_that_answers_to_be_applied_or_tested() {
    // Specification 25 §5.10: `Apply to running runtime` and `Test` act on a runtime that answers;
    // without one the form shows why, and nothing is applied. A wrong implementation applies the
    // form to the project's files, or answers no result at all.
    for (op, reason) in [
        ("comm_apply", form_texts::APPLY_NEEDS_REACHABLE),
        ("comm_test", form_texts::TEST_NEEDS_REACHABLE),
    ] {
        let reply =
            act_json(json!({ "op": op, "args": { "protocol": "Modbus-TCP", "params": {} } }));
        assert!(!reply.applied, "{op}");
        assert_eq!(result(&reply)["protocol"], json!("modbus_tcp"), "{op}");
        assert_eq!(result(&reply)["lifecycle_effect"], json!("blocked"), "{op}");
        assert_eq!(result(&reply)["message"], json!(reason), "{op}");
    }
    let nothing = act_json(json!({ "op": "comm_apply", "args": { "protocol": "  " } }));
    assert_eq!(
        nothing,
        CanvasReply::nothing(),
        "a form without a protocol does nothing"
    );
}

#[test]
fn a_secret_is_never_sent_over_plain_tcp_to_another_computer() {
    // Specification 25 §5.9 and specification 11 §6.9.7: a secret that would travel over plain TCP
    // to another computer is never sent; the form names the secret's field. Over a loopback
    // endpoint the request is sent and the runtime decides (here nothing listens on port 1, so the
    // form shows the connection's failure instead). A wrong implementation sends the secret to the
    // remote runtime, or refuses it on this computer too.
    let secret = json!({ "protocol": "mqtt", "params": { "broker": "10.0.0.7:1883", "password": "s3cret" } });
    for op in ["comm_apply", "comm_test"] {
        let remote = act_json(json!({
            "op": op, "args": secret, "runtime": { "endpoint": "tcp://192.0.2.10:9901" },
        }));
        assert!(!remote.applied, "{op}");
        assert_eq!(
            result(&remote)["message"],
            json!(form_texts::SECRET_NOT_SENT),
            "{op}"
        );
        assert_eq!(
            result(&remote)["field_errors"],
            json!([{ "field": "password", "message": form_texts::SECRET_FIELD_NOT_SENT }]),
            "{op}"
        );
        let local = act_json(json!({
            "op": op, "args": secret, "runtime": { "endpoint": "tcp://127.0.0.1:1" },
        }));
        assert_ne!(
            result(&local)["message"],
            json!(form_texts::SECRET_NOT_SENT),
            "{op}"
        );
        assert_eq!(result(&local)["lifecycle_effect"], json!("blocked"), "{op}");
    }
}

#[test]
fn form_values_that_do_not_fit_their_fields_are_refused_before_they_are_sent() {
    // Specification 25 §5.9: Rust converts each submitted value by its field's schema type; text in
    // a number field is a field error of that field and is never sent as text. The Modbus unit ID
    // `abc` is refused with `Enter a whole number.` and the form's message asks to fix the
    // highlighted fields. A wrong implementation sends `"abc"` to the runtime (which would answer a
    // connection failure here, as nothing listens), or loses the field's name.
    let reply = act_json(json!({
        "op": "comm_apply",
        "runtime": { "endpoint": "tcp://127.0.0.1:1" },
        "args": {
            "protocol": "modbus_tcp",
            "values": { "address": "10.0.0.9:502", "unit_id": "abc" },
            "schema": catalog(),
        },
    }));
    assert!(!reply.applied);
    assert_eq!(
        result(&reply)["message"],
        json!(form_texts::FIX_HIGHLIGHTED_FIELDS)
    );
    assert_eq!(
        result(&reply)["field_errors"],
        json!([{ "field": "unit_id", "message": form_texts::ENTER_WHOLE_NUMBER }])
    );

    let unknown = act_json(json!({
        "op": "comm_apply",
        "runtime": { "endpoint": "tcp://127.0.0.1:1" },
        "args": { "protocol": "no_such_protocol", "values": {}, "schema": catalog() },
    }));
    assert_eq!(
        result(&unknown)["message"],
        json!(form_texts::NO_NATIVE_SCHEMA)
    );
}

#[test]
fn a_saved_form_writes_only_the_fields_its_backend_shows() {
    // Specification 25 §5.9: a conditional field is serialized only while its `visible_when`
    // predicate matches the selected backend; on submit Rust evaluates it again and ignores the
    // values of hidden fields. GPIO with the `libgpiod` backend saves `chip` and not the
    // `sysfs_base` that only the `sysfs` backend shows, and `Save` works with no runtime running
    // (§5.3). A wrong implementation writes the hidden field, or needs a runtime to save.
    let project = TempFolder::new("canvas-save-gpio");
    let schema = catalog();
    let gpio = schema["protocols"]
        .as_array()
        .and_then(|protocols| {
            protocols
                .iter()
                .find(|protocol| protocol["id"] == json!("gpio"))
        })
        .expect("the catalog's GPIO");
    let mut values = Map::new();
    for field in gpio["fields"].as_array().expect("GPIO fields") {
        let default = &field["default"];
        let text = match default {
            Value::String(text) => text.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        values.insert(
            field["id"].as_str().expect("a field ID").to_owned(),
            Value::String(text),
        );
    }
    values.insert("backend".to_owned(), json!("libgpiod"));
    values.insert("sysfs_base".to_owned(), json!("/sys/hidden-gpio"));
    let reply = act_json(json!({
        "op": "comm_save",
        "project": project.path,
        "args": { "protocol": "gpio", "action": "upsert", "values": values, "schema": schema },
    }));
    assert!(reply.applied, "{reply:?}");
    assert!(reply.refresh);
    let io = std::fs::read_to_string(project.join("io.toml")).expect("io.toml was written");
    assert!(io.contains("/dev/gpiochip0"), "{io}");
    assert!(!io.contains("/sys/hidden-gpio"), "{io}");
    assert!(
        result(&reply)["instance_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
        "the saved connection is named: {reply:?}"
    );
}

#[test]
fn a_saved_connection_is_named_only_when_that_is_unambiguous() {
    // Specification 25 §5.4: when the backend omits an ID, topology matching is allowed only when
    // unambiguous. The submitted address `10.0.0.2:502` with a unit ID that the saved parameters do
    // not hold matches only `b`; two connections that both match name none; with no match the only
    // connection of the protocol is named, and two are ambiguous. A wrong implementation picks the
    // last match, as the canvas's TypeScript did, or compares keys the saved parameters lack.
    let topology = json!({
        "hosts": [{
            "runtimes": [{ "endpoints": [
                { "id": "a", "protocol": "modbus_tcp", "params": { "address": "10.0.0.1:502" } },
                { "id": "mqtt", "protocol": "mqtt", "params": { "address": "10.0.0.2:502" } },
            ] }],
            "containers": [{ "runtimes": [{ "endpoints": [
                { "id": "b", "protocol": "modbus_tcp", "params": { "address": "10.0.0.2:502" } },
            ] }] }],
        }],
    });
    let endpoints = endpoints_of(&topology, "modbus_tcp");
    assert_eq!(
        endpoints
            .iter()
            .map(|endpoint| endpoint["id"].clone())
            .collect::<Vec<_>>(),
        vec![json!("a"), json!("b")],
        "bare runtimes first, then those in containers, each of the protocol"
    );
    let submitted = object(json!({ "address": "10.0.0.2:502", "unit_id": 1 }));
    assert_eq!(unambiguous(&endpoints, &submitted).as_deref(), Some("b"));
    let both = object(json!({ "unit_id": 1 }));
    assert_eq!(
        unambiguous(&endpoints, &both),
        None,
        "two matches are ambiguous"
    );
    let neither = object(json!({ "address": "10.0.0.9:502" }));
    assert_eq!(
        unambiguous(&endpoints, &neither),
        None,
        "two candidates are ambiguous"
    );
    assert_eq!(
        unambiguous(&endpoints[..1], &neither).as_deref(),
        Some("a"),
        "the only connection of the protocol"
    );
}

#[test]
fn an_opcua_client_test_tests_its_first_connection() {
    // Specification 25 §5.10 with specification 11's `comm.test`, which reads one connection's
    // parameters: an OPC UA client that holds a list of connections tests the first; any other
    // protocol tests its parameters. A wrong implementation sends the whole list, which `comm.test`
    // cannot read, or picks the first connection of another protocol.
    let opcua = object(json!({
        "enabled": true,
        "connections": [{ "endpoint_url": "opc.tcp://a:4840" }, { "endpoint_url": "opc.tcp://b:4840" }],
    }));
    assert_eq!(
        tested_params("opcua_client", opcua.clone()),
        object(json!({ "endpoint_url": "opc.tcp://a:4840" }))
    );
    assert_eq!(tested_params("ads", opcua.clone()), opcua);
    let single = object(json!({ "endpoint_url": "opc.tcp://a:4840" }));
    assert_eq!(tested_params("opcua_client", single.clone()), single);
}

#[test]
fn the_add_form_adds_to_a_protocol_of_several_instances() {
    // The add form applies `add` to a protocol that holds several instances and can be added to,
    // else `upsert`; an action the form names is kept, trimmed and in lower case. Modbus TCP in the
    // tool's catalog holds several instances. A wrong implementation always upserts, which replaces
    // the existing Modbus device, or ignores the named action.
    let schema = catalog();
    assert_eq!(apply_action("", "modbus_tcp", Some(&schema)), "add");
    assert_eq!(
        apply_action(" UPSERT ", "modbus_tcp", Some(&schema)),
        "upsert"
    );
    assert_eq!(apply_action("", "modbus_tcp", None), "upsert");
    let single = json!({ "protocols": [{ "id": "opcua", "supports_multi_instance": false, "actions": ["add"] }] });
    assert_eq!(apply_action("", "opcua", Some(&single)), "upsert");
}

// ---- trust-lsp.devicesCanvasAct: plans, ADS and the fleet (specification 25 §5.2, §5.4, §5.8) ----

#[test]
fn expose_uses_the_projects_first_server_of_the_protocol() {
    // Specification 25 §5.8: globals are exposed through the server that owns them; the canvas
    // writes the first configured server of the protocol, and a server that is not configured asks
    // to configure it first. A wrong implementation exposes through a server of another protocol,
    // or writes a new server.
    let topology = json!({
        "hosts": [{ "runtimes": [{ "endpoints": [
            { "id": "web", "protocol": "web", "params": { "listen": "0.0.0.0:8080" } },
            { "id": "ua", "protocol": "opcua", "params": { "listen": "0.0.0.0:4840", "expose": ["A"] } },
            { "id": "ua2", "protocol": "opcua", "params": { "listen": "0.0.0.0:4841" } },
        ] }] }],
    });
    assert_eq!(
        endpoint_params(&topology, "opcua"),
        Some(object(json!({ "listen": "0.0.0.0:4840", "expose": ["A"] })))
    );
    let project = TempFolder::new("canvas-expose");
    let reply = act_json(json!({
        "op": "expose",
        "project": project.path,
        "args": { "protocol": "ads_server", "topology": topology, "paths": ["global.Level"] },
    }));
    assert!(!reply.applied);
    assert_eq!(
        reply.notice.map(|notice| (notice.tone, notice.text)),
        Some((
            "warning",
            "Configure the ADS server first, then choose globals to expose.".to_owned()
        ))
    );
}

#[test]
fn ethercat_channels_are_saved_on_the_browsed_bus_sorted_and_once() {
    // Specification 25 §5.8: the channels chosen in an EtherCAT browse are saved on the browsed
    // bus as its `selected_channels`, each trimmed, once and in sorted order; without a browsed bus
    // the project's first configured one is used, and a project without one asks to configure the
    // modules first. A wrong implementation saves the channels in the order chosen, keeps a
    // duplicate or a blank, or writes a bus nobody configured.
    let project = TempFolder::new("canvas-ethercat");
    let not_configured = act_json(json!({
        "op": "ethercat_channels",
        "project": project.path,
        "args": { "topology": { "hosts": [] }, "paths": ["ethercat.slot1.channel0"] },
    }));
    assert!(!not_configured.applied);
    assert_eq!(
        not_configured.notice.map(|notice| notice.text),
        Some(plan_texts::ETHERCAT_NOT_CONFIGURED.to_owned())
    );
    assert!(!project.join("io.toml").exists(), "nothing was written");

    let saved = act_json(json!({
        "op": "ethercat_channels",
        "project": project.path,
        "args": {
            "target": {
                "adapter": "mock", "timeout_ms": 250, "cycle_warn_ms": 5, "on_error": "fault",
                "modules": [
                    { "model": "EK1100", "slot": 0, "channels": 1 },
                    { "model": "EL1008", "slot": 1, "channels": 8 },
                ],
                "mock_inputs": [],
            },
            "paths": [
                " ethercat.slot1.channel3 ",
                "ethercat.slot1.channel0",
                "ethercat.slot1.channel3",
                "  ",
            ],
        },
    }));
    assert!(saved.applied, "{saved:?}");
    assert!(saved.refresh);
    let io: toml::Value =
        toml::from_str(&std::fs::read_to_string(project.join("io.toml")).expect("io.toml"))
            .expect("io.toml is TOML");
    assert_eq!(
        selected_channels(&io),
        Some(vec![
            "ethercat.slot1.channel0".to_owned(),
            "ethercat.slot1.channel3".to_owned(),
        ])
    );
}

/// The `selected_channels` that a TOML document holds, at any depth.
fn selected_channels(value: &toml::Value) -> Option<Vec<String>> {
    match value {
        toml::Value::Table(table) => table.iter().find_map(|(key, value)| {
            if key == "selected_channels" {
                value.as_array().map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect()
                })
            } else {
                selected_channels(value)
            }
        }),
        toml::Value::Array(items) => items.iter().find_map(selected_channels),
        _ => None,
    }
}

#[test]
fn a_typed_ads_port_is_a_whole_number_from_1_to_65535() {
    // Specification 11: an ADS port is a whole number from 1 through 65,535; an empty port field
    // browses the first PLC runtime, 851. A wrong implementation accepts 0, 65536, a sign, a
    // fraction, or overflows on a long number.
    assert_eq!(typed_port(""), Ok(851));
    assert_eq!(typed_port("  852 "), Ok(852));
    assert_eq!(typed_port("1"), Ok(1));
    assert_eq!(typed_port("65535"), Ok(65535));
    let out_of_range = Err("ADS port must be between 1 and 65535.");
    assert_eq!(typed_port("0"), out_of_range);
    assert_eq!(typed_port("65536"), out_of_range);
    assert_eq!(typed_port("99999999999999999999999"), out_of_range);
    let not_a_number = Err("Enter a whole-number ADS port from 1 to 65535.");
    assert_eq!(typed_port("-1"), not_a_number);
    assert_eq!(typed_port("85.1"), not_a_number);
    assert_eq!(typed_port("85x"), not_a_number);

    let reply = act_json(json!({ "op": "ads_port", "args": { "text": "0" } }));
    assert_eq!(
        reply.result,
        Some(json!({ "error": "ADS port must be between 1 and 65535." }))
    );
    let reply = act_json(json!({ "op": "ads_port", "args": { "text": "301" } }));
    assert_eq!(reply.result, Some(json!({ "port": 301 })));
}

#[test]
fn an_ads_tag_batch_without_a_project_names_each_port_once() {
    // Specification 25 §5.4: the tag browser imports the chosen tags of each ADS port; the tags of
    // one port are trimmed, kept once and sorted, a port that is not 1 to 65535 or has no tag is
    // left out, and without an open project nothing is added and the summary says so. A wrong
    // implementation imports port 0, keeps `a` twice, or claims tags were added.
    let reply = act_json(json!({
        "op": "ads_tags_add",
        "args": {
            "target": { "ams_net_id": "5.1.2.3.1.1", "host": "10.0.0.2" },
            "selections": [
                { "port": 851, "paths": [" b", "a", "a", ""] },
                { "port": 0, "paths": ["x"] },
                { "port": 852, "paths": [] },
            ],
            "changed_path": "a",
        },
    }));
    assert!(!reply.applied);
    assert!(!reply.refresh);
    assert_eq!(
        reply.result,
        Some(json!({
            "operation": "add",
            "applied": false,
            "addedCount": 0,
            "restartRequired": false,
            "ports": [{
                "port": 851, "addedCount": 0, "paths": ["a", "b"], "applied": false,
                "message": "Open a truST project before adding ADS tags.",
            }],
            "summary": "No ADS tags were added.",
        }))
    );
}

#[test]
fn removing_an_ads_tag_needs_a_port_a_path_and_a_project() {
    // Specification 25 §5.4: a removal names one tag of one port; a selection without a valid port
    // or a path does nothing, and without an open project nothing is removed. A wrong
    // implementation removes with port 0, or reports a removal.
    let invalid = act_json(json!({
        "op": "ads_tag_remove",
        "args": { "target": {}, "selection": { "port": 0, "path": "MAIN.a" } },
    }));
    assert_eq!(invalid, CanvasReply::nothing());
    let reply = act_json(json!({
        "op": "ads_tag_remove",
        "args": { "target": {}, "selection": { "port": 851, "path": " MAIN.a " } },
    }));
    assert_eq!(
        reply.result,
        Some(json!({
            "operation": "remove", "applied": false, "addedCount": 0, "removedCount": 0,
            "restartRequired": false,
            "ports": [{
                "port": 851, "paths": ["MAIN.a"], "applied": false, "addedCount": 0,
                "message": "Open a truST project before removing ADS tags.",
            }],
            "summary": "No ADS tags were removed.",
        }))
    );
}

#[test]
fn an_offline_import_of_writable_tags_offers_to_start_the_runtime() {
    // Specification 25 §5.4 and §5.8: ADS tag import works for a stopped project, but writable tags
    // need a running runtime to apply the explicit write acknowledgement, so the notice waits for
    // the user and offers `Start runtime`. Without a target host the import says so. A wrong
    // implementation imports writable tags offline, or warns without the action.
    let project = TempFolder::new("canvas-ads-writable");
    let writable = act_json(json!({
        "op": "ads_tags_import",
        "project": project.path,
        "args": {
            "protocol": "ads",
            "target": { "host": "10.0.0.2", "ams_net_id": "5.1.2.3.1.1" },
            "paths": ["ads:symbol:MAIN.a"],
            "writable": true,
        },
    }));
    let notice = writable.notice.expect("a notice");
    assert!(notice.modal);
    assert_eq!(
        notice.text,
        "Write-enabled ADS imports need a running runtime."
    );
    assert_eq!(
        notice
            .actions
            .iter()
            .map(|action| (action.id, action.label))
            .collect::<Vec<_>>(),
        vec![("start_runtime", "Start runtime")]
    );
    let hostless = act_json(json!({
        "op": "ads_tags_import",
        "project": project.path,
        "args": { "protocol": "ads", "target": {}, "paths": ["MAIN.a"] },
    }));
    assert_eq!(
        hostless.notice.map(|notice| notice.text),
        Some("Could not add ADS tags: ADS tag import needs a target host.".to_owned())
    );
    let elsewhere = act_json(json!({
        "op": "ads_tags_import",
        "project": project.path,
        "args": { "protocol": "opcua_client", "target": {}, "paths": ["n"] },
    }));
    assert!(elsewhere.notice.is_some_and(|notice| notice
        .text
        .starts_with("Add tags needs a reachable runtime")));
}

#[test]
fn a_typed_runtime_address_joins_the_fleet_once() {
    // Specification 25 §5.2: `host:port` normalizes to a control endpoint, and an endpoint already
    // in the fleet is not added twice. Without a token the canvas focuses the runtime drawn for the
    // new endpoint. Text that is no control endpoint is refused with the address help (decision of
    // 26 October 2026, specification 25 §5.2). A wrong implementation adds `plc.local` as an
    // endpoint, adds a duplicate, or focuses a runtime ID that the canvas never draws.
    let added = act_json(json!({
        "op": "add_host",
        "args": { "endpoint": " 10.0.0.5:5680 ", "fleet_endpoints": ["tcp://10.0.0.9:9901"] },
    }));
    assert!(added.applied && added.refresh);
    assert_eq!(
        added.result,
        Some(json!({
            "endpoint": "tcp://10.0.0.5:5680",
            "already": false,
            "fleet_endpoints": ["tcp://10.0.0.9:9901", "tcp://10.0.0.5:5680"],
            "focus": configured_runtime_id("tcp://10.0.0.5:5680"),
        }))
    );
    let with_token = act_json(json!({
        "op": "add_host", "args": { "endpoint": "unix:///tmp/rt.sock", "token": true },
    }));
    assert_eq!(result(&with_token)["focus"], Value::Null);
    let again = act_json(json!({
        "op": "add_host",
        "args": { "endpoint": "tcp://10.0.0.9:9901", "fleet_endpoints": ["tcp://10.0.0.9:9901"] },
    }));
    assert!(!again.applied);
    assert_eq!(
        result(&again)["fleet_endpoints"],
        json!(["tcp://10.0.0.9:9901"])
    );
    assert_eq!(
        again.notice.map(|notice| notice.text),
        Some("tcp://10.0.0.9:9901 is already in the fleet.".to_owned())
    );
    let refused = act_json(json!({ "op": "add_host", "args": { "endpoint": "plc.local" } }));
    assert!(!refused.applied);
    assert_eq!(
        refused.notice.map(|notice| (notice.tone, notice.text)),
        Some(("warning", form_texts::RUNTIME_ADDRESS_HELP.to_owned()))
    );
    assert_eq!(
        act_json(json!({ "op": "add_host", "args": {} })),
        CanvasReply::nothing()
    );
}

#[test]
fn an_action_the_canvas_does_not_have_is_an_error() {
    // Specification 25 §5.15: every action is a Rust command; an unknown action is refused with a
    // stable code instead of doing anything. A wrong implementation answers an empty reply.
    let request: CanvasActRequest =
        serde_json::from_value(json!({ "op": "format_disk" })).expect("an action");
    let error = act(&request).expect_err("an unknown action");
    assert_eq!(error.code, "unknown_canvas_action");
}

/// The `ads.toml` of the removal tests: one device on port 301 with the tag in two connections,
/// and the same device on port 851.
const TAG_ADS_TOML: &str = r#"[[connections]]
name = "line_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301
local_net_id = "192.168.77.10.1.1"

[[connections.points]]
var = "var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections]]
name = "duplicate_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301

[[connections.points]]
var = "duplicate_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections]]
name = "line_851"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 851

[[connections.points]]
var = "other_port_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"
"#;

#[test]
fn removing_an_ads_tag_runs_the_runtimes_removal_for_the_device_and_its_port() {
    // Specification 11 §6.8.1b and specification 25 §5.4: removing a tag runs the runtime's
    // `remove-symbols` removal for the device's host and AMS Net ID and the tag's port. The tag of
    // port 301 matches two connections; with no cached snapshot the ST cannot be regenerated, so
    // nothing is written and the runtime's reason is shown. The same tag of another device is
    // already absent, and nothing changes. A wrong implementation drops the device's identity
    // (removing the tag of port 851 or of another device), or edits `ads.toml` itself.
    let project = TempFolder::new("canvas-ads-remove");
    project.write("ads.toml", TAG_ADS_TOML);
    let device = object(json!({ "host": "192.168.77.11", "target_net_id": "100.67.6.217.1.1" }));
    assert_eq!(
        remove_offline(&project.path, &device, 301, "Task 4.Inputs.Var 1"),
        Removal {
            applied: false,
            removed: 0,
            restart: false,
            message:
                "Could not regenerate ADS ST because no cached ADS symbol snapshots were found."
                    .to_owned(),
            documents: Vec::new(),
        }
    );
    let other = object(json!({ "ip": "192.168.77.11", "ams_net_id": "5.5.5.5.1.1" }));
    let absent = remove_offline(&project.path, &other, 301, "Task 4.Inputs.Var 1");
    assert!(absent.applied);
    assert_eq!(absent.removed, 0);
    assert_eq!(
        absent.message,
        "The ADS tag was already absent from ads.toml."
    );
    assert_eq!(
        std::fs::read_to_string(project.join("ads.toml")).expect("ads.toml"),
        TAG_ADS_TOML
    );
}

#[test]
fn a_removal_report_is_shown_with_the_generated_st_it_rewrote() {
    // Specification 25 §5.4: the tag browser shows the removal's own applied flag, count, restart
    // need and message, and the canvas opens the generated Structured Text that the removal
    // rewrote so that its diagnostics refresh. A wrong implementation hides the report, or opens
    // nothing after rewriting the ST.
    let project = Path::new("/home/u/plant/line1");
    let report = json!({
        "schema_version": 1, "applied": true, "matched": true, "removed_count": 2,
        "restart_required": true,
        "files_written": ["ads.toml", "src/generated/ads_generated.st"],
        "message": "Removed 2 ADS tags.",
    });
    assert_eq!(
        removal(project, &report),
        Removal {
            applied: true,
            removed: 2,
            restart: true,
            message: "Removed 2 ADS tags.".to_owned(),
            documents: vec![project
                .join("src")
                .join("generated")
                .join("ads_generated.st")
                .to_string_lossy()
                .into_owned()],
        }
    );
    let mut toml_only = report;
    toml_only["files_written"] = json!(["ads.toml"]);
    assert_eq!(removal(project, &toml_only).documents, Vec::<String>::new());
}

#[test]
fn an_ads_tag_batch_summary_names_its_count_and_the_ports_that_applied() {
    // Specification 25 §5.4 (correct singular and plural grammar): a batch that added two tags on
    // ports 301 and 851 reads `Added 2 tags from ADS ports 301, 851.`; a removal of one tag on one
    // port reads `Removed 1 tag from ADS port 301.`; a port that did not apply is not named, and a
    // batch where none applied says that nothing was added. A wrong implementation names the
    // failed port, or pluralizes one tag or one port.
    let ports = [
        json!({ "port": 301, "applied": true }),
        json!({ "port": 502, "applied": false }),
        json!({ "port": 851, "applied": true }),
    ];
    assert_eq!(
        batch_summary("add", &ports, 2),
        "Added 2 tags from ADS ports 301, 851."
    );
    assert_eq!(
        batch_summary("remove", &ports[..1], 1),
        "Removed 1 tag from ADS port 301."
    );
    assert_eq!(
        batch_summary("add", &ports[1..2], 0),
        "No ADS tags were added."
    );
}

#[test]
fn an_offline_import_names_the_device_port_and_symbols() {
    // Specification 25 §5.4 and §5.8 (per-port imports keep distinct connection identities): a
    // read-only import of a device that answers on ports 301 and 851 imports into the connection
    // named for port 301, `PLC laptop_port_301`, at ADS port 301, with the browse tree's
    // `ads:symbol:` prefix removed and an empty symbol left out; a device without a port is
    // imported at 851. A wrong implementation drops the selected port (importing at 851), keeps
    // the prefix, or names one connection for every port.
    let target = object(json!({
        "name": "PLC laptop", "host": "192.168.77.11", "ams_net_id": "100.67.6.217.1.1",
        "ams_port": 301, "responding_ads_ports": [301, 851], "tc_version": "3.1.4026",
    }));
    let paths = [
        "ads:symbol:GVL.Input".to_owned(),
        "MAIN.b".to_owned(),
        "ads:symbol:".to_owned(),
    ];
    assert_eq!(
        offline_import_params(&target, &paths, None),
        Ok((
            json!({
                "connection_name": "PLC laptop_port_301",
                "symbols": ["GVL.Input", "MAIN.b"],
                "target": {
                    "name": "PLC laptop", "ip": "192.168.77.11", "ams_net_id": "100.67.6.217.1.1",
                    "ams_port": 301, "tc_version": "3.1.4026",
                },
                "write_acknowledged": false,
            }),
            2
        ))
    );
    let portless = object(json!({ "ip": "192.168.77.11" }));
    let (params, _) = offline_import_params(&portless, &paths, Some("line")).expect("an import");
    assert_eq!(params["target"]["ams_port"], json!(851));
    assert_eq!(params["connection_name"], json!("line"));
    assert_eq!(
        offline_import_params(&object(json!({})), &paths, None),
        Err("ADS tag import needs a target host.")
    );
    assert_eq!(
        offline_import_params(&portless, &["ads:symbol:".to_owned()], None),
        Err("Select at least one ADS symbol to import.")
    );
}

#[test]
fn an_ads_find_that_no_port_answered_is_left_out_of_the_cards() {
    // Specification 25 §2: an ADS find with no responding port is left out of the cards; when
    // TwinCAT identity was found and no port answered, the source row says so and names the
    // configured ports, the default list when none is configured. A find with a responding port
    // stays. A wrong implementation shows the portless find, drops the answering one, or names the
    // default ports while the user configured others.
    let answering =
        json!({ "id": "ads:a", "protocol": "ads", "params": { "responding_ads_ports": [851] } });
    let silent =
        json!({ "id": "ads:b", "protocol": "ads", "params": { "responding_ads_ports": [] } });
    let reply = act_json(json!({
        "op": "ads_finds",
        "args": { "candidates": [answering, silent], "ads_ports": [852, 851, 851] },
    }));
    assert_eq!(reply.result, Some(json!({ "candidates": [answering] })));

    let reply = act_json(json!({
        "op": "ads_finds",
        "args": { "candidates": [silent], "ads_ports": [852, 851, 851] },
    }));
    assert_eq!(
        reply.result,
        Some(json!({
            "candidates": [],
            "error": "TwinCAT identity was found, but none of the configured ADS ports (851, 852) responded.",
        }))
    );
    let reply = act_json(json!({ "op": "ads_finds", "args": { "candidates": [silent] } }));
    assert_eq!(
        reply.result.as_ref().map(|result| result["error"].clone()),
        Some(json!(
            "TwinCAT identity was found, but none of the configured ADS ports (301, 501, 851, 852, 853, 854) responded."
        ))
    );
    let reply = act_json(json!({ "op": "ads_finds", "args": { "candidates": [] } }));
    assert_eq!(
        reply.result,
        Some(json!({ "candidates": [] })),
        "no identity, no text"
    );
}
