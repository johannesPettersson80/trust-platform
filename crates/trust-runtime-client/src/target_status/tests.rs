//! Ports the cases of `editors/vscode/src/test/suite/runtime-target.test.ts` with their
//! endpoints, token, texts and codes. A case that needs a runtime gets a scripted one on a local
//! port. Simulate mode is not a probe: the panel's targets decide it (B2). The credential channel
//! is classified by the runtime that receives the request (trust-runtime
//! `classify_comm_credential_channel`), not here.

use std::io::ErrorKind;
use std::net::TcpListener;
use std::time::Duration;

use serde_json::{json, Value};

use super::{probe, ProbeTimeouts, TargetProbe};
use crate::fake_runtime;
use crate::AuthFailure;

/// Probes a scripted runtime that answers `status` with `answer`, or never answers; returns the
/// probe and the requests the runtime received.
fn probe_runtime(answer: Option<&str>, token: Option<&str>) -> (TargetProbe, Vec<Value>) {
    let runtime = fake_runtime::answering(answer.into_iter().map(str::to_string).collect());
    let found = probe(&runtime.endpoint, true, token, ProbeTimeouts::default());
    (found, runtime.requests())
}

#[test]
fn probe_classifies_every_target_status() {
    assert_eq!(
        ProbeTimeouts::default(),
        ProbeTimeouts {
            connect: Duration::from_millis(400),
            status: Duration::from_millis(750),
        }
    );

    // "reports missing endpoint when online runtime has no active endpoint"
    let missing = probe("", true, None, ProbeTimeouts::default());
    assert_eq!(missing, TargetProbe::MissingEndpoint);
    assert!(!missing.answered());

    // "reports missing endpoint when configured endpoint is disabled"
    let disabled = probe(
        "tcp://127.0.0.1:9901",
        false,
        None,
        ProbeTimeouts::default(),
    );
    assert_eq!(disabled, TargetProbe::MissingEndpoint);
    assert!(!disabled.answered());
    // ... and a runtime listening there is never contacted.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("address"));
    assert_eq!(
        probe(&endpoint, false, Some("token"), ProbeTimeouts::default()),
        TargetProbe::MissingEndpoint
    );
    listener.set_nonblocking(true).expect("nonblocking");
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock),
        "a disabled endpoint is never contacted"
    );

    // "reports online reachable when probe and status succeed"
    let (reachable, seen) = probe_runtime(
        Some(r#"{"id":1,"ok":true,"result":{"state":"running"}}"#),
        Some("token"),
    );
    assert_eq!(
        reachable,
        TargetProbe::Reachable(json!({"state": "running"}))
    );
    assert!(reachable.answered());
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0]["type"], "status");
    assert_eq!(seen[0]["auth"], "token");

    // "reports online unreachable when endpoint probe fails": nothing listens there, or the
    // text is not a control endpoint.
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = format!("tcp://{}", closed.local_addr().expect("address"));
    drop(closed);
    let unreachable = probe(&endpoint, true, None, ProbeTimeouts::default());
    assert_eq!(unreachable, TargetProbe::Unreachable);
    assert!(!unreachable.answered());
    assert_eq!(
        probe("192.168.10.20:9901", true, None, ProbeTimeouts::default()),
        TargetProbe::Unreachable
    );

    // "reports online unreachable when status request fails after probe": no answer in time,
    // or an answer that is not a control response.
    for answer in [None, Some("control request timeout")] {
        let (failed, seen) = probe_runtime(answer, None);
        assert_eq!(failed, TargetProbe::Unreachable, "{answer:?}");
        assert!(!failed.answered());
        assert_eq!(seen.len(), 1, "status was sent");
    }

    // "reports auth failed when status request is rejected by auth"
    let (rejected, _) = probe_runtime(
        Some(
            r#"{"id":1,"ok":false,"error":"invalid auth token","error_code":"invalid_auth_token"}"#,
        ),
        Some("token"),
    );
    assert_eq!(rejected, TargetProbe::AuthFailed(AuthFailure::Rejected));
    assert!(rejected.answered());

    // "reports missing auth token as a distinct auth failure"
    let (missing_token, seen) = probe_runtime(
        Some(
            r#"{"id":1,"ok":false,"error":"missing auth token","error_code":"missing_auth_token"}"#,
        ),
        None,
    );
    assert_eq!(missing_token, TargetProbe::AuthFailed(AuthFailure::Missing));
    assert!(missing_token.answered());
    assert!(seen[0].get("auth").is_none(), "no token, no auth field");
}
