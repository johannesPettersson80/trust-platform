//! Specification 11, "A runtime's trust in another runtime's certificate": a call to another runtime
//! trusts the CA bundle `[runtime.tls] ca_path` names, and that bundle alone, or the public roots
//! when no bundle is named. The peer is a real HTTPS server on 127.0.0.1 that presents the fixture
//! certificate, a self-signed end-entity certificate naming 127.0.0.1.

use std::time::Duration;

use super::*;

const PEER_CERTIFICATE: &[u8] = include_bytes!("../../tests/fixtures/tls/server-cert.pem");
const PEER_KEY: &[u8] = include_bytes!("../../tests/fixtures/tls/server-key.pem");
/// A CA that issued nothing the peer presents.
const ANOTHER_AUTHORITY: &[u8] = include_bytes!("../io/mqtt/tests/another-authority.pem");

/// A runtime whose `ca_path` names a bundle holding `ca_pem`.
fn materials(ca_pem: &[u8]) -> TlsMaterials {
    TlsMaterials {
        cert_path: "certs/runtime.crt".into(),
        key_path: "certs/runtime.key".into(),
        ca_path: Some("certs/ca.crt".into()),
        certificate_pem: PEER_CERTIFICATE.to_vec(),
        private_key_pem: PEER_KEY.to_vec(),
        ca_pem: ca_pem.to_vec(),
    }
}

/// A runtime with TLS and no `ca_path`, as `load_tls_materials` gives it: its own certificate
/// stands in its CA bundle.
fn without_a_bundle() -> TlsMaterials {
    TlsMaterials {
        ca_path: None,
        ..materials(PEER_CERTIFICATE)
    }
}

/// Starts a peer that answers one request with 200, and returns its URL.
fn peer() -> String {
    let server = tiny_http::Server::https(
        "127.0.0.1:0",
        tiny_http::SslConfig {
            certificate: PEER_CERTIFICATE.to_vec(),
            private_key: PEER_KEY.to_vec(),
        },
    )
    .expect("the peer listens");
    let port = server.server_addr().to_ip().expect("an IP address").port();
    std::thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_secs(5)) {
            let _ = request.respond(tiny_http::Response::from_string("{\"ok\":true}"));
        }
    });
    format!("https://127.0.0.1:{port}/api/control")
}

/// Calls `url` as the web layer's clients do, with `tls`.
fn call(tls: TlsConfig, url: &str) -> Result<u16, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build()
        .into();
    agent
        .get(url)
        .call()
        .map(|response| response.status().as_u16())
        .map_err(|error| error.to_string())
}

/// Catches the fault G280 found: a call that ignores the runtime's CA bundle and so refuses a peer
/// of the runtime's own site with `UnknownIssuer`.
#[test]
fn a_runtime_reaches_a_peer_whose_certificate_its_ca_bundle_holds() {
    let tls = peer_tls_config(Some(&materials(PEER_CERTIFICATE))).expect("the bundle loads");
    assert_eq!(call(tls, &peer()), Ok(200));
}

/// Catches a call that trusts any certificate, or that adds other roots to the bundle.
#[test]
fn a_runtime_refuses_a_peer_its_ca_bundle_did_not_issue() {
    let tls = peer_tls_config(Some(&materials(ANOTHER_AUTHORITY))).expect("the bundle loads");
    let refused = call(tls, &peer()).expect_err("the peer is not trusted");
    assert!(refused.contains("UnknownIssuer"), "{refused}");
}

/// Catches a runtime without a bundle that verifies nothing, or that trusts its own certificate
/// instead of the public roots, which did not issue the peer's (the peer presents the same
/// certificate the runtime holds).
#[test]
fn without_a_ca_bundle_a_runtime_verifies_a_peer_against_the_public_roots() {
    for materials in [None, Some(without_a_bundle())] {
        let tls = peer_tls_config(materials.as_ref()).expect("the public roots");
        let refused = call(tls, &peer()).expect_err("the peer is not trusted");
        assert!(refused.contains("UnknownIssuer"), "{refused}");
    }
}

/// Catches a bundle without a certificate that would leave the runtime trusting nothing, or the
/// public roots, instead of stopping its start.
#[test]
fn a_ca_bundle_without_a_certificate_stops_the_start() {
    for bundle in [&b""[..], b"plain text", PEER_KEY] {
        let refused = peer_tls_config(Some(&materials(bundle))).expect_err("no certificate");
        assert_eq!(
            refused.to_string(),
            "control error 'parse tls ca certificate: no certificates found'"
        );
    }
}
