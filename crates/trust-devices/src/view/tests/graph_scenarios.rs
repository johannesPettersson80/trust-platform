//! The scenarios that the golden views add for the connections, devices, links, mesh bus, search,
//! filter and issues (design §4.3, B5), each with the catalog of `trust-runtime comm schema` and
//! projected at `NOW_MS`.

use serde_json::json;

use super::fixtures::{host, with_failed_start};
use super::peers::{
    connector_answer, endpoint_of, peer, peer_view, plain, topology_of, view_of, PEER,
};
use crate::request::{RunMode, ViewOptions};
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::types::DevicesView;

/// A runtime with a Modbus meter, an MQTT connection to a shared broker and a mesh peer, with
/// the connector status of the first two.
pub(super) fn connections_and_connector_status() -> DevicesView {
    peer_view(fleet_topology(), Some(connector_answer()), plain())
}

/// The same runtime searched for `modbus`, with its mesh filtered out.
pub(super) fn search_and_filter() -> DevicesView {
    peer_view(
        fleet_topology(),
        None,
        ViewOptions {
            search: "modbus".to_owned(),
            hidden_protocols: vec!["mesh".to_owned()],
            ..ViewOptions::default()
        },
    )
}

/// The simulator did not start, and the runtime's Modbus meter does not answer.
pub(super) fn faults_and_warnings() -> DevicesView {
    let mut topology = fleet_topology();
    let modbus = &mut topology["hosts"][0]["runtimes"][0]["endpoints"][0];
    modbus["health"] = json!("error");
    modbus["detail"] = json!("Unit 1 does not answer.");
    view_of(
        vec![peer(PEER, "running", topology, None)],
        with_failed_start(
            host(RunMode::Simulate, ""),
            "port_conflict",
            "The runtime port is already in use.",
        ),
        plain(),
    )
}

/// A runtime that serves OPC UA and ADS, and reads tags from one ADS device on two ports.
pub(super) fn servers_and_ads_client() -> DevicesView {
    let mut opcua = endpoint_of(
        "endpoint:runtime-a:opcua",
        "opcua",
        "OPC UA server",
        "connected",
        "Serving.",
    );
    opcua["kind"] = json!("service");
    opcua["role"] = json!("server");
    opcua["params"] = json!({
        "listen": "127.0.0.1:4840", "endpoint_path": "/trust",
        "expose": ["global.TankLevel", "global.PumpRunning"],
    });
    let mut ads_server = endpoint_of(
        "endpoint:runtime-a:ads_server",
        "ads_server",
        "ADS server",
        "connected",
        "ADS server runtime is active and listening.",
    );
    ads_server["kind"] = json!("service");
    ads_server["role"] = json!("server");
    ads_server["params"] = json!({
        "listen": "127.0.0.1:48898", "ams_net_id": "127.0.0.1.1.1", "ads_port": 851,
        "expose": ["global.TankLevel"], "clients_summary": ["127.0.0.1.1.100 (from 127.0.0.1)"],
    });
    ads_server["live"] = json!({
        "value": {
            "connected_clients": 1, "proof_status": "external_client_verified",
            "external_client_verified": true, "external_client_kind": "loopback-ads-client",
            "external_client_name": "trust-runtime-doctor",
        },
        "last_seen_ms": 1_760_692_128_112_u64,
    });
    let mut ads = endpoint_of(
        "endpoint:runtime-a:ads",
        "ads",
        "ADS client",
        "connected",
        "Reading configured ADS tags.",
    );
    ads["kind"] = json!("peer");
    ads["role"] = json!("client");
    ads["supports_test"] = json!(true);
    ads["params"] = json!({ "connections": [
        { "name": "internal_port_301", "host": "192.168.77.11",
          "target_net_id": "100.67.6.217.1.1", "ams_port": 301,
          "points": [{ "var": "TaskInput", "symbol": "Task.Input", "type": "BOOL", "access": "read" }] },
        { "name": "internal_port_851", "host": "192.168.77.11",
          "target_net_id": "100.67.6.217.1.1", "ams_port": 851,
          "points": [{ "var": "Level", "symbol": "GVL.Level", "type": "REAL", "access": "read" }] },
    ] });
    peer_view(
        topology_of(vec![opcua, ads_server, ads], Vec::new(), Vec::new()),
        None,
        plain(),
    )
}
