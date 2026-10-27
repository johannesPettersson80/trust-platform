use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::opcua::{OpcUaClientEndpointInfo, OpcUaMessageSecurityMode, OpcUaSecurityPolicy};

use super::super::report::{CandidateSink, Deadline, DiscoverCandidate};
use super::*;
use crate::search::model::Identity;

fn scope() -> DiscoverScope {
    DiscoverScope::default()
}

#[test]
fn opcua_endpoint_requires_nonempty_host() {
    for host in [None, Some(""), Some(" \t ")] {
        let error = opcua_endpoint_url(&DiscoverScope {
            host: host.map(str::to_string),
            ..scope()
        })
        .expect_err("empty OPC UA host accepted");
        assert!(error.contains("scope.host"));
    }
}

#[test]
fn opcua_endpoint_normalizes_valid_authorities() {
    for (host, expected) in [
        ("server.internal", "opc.tcp://server.internal:4840"),
        ("server.internal:4841", "opc.tcp://server.internal:4841"),
        ("server.internal/trust", "opc.tcp://server.internal/trust"),
        (
            "opc.tcp://server.internal:4841/trust",
            "opc.tcp://server.internal:4841/trust",
        ),
        ("127.0.0.1", "opc.tcp://127.0.0.1:4840"),
        ("[::1]", "opc.tcp://[::1]:4840"),
        ("[::1]:4841", "opc.tcp://[::1]:4841"),
    ] {
        let endpoint = opcua_endpoint_url(&DiscoverScope {
            host: Some(format!(" {host} ")),
            ..scope()
        })
        .expect("OPC UA endpoint");
        assert_eq!(endpoint, expected, "{host}");
    }
}

#[test]
fn opcua_endpoint_rejects_malformed_or_unsupported_urls() {
    for host in [
        "http://server.internal:4840",
        "https://server.internal:4840",
        "user@server.internal:4840",
        "server.internal:0",
        "server.internal:notaport",
        "server.internal:4840?query",
        "server.internal:4840#fragment",
        "::1",
    ] {
        assert!(
            opcua_endpoint_url(&DiscoverScope {
                host: Some(host.to_string()),
                ..scope()
            })
            .is_err(),
            "invalid OPC UA endpoint accepted: {host}"
        );
    }
}

/// An `OpcUaClientEndpointInfo` that is either usable by truST or announces a policy it cannot use.
fn endpoint(unsupported: bool) -> OpcUaClientEndpointInfo {
    OpcUaClientEndpointInfo {
        endpoint_url: "opc.tcp://10.0.0.5:4840".to_string(),
        application_name: Some("Line PLC".to_string()),
        application_uri: Some("urn:acme:plc".to_string()),
        product_uri: Some("urn:acme:product".to_string()),
        security_policy: if unsupported {
            None
        } else {
            Some(OpcUaSecurityPolicy::Basic256Sha256)
        },
        security_policy_uri: if unsupported {
            "http://opcfoundation.org/UA/SecurityPolicy#BrainpoolP256r1".to_string()
        } else {
            "http://opcfoundation.org/UA/SecurityPolicy#Basic256Sha256".to_string()
        },
        security_mode: if unsupported {
            None
        } else {
            Some(OpcUaMessageSecurityMode::SignAndEncrypt)
        },
        anonymous_supported: true,
        username_supported: false,
        unsupported,
        certificate_sha256: Some("ab".repeat(32)),
    }
}

#[test]
fn opcua_endpoints_include_unsupported_ones_marked() {
    let supported = endpoint_candidate(&endpoint(false));
    assert_eq!(supported.params["unsupported"], json!(false));
    assert!(supported.warnings.is_empty());
    assert_eq!(supported.params["security_policy"], json!("basic256sha256"));
    assert_eq!(supported.params["security_mode"], json!("sign_and_encrypt"));
    assert_eq!(supported.confidence, "confirmed");

    let unsupported = endpoint_candidate(&endpoint(true));
    assert_eq!(unsupported.params["unsupported"], json!(true));
    assert!(
        !unsupported.warnings.is_empty(),
        "an endpoint truST cannot use carries a warning"
    );
    assert_eq!(unsupported.params["security_mode"], Value::Null);
    assert_eq!(
        unsupported.params["security_policy_uri"],
        json!("http://opcfoundation.org/UA/SecurityPolicy#BrainpoolP256r1")
    );
    // The raw policy URI keeps the unsupported endpoint distinct.
    assert!(
        unsupported.id.contains("BrainpoolP256r1"),
        "{}",
        unsupported.id
    );
}

#[test]
fn opcua_identity_has_uris_and_certificate_sha256() {
    let find = opcua_find(endpoint_candidate(&endpoint(false)), "10.0.0.5");
    let Identity::Opcua(identity) = &find.identity else {
        panic!("expected an OPC UA identity");
    };
    assert_eq!(identity.endpoint_url, "opc.tcp://10.0.0.5:4840");
    assert_eq!(identity.application_uri.as_deref(), Some("urn:acme:plc"));
    assert_eq!(identity.product_uri.as_deref(), Some("urn:acme:product"));
    assert_eq!(identity.certificate_sha256, Some("ab".repeat(32)));
    assert_eq!(identity.security_policy.as_deref(), Some("basic256sha256"));
    assert!(!identity.unsupported);
}

/// A sink that keeps what an OPC UA `discover` found and warned.
#[derive(Default)]
struct Collected {
    found: Vec<DiscoverCandidate>,
    warnings: Vec<String>,
}

impl CandidateSink for Collected {
    fn found(&mut self, candidate: DiscoverCandidate) {
        self.found.push(candidate);
    }

    fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

#[test]
fn opcua_get_endpoints_bounded_by_the_deadline() {
    // A server that accepts connections and never answers: GetEndpoints must end at the search
    // deadline rather than wait for the OPC UA client's own timeout.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind silent OPC UA port");
    let address = listener.local_addr().expect("silent address");
    let server = thread::spawn(move || {
        let mut open = Vec::new();
        while let Ok((stream, _)) = listener.accept() {
            open.push(stream);
        }
    });

    let scope = DiscoverScope {
        host: Some(address.to_string()),
        ..scope()
    };
    let deadline = Deadline::at(Instant::now() + Duration::from_millis(300));
    let mut sink = Collected::default();
    let started = Instant::now();
    discover(&scope, deadline, &mut sink).expect("discover returns");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "OPC UA GetEndpoints was not bounded by the deadline: {elapsed:?}"
    );
    assert!(
        sink.found.is_empty(),
        "a silent server advertised endpoints"
    );
    drop(server);
}

#[cfg(feature = "opcua-wire")]
#[test]
fn opcua_endpoint_info_marks_an_unsupported_policy_from_wire() {
    use ::opcua::types::{
        ApplicationDescription, ApplicationType, ByteString, EndpointDescription, LocalizedText,
        MessageSecurityMode, UAString,
    };
    let wire = EndpointDescription {
        endpoint_url: UAString::from("opc.tcp://10.0.0.5:4840"),
        server: ApplicationDescription {
            application_uri: UAString::from("urn:acme:plc"),
            product_uri: UAString::from("urn:acme:product"),
            application_name: LocalizedText::new("en", "Line PLC"),
            application_type: ApplicationType::Server,
            gateway_server_uri: UAString::null(),
            discovery_profile_uri: UAString::null(),
            discovery_urls: None,
        },
        server_certificate: ByteString::from(vec![0x01, 0x02, 0x03]),
        security_mode: MessageSecurityMode::SignAndEncrypt,
        security_policy_uri: UAString::from(
            "http://opcfoundation.org/UA/SecurityPolicy#BrainpoolP256r1",
        ),
        user_identity_tokens: None,
        transport_profile_uri: UAString::null(),
        security_level: 0,
    };
    let info = crate::opcua::endpoint_info_from_wire(wire);
    assert!(info.unsupported);
    assert_eq!(info.security_policy, None);
    // Spec 11 (the OPC UA identity bullet): every endpoint is reported with its security policy
    // and mode, including the ones truST cannot use, so the mode survives the unknown policy.
    assert_eq!(
        info.security_mode,
        Some(OpcUaMessageSecurityMode::SignAndEncrypt)
    );
    assert_eq!(info.application_uri.as_deref(), Some("urn:acme:plc"));
    assert_eq!(info.product_uri.as_deref(), Some("urn:acme:product"));
    assert_eq!(
        info.certificate_sha256.as_deref(),
        Some("039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81")
    );
}
