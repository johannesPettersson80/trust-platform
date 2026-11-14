//! The waits of the shared client over a runtime's socket (specification 11, 6.8.1).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

use serde_json::{json, Value};

use super::socket_client::{connect, exchange, reply_wait, request, ConnectFailed};
use crate::control::ControlEndpoint;

/// A runtime's socket on this computer that reads one request and answers it, with the request's
/// id, `delay` after it read it; gives its endpoint.
fn answering_after(delay: Duration) -> (ControlEndpoint, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = ControlEndpoint::Tcp(listener.local_addr().expect("local address"));
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the request");
        let mut writer = stream.try_clone().expect("clone the connection");
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .expect("read the request");
        let request: Value = serde_json::from_str(&line).expect("a JSON request line");
        std::thread::sleep(delay);
        // A client that gave up has closed the connection, so this write may fail.
        let _ = writeln!(
            writer,
            "{}",
            json!({"id": request["id"], "ok": true, "result": {}})
        );
    });
    (endpoint, server)
}

#[test]
fn the_reply_to_a_reload_is_awaited_longer_than_the_reply_to_another_request() {
    // Specification 11, 6.8.1: a client waits 750 milliseconds for a reply, and 70 seconds for the
    // reply to `bytecode.reload`, which the runtime gives within 65 seconds (6.7.1). A runtime that
    // answers two seconds after the request is too late for `status` and in time for a reload.
    // Wrong behaviour: a reload that the runtime may still apply reported as failed after 750
    // milliseconds, or every request waiting as long as a reload.
    let delay = Duration::from_secs(2);
    let (endpoint, server) = answering_after(delay);
    let error = request(&endpoint, &json!({"type": "status"}))
        .expect_err("a reply two seconds late is too late for status");
    server.join().expect("the runtime");
    assert!(
        error.downcast_ref::<ConnectFailed>().is_none(),
        "the connection was made: {error:#}"
    );

    let (endpoint, server) = answering_after(delay);
    let response = request(
        &endpoint,
        &json!({"type": "bytecode.reload", "params": {"bytes": ""}}),
    )
    .unwrap_or_else(|error| panic!("a reload answered after two seconds: {error:#}"));
    server.join().expect("the runtime");
    assert_eq!(response, json!({"id": 1, "ok": true, "result": {}}));

    // The waits themselves, as the specification gives them.
    assert_eq!(reply_wait("status"), Duration::from_millis(750));
    assert_eq!(reply_wait("bytecode.reload"), Duration::from_secs(70));
}

#[test]
fn on_a_kept_connection_only_the_reload_waits_a_reloads_time() {
    // Specification 11, 6.8.1: the reply to `bytecode.reload` is awaited 70 seconds and every
    // other reply 750 milliseconds, on a connection that the caller keeps for several requests, as
    // the `trust-runtime ui` console keeps its own. A runtime answers each request two seconds
    // after it read it: the reload's reply is taken, and the `status` after it on the same
    // connection is given up on. Wrong behaviour: a connection left with the reload's wait for every
    // later request, which then waits 70 seconds for a runtime that stopped answering.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = ControlEndpoint::Tcp(listener.local_addr().expect("local address"));
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the requests");
        let mut writer = stream.try_clone().expect("clone the connection");
        let mut reader = BufReader::new(stream);
        for _ in 0..2 {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let request: Value = serde_json::from_str(&line).expect("a JSON request line");
            std::thread::sleep(Duration::from_secs(2));
            // A client that gave up has closed the connection, so this write may fail.
            let _ = writeln!(
                writer,
                "{}",
                json!({"id": request["id"], "ok": true, "result": {}})
            );
        }
    });
    let mut client = connect(&endpoint, None).expect("connect");
    let reload = exchange(
        &mut client,
        &json!({"type": "bytecode.reload", "params": {"bytes": ""}}),
    )
    .unwrap_or_else(|error| panic!("a reload answered after two seconds: {error:#}"));
    assert_eq!(reload["id"], json!(1));
    exchange(&mut client, &json!({"type": "status"}))
        .expect_err("a status answered after two seconds is too late");
    drop(client);
    server.join().expect("the runtime");
}
