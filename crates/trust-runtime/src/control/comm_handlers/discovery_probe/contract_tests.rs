use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::*;

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

fn modbus_exchange(
    unit_id: u8,
    pdu: Vec<u8>,
    transaction_id: u16,
    response: Vec<u8>,
) -> (Result<ModbusProbeResponse, std::io::Error>, Vec<u8>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Modbus server");
    let address = listener.local_addr().expect("Modbus address");
    let expected_request_len = 7 + pdu.len();
    let (tx, rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept Modbus client");
        let mut request = Vec::with_capacity(expected_request_len);
        while request.len() < expected_request_len {
            let mut chunk = [0u8; 256];
            let wanted = (expected_request_len - request.len()).min(chunk.len());
            match stream.read(&mut chunk[..wanted]) {
                Ok(0) => break,
                Ok(read) => request.extend_from_slice(&chunk[..read]),
                Err(error) => panic!("read Modbus request: {error}"),
            }
        }
        let complete = request.len() == expected_request_len;
        tx.send(request).expect("send Modbus request");
        if complete {
            stream.write_all(&response).expect("write Modbus response");
        }
    });
    let mut stream = TcpStream::connect(address).expect("connect Modbus server");
    let result = send_modbus_request(&mut stream, unit_id, &pdu, transaction_id);
    if result
        .as_ref()
        .is_err_and(|error| error.kind() == std::io::ErrorKind::InvalidInput)
    {
        let _ = stream.shutdown(Shutdown::Write);
    }
    let request = rx.recv().expect("receive Modbus request");
    server.join().expect("join Modbus server");
    (result, request)
}

fn modbus_response(transaction_id: u16, protocol_id: u16, unit_id: u8, pdu: &[u8]) -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(&transaction_id.to_be_bytes());
    response.extend_from_slice(&protocol_id.to_be_bytes());
    response.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
    response.push(unit_id);
    response.extend_from_slice(pdu);
    response
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

fn modbus_error(result: Result<ModbusProbeResponse, std::io::Error>) -> std::io::Error {
    match result {
        Ok(_) => panic!("malformed Modbus exchange was accepted"),
        Err(error) => error,
    }
}

#[test]
fn outcome_constructors_use_closed_confidence_and_default_flags() {
    let confirmed = DiscoveryProbeOutcome::confirmed("wire", "confirmed");
    assert_eq!(confirmed.confidence, CONFIRMED);
    assert_eq!(confirmed.source, "wire");
    assert_eq!(confirmed.detail, "confirmed");
    assert!(confirmed.warnings.is_empty());
    assert!(!confirmed.auth_required);

    let likely = DiscoveryProbeOutcome::likely("wire", "likely");
    assert_eq!(likely.confidence, LIKELY);
    assert!(!likely.auth_required);

    let reachable = DiscoveryProbeOutcome::port_reachable("tcp", "reachable");
    assert_eq!(reachable.confidence, PORT_REACHABLE);
    assert!(!reachable.auth_required);
}

#[test]
fn outcome_modifiers_preserve_warning_order_and_auth_flag() {
    let outcome = DiscoveryProbeOutcome::likely("mqtt", "rejected")
        .with_warning("first")
        .with_warning("second")
        .with_auth_required();
    assert_eq!(outcome.warnings, vec!["first", "second"]);
    assert!(outcome.auth_required);
    assert_eq!(outcome.confidence, LIKELY);
}

#[test]
fn modbus_device_id_request_has_exact_mbap_and_pdu() {
    let response = modbus_response(1, 0, 17, &[0x2b, 0x0e]);
    let (result, request) = modbus_exchange(17, vec![0x2b, 0x0e, 0x01, 0x00], 1, response);
    assert!(matches!(result, Ok(ModbusProbeResponse::Normal)));
    assert_eq!(
        request,
        vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x11, 0x2b, 0x0e, 0x01, 0x00]
    );
}

#[test]
fn modbus_safe_read_request_has_exact_transaction_unit_address_and_quantity() {
    let response = modbus_response(2, 0, 255, &[0x03, 0x02, 0x00, 0x00]);
    let (result, request) = modbus_exchange(255, vec![0x03, 0x12, 0x34, 0x00, 0x7d], 2, response);
    assert!(matches!(result, Ok(ModbusProbeResponse::Normal)));
    assert_eq!(
        request,
        vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0xff, 0x03, 0x12, 0x34, 0x00, 0x7d]
    );
}

#[test]
fn modbus_normal_and_explicit_exception_responses_are_distinct() {
    let normal = modbus_exchange(
        1,
        vec![0x03, 0, 0, 0, 1],
        7,
        modbus_response(7, 0, 1, &[0x03, 0x02, 0, 0]),
    )
    .0;
    assert!(matches!(normal, Ok(ModbusProbeResponse::Normal)));

    let exception = modbus_exchange(
        1,
        vec![0x03, 0, 0, 0, 1],
        8,
        modbus_response(8, 0, 1, &[0x83, 0x02]),
    )
    .0;
    assert!(matches!(exception, Ok(ModbusProbeResponse::Exception(2))));
}

#[test]
fn modbus_response_must_match_transaction_id() {
    let result = modbus_exchange(
        1,
        vec![0x03, 0, 0, 0, 1],
        7,
        modbus_response(8, 0, 1, &[0x03, 0x02, 0, 0]),
    )
    .0;
    assert_eq!(modbus_error(result).kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn modbus_response_must_use_protocol_id_zero() {
    let result = modbus_exchange(
        1,
        vec![0x03, 0, 0, 0, 1],
        7,
        modbus_response(7, 1, 1, &[0x03, 0x02, 0, 0]),
    )
    .0;
    assert_eq!(modbus_error(result).kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn modbus_response_must_match_requested_unit_id() {
    let result = modbus_exchange(
        17,
        vec![0x03, 0, 0, 0, 1],
        7,
        modbus_response(7, 0, 18, &[0x03, 0x02, 0, 0]),
    )
    .0;
    assert_eq!(modbus_error(result).kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn modbus_response_length_has_closed_two_through_260_domain() {
    for length in [0u16, 1, 261, u16::MAX] {
        let mut response = Vec::new();
        response.extend_from_slice(&7u16.to_be_bytes());
        response.extend_from_slice(&0u16.to_be_bytes());
        response.extend_from_slice(&length.to_be_bytes());
        let result = modbus_exchange(1, vec![0x03], 7, response).0;
        assert_eq!(
            modbus_error(result).kind(),
            std::io::ErrorKind::InvalidData,
            "length {length}"
        );
    }
}

#[test]
fn modbus_partial_response_body_is_rejected() {
    let mut response = Vec::new();
    response.extend_from_slice(&7u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&5u16.to_be_bytes());
    response.extend_from_slice(&[1, 0x03]);
    let result = modbus_exchange(1, vec![0x03], 7, response).0;
    assert_eq!(
        modbus_error(result).kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn modbus_unexpected_function_is_rejected() {
    let result = modbus_exchange(
        1,
        vec![0x03, 0, 0, 0, 1],
        7,
        modbus_response(7, 0, 1, &[0x04, 0x02, 0, 0]),
    )
    .0;
    assert_eq!(modbus_error(result).kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn modbus_exception_requires_explicit_exception_code() {
    let result = modbus_exchange(
        1,
        vec![0x03, 0, 0, 0, 1],
        7,
        modbus_response(7, 0, 1, &[0x83]),
    )
    .0;
    assert_eq!(
        modbus_error(result).kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn modbus_empty_request_pdu_returns_error_without_panicking() {
    let response = modbus_response(7, 0, 1, &[0x03]);
    let observed = catch_unwind(AssertUnwindSafe(|| {
        modbus_exchange(1, Vec::new(), 7, response).0
    }));
    assert!(observed.is_ok(), "empty PDU panicked");
    assert_eq!(
        modbus_error(observed.expect("no panic")).kind(),
        std::io::ErrorKind::InvalidInput
    );
}

#[test]
fn modbus_request_pdu_above_253_bytes_is_rejected_before_use() {
    let pdu = vec![0x03; 254];
    let response = modbus_response(7, 0, 1, &[0x03]);
    let result = modbus_exchange(1, pdu, 7, response).0;
    assert_eq!(
        modbus_error(result).kind(),
        std::io::ErrorKind::InvalidInput
    );
}

#[test]
fn unreachable_modbus_and_mqtt_targets_return_no_observation() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused socket");
    let address = listener.local_addr().expect("unused address");
    drop(listener);
    assert!(probe_modbus_tcp(
        address,
        Duration::from_millis(25),
        ModbusDiscoveryProbe {
            unit_id: 1,
            safe_read: None
        }
    )
    .is_none());
    assert!(probe_mqtt(address, Duration::from_millis(25)).is_none());
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
fn mqtts_port_reports_only_tcp_reachability_and_writes_nothing() {
    let listener = TcpListener::bind("127.0.0.1:8883").expect("bind MQTTS test port");
    let address = listener.local_addr().expect("MQTTS address");
    let (tx, rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept MQTTS client");
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("set read timeout");
        let mut byte = [0];
        tx.send(stream.read(&mut byte)).expect("send read outcome");
    });
    let outcome = probe_mqtt(address, Duration::from_millis(250)).expect("MQTTS outcome");
    assert_eq!(outcome.confidence, PORT_REACHABLE);
    assert_eq!(outcome.source, "tcp_connect");
    assert!(!outcome.warnings.is_empty());
    let read = rx.recv().expect("read outcome");
    match read {
        Ok(0) | Err(_) => {}
        Ok(count) => panic!("MQTTS discovery wrote {count} unexpected plaintext bytes"),
    }
    server.join().expect("join MQTTS server");
}
