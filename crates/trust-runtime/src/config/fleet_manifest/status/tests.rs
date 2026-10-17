//! The control exchange of the fleet commands: the response checks, the line limit and the
//! transport errors that a stop asks again after. Moved with the code from the `fleet` command.

use super::*;

#[test]
fn control_request_rejects_a_mismatched_response_id() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind control listener");
    let address = listener.local_addr().expect("control listener address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept control request");
        let mut request = String::new();
        BufReader::new(stream.try_clone().expect("clone fake stream"))
            .read_line(&mut request)
            .expect("read control request");
        writeln!(
            stream,
            "{}",
            serde_json::json!({"id": 99, "ok": true, "result": {}})
        )
        .expect("write mismatched response");
    });

    let error = request_control(format!("tcp://{address}").as_str(), Some("token"), "status")
        .expect_err("mismatched response id must fail");
    server.join().expect("join fake control server");

    assert!(
        error.to_string().contains("response id"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn control_request_rejects_malformed_or_ambiguous_response_envelopes() {
    for response in [
        "not-json",
        r#"{"id":1}"#,
        r#"{"id":1,"ok":"true","result":{}}"#,
    ] {
        let error = control_error_for_response(response);
        assert!(
            error.contains("not JSON") || error.contains("control request failed"),
            "unexpected error for {response:?}: {error}"
        );
    }
}

#[test]
fn control_response_reader_rejects_an_oversized_line() {
    let bytes = vec![b'x'; MAX_CONTROL_RESPONSE_BYTES + 1];
    let mut reader = std::io::Cursor::new(bytes);

    let error = read_control_response(&mut reader)
        .expect_err("oversized control response must be rejected");

    assert!(
        error.to_string().contains("exceeded"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn only_transport_boundary_errors_are_inconclusive_stop_probes() {
    for kind in [
        std::io::ErrorKind::ConnectionReset,
        std::io::ErrorKind::ConnectionAborted,
        std::io::ErrorKind::BrokenPipe,
        std::io::ErrorKind::UnexpectedEof,
        std::io::ErrorKind::TimedOut,
        std::io::ErrorKind::WouldBlock,
    ] {
        let transport = control_transport_error(std::io::Error::from(kind));
        assert!(interrupted_stop_probe(
            &transport.context("control exchange")
        ));
        let configuration = anyhow::Error::from(std::io::Error::from(kind));
        assert!(!interrupted_stop_probe(&configuration));
    }
    let other = control_transport_error(std::io::Error::from(std::io::ErrorKind::InvalidData));
    assert!(!interrupted_stop_probe(&other));
}

fn control_error_for_response(response: &str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fake control listener");
    let address = listener.local_addr().expect("read fake control address");
    let response = response.to_owned();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept control request");
        let mut request = String::new();
        BufReader::new(stream.try_clone().expect("clone fake stream"))
            .read_line(&mut request)
            .expect("read control request");
        writeln!(stream, "{response}").expect("write fake response");
    });

    let error = request_control(format!("tcp://{address}").as_str(), Some("token"), "status")
        .expect_err("invalid response must fail");
    server.join().expect("join fake control server");
    format!("{error:#}")
}
