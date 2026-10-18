use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::search::{
    Identity, SearchHost, SearchRegistry, SearchRequest, SearchState, SearchStatus,
};

use super::super::probe::CONFIRMED;
use super::super::report::{CandidateSink, Deadline, DiscoverCandidate};
use super::*;

fn scope() -> DiscoverScope {
    DiscoverScope::default()
}

#[test]
fn modbus_safe_read_defaults_and_bounds_are_exact() {
    let default = modbus_discovery_probe(&scope()).expect("default probe");
    assert_eq!(default.unit_id, 1);
    assert_eq!(default.safe_read, None);

    let with_address = modbus_discovery_probe(&DiscoverScope {
        unit_id: Some(255),
        probe_read_address: Some(400),
        ..scope()
    })
    .expect("safe read defaults");
    assert_eq!(with_address.unit_id, 255);
    assert_eq!(
        with_address.safe_read,
        Some(ModbusSafeReadProbe {
            address: 400,
            quantity: 1
        })
    );

    for quantity in [1, 125] {
        let probe = modbus_discovery_probe(&DiscoverScope {
            probe_read_address: Some(u16::MAX),
            probe_read_quantity: Some(quantity),
            ..scope()
        })
        .expect("boundary quantity");
        assert_eq!(probe.safe_read.expect("safe read").quantity, quantity);
    }
}

#[test]
fn modbus_safe_read_rejects_orphan_or_out_of_range_quantity() {
    let orphan = modbus_discovery_probe(&DiscoverScope {
        probe_read_quantity: Some(1),
        ..scope()
    })
    .expect_err("quantity without address accepted");
    assert!(orphan.contains("probe_read_address is required"));

    for quantity in [0, 126, u16::MAX] {
        let error = modbus_discovery_probe(&DiscoverScope {
            probe_read_address: Some(0),
            probe_read_quantity: Some(quantity),
            ..scope()
        })
        .expect_err("invalid quantity accepted");
        assert!(error.contains("between 1 and 125"), "{error}");
    }
}

#[test]
fn modbus_socket_target_defaults_port_and_preserves_explicit_socket() {
    assert_eq!(
        modbus_socket_target("127.0.0.1").expect("default port"),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), MODBUS_PORT)
    );
    assert_eq!(
        modbus_socket_target("127.0.0.1:1502").expect("explicit port"),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1502)
    );
    assert_eq!(
        modbus_socket_target("[::1]:1502").expect("IPv6 socket"),
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1502)
    );
}

#[test]
fn socket_targets_reject_zero_invalid_and_non_authority_ports() {
    for host in [
        "127.0.0.1:0",
        "127.0.0.1:notaport",
        "http://127.0.0.1:502",
        "user@127.0.0.1:502",
        "127.0.0.1:502/path",
        "127.0.0.1:502?query",
        "127.0.0.1:502#fragment",
        "::1",
    ] {
        assert!(
            modbus_socket_target(host).is_err(),
            "invalid Modbus socket target accepted: {host}"
        );
    }
}

#[test]
fn modbus_device_id_probe_confirms_protocol_response() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = [0u8; 11];
        stream.read_exact(&mut request).expect("read request");
        assert_eq!(&request[2..4], &[0, 0]);
        assert_eq!(request[6], 1);
        assert_eq!(&request[7..11], &[0x2b, 0x0e, 0x01, 0x00]);
        let response = [
            0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x01, 0x2b, 0x0e, 0x01, 0x01, 0x00, 0x00, 0x00,
        ];
        stream.write_all(&response).expect("write response");
    });

    let outcome = probe_modbus_tcp(
        addr,
        Duration::from_millis(250),
        ModbusDiscoveryProbe {
            unit_id: 1,
            safe_read: None,
        },
    )
    .expect("outcome");

    assert_eq!(outcome.outcome.confidence, CONFIRMED);
    assert_eq!(outcome.outcome.source, "modbus_device_id");
    handle.join().expect("join");
}

#[test]
fn modbus_safe_read_probe_confirms_when_device_id_is_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut first, _) = listener.accept().expect("accept first");
        let mut device_id_request = [0u8; 11];
        first
            .read_exact(&mut device_id_request)
            .expect("read device id request");
        assert_eq!(&device_id_request[7..11], &[0x2b, 0x0e, 0x01, 0x00]);
        drop(first);

        let (mut second, _) = listener.accept().expect("accept second");
        let mut read_request = [0u8; 12];
        second
            .read_exact(&mut read_request)
            .expect("read safe read request");
        assert_eq!(&read_request[7..12], &[0x03, 0x01, 0x90, 0x00, 0x01]);
        let response = [
            0x00, 0x02, 0x00, 0x00, 0x00, 0x05, 0x11, 0x03, 0x02, 0x00, 0x00,
        ];
        second.write_all(&response).expect("write response");
    });

    let outcome = probe_modbus_tcp(
        addr,
        Duration::from_millis(250),
        ModbusDiscoveryProbe {
            unit_id: 17,
            safe_read: Some(ModbusSafeReadProbe {
                address: 400,
                quantity: 1,
            }),
        },
    )
    .expect("outcome");

    assert_eq!(outcome.outcome.confidence, CONFIRMED);
    assert_eq!(outcome.outcome.source, "modbus_safe_read");
    handle.join().expect("join");
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

fn modbus_error(result: Result<ModbusProbeResponse, std::io::Error>) -> std::io::Error {
    match result {
        Ok(_) => panic!("malformed Modbus exchange was accepted"),
        Err(error) => error,
    }
}

#[test]
fn modbus_device_id_request_has_exact_mbap_and_pdu() {
    let response = modbus_response(1, 0, 17, &[0x2b, 0x0e]);
    let (result, request) = modbus_exchange(17, vec![0x2b, 0x0e, 0x01, 0x00], 1, response);
    assert!(matches!(result, Ok(ModbusProbeResponse::Normal(_))));
    assert_eq!(
        request,
        vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x11, 0x2b, 0x0e, 0x01, 0x00]
    );
}

#[test]
fn modbus_safe_read_request_has_exact_transaction_unit_address_and_quantity() {
    let response = modbus_response(2, 0, 255, &[0x03, 0x02, 0x00, 0x00]);
    let (result, request) = modbus_exchange(255, vec![0x03, 0x12, 0x34, 0x00, 0x7d], 2, response);
    assert!(matches!(result, Ok(ModbusProbeResponse::Normal(_))));
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
    assert!(matches!(normal, Ok(ModbusProbeResponse::Normal(_))));

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

/// An FC43/14 device-identification PDU: `objects` are `(object id, value)` pairs, so the answer
/// names exactly the requested basic objects.
fn fc43_device_id_pdu(objects: &[(u8, &[u8])]) -> Vec<u8> {
    let mut pdu = vec![
        0x2b,
        0x0e,
        0x01,
        0x01,
        0x00,
        0x00,
        u8::try_from(objects.len()).expect("object count fits a byte"),
    ];
    for (object_id, value) in objects {
        pdu.push(*object_id);
        pdu.push(u8::try_from(value.len()).expect("object length fits a byte"));
        pdu.extend_from_slice(value);
    }
    pdu
}

#[test]
fn fc43_basic_objects_decode_vendor_product_revision() {
    let pdu = fc43_device_id_pdu(&[(0x00, b"ACME"), (0x01, b"PC-1000"), (0x02, b"1.2.3")]);
    let decoded = decode_fc43_basic_objects(&pdu).expect("basic objects decoded");
    assert_eq!(decoded.vendor_name.as_deref(), Some("ACME"));
    assert_eq!(decoded.product_code.as_deref(), Some("PC-1000"));
    assert_eq!(decoded.revision.as_deref(), Some("1.2.3"));

    // An answer that names none of the basic objects is no identification.
    assert_eq!(
        decode_fc43_basic_objects(&fc43_device_id_pdu(&[(0x07, b"vendor-specific")])),
        None
    );
    // A PDU that is not FC43/14, or is truncated, is no identification.
    assert_eq!(decode_fc43_basic_objects(&[0x03, 0x02, 0x00, 0x00]), None);
    assert_eq!(decode_fc43_basic_objects(&[0x2b, 0x0e]), None);

    // The probe carries the decoded objects into its identification.
    let response_pdu = pdu.clone();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = [0u8; 11];
        stream.read_exact(&mut request).expect("read request");
        let response = modbus_response(1, 0, 1, &response_pdu);
        stream.write_all(&response).expect("write response");
    });
    let outcome = probe_modbus_tcp(
        addr,
        Duration::from_millis(250),
        ModbusDiscoveryProbe {
            unit_id: 1,
            safe_read: None,
        },
    )
    .expect("outcome");
    let identification = outcome.identification.expect("probe identification");
    assert_eq!(identification.vendor_name.as_deref(), Some("ACME"));
    assert_eq!(identification.product_code.as_deref(), Some("PC-1000"));
    assert_eq!(identification.revision.as_deref(), Some("1.2.3"));
    handle.join().expect("join");
}

/// Runs a search to its end, or gives up after ten seconds.
fn run_search_to_end(request: SearchRequest) -> SearchStatus {
    let registry = SearchRegistry::new();
    let id = registry
        .start(request, SearchHost::ThisHost)
        .expect("start search");
    let give_up = Instant::now() + Duration::from_secs(10);
    loop {
        let status = registry.status(id.as_str(), None).expect("search status");
        if status.state != SearchState::Running || Instant::now() >= give_up {
            return status;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn modbus_probes_the_requested_unit_ids() {
    // A server that answers FC43/14 for whichever unit ID the request names, for at most three
    // seconds, so a regression that probes only one unit ID fails instead of hanging.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unit-id server");
    let address = listener.local_addr().expect("unit-id address");
    let server = thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("nonblocking unit-id listener");
        let give_up = Instant::now() + Duration::from_secs(3);
        let mut probed = Vec::new();
        while probed.len() < 2 && Instant::now() < give_up {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0u8; 11];
                    if stream.read_exact(&mut request).is_err() {
                        continue;
                    }
                    let unit_id = request[6];
                    probed.push(unit_id);
                    let response =
                        modbus_response(1, 0, unit_id, &[0x2b, 0x0e, 0x01, 0x01, 0x00, 0x00, 0x00]);
                    let _ = stream.write_all(&response);
                }
                Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
        probed
    });

    let request = SearchRequest::from_params(Some(json!({
        "only": ["modbus_tcp"],
        "targets": { "hosts": [address.to_string()] },
        "modbus": { "unit_ids": [1, 7] },
        "deadline_ms": 2000,
    })))
    .expect("search request");
    let status = run_search_to_end(request);

    let mut units: Vec<u8> = status
        .devices
        .iter()
        .flat_map(|device| device.finds.iter())
        .filter_map(|find| match &find.identity {
            Identity::Modbus(identity) => Some(identity.unit_id),
            _ => None,
        })
        .collect();
    units.sort_unstable();
    assert_eq!(units, vec![1, 7], "requested unit IDs were not both probed");

    let mut probed = server.join().expect("join unit-id server");
    probed.sort_unstable();
    assert_eq!(probed, vec![1, 7], "the server saw the wrong unit IDs");
}

/// A sink that counts the candidates a parallel probe found.
#[derive(Default)]
struct Collected {
    found: usize,
}

impl CandidateSink for Collected {
    fn found(&mut self, _candidate: DiscoverCandidate) {
        self.found += 1;
    }

    fn warn(&mut self, _warning: String) {}
}

#[test]
fn subnet_probes_run_in_parallel_within_the_deadline() {
    const HOSTS: usize = 8;
    const SERVER_DELAY: Duration = Duration::from_millis(400);

    let mut targets = Vec::with_capacity(HOSTS);
    let mut servers = Vec::with_capacity(HOSTS);
    for _ in 0..HOSTS {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind subnet listener");
        targets.push(listener.local_addr().expect("subnet address"));
        servers.push(thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept subnet probe");
            let mut request = [0u8; 11];
            stream
                .read_exact(&mut request)
                .expect("read subnet request");
            thread::sleep(SERVER_DELAY);
            let unit_id = request[6];
            let response =
                modbus_response(1, 0, unit_id, &[0x2b, 0x0e, 0x01, 0x01, 0x00, 0x00, 0x00]);
            stream.write_all(&response).expect("write subnet response");
        }));
    }

    // Eight hosts at 400 ms each take 3.2 s one after another, so a sequential probe cannot reach
    // every host inside the 1.5 s deadline; probing in parallel does.
    let deadline = Deadline::at(Instant::now() + Duration::from_millis(1_500));
    let mut sink = Collected::default();
    let started = Instant::now();
    probe_targets_in_parallel(
        &targets,
        ModbusDiscoveryProbe {
            unit_id: 1,
            safe_read: None,
        },
        Duration::from_secs(2),
        deadline,
        &mut sink,
    );
    let elapsed = started.elapsed();
    for server in servers {
        server.join().expect("join subnet server");
    }
    assert_eq!(
        sink.found, HOSTS,
        "not every subnet host was probed before the deadline"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "subnet probes were not parallel: {elapsed:?}"
    );
}
