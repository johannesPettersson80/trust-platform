//! The browse entry of the connection inspector (design D5c R10; `W/browseActions.ts:18-35`): the
//! browse that each protocol offers, the request it opens, and what a remote browse needs first.

use serde_json::{json, Value};

use super::fixtures::{
    action, gathered, host, request, stopped_browsable_project, unanswered, CONFIGURED_CONTROL,
    NOW_MS,
};
use super::peers::{connection, endpoint_of, peer_view, plain, topology_of};
use crate::request::RunMode;
use crate::targets::TargetOrigin;
use crate::view::project as project_view;
use crate::view::types::{Action, Emphasis, Op, Pane, UiOp};

/// An endpoint of `Line runtime` of `protocol`, named `name`, that answers as a field driver.
fn driver(protocol: &str, name: &str) -> Value {
    endpoint_of(
        &format!("endpoint:runtime-a:{protocol}"),
        protocol,
        name,
        "connected",
        "Serving.",
    )
}

/// An endpoint of `Line runtime` of `protocol` that serves, as a server does.
fn served(protocol: &str, name: &str) -> Value {
    let mut server = driver(protocol, name);
    server["kind"] = json!("service");
    server["role"] = json!("server");
    server
}

/// The request that a browse action opens, which must open a browse pane.
fn browse_request(browse: &Action) -> &Value {
    let Op::Ui {
        ui: UiOp::Open {
            pane: Pane::Browse { request: body },
        },
    } = &browse.op
    else {
        panic!("the browse opens a browse pane: {:?}", browse.op);
    };
    body
}

/// Catches an inspector that offers no browse, a wrong label or origin kind, a browse placed
/// before the other actions, and a request that carries the session or seq itself.
#[test]
fn the_inspector_offers_the_browse_of_its_protocol() {
    let mut client = driver("opcua_client", "OPC UA client");
    client["supports_test"] = json!(true);
    let topology = topology_of(
        vec![
            driver("ads", "ADS PLC"),
            driver("ethercat", "EtherCAT bus"),
            client,
            served("opcua", "OPC UA server"),
            served("ads_server", "ADS server"),
            driver("modbus_tcp", "Modbus meter"),
            driver("mqtt", "MQTT broker"),
        ],
        Vec::new(),
        Vec::new(),
    );
    let view = peer_view(topology, None, plain());
    for (name, label, kind) in [
        ("ADS PLC", "Manage tags", "symbols"),
        ("EtherCAT bus", "Browse channels", "channels"),
        ("OPC UA client", "Browse nodes", "nodes"),
        ("OPC UA server", "Choose globals", "symbols"),
        ("ADS server", "Choose globals", "symbols"),
    ] {
        let conn = connection(&view, name);
        let actions = &conn.inspector.primary;
        let browse = action(actions, "browse");
        assert_eq!(browse.label, label, "{name} offers {label}");
        assert_eq!(
            browse.emphasis,
            Emphasis::Secondary,
            "{name} shows the entry as secondary"
        );
        assert!(
            browse.enabled,
            "{name} browses from its answering runtime or its own globals"
        );
        assert_eq!(
            actions.last().map(|last| last.id.as_str()),
            Some("browse"),
            "{name} lists the browse after its other actions"
        );
        let body = browse_request(browse);
        assert_eq!(body["kind"], kind, "{name} asks the origin for {kind}");
        assert_eq!(body["trust_server_certificate"], false);
        assert_eq!(body["source"]["connection"], conn.id.as_str());
        assert_eq!(body["source"]["connection_name"], name);
        assert_eq!(
            body.as_object().map(serde_json::Map::len),
            Some(3),
            "the body holds three keys; the extension adds session and seq"
        );
    }
    for name in ["Modbus meter", "MQTT broker"] {
        let actions = &connection(&view, name).inspector.primary;
        assert!(
            actions.iter().all(|action| action.id != "browse"),
            "{name} offers no browse"
        );
    }
    let ids: Vec<&str> = connection(&view, "OPC UA client")
        .inspector
        .primary
        .iter()
        .map(|action| action.id.as_str())
        .collect();
    let test_at = ids.iter().position(|id| *id == "test");
    let browse_at = ids.iter().position(|id| *id == "browse");
    assert!(
        matches!((test_at, browse_at), (Some(test), Some(browse)) if test < browse),
        "the browse follows Test: {ids:?}"
    );
}

/// Catches a remote browse that is enabled while its runtime is stopped, a missing or invented
/// reason, and a project's own globals that wait for a runtime.
#[test]
fn a_remote_browse_needs_its_runtime_and_says_so() {
    let view = project_view(
        &gathered(
            stopped_browsable_project(),
            vec![unanswered(TargetOrigin::Configured, CONFIGURED_CONTROL)],
        ),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    for name in ["ADS PLC", "EtherCAT bus", "OPC UA client"] {
        let browse = action(&connection(&view, name).inspector.primary, "browse");
        assert!(
            !browse.enabled,
            "{name} cannot browse while its runtime is stopped"
        );
        assert_eq!(
            browse.reason.as_deref(),
            Some("Start or connect the runtime, then Browse again to load symbols."),
            "{name} says what to do"
        );
    }
    let globals = action(
        &connection(&view, "OPC UA server").inspector.primary,
        "browse",
    );
    assert!(
        globals.enabled,
        "the project's own globals open without a runtime"
    );
    assert_eq!(globals.reason, None);
}
