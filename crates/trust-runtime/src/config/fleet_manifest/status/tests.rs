//! The control exchange of the fleet commands: the response checks, the line limit, the line end
//! and the transport errors that a stop asks again after (specification 11, 6.8.1 and 6.8.3).

use std::io::{BufRead, BufReader, Write};

use super::*;

/// A runtime's socket that reads one request and answers it with exactly `answer`, then closes the
/// connection; gives its endpoint.
fn answering(answer: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind control listener");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("listener address"));
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept control request");
        let mut writer = stream.try_clone().expect("clone fake stream");
        let mut request = String::new();
        BufReader::new(stream)
            .read_line(&mut request)
            .expect("read control request");
        // The client may have closed the connection already, so this write may fail.
        let _ = writer.write_all(&answer);
    });
    (endpoint, server)
}

/// The error of a status request that the runtime answers with exactly `answer`.
fn control_error_for(answer: Vec<u8>) -> anyhow::Error {
    let (endpoint, server) = answering(answer);
    let error = request_control(&endpoint, Some("token"), "status")
        .expect_err("the answer must be refused");
    server.join().expect("join fake control server");
    error
}

#[test]
fn control_request_rejects_a_mismatched_response_id() {
    let error = control_error_for(b"{\"id\":99,\"ok\":true,\"result\":{}}\n".to_vec());
    assert!(
        format!("{error:#}").contains("carries id 99 instead of 1"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn control_request_rejects_malformed_or_ambiguous_response_envelopes() {
    for (answer, refusal) in [
        ("not-json", "not a control response"),
        (r#"{"id":1}"#, "control request failed"),
        (
            r#"{"id":1,"ok":"true","result":{}}"#,
            "control request failed",
        ),
    ] {
        let error = control_error_for(format!("{answer}\n").into_bytes());
        assert!(
            format!("{error:#}").contains(refusal),
            "unexpected error for {answer:?}: {error:#}"
        );
    }
}

#[test]
fn control_request_rejects_an_oversized_line_and_one_cut_before_its_line_end() {
    // Specification 11, 6.8.1: a response line is at most 1 MiB and ends with a newline. Catches the
    // fleet's own reader, which took a line that the end of the connection cut as complete.
    for (answer, refusal) in [
        (vec![b'x'; 1024 * 1024 + 1], "longer than 1048576 bytes"),
        (
            b"{\"id\":1,\"ok\":true,\"result\":{}}".to_vec(),
            "ends before its line end",
        ),
    ] {
        let error = control_error_for(answer);
        assert!(
            format!("{error:#}").contains(refusal),
            "{refusal}: {error:#}"
        );
    }
}

#[test]
fn only_transport_boundary_errors_are_inconclusive_stop_probes() {
    // Specification 11, 6.8.3: a connection reset, broken pipe, aborted connection, empty EOF or a
    // read or write that waited out is inconclusive during a stop confirmation; the same kind of
    // I/O error outside the control exchange is not, nor is any other transport failure.
    for kind in [
        std::io::ErrorKind::ConnectionReset,
        std::io::ErrorKind::ConnectionAborted,
        std::io::ErrorKind::BrokenPipe,
        std::io::ErrorKind::UnexpectedEof,
        std::io::ErrorKind::TimedOut,
        std::io::ErrorKind::WouldBlock,
    ] {
        let transport = anyhow::Error::from(ControlError::io(&std::io::Error::from(kind)));
        assert!(interrupted_stop_probe(
            &transport.context("control exchange")
        ));
        let configuration = anyhow::Error::from(std::io::Error::from(kind));
        assert!(!interrupted_stop_probe(&configuration));
    }
    let other = anyhow::Error::from(ControlError::io(&std::io::Error::from(
        std::io::ErrorKind::InvalidData,
    )));
    assert!(!interrupted_stop_probe(&other));
}

#[test]
fn a_status_read_that_the_runtime_broke_off_is_an_inconclusive_probe() {
    // Specification 11, 6.8.3: a runtime that took the status request and closed the connection
    // without an answer, as one that stops does, leaves the stop to ask again. Catches the kind of
    // the end of the connection lost on the way through the shared client.
    let error = control_error_for(Vec::new());
    assert!(interrupted_stop_probe(&error), "{error:#}");
}
