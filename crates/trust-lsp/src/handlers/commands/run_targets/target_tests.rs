//! The status of the configured run target (specification 25 §5.1), with the probe's answers
//! given, and once against a fake runtime.

use serde_json::{json, Value};
use trust_runtime_client::{AuthFailure, TargetProbe};

use super::super::fake_runtime::FakeRuntime;
use super::{answer, status, Args};

fn args(value: Value) -> Args {
    serde_json::from_value(value).expect("the settings")
}

fn probed(settings: Value, found: TargetProbe) -> Value {
    status(&args(settings), move |_, _, _| found)
}

/// The Simulator is never contacted, and its credential channel is still the configured
/// endpoint's. Catches a probe of a remote endpoint while the user simulates.
#[test]
fn the_simulator_is_never_probed() {
    let target = status(
        &args(json!({ "mode": "simulate", "endpoint": "tcp://192.168.10.20:9901" })),
        |_, _, _| panic!("the simulator must not be probed"),
    );
    assert_eq!(target["status"], "simulate");
    assert_eq!(target["reachable"], false);
    assert_eq!(target["label"], "Simulator");
    assert_eq!(target["credentialChannel"], "untrusted_remote_plain_tcp");
}

/// An online target without an endpoint, or with its endpoint turned off, has a missing endpoint
/// and no credential channel; one that trust-runtime reports as not running is unreachable.
/// Catches a disabled endpoint that is still contacted, and a not-running workspace runtime shown
/// as unset.
#[test]
fn missing_disabled_and_not_running_endpoints() {
    let missing = probed(
        json!({ "mode": "online", "endpoint": "" }),
        TargetProbe::MissingEndpoint,
    );
    assert_eq!(missing["status"], "missing_endpoint");
    assert_eq!(missing["credentialChannel"], "unavailable");
    assert_eq!(missing["label"], "Online runtime");
    let disabled = status(
        &args(
            json!({ "mode": "online", "endpoint": "tcp://127.0.0.1:9901", "endpointEnabled": false }),
        ),
        |endpoint, enabled, _| {
            assert!(!enabled && endpoint.is_empty());
            TargetProbe::MissingEndpoint
        },
    );
    assert_eq!(disabled["status"], "missing_endpoint");
    assert_eq!(disabled["endpoint"], "tcp://127.0.0.1:9901");
    assert_eq!(disabled["credentialChannel"], "unavailable");
    let not_running = probed(
        json!({ "mode": "online", "unavailableReason": "runtime is not running: no endpoint file" }),
        TargetProbe::MissingEndpoint,
    );
    assert_eq!(not_running["status"], "online_unreachable");
    assert_eq!(not_running["reachable"], false);
}

/// A runtime that answers is reachable; one that does not is unreachable; one that refuses the
/// token failed on its credentials and is reachable, the kind named. The label is the endpoint.
/// Catches an authentication failure shown as unreachable, which hides the token prompt.
#[test]
fn reachable_unreachable_and_credential_failures() {
    let settings =
        json!({ "mode": "online", "endpoint": "tcp://127.0.0.1:9901", "authToken": "t" });
    let reachable = probed(settings.clone(), TargetProbe::Reachable(json!({})));
    assert_eq!(reachable["status"], "online_reachable");
    assert_eq!(reachable["reachable"], true);
    assert_eq!(reachable["label"], "tcp://127.0.0.1:9901");
    assert_eq!(reachable["credentialChannel"], "trusted_same_host");
    let unreachable = probed(settings.clone(), TargetProbe::Unreachable);
    assert_eq!(unreachable["status"], "online_unreachable");
    assert_eq!(unreachable["reachable"], false);
    for (failure, kind) in [
        (AuthFailure::Rejected, "rejected"),
        (AuthFailure::Missing, "missing"),
    ] {
        let target = probed(settings.clone(), TargetProbe::AuthFailed(failure));
        assert_eq!(target["status"], "auth_failed");
        assert_eq!(target["authFailureKind"], kind);
        assert_eq!(target["reachable"], true);
    }
}

/// Against a runtime on the network the token reaches `status` and is never echoed. Catches a
/// token left out of the probe, and a token in the answer that the extension logs.
#[test]
fn the_probe_sends_the_token_and_never_echoes_it() {
    let runtime = FakeRuntime::answering(json!({ "state": "running" }));
    let answer = answer(json!({
        "action": "target",
        "mode": "online",
        "endpoint": runtime.endpoint,
        "authToken": "secret-token",
        "label": "Line 1",
    }))
    .expect("an answer");
    assert_eq!(answer["target"]["status"], "online_reachable");
    assert_eq!(answer["target"]["label"], "Line 1");
    assert_eq!(runtime.requests()[0]["auth"], "secret-token");
    assert!(!answer.to_string().contains("secret-token"));
}
