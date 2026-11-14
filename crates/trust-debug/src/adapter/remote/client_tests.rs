//! The waits and checks of the shared control client that the debugger's remote session goes
//! through (specification 11, 6.8.1).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use super::{RemoteEndpoint, RemoteSession};

/// A runtime's socket that reads one request, waits `delay`, writes `answer` and closes the
/// connection; gives its endpoint.
fn answering(answer: Vec<u8>, delay: Duration) -> (RemoteEndpoint, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = RemoteEndpoint::Tcp(listener.local_addr().expect("local address"));
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the request");
        let mut writer = stream.try_clone().expect("clone the connection");
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .expect("read the request");
        std::thread::sleep(delay);
        // A client that gave up has closed the connection, so this write may fail.
        let _ = writer.write_all(&answer);
    });
    (endpoint, server)
}

#[test]
fn an_answer_with_another_requests_id_or_cut_before_its_line_end_is_refused() {
    // Specification 11, 6.8.1: a response ends with a newline and carries its request's id.
    // Catches the debugger's own client, which took an answer that a stopping runtime cut before its
    // line end, or one written for another request, as the reply.
    for (answer, refusal) in [
        (
            b"{\"id\":7,\"ok\":true,\"result\":{}}\n".to_vec(),
            "carries id 7 instead of 1",
        ),
        (
            b"{\"id\":1,\"ok\":true,\"result\":{}}".to_vec(),
            "ends before its line end",
        ),
    ] {
        let (endpoint, server) = answering(answer, Duration::ZERO);
        let mut session = RemoteSession::connect(endpoint, None).expect("connect");
        let Err(error) = session.pause() else {
            panic!("{refusal}: the answer was taken");
        };
        server.join().expect("the runtime");
        assert!(error.contains(refusal), "{refusal}: {error}");
    }
}

#[test]
fn a_runtime_that_does_not_answer_is_given_up_on() {
    // Specification 11, 6.8.1: a reply that is not a reload's is awaited 750 milliseconds. Catches
    // the debugger's own client, which had no wait: a runtime that took a request and never answered
    // held the debug session indefinitely.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = RemoteEndpoint::Tcp(listener.local_addr().expect("local address"));
    let (release, released) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the request");
        let mut line = String::new();
        BufReader::new(stream.try_clone().expect("clone the connection"))
            .read_line(&mut line)
            .expect("read the request");
        // Hold the connection without an answer until the test is done.
        let _ = released.recv_timeout(Duration::from_secs(30));
        drop(stream);
    });
    let (done, outcome) = mpsc::channel();
    let caller = std::thread::spawn(move || {
        let mut session = RemoteSession::connect(endpoint, None).expect("connect");
        let _ = done.send(session.pause());
    });
    let result = outcome.recv_timeout(Duration::from_secs(10));
    release.send(()).expect("release the runtime");
    server.join().expect("the runtime");
    caller.join().expect("the caller");
    let Ok(Err(_)) = result else {
        panic!("the request was not given up within 10 seconds: {result:?}");
    };
}

#[test]
fn a_reload_reply_two_seconds_after_the_request_is_taken() {
    // Specification 11, 6.8.1: the reply to `bytecode.reload` is awaited 70 seconds, because the
    // runtime may take 65 to answer it (6.7.1). Catches the debugger's online change given the 750
    // milliseconds of its other requests, which reports a change that the runtime still applies as
    // failed.
    let (endpoint, server) = answering(
        b"{\"id\":1,\"ok\":true,\"result\":{\"status\":\"reloaded\"}}\n".to_vec(),
        Duration::from_secs(2),
    );
    let mut session = RemoteSession::connect(endpoint, None).expect("connect");
    session
        .reload_program(&[])
        .unwrap_or_else(|error| panic!("a reload answered after two seconds: {error}"));
    server.join().expect("the runtime");
}
