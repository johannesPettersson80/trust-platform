//! The endpoint, the credentials and the transport security of the computer's driver
//! (specification 32 §3): the rules of the parameters, which the shared module decides, and what
//! the computer adds, reading the TLS files and building its TLS client with `native-tls`, then a
//! session over TLS with the broker stand-in.

use std::path::PathBuf;

use trust_runtime_services::io_drivers::mqtt::{BrokerEndpoint, ClientAuth, MqttTls};

use super::broker::{tls_acceptor, tls_fixture, Broker};
use super::{
    eventually, faulted_with, read, read_fresh, refusal, refused, start, table, MqttIoDriver,
};
use crate::error::RuntimeError;
use crate::io::IoDriver;

/// The path of a certificate of another authority, `CN=another-broker`, which did not sign the
/// broker stand-in's certificate, as a TOML string holds it on every system.
fn another_authority() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/io/mqtt/tests/another-authority.pem")
        .to_string_lossy()
        .replace('\\', "/")
}

/// A plaintext broker is taken only on the loopback or with `allow_insecure_remote = true`
/// (specification 32 §3). The shared rules' text replaces the computer's (`mqtt insecure remote
/// broker '10.10.0.9' requires allow_insecure_remote=true`). The allowance is checked by
/// validation, which the start runs first, so that the test connects to no remote host. Catches a
/// remote broker reached in plaintext without the allowance.
#[test]
fn security_test_rejects_remote_insecure_broker() {
    assert_eq!(
        refusal("broker = \"10.10.0.9:1883\""),
        refused(
            "allow_insecure_remote",
            "must be true for the remote broker \"10.10.0.9\" without TLS; only localhost, \
             127.0.0.1 and ::1 are local"
        )
    );
    assert_eq!(
        MqttIoDriver::validate_params(&table(
            "broker = \"10.10.0.9:1883\"\nallow_insecure_remote = true"
        )),
        Ok(())
    );
}

/// A remote broker with TLS and a readable authority is taken (specification 32 §3), checked by
/// validation, which reads the authority's file. Catches TLS refused for a remote broker.
#[test]
fn security_test_allows_remote_broker_when_tls_configured() {
    let text = format!(
        "broker = \"mqtt.example.test:8883\"\ntls = true\ntls_ca_path = \"{}\"",
        tls_fixture("server-cert.pem")
    );
    assert_eq!(MqttIoDriver::validate_params(&table(&text)), Ok(()));
}

/// The `mqtts://` scheme turns TLS on, and the broker's host and port are read without the
/// scheme (specification 32 §3). Catches a TLS scheme that connects in plaintext.
#[test]
fn security_test_mqtts_scheme_implies_tls() {
    let ca = tls_fixture("server-cert.pem");
    let params = read(&format!(
        "broker = \"mqtts://mqtt.example.test:8883\"\ntls_ca_path = \"{ca}\""
    ))
    .expect("an mqtts broker");
    assert_eq!(
        params.broker,
        BrokerEndpoint {
            host: "mqtt.example.test".into(),
            port: 8883,
        }
    );
    assert_eq!(
        params.tls,
        Some(MqttTls {
            ca_path: ca.into(),
            client_auth: None,
            alpn: None,
        })
    );
}

/// TLS without an authority to trust is refused, when the driver is validated and when it starts
/// (specification 32 §3: TLS requires a readable `tls_ca_path`). The shared rules' text replaces
/// the computer's (`mqtt tls=true requires tls_ca_path`). Catches TLS without trust roots.
#[test]
fn security_test_rejects_tls_without_ca_path() {
    assert_eq!(
        refusal("broker = \"mqtt.example.test:8883\"\ntls = true"),
        refused(
            "tls_ca_path",
            "is required with TLS, the path of the PEM file of the broker's certificate authority"
        )
    );
}

/// A TLS file named with TLS off is refused (specification 32 §3: TLS-only fields are rejected
/// when TLS is disabled). The shared rules' text replaces the computer's (`mqtt tls_* parameters
/// require io.params.tls=true`). Catches a TLS setting silently ignored, which would send in
/// plaintext what the operator meant to protect.
#[test]
fn security_test_rejects_tls_fields_when_tls_disabled() {
    assert_eq!(
        refusal(&format!(
            "broker = \"127.0.0.1:1883\"\ntls = false\ntls_ca_path = \"{}\"",
            tls_fixture("server-cert.pem")
        )),
        refused(
            "tls_ca_path",
            "needs TLS: set tls = true or use an mqtts:// or ssl:// broker"
        )
    );
}

/// An empty ALPN list has no effect with TLS off (specification 32 §3). Catches the schema's
/// default refused for a plaintext broker.
#[test]
fn security_test_allows_empty_alpn_list_when_tls_disabled() {
    assert_eq!(
        MqttIoDriver::validate_params(&table(
            "broker = \"127.0.0.1:1883\"\ntls = false\ntls_alpn = []"
        )),
        Ok(())
    );
}

/// The client's certificate and key are an all-or-nothing pair (specification 32 §3). The shared
/// rules' text replaces the computer's (`mqtt mTLS requires tls_client_cert_path and
/// tls_client_key_path together`). Catches half a mutual-TLS pair, which would connect without the
/// client's identity.
#[test]
fn security_test_rejects_partial_mtls_pair() {
    assert_eq!(
        refusal(&format!(
            "broker = \"mqtt.example.test:8883\"\ntls = true\ntls_ca_path = \"{}\"\n\
             tls_client_cert_path = \"{}\"",
            tls_fixture("server-cert.pem"),
            tls_fixture("server-cert.pem")
        )),
        refused(
            "tls_client_key_path",
            "is required with tls_client_cert_path: mutual TLS takes both"
        )
    );
}

/// The TLS settings read into the parameters, the keep alive with them, and the computer builds its
/// TLS client from the files they name: the authority, the client's certificate and key, and the
/// ALPN protocol `mqtt` (specification 32 §3). Catches a path or an ALPN protocol lost on the way
/// to the client, and a client that cannot be built from valid files.
#[test]
fn tls_transport_test_builds_the_native_tls_client() {
    let ca = tls_fixture("server-cert.pem");
    let key = tls_fixture("server-key.pem");
    let params = read(&format!(
        "broker = \"mqtt.example.test:8883\"\nclient_id = \"tls-test\"\nkeep_alive_s = 11\n\
         tls = true\ntls_ca_path = \"{ca}\"\ntls_client_cert_path = \"{ca}\"\n\
         tls_client_key_path = \"{key}\"\ntls_alpn = [\"mqtt\"]"
    ))
    .expect("TLS parameters");
    assert_eq!(params.keep_alive_s, 11);
    assert_eq!(
        params.tls,
        Some(MqttTls {
            ca_path: ca.as_str().into(),
            client_auth: Some(ClientAuth {
                cert_path: ca.as_str().into(),
                key_path: key.into(),
            }),
            alpn: Some(vec!["mqtt".into()]),
        })
    );
    assert!(matches!(super::super::tls::client(&params), Ok(Some(_))));
}

/// A TLS file that cannot be read is refused with its key and its path, in the computer's words,
/// when the driver is validated and when it starts, so that no worker starts with it
/// (specification 32 §2 and §3: TLS requires a readable `tls_ca_path`). The authority's file is
/// read first, then the client's certificate and key. Catches a missing file found only when the
/// broker is reached, and a refusal that does not say which file.
#[test]
fn a_tls_file_that_cannot_be_read_is_refused() {
    let ca = tls_fixture("server-cert.pem");
    let key = tls_fixture("server-key.pem");
    let missing = tls_fixture("missing.pem");
    for (key_name, ca, cert, key) in [
        ("tls_ca_path", &missing, &ca, &key),
        ("tls_client_cert_path", &ca, &missing, &key),
        ("tls_client_key_path", &ca, &ca, &missing),
    ] {
        let error = refusal(&format!(
            "broker = \"mqtts://127.0.0.1:8883\"\ntls_ca_path = \"{ca}\"\n\
             tls_client_cert_path = \"{cert}\"\ntls_client_key_path = \"{key}\""
        ))
        .expect_err("a missing file is refused");
        let expected = format!("mqtt {key_name} '{missing}' cannot be read: ");
        assert!(
            matches!(&error, RuntimeError::InvalidConfig(text) if text.starts_with(&expected)),
            "{key_name}: {error}"
        );
    }
}

/// A file that does not hold what its key names is refused when the driver is validated and when
/// it starts: a private key as the authority, and a certificate and a key given the other way
/// round (specification 32 §3), in the computer's words. Catches a TLS client built without its
/// authority, which would trust nothing or anything.
#[test]
fn a_tls_file_that_does_not_hold_its_certificate_is_refused() {
    let ca = tls_fixture("server-cert.pem");
    let key = tls_fixture("server-key.pem");
    let error = refusal(&format!(
        "broker = \"mqtts://127.0.0.1:8883\"\ntls_ca_path = \"{key}\""
    ))
    .expect_err("a key is no authority");
    assert!(
        matches!(&error, RuntimeError::InvalidConfig(text)
            if text.starts_with("mqtt tls_ca_path is not a valid PEM CA: ")),
        "{error}"
    );
    let error = refusal(&format!(
        "broker = \"mqtts://127.0.0.1:8883\"\ntls_ca_path = \"{ca}\"\n\
         tls_client_cert_path = \"{key}\"\ntls_client_key_path = \"{ca}\""
    ))
    .expect_err("a swapped pair is refused");
    assert!(
        matches!(&error, RuntimeError::InvalidConfig(text)
            if text.starts_with("mqtt mTLS client certificate/key are not valid PKCS#8 PEM: ")),
        "{error}"
    );
}

/// The driver talks MQTT to its broker over TLS: the handshake verifies the broker's certificate
/// for `127.0.0.1` with the authority of `tls_ca_path`, the CONNECT carries the keep alive, and a
/// message and an image go through (specification 32 §3). Catches a TLS client that does not carry
/// the session, or that is not used for an `mqtts://` broker.
#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "the fixture's self-signed certificate (CA:FALSE, ten years' validity) is an \
              authority that the TLS library of Linux (OpenSSL) takes, while the trust policies \
              of macOS and Windows may refuse it"
)]
fn the_driver_talks_mqtt_to_its_broker_over_tls() {
    let broker = Broker::start_tls(tls_acceptor());
    let mut driver = start(&format!(
        "broker = \"mqtts://127.0.0.1:{}\"\ntls_ca_path = \"{}\"\nkeep_alive_s = 11\n\
         topic_in = \"line/in\"\ntopic_out = \"line/out\"",
        broker.port(),
        tls_fixture("server-cert.pem")
    ));
    broker.wait_for("subscription over TLS", |record| {
        record.subscriptions == ["line/in"]
    });
    assert_eq!(broker.record(|record| record.connects[0].keep_alive_s), 11);
    broker.deliver(&[("line/in", &[4, 2])]);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [4, 2]);
    assert_eq!(driver.write_outputs(&[7]), Ok(()));
    broker.wait_for("image over TLS", |record| {
        record.published("line/out") == [vec![7]]
    });
}

/// The TLS client refuses a broker whose certificate its authority did not sign: the handshake
/// fails, the driver's health says so after `mqtt connect failed:`, and no CONNECT reaches the
/// broker (specification 32 §3). The client's side is what the test reads: Windows' TLS checks a
/// certificate authority of the client's own after the handshake, so its broker sees a handshake
/// that completed and a connection that ended, not a failed one. Catches a client that skips the
/// verification of the broker.
#[test]
fn the_tls_client_refuses_a_broker_it_cannot_verify() {
    let broker = Broker::start_tls(tls_acceptor());
    let driver = start(&format!(
        "broker = \"mqtts://127.0.0.1:{}\"\ntls_ca_path = \"{}\"\nreconnect_ms = 60000",
        broker.port(),
        another_authority()
    ));
    broker.wait_for("the client's connection", |record| record.accepted >= 1);
    eventually("failed handshake in the health", || {
        faulted_with(&driver.health(), "mqtt connect failed: TLS handshake")
    });
    assert!(broker.record(|record| record.connects.is_empty()));
}

/// A broker that does not speak TLS fails the handshake of a TLS driver, which reports it after
/// `mqtt connect failed:` and sends it no CONNECT in plaintext (specification 32 §3). Catches a
/// driver that falls back to plaintext.
#[test]
fn a_broker_without_tls_fails_the_handshake() {
    let broker = Broker::start();
    let driver = start(&format!(
        "broker = \"mqtts://127.0.0.1:{}\"\ntls_ca_path = \"{}\"\nreconnect_ms = 60000",
        broker.port(),
        tls_fixture("server-cert.pem")
    ));
    eventually("failed handshake in the health", || {
        faulted_with(&driver.health(), "mqtt connect failed: TLS handshake")
    });
    assert!(broker.record(|record| record.connects.is_empty()));
}
