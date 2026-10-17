use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::super::probe::{CONFIRMED, LIKELY, PORT_REACHABLE};
use super::super::scope::resolve_authority_all;
use super::*;

fn scope() -> DiscoverScope {
    DiscoverScope::default()
}

#[test]
fn mqtt_missing_host_returns_none_and_one_stable_warning() {
    for host in [None, Some(""), Some(" \t ")] {
        let mut warnings = Vec::new();
        let targets = mqtt_targets(
            &DiscoverScope {
                host: host.map(str::to_string),
                ..scope()
            },
            &mut warnings,
        )
        .expect("missing MQTT host");
        assert_eq!(targets, None);
        assert_eq!(
            warnings,
            vec![
                "MQTT discovery needs a broker host; broad network scans are not run for MQTT."
                    .to_string()
            ]
        );
    }
}

#[test]
fn mqtt_explicit_socket_produces_one_target() {
    for (host, expected) in [
        (
            "127.0.0.1:1884",
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1884),
        ),
        (
            "[::1]:8883",
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 8883),
        ),
    ] {
        let mut warnings = Vec::new();
        let targets = mqtt_targets(
            &DiscoverScope {
                host: Some(host.to_string()),
                ..scope()
            },
            &mut warnings,
        )
        .expect("MQTT target")
        .expect("directed targets");
        assert_eq!(targets, vec![expected]);
        assert!(warnings.is_empty());
    }
}

#[test]
fn mqtt_omitted_port_probes_cleartext_then_tls_without_duplicates() {
    let mut warnings = Vec::new();
    let targets = mqtt_targets(
        &DiscoverScope {
            host: Some("127.0.0.1".to_string()),
            ..scope()
        },
        &mut warnings,
    )
    .expect("MQTT targets")
    .expect("directed targets");
    assert_eq!(
        targets,
        vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MQTT_PORT),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MQTTS_PORT),
        ]
    );
    let unique: std::collections::BTreeSet<_> = targets.iter().copied().collect();
    assert_eq!(unique.len(), targets.len());
}

#[test]
fn mqtt_target_rejects_malformed_authorities_before_probe() {
    for host in [
        "127.0.0.1:0",
        "mqtt://127.0.0.1",
        "mqtts://127.0.0.1",
        "user@127.0.0.1:1883",
        "127.0.0.1:1883/path",
        "127.0.0.1:1883?query",
        "127.0.0.1:1883#fragment",
        "127.0.0.1:notaport",
        "::1",
    ] {
        let mut warnings = Vec::new();
        assert!(
            mqtt_targets(
                &DiscoverScope {
                    host: Some(host.to_string()),
                    ..scope()
                },
                &mut warnings
            )
            .is_err(),
            "invalid MQTT authority accepted: {host}"
        );
    }
}

#[test]
fn mqtt_probe_uses_clean_session_and_disconnects_after_connack() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut fixed = [0u8; 2];
        stream.read_exact(&mut fixed).expect("connect fixed header");
        assert_eq!(fixed[0], 0x10);
        let mut body = vec![0u8; fixed[1] as usize];
        stream.read_exact(&mut body).expect("connect body");
        assert_eq!(&body[0..6], &[0x00, 0x04, b'M', b'Q', b'T', b'T']);
        assert_eq!(body[6], 0x04);
        assert_eq!(body[7], 0x02, "clean_session must be set");
        stream
            .write_all(&[0x20, 0x02, 0x00, 0x00])
            .expect("connack");
        let mut disconnect = [0u8; 2];
        stream.read_exact(&mut disconnect).expect("disconnect");
        tx.send(disconnect).expect("send disconnect");
    });

    let outcome = probe_mqtt(addr, Duration::from_millis(250)).expect("outcome");

    assert_eq!(outcome.confidence, CONFIRMED);
    assert_eq!(outcome.source, "mqtt_connack");
    assert_eq!(rx.recv().expect("disconnect"), [0xe0, 0x00]);
    handle.join().expect("join");
}

#[test]
fn mqtt_probe_classifies_auth_rejected_connack_separately() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut fixed = [0u8; 2];
        stream.read_exact(&mut fixed).expect("connect fixed header");
        let mut body = vec![0u8; fixed[1] as usize];
        stream.read_exact(&mut body).expect("connect body");
        stream
            .write_all(&[0x20, 0x02, 0x00, 0x05])
            .expect("auth rejected connack");
        let mut disconnect = [0u8; 2];
        stream.read_exact(&mut disconnect).expect("disconnect");
        assert_eq!(disconnect, [0xe0, 0x00]);
    });

    let outcome = probe_mqtt(addr, Duration::from_millis(250)).expect("outcome");

    assert_eq!(outcome.confidence, LIKELY);
    assert_eq!(outcome.source, "mqtt_connack_auth_rejected");
    assert!(outcome.auth_required);
    assert!(outcome
        .warnings
        .iter()
        .any(|warning| warning.contains("requires credentials")));
    handle.join().expect("join");
}

fn stream_with_server_bytes(bytes: Vec<u8>) -> (TcpStream, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind byte server");
    let address = listener.local_addr().expect("byte server address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept byte client");
        stream.write_all(&bytes).expect("write scripted bytes");
    });
    let stream = TcpStream::connect(address).expect("connect byte server");
    (stream, handle)
}

fn capture_client_write(
    write: impl FnOnce(&mut TcpStream) -> std::io::Result<()> + Send + 'static,
) -> (std::io::Result<()>, Vec<u8>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture address");
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept capture client");
        let mut bytes = Vec::new();
        stream
            .read_to_end(&mut bytes)
            .expect("capture client bytes");
        tx.send(bytes).expect("send captured bytes");
    });
    let writer = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).expect("connect capture server");
        let result = write(&mut stream);
        let _ = stream.shutdown(Shutdown::Write);
        result
    });
    let result = writer.join().expect("join writer");
    let bytes = rx.recv().expect("receive captured bytes");
    reader.join().expect("join capture reader");
    (result, bytes)
}

fn connack(bytes: &[u8]) -> Result<u8, std::io::Error> {
    let (mut stream, handle) = stream_with_server_bytes(bytes.to_vec());
    let result = read_mqtt_connack(&mut stream);
    handle.join().expect("join CONNACK server");
    result
}

fn remaining_length(bytes: &[u8]) -> Result<usize, std::io::Error> {
    let (mut stream, handle) = stream_with_server_bytes(bytes.to_vec());
    let result = read_remaining_length(&mut stream);
    handle.join().expect("join remaining-length server");
    result
}

#[test]
fn mqtt_remaining_length_encoder_uses_canonical_boundaries() {
    for (length, expected) in [
        (0, vec![0x00]),
        (1, vec![0x01]),
        (127, vec![0x7f]),
        (128, vec![0x80, 0x01]),
        (16_383, vec![0xff, 0x7f]),
        (16_384, vec![0x80, 0x80, 0x01]),
        (2_097_151, vec![0xff, 0xff, 0x7f]),
        (2_097_152, vec![0x80, 0x80, 0x80, 0x01]),
        (268_435_455, vec![0xff, 0xff, 0xff, 0x7f]),
    ] {
        let mut encoded = Vec::new();
        encode_remaining_length(length, &mut encoded);
        assert_eq!(encoded, expected, "remaining length {length}");
    }
}

#[test]
fn mqtt_remaining_length_encoder_rejects_values_above_four_byte_maximum() {
    for length in [268_435_456, usize::MAX] {
        let mut encoded = Vec::new();
        encode_remaining_length(length, &mut encoded);
        assert!(
            encoded.is_empty(),
            "out-of-range length {length} emitted {encoded:?}"
        );
    }
}

#[test]
fn mqtt_remaining_length_decoder_accepts_canonical_boundaries() {
    for (encoded, expected) in [
        (vec![0x00], 0),
        (vec![0x7f], 127),
        (vec![0x80, 0x01], 128),
        (vec![0xff, 0x7f], 16_383),
        (vec![0x80, 0x80, 0x01], 16_384),
        (vec![0xff, 0xff, 0x7f], 2_097_151),
        (vec![0x80, 0x80, 0x80, 0x01], 2_097_152),
        (vec![0xff, 0xff, 0xff, 0x7f], 268_435_455),
    ] {
        assert_eq!(
            remaining_length(&encoded).expect("canonical remaining length"),
            expected
        );
    }
}

#[test]
fn mqtt_remaining_length_decoder_rejects_truncated_and_five_byte_forms() {
    for encoded in [
        vec![],
        vec![0x80],
        vec![0x80, 0x80],
        vec![0x80, 0x80, 0x80],
        vec![0x80, 0x80, 0x80, 0x80],
        vec![0xff, 0xff, 0xff, 0xff, 0x01],
    ] {
        assert!(
            remaining_length(&encoded).is_err(),
            "invalid remaining length accepted: {encoded:?}"
        );
    }
}

#[test]
fn mqtt_remaining_length_decoder_rejects_nonminimal_encodings() {
    for encoded in [
        vec![0x80, 0x00],
        vec![0x81, 0x00],
        vec![0x80, 0x80, 0x00],
        vec![0x80, 0x80, 0x80, 0x00],
    ] {
        assert!(
            remaining_length(&encoded).is_err(),
            "nonminimal remaining length accepted: {encoded:?}"
        );
    }
}

#[test]
fn mqtt_connect_packet_has_exact_protocol_flags_keepalive_and_client_id() {
    let (result, packet) =
        capture_client_write(|stream| write_mqtt_connect(stream, b"trust-discovery-contract"));
    result.expect("write MQTT CONNECT");
    assert_eq!(packet[0], 0x10);
    assert_eq!(packet[1] as usize, packet.len() - 2);
    assert_eq!(&packet[2..8], &[0x00, 0x04, b'M', b'Q', b'T', b'T']);
    assert_eq!(packet[8], 0x04);
    assert_eq!(packet[9], 0x02);
    assert_eq!(&packet[10..12], &[0x00, 0x00]);
    assert_eq!(
        u16::from_be_bytes([packet[12], packet[13]]) as usize,
        b"trust-discovery-contract".len()
    );
    assert_eq!(&packet[14..], b"trust-discovery-contract");
}

#[test]
fn mqtt_connect_rejects_client_id_larger_than_u16_before_writing() {
    let oversized = vec![b'x'; usize::from(u16::MAX) + 1];
    let (result, packet) =
        capture_client_write(move |stream| write_mqtt_connect(stream, &oversized));
    assert_eq!(
        result.expect_err("oversized client ID accepted").kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert!(packet.is_empty());
}

#[test]
fn mqtt_connack_accepts_exact_valid_return_code_domain() {
    for code in 0..=5 {
        assert_eq!(
            connack(&[0x20, 0x02, 0x00, code]).expect("valid CONNACK"),
            code
        );
    }
}

#[test]
fn mqtt_connack_rejects_wrong_packet_type() {
    for packet_type in [0x00, 0x10, 0x21, 0x30, 0xff] {
        assert_eq!(
            connack(&[packet_type, 0x02, 0x00, 0x00])
                .expect_err("wrong packet type accepted")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}

#[test]
fn mqtt_connack_requires_exact_remaining_length_two() {
    for bytes in [
        vec![0x20, 0x00],
        vec![0x20, 0x01, 0x00],
        vec![0x20, 0x03, 0x00, 0x00, 0x00],
        vec![0x20, 0x04, 0x00, 0x00, 0x00, 0x00],
    ] {
        assert!(
            connack(&bytes).is_err(),
            "invalid CONNACK length accepted: {bytes:?}"
        );
    }
}

#[test]
fn mqtt_connack_rejects_truncated_body() {
    for bytes in [vec![0x20], vec![0x20, 0x02], vec![0x20, 0x02, 0x00]] {
        assert!(connack(&bytes).is_err(), "truncated CONNACK accepted");
    }
}

#[test]
fn mqtt_connack_rejects_reserved_ack_flags() {
    for flags in [0x02, 0x7f, 0xff] {
        assert!(
            connack(&[0x20, 0x02, flags, 0x00]).is_err(),
            "reserved CONNACK flags accepted: {flags}"
        );
    }
}

#[test]
fn mqtt_connack_rejects_unknown_return_codes() {
    for code in [6, 7, 0x7f, 0xff] {
        assert!(
            connack(&[0x20, 0x02, 0x00, code]).is_err(),
            "unknown CONNACK code accepted: {code}"
        );
    }
}

#[test]
fn mqtt_connack_rejects_session_present_on_failed_connection() {
    for code in 1..=5 {
        assert!(
            connack(&[0x20, 0x02, 0x01, code]).is_err(),
            "session-present accepted with failure code {code}"
        );
    }
    assert_eq!(
        connack(&[0x20, 0x02, 0x01, 0x00]).expect("resumed accepted session"),
        0
    );
}

#[test]
fn mqtt_probe_classifies_all_valid_rejection_codes_without_confirmation() {
    for code in 1u8..=5 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind MQTT server");
        let address = listener.local_addr().expect("MQTT address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept MQTT client");
            let mut fixed = [0; 2];
            stream.read_exact(&mut fixed).expect("read MQTT header");
            let mut body = vec![0; usize::from(fixed[1])];
            stream.read_exact(&mut body).expect("read MQTT body");
            stream
                .write_all(&[0x20, 0x02, 0x00, code])
                .expect("write CONNACK");
            let mut disconnect = [0; 2];
            stream.read_exact(&mut disconnect).expect("read DISCONNECT");
            assert_eq!(disconnect, [0xe0, 0x00]);
        });
        let outcome = probe_mqtt(address, Duration::from_millis(250)).expect("MQTT outcome");
        assert_eq!(outcome.confidence, LIKELY);
        assert_eq!(outcome.auth_required, matches!(code, 4 | 5));
        assert_eq!(
            outcome.source,
            if matches!(code, 4 | 5) {
                "mqtt_connack_auth_rejected"
            } else {
                "mqtt_connack_rejected"
            }
        );
        server.join().expect("join MQTT server");
    }
}

#[test]
fn mqtt_probes_every_resolved_address() {
    // The old fault probed only the first resolved address; `localhost` and a literal address
    // must expand to every resolved address, each at 1883 and then 8883.
    for host in ["localhost", "127.0.0.1"] {
        let mut warnings = Vec::new();
        let targets = mqtt_targets(
            &DiscoverScope {
                host: Some(host.to_string()),
                ..scope()
            },
            &mut warnings,
        )
        .expect("MQTT targets")
        .expect("directed targets");
        assert!(warnings.is_empty());

        let resolved = resolve_authority_all(host, MQTT_PORT, "MQTT").expect("resolve MQTT host");
        let expected: Vec<SocketAddr> = resolved
            .iter()
            .flat_map(|address| {
                [
                    SocketAddr::new(address.ip(), MQTT_PORT),
                    SocketAddr::new(address.ip(), MQTTS_PORT),
                ]
            })
            .collect();
        assert_eq!(targets, expected, "{host}");
    }
}

#[test]
fn mqtts_tls_handshake_result() {
    let cert = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/tls/server-cert.pem"
    ))
    .expect("read TLS test certificate");
    let key = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/tls/server-key.pem"
    ))
    .expect("read TLS test key");
    let identity = native_tls::Identity::from_pkcs8(&cert, &key).expect("build TLS identity");
    let acceptor = native_tls::TlsAcceptor::new(identity).expect("build TLS acceptor");

    // A completed TLS handshake on 8883 proves reachability, never an MQTT broker.
    let listener = TcpListener::bind("127.0.0.1:8883").expect("bind MQTTS test port");
    let address = listener.local_addr().expect("MQTTS address");
    let (tx, rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept MQTTS client");
        tx.send(acceptor.accept(stream).is_ok())
            .expect("send TLS outcome");
    });
    let outcome = probe_mqtt(address, Duration::from_millis(1_000)).expect("MQTTS outcome");
    assert_eq!(outcome.confidence, PORT_REACHABLE);
    assert_eq!(outcome.source, "tls_handshake");
    assert!(outcome
        .warnings
        .iter()
        .any(|warning| warning.contains("TLS handshake succeeded")));
    assert!(
        rx.recv_timeout(Duration::from_secs(5))
            .expect("TLS outcome"),
        "the server TLS handshake failed"
    );
    server.join().expect("join MQTTS server");

    // A plaintext listener on 8883 fails the handshake: only the port is proven.
    let listener = TcpListener::bind("127.0.0.1:8883").expect("rebind MQTTS test port");
    let address = listener.local_addr().expect("MQTTS address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept MQTTS client");
        let mut byte = [0u8; 1];
        let _ = stream.read(&mut byte);
    });
    let outcome = probe_mqtt(address, Duration::from_millis(1_000)).expect("MQTTS outcome");
    assert_eq!(outcome.confidence, PORT_REACHABLE);
    assert_eq!(outcome.source, "tcp_connect");
    assert!(outcome
        .warnings
        .iter()
        .any(|warning| warning.contains("TLS handshake failed")));
    server.join().expect("join plaintext server");
}
