//! Tests of the simulator journey's HTTP transport, against a tiny_http server, the runtime's own,
//! on 127.0.0.1. One test uses a bare TCP server instead, because tiny_http always closes an HTTP/1.0
//! connection after its answer, and only a server that keeps it open shows that the transport stops
//! at Content-Length.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tiny_http::{HTTPVersion, Header, Request, Response, Server, StatusCode};

use super::super::error::{CANCELLED, NOT_CONNECTED};
use super::SimulatorHmi;

const COOKIE: &str = "trust_hmi_session=4f1c0ffee";
const TOKEN: &str = "access-token-7";

fn hmi(base: &str, timeout: Duration) -> SimulatorHmi<'_> {
    SimulatorHmi {
        base,
        token: Some(TOKEN),
        cookie: COOKIE,
        timeout,
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}

/// A tiny_http server that hands each request it receives within two seconds, at most `count`, to
/// `respond`; its address and the server's thread.
fn serve(
    count: usize,
    respond: impl Fn(Request) + Send + 'static,
) -> (String, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").expect("a test server");
    let base = format!(
        "http://{}",
        server.server_addr().to_ip().expect("an IP listener")
    );
    let thread = thread::spawn(move || {
        for _ in 0..count {
            match server.recv_timeout(Duration::from_secs(2)) {
                Ok(Some(request)) => respond(request),
                _ => return,
            }
        }
    });
    (base, thread)
}

fn json_response(status: u16, body: &Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string()).with_status_code(StatusCode(status))
}

/// Catches: an answer read to the end of its connection instead of its Content-Length, which waits
/// for the timeout while the server keeps the connection open, or read past it.
#[test]
fn a_body_sized_by_content_length_ends_there() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let (done, finished) = mpsc::channel::<()>();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the request");
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let answer = b"HTTP/1.0 200 OK\r\nContent-Length: 12\r\n\r\n{\"ok\": true}and more bytes";
        stream.write_all(answer).expect("the answer");
        let _ = finished.recv_timeout(Duration::from_secs(10));
    });
    let started = Instant::now();
    let reply = hmi(&base, Duration::from_secs(5)).send("/api/hmi/page?name=a", None);
    assert_eq!(reply, Ok((200, json!({ "ok": true }))));
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "it did not wait for the connection's end"
    );
    let _ = done.send(());
    let _ = server.join();
}

/// Catches: a transport that needs Content-Length, and so fails an answer that simply ends with its
/// connection, as HTTP/1.0 allows.
#[test]
fn a_body_without_content_length_is_read_to_the_end() {
    let (base, server) = serve(1, |request| {
        let body: &[u8] = br#"{"page": null}"#;
        let _ = request.respond(Response::new(StatusCode(200), Vec::new(), body, None, None));
    });
    let reply = hmi(&base, Duration::from_secs(5)).send("/api/hmi/page?name=a", None);
    assert_eq!(reply, Ok((200, json!({ "page": null }))));
    let _ = server.join();
}

/// Catches: a request sent as HTTP/1.1, which tiny_http, like the runtime, answers in chunks once the
/// body reaches 32 KiB: the chunk sizes would break the JSON.
#[test]
fn a_body_over_32_kib_arrives_whole() {
    let expected = json!({ "texts": "x".repeat(40 * 1024) });
    let body = expected.to_string();
    let (versions, version) = mpsc::channel();
    let (base, server) = serve(1, move |request| {
        let _ = versions.send(request.http_version().clone());
        let _ = request.respond(Response::from_string(body.clone()));
    });
    let reply = hmi(&base, Duration::from_secs(5)).send("/api/hmi/page?name=a", None);
    assert_eq!(reply, Ok((200, expected)));
    assert_eq!(version.recv().expect("the request"), HTTPVersion(1, 0));
    let _ = server.join();
}

/// Catches: a status other than 200 turned into a transport failure, which would hide the
/// simulator's refusal from the journey's outcome; and a request without the access token, the named
/// session's cookie or its JSON body.
#[test]
fn a_status_other_than_200_is_passed_through() {
    let (seen, received) = mpsc::channel();
    let (base, server) = serve(1, move |mut request| {
        let header = |name: &'static str| {
            let found = request
                .headers()
                .iter()
                .find(|header| header.field.equiv(name));
            found.map(|header| header.value.as_str().to_owned())
        };
        let headers = (
            header("X-Trust-Token"),
            header("Cookie"),
            header("Content-Type"),
        );
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);
        let _ = seen.send((request.method().to_string(), headers, body));
        let _ = request.respond(json_response(409, &json!({ "error": "the page changed" })));
    });
    let command = json!({ "id": "c-1", "page": "a" });
    let reply = hmi(&base, Duration::from_secs(5)).send("/api/hmi/command", Some(&command));
    assert_eq!(reply, Ok((409, json!({ "error": "the page changed" }))));
    let (method, headers, body) = received.recv().expect("the request");
    assert_eq!(method, "POST");
    assert_eq!(
        headers,
        (
            Some(TOKEN.to_owned()),
            Some(COOKIE.to_owned()),
            Some("application/json".to_owned())
        )
    );
    assert_eq!(serde_json::from_str::<Value>(&body).expect("JSON"), command);
    let _ = server.join();
}

/// Catches: a transport that follows a redirect, which would carry the named session's cookie and the
/// access token to another address.
#[test]
fn a_redirect_is_reported_and_not_followed() {
    let (seen, received) = mpsc::channel();
    let (base, server) = serve(2, move |request| {
        let _ = seen.send(request.url().to_owned());
        let location = Header::from_bytes("Location", "/elsewhere").expect("a header");
        let _ = request.respond(Response::empty(StatusCode(302)).with_header(location));
    });
    let error = hmi(&base, Duration::from_secs(5))
        .send("/api/hmi/page?name=a", None)
        .expect_err("a redirect");
    assert_eq!(error.code, NOT_CONNECTED);
    assert_eq!(
        error.message,
        format!("the simulator's HMI at {base} answered with a redirect, which a journey never follows.")
    );
    let _ = server.join();
    assert_eq!(
        received.try_iter().collect::<Vec<_>>(),
        ["/api/hmi/page?name=a"]
    );
}

/// Catches: a request that waits without end for a simulator that never answers.
#[test]
fn a_server_that_never_answers_ends_at_the_timeout() {
    let server = Server::http("127.0.0.1:0").expect("a test server");
    let base = format!(
        "http://{}",
        server.server_addr().to_ip().expect("an IP listener")
    );
    let (done, finished) = mpsc::channel::<()>();
    let holder = thread::spawn(move || {
        let held = server.recv_timeout(Duration::from_secs(5));
        let _ = finished.recv_timeout(Duration::from_secs(10));
        drop(held);
    });
    let started = Instant::now();
    let error = hmi(&base, Duration::from_millis(300))
        .send("/api/hmi/page?name=a", None)
        .expect_err("a timeout");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "it ended at the timeout"
    );
    assert_eq!(error.code, NOT_CONNECTED);
    assert_eq!(
        error.message,
        format!("the simulator's HMI at {base} did not answer within 300ms.")
    );
    let _ = done.send(());
    let _ = holder.join();
}

/// Catches: an answer's headers read without bound, which a broken or hostile server could use to
/// make the language server read until its memory runs out.
#[test]
fn headers_longer_than_16_kib_are_refused() {
    let (base, server) = serve(1, |request| {
        let padding = Header::from_bytes("X-Padding", vec![b'a'; 64 * 1024]).expect("a header");
        let _ = request.respond(json_response(200, &json!({})).with_header(padding));
    });
    let error = hmi(&base, Duration::from_secs(5))
        .send("/api/hmi/page?name=a", None)
        .expect_err("refused");
    assert_eq!(
        error.message,
        format!("the simulator's HMI at {base} answered with headers longer than 16 KiB.")
    );
    let _ = server.join();
}

/// Catches: a body read without bound, whether it announces its length or not.
#[test]
fn a_body_longer_than_8_mib_is_refused() {
    let (base, server) = serve(2, |request| {
        let big = vec![b' '; 8 * 1024 * 1024 + 1];
        let length = if request.url().ends_with("sized") {
            Some(big.len())
        } else {
            None
        };
        let _ = request.respond(Response::new(
            StatusCode(200),
            Vec::new(),
            &big[..],
            length,
            None,
        ));
    });
    for path in ["/sized", "/unsized"] {
        let error = hmi(&base, Duration::from_secs(10))
            .send(path, None)
            .expect_err("refused");
        assert_eq!(
            error.message,
            format!("the simulator's HMI at {base} answered with a body longer than 8 MiB."),
            "{path}"
        );
    }
    let _ = server.join();
}

/// Catches: a cookie with a line break written into the request, where it would add a header of its
/// own; and a refusal that quotes the cookie.
#[test]
fn a_header_value_http_does_not_allow_is_refused_without_quoting_it() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    listener
        .set_nonblocking(true)
        .expect("a non-blocking listener");
    let hmi = SimulatorHmi {
        base: &base,
        token: Some(TOKEN),
        cookie: "trust_hmi_session=5ecret\r\nX-Injected: 1",
        timeout: Duration::from_secs(5),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let error = hmi.send("/api/hmi/page?name=a", None).expect_err("refused");
    assert_eq!(error.code, NOT_CONNECTED);
    assert!(
        !error.message.contains("5ecret") && !error.message.contains("X-Injected"),
        "{error}"
    );
    assert!(listener.accept().is_err(), "nothing was sent");
}

/// Catches: an `https` address reached without TLS, and a TLS failure reported without saying that
/// TLS failed. The server answers in plain HTTP at once, as a simulator without TLS would; tiny_http
/// would wait for a line that a TLS greeting never sends.
#[test]
fn an_https_address_that_fails_tls_names_tls() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let address = format!("https://{}", listener.local_addr().expect("an address"));
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the connection");
        let _ = stream.write_all(b"HTTP/1.0 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
    });
    let error = hmi(&address, Duration::from_secs(5))
        .send("/api/hmi/page?name=a", None)
        .expect_err("no TLS");
    assert_eq!(error.code, NOT_CONNECTED);
    let prefix = format!("the simulator's HMI at {address} could not be reached: TLS: ");
    assert!(error.message.starts_with(&prefix), "{}", error.message);
    let _ = server.join();
}

/// Catches: a cancel that waits for a request in flight to time out, so that a journey on a hung
/// simulator ends 15 s later as `NOT_CONNECTED` instead of at once as `CANCELLED`. A read waits one
/// 100 ms slice at most before it sees the cancel.
#[test]
fn a_cancel_ends_a_request_in_flight() {
    let hung = TcpListener::bind("127.0.0.1:0").expect("a listener that never answers");
    let base = format!("http://{}", hung.local_addr().expect("its address"));
    let hmi = hmi(&base, Duration::from_secs(10));
    let cancel = Arc::clone(&hmi.cancelled);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        cancel.store(true, Ordering::SeqCst);
    });
    let started = Instant::now();
    let error = hmi
        .send("/api/hmi/page?name=a", None)
        .expect_err("cancelled");
    let took = started.elapsed();
    assert_eq!(error.code, CANCELLED, "{error}");
    assert!(
        took < Duration::from_millis(800),
        "ended {took:?} after the request, 300 ms after it went out"
    );
    drop(hung);
}
