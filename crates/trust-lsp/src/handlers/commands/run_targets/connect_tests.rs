//! A Connect and the kind of a lifecycle failure (specification 25 §5.1, §5.10 and §6.4).

use std::cell::Cell;

use serde_json::{json, Value};
use trust_runtime_client::AuthFailure;

use super::super::request::Outcome;
use super::{answer, connect, ConnectArgs, Contact, DEBUG_DISABLED};

/// A runtime that answers a Connect as it is told.
struct Told {
    reachable: bool,
    status: Outcome,
    asked: Cell<usize>,
}

impl Contact for Told {
    fn reachable(&self, _endpoint: &str) -> bool {
        self.reachable
    }

    fn status(&self, _endpoint: &str, _token: Option<&str>) -> Outcome {
        self.asked.set(self.asked.get() + 1);
        self.status.clone()
    }
}

fn told(reachable: bool, status: Outcome) -> Told {
    Told {
        reachable,
        status,
        asked: Cell::new(0),
    }
}

fn connect_to(arguments: Value, runtime: &Told) -> Value {
    let args: ConnectArgs = serde_json::from_value(arguments).expect("the arguments");
    connect(&args, runtime)
}

fn refused(auth: Option<AuthFailure>, code: Option<&str>) -> Outcome {
    Outcome::Refused {
        error: "refused".to_owned(),
        code: code.map(str::to_owned),
        auth,
        result: None,
    }
}

/// Without an endpoint nothing is contacted: a workspace runtime that trust-runtime reports as not
/// running is a stale runtime with trust-runtime's reason, any other target has no endpoint set.
#[test]
fn a_connect_without_an_endpoint() {
    let runtime = told(true, Outcome::Result(json!({})));
    let unset = connect_to(json!({ "endpoint": " " }), &runtime);
    assert_eq!(
        unset["failure"],
        json!({ "kind": "failed_spawn", "message": "Runtime endpoint not set." })
    );
    let stale = connect_to(json!({ "problem": "runtime is not running" }), &runtime);
    assert_eq!(
        stale["failure"],
        json!({ "kind": "stale_runtime", "message": "runtime is not running" })
    );
    assert_eq!(runtime.asked.get(), 0);
}

/// A runtime that accepts no connection is stale, its message naming the address, or the stopped
/// local runtime for a Unix socket; its failure belongs to its endpoint, so that its node shows it,
/// and its remedy is Devices & Connections (specification 25 §5.1). Catches a raw `unix://` path in
/// the message, a failure without its endpoint, and an unreachable runtime offered no remedy.
#[test]
fn a_runtime_that_accepts_no_connection() {
    let runtime = told(false, Outcome::Result(json!({})));
    let remote = connect_to(json!({ "endpoint": "tcp://10.0.0.2:9901" }), &runtime);
    assert_eq!(remote["outcome"], "failed");
    assert_eq!(
        remote["failure"],
        json!({
            "kind": "stale_runtime",
            "message": "Runtime is not reachable at 10.0.0.2:9901.",
            "detail": "tcp://10.0.0.2:9901",
            "endpoint": "tcp://10.0.0.2:9901",
            "remedy": "openDevices",
        })
    );
    let local = connect_to(json!({ "endpoint": "unix:///tmp/r.sock" }), &runtime);
    assert_eq!(
        local["failure"]["message"],
        "Local runtime is stopped. Start it to connect."
    );
    assert_eq!(local["failure"]["remedy"], "openDevices");
    assert_eq!(runtime.asked.get(), 0);
}

/// A credential refusal is a permission failure that says what to do: no token sent or a missing
/// one asks for a token, a rejected one asks to check it (specification 25 §6.4), and its remedy is
/// a token stored for the endpoint, told by the refusal's code (§5.1). Catches a rejected token
/// reported as missing, and a refused token offered no Set auth token.
#[test]
fn credential_refusals() {
    let endpoint = "tcp://10.0.0.2:9901";
    let missing = told(
        true,
        refused(Some(AuthFailure::Missing), Some("missing_auth_token")),
    );
    let answer = connect_to(json!({ "endpoint": endpoint, "authToken": "t" }), &missing);
    assert_eq!(answer["failure"]["kind"], "workspace_permission");
    assert_eq!(
        answer["failure"]["message"],
        "No auth token provided — this runtime requires one."
    );
    assert_eq!(answer["failure"]["remedy"], "setAuthToken");
    let rejected = told(
        true,
        refused(Some(AuthFailure::Rejected), Some("invalid_auth_token")),
    );
    let answer = connect_to(json!({ "endpoint": endpoint, "authToken": "t" }), &rejected);
    assert_eq!(
        answer["failure"]["message"],
        "Auth token rejected — check it and try again."
    );
    assert_eq!(answer["failure"]["remedy"], "setAuthToken");
    let tokenless = connect_to(json!({ "endpoint": endpoint }), &rejected);
    assert_eq!(
        tokenless["failure"]["message"],
        "No auth token provided — this runtime requires one."
    );
    assert_eq!(tokenless["failure"]["endpoint"], endpoint);
    assert_eq!(tokenless["failure"]["remedy"], "setAuthToken");
}

/// Any other failed `status` is a stale runtime that names the failure. A runtime that refuses the
/// request, or whose answer does not read, did answer, so it has no remedy; one that does not answer
/// is not reachable, so its remedy is Devices & Connections (specification 25 §5.1). Catches a
/// refusal without its code in the message, a refusal offered Devices & Connections as if the
/// runtime were not reachable, and a request that timed out offered nothing.
#[test]
fn a_failed_status() {
    let endpoint = "tcp://10.0.0.2:9901";
    let answer = connect_to(
        json!({ "endpoint": endpoint }),
        &told(true, refused(None, Some("busy"))),
    );
    assert_eq!(
        answer["failure"],
        json!({
            "kind": "stale_runtime",
            "message": "Runtime status check failed: busy: refused",
            "endpoint": endpoint,
        })
    );
    let broken = connect_to(
        json!({ "endpoint": endpoint }),
        &told(
            true,
            Outcome::Transport("control request timeout".to_owned()),
        ),
    );
    assert_eq!(
        broken["failure"]["message"],
        "Runtime status check failed: control request timeout"
    );
    assert_eq!(broken["failure"]["remedy"], "openDevices");
    let unreadable = connect_to(
        json!({ "endpoint": endpoint }),
        &told(true, Outcome::Malformed("expected value".to_owned())),
    );
    assert_eq!(
        unreadable["failure"],
        json!({
            "kind": "stale_runtime",
            "message": "Runtime status check failed: expected value",
            "endpoint": endpoint,
        })
    );
}

/// A runtime that answers but keeps debugging disabled, at the top of its status or in its
/// `control_status`, is the target without a session; one that allows debugging is attached under
/// the target's label, else its address. Catches a debug-disabled runtime reported as a failed
/// Connect, which leaves the previous target selected.
#[test]
fn debugging_disabled_and_attach() {
    let endpoint = "tcp://10.0.0.2:9901";
    for status in [
        json!({ "debug_enabled": false }),
        json!({ "control_status": { "debug_enabled": false } }),
    ] {
        let answer = connect_to(
            json!({ "endpoint": endpoint }),
            &told(true, Outcome::Result(status)),
        );
        assert_eq!(answer["outcome"], "debugDisabled");
        assert_eq!(
            answer["failure"],
            json!({ "kind": "debug_disabled", "message": DEBUG_DISABLED, "endpoint": endpoint })
        );
    }
    let attach = connect_to(
        json!({ "endpoint": endpoint, "targetLabel": "Line 1" }),
        &told(true, Outcome::Result(json!({ "debug_enabled": true }))),
    );
    assert_eq!(
        attach,
        json!({ "ok": true, "outcome": "attach", "sessionName": "truST Remote (Line 1)" })
    );
    let unnamed = connect_to(
        json!({ "endpoint": endpoint }),
        &told(true, Outcome::Result(json!({}))),
    );
    assert_eq!(unnamed["sessionName"], "truST Remote (10.0.0.2:9901)");
}

/// The kind of a lifecycle failure from its text, with the extension's message. Catches a missing
/// executable reported as a failed start without the hint to install the tools.
#[test]
fn a_failure_kind_from_its_text() {
    for (detail, kind, message) in [
        (
            "spawn trust-runtime ENOENT",
            "missing_binary",
            "Required runtime/debug binary was not found.",
        ),
        (
            "EADDRINUSE: address already in use",
            "port_conflict",
            "The runtime port is already in use.",
        ),
        (
            "EACCES: permission denied",
            "workspace_permission",
            "The workspace or runtime path is not writable.",
        ),
        (
            "debug session timed out",
            "stale_runtime",
            "A stale runtime or debug session blocked startup.",
        ),
        (
            "something else broke",
            "failed_spawn",
            "something else broke",
        ),
        ("", "failed_spawn", "Runtime failed to start."),
    ] {
        let failure =
            answer("failure", json!({ "detail": detail })).expect("an answer")["failure"].clone();
        assert_eq!(failure["kind"], kind, "{detail}");
        assert_eq!(failure["message"], message, "{detail}");
        assert_eq!(failure["detail"], detail);
    }
}

/// The failure of Live Values' I/O state request, `failure` with `request: "ioState"`.
fn io_state_failure(detail: Option<&str>) -> Value {
    let mut argument = json!({ "request": "ioState" });
    if let Some(detail) = detail {
        argument["detail"] = json!(detail);
    }
    answer("failure", argument).expect("an answer")["failure"].clone()
}

/// No session to ask, or a session that is gone, is `no_session` (specification 25 §4.4): VS
/// Code's `Canceled` (one `l`) for the requests of a session that ends, `No debugger available`,
/// `Can not send 'stIoState'` however quoted and spaced, the extension's own text, and the debug
/// adapter's answer without a runtime. Catches the request of a session that ended told as a lost
/// connection, and the adapter's raw no-session answer shown to the user.
#[test]
fn an_io_state_request_without_a_session() {
    assert_eq!(
        io_state_failure(None),
        json!({ "kind": "no_session", "message": "No active Structured Text debug session." })
    );
    for detail in [
        "Canceled",
        "canceled",
        "No debugger available found. Can not send 'stIoState'.",
        "can  not send \"stIoState\"",
        "No active Structured Text debug session.",
        "no program runs: launch or attach first",
    ] {
        assert_eq!(
            io_state_failure(Some(detail)),
            json!({
                "kind": "no_session",
                "message": "No active Structured Text debug session.",
                "detail": detail,
            }),
            "{detail}"
        );
    }
}

/// A connection that failed under the request is `connection_lost`, with what Live Values says
/// (specification 25 §4.4): a timeout, a socket, a reset, refused or broken connection, and a
/// connection `cancelled` (two `l`), closed, terminated or not connected. Catches a timed-out read
/// told as a start that a stale runtime blocked, which kept the old values on screen.
#[test]
fn an_io_state_request_whose_connection_failed() {
    for detail in [
        "I/O state request timed out.",
        "socket hang up",
        "read ECONNRESET",
        "connect ECONNREFUSED 127.0.0.1:18085",
        "write EPIPE",
        "Request cancelled",
        "runtime connection closed",
        "debug session terminated",
        "attach session is not connected",
        "runtime disconnected",
        "control request timeout",
    ] {
        assert_eq!(
            io_state_failure(Some(detail)),
            json!({
                "kind": "connection_lost",
                "message": "Live Values lost connection to the runtime. Restart or reconnect the runtime, then retry.",
                "detail": detail,
            }),
            "{detail}"
        );
    }
}

/// Any other failure is `request_failed` and names its own text (specification 25 §4.4); a request
/// other than the I/O state's is refused. Catches a refused read told that the workspace is not
/// writable, and a missing runtime binary named for a read that never starts one.
#[test]
fn an_io_state_request_that_failed_otherwise() {
    for (detail, message) in [
        (
            "permission denied",
            "I/O state request failed: permission denied",
        ),
        (
            "  trust-runtime answered an unknown I/O address  ",
            "I/O state request failed: trust-runtime answered an unknown I/O address",
        ),
        ("", "I/O state request failed."),
    ] {
        assert_eq!(
            io_state_failure(Some(detail)),
            json!({ "kind": "request_failed", "message": message, "detail": detail }),
            "{detail}"
        );
    }
    assert_eq!(
        answer("failure", json!({ "request": "stIoWrite", "detail": "x" })),
        Err("A failure is read for a start or for the request ioState, not stIoWrite.".to_owned())
    );
}
