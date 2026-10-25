//! The serial endpoint of `trust-runtime ctl` (specification 11, 6.8.1; specification 34, PORT-05):
//! that the command line accepts it, and that a request travels over a serial stream as it does over
//! a socket. A canned stream stands for the chip's port; no test opens a real one.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use trust_runtime_client::ByteStream;

use super::*;

#[test]
fn the_cli_accepts_a_serial_endpoint() {
    // Specification 11 6.8.1: `--endpoint` takes `serial://<port>` beside the socket
    // endpoints, and `ctl` takes `--endpoint` and `--token` before or after the action, so
    // `ctl --endpoint serial:///dev/ttyACM0 install program.trsta` and `ctl install --endpoint
    // serial:///dev/ttyACM0 program.trsta` parse alike. The endpoint resolves to a serial target
    // without opening any port.
    // Wrong behaviour: the scheme is refused as `unsupported endpoint`, an option after the
    // action is refused by the parser, or the token is lost on the way.
    use clap::Parser;

    use crate::cli::{Cli, Command};

    for args in [
        [
            "trust-runtime",
            "ctl",
            "--endpoint",
            "serial:///dev/ttyACM0",
            "--token",
            "secret",
            "install",
            "program.trsta",
            "--restart",
            "cold",
        ],
        [
            "trust-runtime",
            "ctl",
            "install",
            "--endpoint",
            "serial:///dev/ttyACM0",
            "--token",
            "secret",
            "program.trsta",
            "--restart",
            "cold",
        ],
    ] {
        let cli = Cli::try_parse_from(args).expect("the options come before or after install");
        let Some(Command::Ctl {
            endpoint,
            token,
            action: ControlAction::Install { container, restart },
            ..
        }) = cli.command
        else {
            panic!("expected ctl install");
        };
        assert_eq!(endpoint.as_deref(), Some("serial:///dev/ttyACM0"));
        assert_eq!(token.as_deref(), Some("secret"));
        assert_eq!(container, PathBuf::from("program.trsta"));
        assert_eq!(restart.as_deref(), Some("cold"));
    }

    let target = resolve_control_target(
        None,
        Some("serial:///dev/ttyACM0".to_string()),
        Some("secret".to_string()),
    )
    .unwrap_or_else(|error| panic!("a serial endpoint resolves: {error:#}"));
    assert!(
        matches!(
            &target.endpoint,
            CtlEndpoint::Serial(ClientEndpoint::Serial(port)) if port == "/dev/ttyACM0"
        ),
        "a serial endpoint stays a serial target"
    );
    assert_eq!(target.auth_token.as_deref(), Some("secret"));
}

/// A chip's serial stream that records what the client writes and answers with one canned
/// reply line.
#[derive(Debug)]
struct Canned {
    written: Arc<Mutex<Vec<u8>>>,
    reply: VecDeque<u8>,
}

impl Canned {
    fn answering(reply: &str) -> (Self, Arc<Mutex<Vec<u8>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));
        let stream = Self {
            written: Arc::clone(&written),
            reply: format!("{reply}\n").into_bytes().into(),
        };
        (stream, written)
    }
}

impl Read for Canned {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.reply.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "nothing to read",
            ));
        }
        let count = buf.len().min(self.reply.len());
        for slot in &mut buf[..count] {
            *slot = self.reply.pop_front().expect("count bytes are queued");
        }
        Ok(count)
    }
}

impl Write for Canned {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.written
            .lock()
            .expect("the lock is not poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl ByteStream for Canned {
    fn set_timeout(&self, _timeout: Duration) -> std::io::Result<()> {
        Ok(())
    }

    fn frame_bytes(&self) -> Option<usize> {
        Some(8_192)
    }
}

#[test]
fn ctl_over_a_serial_stream_sends_the_actions_request_and_returns_the_whole_response() {
    // Specification 11 6.8.1: over a serial endpoint the same requests and responses travel
    // as over a socket. The request is the action's: numeric id 1, its `type`, the token as
    // `auth`, and only the action's parameters; the response that the command prints is the
    // whole response object, the audit record id included. Wrong behaviour: the parameters or
    // the token are lost on the way to the chip, or the response is cut to its result.
    let reply = r#"{"id":1,"ok":true,"result":{"queued":true},"audit_id":"audit-9"}"#;
    let (stream, written) = Canned::answering(reply);
    let mut client = ControlClient::from_stream(stream, Some("secret"));
    let action = ControlAction::IoWrite {
        address: "%QX0.0".into(),
        value: "TRUE".into(),
    };
    let request = build_request(&action, Some("secret"));

    let response = exchange_with(&mut client, &request)
        .unwrap_or_else(|error| panic!("the chip answers the write: {error:#}"));

    assert_eq!(
        response,
        json!({"id": 1, "ok": true, "result": {"queued": true}, "audit_id": "audit-9"})
    );
    let sent = written.lock().expect("the lock is not poisoned").clone();
    assert_eq!(sent.last(), Some(&b'\n'), "the request is one line");
    assert_eq!(
        serde_json::from_slice::<JsonValue>(&sent).expect("the request is JSON"),
        json!({
            "id": 1,
            "type": "io.write",
            "auth": "secret",
            "params": {"address": "%QX0.0", "value": "TRUE"}
        })
    );
}

#[test]
fn ctl_over_a_serial_stream_refuses_a_response_without_the_requests_id() {
    // Specification 11 6.8.1: a missing response id is a protocol failure, on a serial
    // endpoint as on a socket. Wrong behaviour: the shared client takes an answer without an
    // id as the request's, and the command prints it as a success.
    let (stream, _) = Canned::answering(r#"{"ok":true,"result":{"ok":true}}"#);
    let mut client = ControlClient::from_stream(stream, None);
    let request = build_request(&ControlAction::Health, None);

    let Err(error) = exchange_with(&mut client, &request) else {
        panic!("an answer without an id is a protocol failure");
    };

    assert_eq!(
        error.to_string(),
        "control response id is missing or does not match request id 1"
    );
}

#[test]
fn a_serial_endpoint_without_a_usable_port_is_refused_plainly() {
    // Specification 11 6.8.1: the port is not empty, and on Linux and macOS it is an absolute
    // path. The refusal names the endpoint and shows the accepted forms. Wrong behaviour: the
    // CLI opens whatever follows `serial://`, a path relative to the working directory
    // included, or says only `unsupported endpoint`.
    #[cfg(unix)]
    let refused = ["serial://", "serial://   ", "serial://ttyACM0"];
    #[cfg(not(unix))]
    let refused = ["serial://", "serial://   "];
    for endpoint in refused {
        let Err(error) = resolve_control_target(None, Some(endpoint.to_string()), None) else {
            panic!("{endpoint:?} is not a serial endpoint");
        };
        assert_eq!(
            error.to_string(),
            format!(
                "'{endpoint}' is not a serial endpoint (serial:///dev/ttyACM0, or \
                 serial://COM5 on Windows)"
            )
        );
    }
}

#[test]
fn a_tcp_endpoint_on_another_host_is_reached_through_the_shared_client() {
    // Specification 11 6.8.1 and specification 34 ARCH-12: `tcp://host:port` on another host, a
    // chip's control connection over its network, goes through the shared control client with the
    // serial endpoint's waits; a loopback `tcp://` endpoint stays a runtime's socket on this
    // computer, and a `tcp://` without a host is refused.
    // Wrong behaviour: a chip's address refused as `tcp endpoint must be loopback`, which leaves a
    // chip on the network unreachable from `ctl`, or a computer's own loopback socket sent through
    // the chip's client with its longer waits.
    match CtlEndpoint::parse("tcp://192.168.4.23:9901").expect("a chip's endpoint parses") {
        CtlEndpoint::Network(ClientEndpoint::Tcp(host)) => assert_eq!(host, "192.168.4.23:9901"),
        other => panic!("a chip on the network is reached through the client: {other:?}"),
    }
    match CtlEndpoint::parse("tcp://bench-c6.local:9901").expect("a host name parses") {
        CtlEndpoint::Network(ClientEndpoint::Tcp(host)) => assert_eq!(host, "bench-c6.local:9901"),
        other => panic!("a host name is reached through the client: {other:?}"),
    }
    assert!(matches!(
        CtlEndpoint::parse("tcp://127.0.0.1:9901").expect("a loopback endpoint parses"),
        CtlEndpoint::Socket(ControlEndpoint::Tcp(_))
    ));
    assert!(CtlEndpoint::parse("tcp://").is_err());
}
