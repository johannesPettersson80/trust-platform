//! Endpoint and token precedence of the control helper (spec 11 section 6.8.1), and the shared
//! client's waits and checks of a response, which its requests now have (spec 11, 6.8.1).
//!
//! The environment token is merged by `call_control_request`, so these tests call the resolver
//! directly, or name a token, and never depend on the `TRUST_CTL_TOKEN` of the machine that runs
//! them.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::json;

use super::{call_control_request, resolve_control_target};

static PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch project directory, removed when the test ends.
struct ScratchProject(PathBuf);

impl ScratchProject {
    fn new() -> Self {
        let sequence = PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = trust_test_support::unique_temp_path(&format!(
            "trust-dev-ctl-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create ctl test project");
        Self(path)
    }

    /// A project whose `runtime.toml` names the endpoint and the control token.
    fn with_target(endpoint: &str, token: &str) -> Self {
        let project = Self::new();
        let control = format!("endpoint = \"{endpoint}\"\nauth_token = \"{token}\"");
        std::fs::write(project.path().join("runtime.toml"), runtime_toml(&control))
            .expect("write runtime config");
        project
    }

    /// A project whose `runtime.toml` cannot be parsed, so any read of it fails.
    fn unreadable() -> Self {
        let project = Self::new();
        std::fs::write(project.path().join("runtime.toml"), "not = [valid")
            .expect("write malformed runtime config");
        project
    }

    fn path(&self) -> PathBuf {
        self.0.clone()
    }
}

impl Drop for ScratchProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A complete runtime.toml, valid for the loader, whose `[runtime.control]` table holds
/// `control_lines`.
fn runtime_toml(control_lines: &str) -> String {
    format!(
        r#"
[bundle]
version = 1

[resource]
name = "ctl-test"
cycle_interval_ms = 10

[runtime]
execution_backend = "vm"

[runtime.control]
{control_lines}
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#
    )
}

#[test]
fn endpoint_override_keeps_the_project_token() {
    let project = ScratchProject::with_target("tcp://127.0.0.1:1", "project-token");

    let target = resolve_control_target(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        None,
    )
    .expect("resolve target with endpoint override");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9");
    assert_eq!(target.auth_token.as_deref(), Some("project-token"));
}

#[test]
fn explicit_token_wins_and_the_project_is_not_read_for_it() {
    let project = ScratchProject::unreadable();

    let target = resolve_control_target(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        Some("explicit-token".to_string()),
    )
    .expect("explicit token needs no project read");

    assert_eq!(target.auth_token.as_deref(), Some("explicit-token"));
}

#[test]
fn project_supplies_endpoint_and_token_without_an_override() {
    let project = ScratchProject::with_target("tcp://127.0.0.1:1", "project-token");

    let target =
        resolve_control_target(Some(project.path()), None, None).expect("resolve project target");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:1");
    assert_eq!(target.auth_token.as_deref(), Some("project-token"));
}

#[test]
fn endpoint_override_without_a_project_sends_no_token() {
    let target = resolve_control_target(None, Some("tcp://127.0.0.1:9".to_string()), None)
        .expect("resolve endpoint-only target");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9");
    assert!(target.auth_token.is_none());
}

#[test]
fn a_target_is_required() {
    let Err(err) = resolve_control_target(None, None, None) else {
        panic!("a target without endpoint or project must be rejected");
    };

    assert!(
        err.to_string().contains("--endpoint or --project required"),
        "{err}"
    );
}

#[test]
fn a_stale_endpoint_file_reports_that_the_runtime_is_not_running() {
    let project = ScratchProject::new();
    let closed = trust_test_support::refused_loopback_port();
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], closed.port()));
    let published = trust_runtime::control::endpoint::PublishedEndpoint::publish(
        &project.path(),
        &trust_runtime::control::ControlEndpoint::Tcp(address),
        None,
    )
    .expect("publish a stale endpoint file");

    let Err(error) = super::call_control_request(
        Some(project.path()),
        None,
        Some("explicit-token".to_string()),
        "status",
        None,
    ) else {
        panic!("nothing listens at the published endpoint");
    };

    assert!(
        error.to_string().starts_with("runtime is not running"),
        "{error:#}"
    );
    drop(published);
}

/// A runtime's socket on this computer that reads one request, waits `delay`, writes `answer` and
/// closes the connection; gives its endpoint as `--endpoint` text.
fn answering(answer: Vec<u8>, delay: Duration) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("local address"));
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

/// `call_control_request` of `request_type` to `endpoint` with a named token.
fn call(endpoint: &str, request_type: &str) -> anyhow::Result<super::ControlCallResult> {
    call_control_request(
        None,
        Some(endpoint.to_string()),
        Some("token".to_string()),
        request_type,
        None,
    )
}

#[test]
fn an_answer_without_the_requests_id_or_its_line_end_is_refused() {
    // Spec 11, 6.8.1: a response must end with a newline and carry its request's id. Wrong
    // behaviour: an answer that a runtime cut off, or one written for another request, taken as
    // the reply to this request, as trust-dev's own client took it until it used the shared one.
    for (answer, refusal) in [
        (
            b"{\"ok\":true,\"result\":{}}\n".to_vec(),
            "control response id is missing or does not match request id 1",
        ),
        (
            b"{\"id\":1,\"ok\":true,\"result\":{}}".to_vec(),
            "ends before its line end",
        ),
    ] {
        let (endpoint, server) = answering(answer, Duration::ZERO);
        let Err(error) = call(&endpoint, "status") else {
            panic!("{refusal}: the answer was taken");
        };
        server.join().expect("the runtime");
        assert!(
            format!("{error:#}").contains(refusal),
            "{refusal}: {error:#}"
        );
    }
}

#[test]
fn a_runtime_that_does_not_answer_is_given_up_on() {
    // Spec 11, 6.8.1: reads wait at most 750 milliseconds for a reply that is not a reload's.
    // Wrong behaviour: a request that waits for ever on a runtime that took it and never answers,
    // as trust-dev's own client did, which had no wait at all.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("local address"));
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
        let _ = done.send(call(&endpoint, "status").map(|_| ()));
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
    // Spec 11, 6.8.1: the reply to `bytecode.reload` is awaited at most 70 seconds, because the
    // runtime may take 65 to answer it (6.7.1). trust-dev sends its reloads through the shared
    // client, whose other requests wait 750 milliseconds. Wrong behaviour: a reload that the runtime
    // still applies reported to the agent as failed.
    let (endpoint, server) = answering(
        b"{\"id\":1,\"ok\":true,\"result\":{\"status\":\"reloaded\"}}\n".to_vec(),
        Duration::from_secs(2),
    );
    let reply = call(&endpoint, "bytecode.reload")
        .unwrap_or_else(|error| panic!("a reload answered after two seconds: {error:#}"));
    server.join().expect("the runtime");
    assert_eq!(reply.result, json!({"status": "reloaded"}));
    assert_eq!(reply.endpoint, endpoint);
}
