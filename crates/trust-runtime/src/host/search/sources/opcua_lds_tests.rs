use std::net::{IpAddr, Ipv4Addr};

use crate::connectors::DiscoveryConfidence;
use crate::opcua::{
    OpcUaClientEndpointInfo, OpcUaFoundServer, OpcUaMessageSecurityMode, OpcUaSecurityPolicy,
};
use crate::search::merge::Devices;
use crate::search::model::SourceId;

use super::super::mdns_services::{self, Announcement};
use super::super::opcua::{endpoint_candidate, endpoint_host, opcua_find};
use super::super::report::DiscoverCandidate;
use super::*;

/// One server that a discovery server listed.
fn found_server(name: &str, discovery_urls: &[&str]) -> OpcUaFoundServer {
    OpcUaFoundServer {
        application_name: Some(name.to_string()),
        discovery_urls: discovery_urls
            .iter()
            .map(|url| (*url).to_string())
            .collect(),
    }
}

/// The endpoint that a server answered its GetEndpoints call with.
fn endpoint_info() -> OpcUaClientEndpointInfo {
    OpcUaClientEndpointInfo {
        endpoint_url: "opc.tcp://10.0.0.6:4840/UA/Plant".to_string(),
        application_name: Some("Line PLC".to_string()),
        application_uri: Some("urn:acme:plc".to_string()),
        product_uri: Some("urn:acme:product".to_string()),
        security_policy: Some(OpcUaSecurityPolicy::None),
        security_policy_uri: "http://opcfoundation.org/UA/SecurityPolicy#None".to_string(),
        security_mode: Some(OpcUaMessageSecurityMode::None),
        anonymous_supported: true,
        username_supported: false,
        unsupported: false,
        certificate_sha256: None,
    }
}

/// The announcement of the server that `endpoint_info` describes.
fn line_plc_announcement() -> Announcement {
    Announcement {
        fullname: "line-plc._opcua-tcp._tcp.local.".to_string(),
        addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 6))],
        port: 4840,
        path: Some("/UA/Plant".to_string()),
    }
}

#[test]
fn opcua_servers_found_through_find_servers_and_mdns() {
    // Specification 11 ("Sources"): the OPC UA source finds servers through the local discovery
    // servers (FindServers), mDNS `_opcua-tcp._tcp.local.`, and GetEndpoints for every server
    // found.
    let servers = vec![
        found_server(
            "Line PLC",
            &[
                "opc.tcp://10.0.0.6:4840/UA/Plant",
                "opc.tcp://10.0.0.6:4841",
            ],
        ),
        found_server("Cell PLC", &["opc.tcp://10.0.0.7:4840"]),
        found_server("Bench PLC", &[]),
    ];
    let (urls, warnings) = discovery_urls(&servers);
    assert_eq!(
        urls,
        vec![
            "opc.tcp://10.0.0.6:4840/UA/Plant".to_string(),
            "opc.tcp://10.0.0.6:4841".to_string(),
            "opc.tcp://10.0.0.7:4840".to_string(),
        ]
    );
    assert_eq!(
        warnings,
        vec!["The discovery server listed Bench PLC without a discovery URL."]
    );

    // A discovery server is asked at the address it names, so the server that its discovery URL
    // names is the device that mDNS announces at the same address.
    assert_eq!(endpoint_host(&urls[0]), Some("10.0.0.6"));
    assert_eq!(
        mdns_services::opcua_endpoint_url(&line_plc_announcement()),
        Some(urls[0].clone())
    );

    // This computer's own local discovery server is asked first, and then each known address at
    // port 4840 unless the address names one.
    assert_eq!(LOCAL_DISCOVERY_SERVER, "opc.tcp://localhost:4840");
    assert_eq!(
        discovery_url("10.0.0.6").expect("a known address"),
        "opc.tcp://10.0.0.6:4840"
    );
    assert_eq!(
        discovery_url("10.0.0.6:4841").expect("a known address with a port"),
        "opc.tcp://10.0.0.6:4841"
    );
}

#[test]
fn a_server_found_three_ways_is_one_device() {
    // Specification 11 ("Devices"): results that share a network address are reported as one
    // device, and not merged by any rule of this slice's own.
    let endpoint = endpoint_info();
    let address = "10.0.0.6";

    // The three ways one search finds the same server: a discovery server lists it, it announces
    // `_opcua-tcp._tcp.local.`, and the known address answers GetEndpoints.
    let through_lds = opcua_find(
        DiscoverCandidate {
            source: LDS_SOURCE,
            ..endpoint_candidate(&endpoint)
        },
        address,
    );
    let mdns_services::Announced::Found(through_mdns) =
        mdns_services::announced_opcua(&line_plc_announcement(), |_| Ok(vec![endpoint.clone()]))
    else {
        panic!("an announced server that answered GetEndpoints is a device");
    };
    assert_eq!(through_mdns.len(), 1);
    let through_mdns = through_mdns[0].clone();
    let through_known_address = opcua_find(endpoint_candidate(&endpoint), address);

    // The port answered TCP before any GetEndpoints did: the weaker sighting of the same result.
    let weak = opcua_find(
        DiscoverCandidate {
            source: "tcp_connect",
            confidence: "port_reachable",
            ..endpoint_candidate(&endpoint)
        },
        address,
    );

    let mut devices = Devices::default();
    devices.add(weak);
    devices.add(through_lds);
    devices.add(through_mdns);
    devices.add(through_known_address);
    let (all, _) = devices.snapshot(None);
    assert_eq!(all.len(), 1, "one server found three ways is one device");
    assert_eq!(all[0].id, "dev:ip:10.0.0.6");
    assert_eq!(all[0].addresses, vec!["10.0.0.6"]);
    assert_eq!(all[0].finds.len(), 1, "the same endpoint is one result");
    assert_eq!(all[0].finds[0].protocol, SourceId::Opcua);
    // Specification 11 ("Presence"): a result seen again keeps the stronger confidence.
    assert_eq!(all[0].finds[0].confidence, DiscoveryConfidence::Confirmed);
}
