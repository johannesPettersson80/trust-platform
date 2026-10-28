use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::json;

use crate::connectors::DiscoveryConfidence;
use crate::error::RuntimeError;
use crate::opcua::{OpcUaClientEndpointInfo, OpcUaMessageSecurityMode, OpcUaSecurityPolicy};
use crate::search::model::{DeviceKey, SourceId};

use super::super::probe::DiscoveryProbeOutcome;
use super::*;

/// An MQTT broker that announced `_mqtt._tcp.local.` at 10.0.0.5:1883.
fn broker() -> Announcement {
    Announcement {
        fullname: "broker-1._mqtt._tcp.local.".to_string(),
        addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))],
        port: 1883,
        path: None,
    }
}

/// An OPC UA server that announced `_opcua-tcp._tcp.local.` at 10.0.0.6:4840.
fn server(path: Option<&str>) -> Announcement {
    Announcement {
        fullname: "line-plc._opcua-tcp._tcp.local.".to_string(),
        addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 6))],
        port: 4840,
        path: path.map(str::to_string),
    }
}

/// The endpoint that a server answered its GetEndpoints call with.
fn endpoint() -> OpcUaClientEndpointInfo {
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

#[test]
fn mdns_services_find_mqtt_and_opcua_announcements() {
    // Specification 11 ("Sources"): the MQTT source finds brokers through mDNS
    // `_mqtt._tcp.local.`, and the OPC UA source finds servers through mDNS
    // `_opcua-tcp._tcp.local.` and their GetEndpoints.
    assert_eq!(
        mqtt_probe_target(&broker()),
        Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
            1883
        ))
    );

    assert_eq!(
        opcua_endpoint_url(&server(Some("/UA/Plant"))),
        Some("opc.tcp://10.0.0.6:4840/UA/Plant".to_string())
    );
    // A service that names no `path` TXT entry announces the server root, and so does one that
    // names it empty.
    for path in [None, Some("")] {
        assert_eq!(
            opcua_endpoint_url(&server(path)),
            Some("opc.tcp://10.0.0.6:4840/".to_string()),
            "path={path:?}"
        );
    }

    // Specification 11 ("Sources" and "Confidence"): a service is a device only when its probe
    // answers, with the confidence that exchange gives, and it is found through mDNS.
    let Announced::Found(finds) = announced_mqtt(&broker(), |target| {
        assert_eq!(
            target,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), 1883)
        );
        Some(DiscoveryProbeOutcome::confirmed(
            "mqtt_connack",
            "MQTT CONNECT was accepted with clean_session=true.",
        ))
    }) else {
        panic!("an announced broker that answered its probe is a device");
    };
    assert_eq!(finds.len(), 1);
    assert_eq!(finds[0].protocol, SourceId::Mqtt);
    assert_eq!(finds[0].source, "mdns");
    assert_eq!(finds[0].confidence, DiscoveryConfidence::Confirmed);
    assert_eq!(finds[0].params["broker"], json!("10.0.0.5:1883"));

    let Announced::Found(finds) = announced_opcua(&server(Some("/UA/Plant")), |url| {
        assert_eq!(url, "opc.tcp://10.0.0.6:4840/UA/Plant");
        Ok(vec![endpoint()])
    }) else {
        panic!("an announced server that answered GetEndpoints is a device");
    };
    assert_eq!(finds.len(), 1);
    assert_eq!(finds[0].protocol, SourceId::Opcua);
    assert_eq!(finds[0].source, "mdns");
    assert_eq!(finds[0].confidence, DiscoveryConfidence::Confirmed);
    assert_eq!(
        finds[0].params["endpoint_url"],
        json!("opc.tcp://10.0.0.6:4840/UA/Plant")
    );
    // The device is the address the service announced, so that the same server found by a
    // discovery server or by a known address is one device (specification 11, "Devices").
    assert_eq!(
        finds[0].keys,
        vec![DeviceKey::Address("10.0.0.6".to_string())]
    );
}

#[test]
fn an_announced_service_that_does_not_answer_is_a_warning_not_a_device() {
    // Specification 11 ("Sources"): a service that does not answer its probe is a warning that
    // names it, not a device.
    let Announced::Warning(warning) = announced_mqtt(&broker(), |_| None) else {
        panic!("a broker that did not answer its MQTT probe is not a device");
    };
    assert_eq!(
        warning,
        "The announced MQTT broker 10.0.0.5:1883 did not answer its MQTT probe; it is not a device."
    );

    let Announced::Warning(warning) = announced_opcua(&server(None), |_| {
        Err(RuntimeError::ControlError(
            "the server closed the connection".into(),
        ))
    }) else {
        panic!("a server whose GetEndpoints did not complete is not a device");
    };
    assert_eq!(
        warning,
        "OPC UA discovery did not complete: control error 'the server closed the connection'"
    );

    // A server that answers with no endpoint has announced nothing to connect to.
    let Announced::Warning(warning) = announced_opcua(&server(None), |_| Ok(Vec::new())) else {
        panic!("a server that advertised no endpoint is not a device");
    };
    assert_eq!(
        warning,
        "No OPC UA endpoints were advertised by opc.tcp://10.0.0.6:4840/."
    );
}
