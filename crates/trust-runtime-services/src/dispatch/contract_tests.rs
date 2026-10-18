//! Dispatcher contracts: the refusal codes and texts, the debug-surface gate, the platform
//! fallthrough and the audit records, exactly as trust-runtime answers today. The access
//! capability tests moved from trust-runtime (`control/tests/control_request_boundary_contract.rs`)
//! with the pipeline.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::value::Duration;
use trust_runtime_core::value::Value as RuntimeValue;

use super::*;
use crate::live_values::io_requests::IoSnapshotRead;
use crate::live_values::io_snapshot::IoSnapshot;
use crate::live_values::queues::{ForcedVarTarget, PendingVarTarget};
use crate::tests::fake_host::{FakeHost, StoredAudit, WALL_TIME_MS};

const NETWORK: Option<&str> = Some("127.0.0.1:9000");
const UNIX: Option<&str> = Some("unix");
const REMOTE: Option<&str> = Some("192.0.2.1:9000");

fn wire(response: &ControlResponse) -> String {
    serde_json::to_string(response).expect("serialize response")
}

fn only_audit(host: &FakeHost) -> StoredAudit {
    let audits = host.audits();
    assert_eq!(audits.len(), 1, "one audit record per request: {audits:?}");
    audits[0].clone()
}

#[test]
fn access_capabilities_are_exact_for_each_role() {
    for (role, mutation_allowed, reason) in [
        (
            AccessRole::Viewer,
            false,
            Some("Viewer role — connect with an engineer token to write or force."),
        ),
        (
            AccessRole::Operator,
            false,
            Some("Operator role — connect with an engineer token to write or force."),
        ),
        (AccessRole::Engineer, true, None),
        (AccessRole::Admin, true, None),
    ] {
        let access = access_capabilities_json(role);
        assert_eq!(access["role"], json!(role.as_str()));
        assert_eq!(access["io"]["write"], json!(mutation_allowed));
        assert_eq!(access["io"]["force"], json!(mutation_allowed));
        assert_eq!(access["io"]["release"], json!(mutation_allowed));
        assert_eq!(access["hmi"]["write"], json!(mutation_allowed));
        assert_eq!(
            access["reason"],
            reason.map_or(Value::Null, |text| json!(text))
        );
    }
}

#[test]
fn capability_reason_capitalization_handles_empty_ascii_and_unicode() {
    assert_eq!(capitalize_role(""), "");
    assert_eq!(capitalize_role("viewer"), "Viewer");
    assert_eq!(capitalize_role("operator"), "Operator");
    assert_eq!(capitalize_role("åskådare"), "Åskådare");
}

#[test]
fn attaching_access_mutates_only_object_results() {
    let mut object = ControlResponse::ok(1, json!({"status": "running"}));
    attach_access_capabilities(&mut object, AccessRole::Viewer);
    assert_eq!(
        object
            .result
            .as_ref()
            .and_then(|value| value.get("access"))
            .and_then(|value| value.get("role")),
        Some(&json!("viewer"))
    );

    let mut array = ControlResponse::ok(2, json!([]));
    attach_access_capabilities(&mut array, AccessRole::Admin);
    assert_eq!(array.result, Some(json!([])));

    let mut scalar = ControlResponse::ok(3, json!("running"));
    attach_access_capabilities(&mut scalar, AccessRole::Admin);
    assert_eq!(scalar.result, Some(json!("running")));

    let mut error = ControlResponse::error(4, "failed".into());
    attach_access_capabilities(&mut error, AccessRole::Admin);
    assert!(error.result.is_none());
}

#[test]
fn unknown_operation_is_unsupported_for_admin_and_forbidden_below_admin() {
    let host = FakeHost::running();
    let response = dispatch_value(json!({"id": 6, "type": "future.operation"}), &host, UNIX);
    assert_eq!(
        wire(&response),
        r#"{"id":6,"ok":false,"error":"unsupported request","audit_id":"audit-1"}"#
    );
    assert_eq!(*host.extension_calls.borrow(), vec!["future.operation"]);

    let host = FakeHost::running();
    let response = dispatch_value(json!({"id": 7, "type": "future.operation"}), &host, NETWORK);
    assert_eq!(
        wire(&response),
        r#"{"id":7,"ok":false,"error":"forbidden: requires role admin","error_code":"insufficient_role","audit_id":"audit-1"}"#
    );
    assert!(
        host.extension_calls.borrow().is_empty(),
        "a refused request never reaches the platform"
    );
}

#[test]
fn missing_and_invalid_tokens_are_refused_with_their_codes() {
    for (auth, error, code, auth_present) in [
        (None, "missing auth token", "missing_auth_token", false),
        (
            Some("wrong"),
            "invalid auth token",
            "invalid_auth_token",
            true,
        ),
    ] {
        let mut host = FakeHost::running();
        host.control_token = Some("secret");
        host.requires_auth = true;
        let mut request = json!({"id": 3, "type": "status"});
        if let Some(auth) = auth {
            request["auth"] = json!(auth);
        }
        let response = dispatch_value(request, &host, UNIX);
        assert!(!response.ok);
        assert_eq!(response.error.as_deref(), Some(error));
        assert_eq!(response.error_code.as_deref(), Some(code));
        assert!(response.result.is_none(), "status is not answered");
        let audit = only_audit(&host);
        assert_eq!(audit.error.as_deref(), Some(error));
        assert_eq!(audit.auth_present, auth_present);
        assert!(!audit.ok);
    }
}

#[test]
fn role_refusal_names_the_required_role() {
    for (kind, required) in [
        ("pause", "operator"),
        ("restart", "operator"),
        ("io.write", "engineer"),
        ("shutdown", "admin"),
        ("bytecode.reload", "admin"),
        ("program.install", "admin"),
    ] {
        let host = FakeHost::running();
        let response = dispatch_value(json!({"id": 1, "type": kind}), &host, NETWORK);
        assert_eq!(
            response.error.as_deref(),
            Some(format!("forbidden: requires role {required}").as_str()),
            "kind={kind}"
        );
        assert_eq!(response.error_code.as_deref(), Some("insufficient_role"));
        assert!(host.actions().is_empty(), "kind={kind}: nothing happened");
        assert!(
            host.pending_restart.borrow().is_none(),
            "kind={kind}: no restart queued"
        );
    }
}

#[test]
fn pairing_token_acts_with_its_stored_role() {
    let mut host = FakeHost::running();
    host.control_token = Some("secret");
    host.requires_auth = true;
    host.pairing = Some(("paired", AccessRole::Operator));

    let paused = dispatch_value(
        json!({"id": 1, "type": "pause", "auth": "paired"}),
        &host,
        NETWORK,
    );
    assert_eq!(paused.result, Some(json!({"status": "paused"})));

    let refused = dispatch_value(
        json!({"id": 2, "type": "shutdown", "auth": "paired"}),
        &host,
        NETWORK,
    );
    assert_eq!(
        refused.error.as_deref(),
        Some("forbidden: requires role admin")
    );
    assert_eq!(
        refused.after_response, None,
        "a refused shutdown stops nothing"
    );
}

#[test]
fn debug_surface_is_refused_while_debugging_is_disabled() {
    for kind in ["debug.state", "pause", "var.force", "breakpoints.set"] {
        let mut host = FakeHost::running();
        host.debug_enabled = false;
        let response = dispatch_value(json!({"id": 7702, "type": kind}), &host, UNIX);
        assert_eq!(
            wire(&response),
            r#"{"id":7702,"ok":false,"error":"debug disabled","error_code":"feature_disabled","audit_id":"audit-1"}"#,
            "kind={kind}"
        );
        assert_eq!(
            Vec::from(host.events.borrow().clone()),
            vec![RuntimeEvent::FeatureDisabled {
                feature: "debug".into(),
                request_type: Some(kind.into()),
                time: Some(Duration::from_millis(1_792_108_800_000)),
            }],
            "kind={kind}"
        );
        assert!(
            host.actions().is_empty(),
            "kind={kind}: the scan is untouched"
        );
        assert!(host.extension_calls.borrow().is_empty(), "kind={kind}");
        assert_eq!(only_audit(&host).error.as_deref(), Some("debug disabled"));
    }

    let mut host = FakeHost::running();
    host.debug_enabled = false;
    let forced = dispatch_value(
        json!({"id": 1, "type": "io.force", "params": {"address": "%QX0.0", "value": "TRUE"}}),
        &host,
        UNIX,
    );
    assert_eq!(
        forced.result,
        Some(json!({"status": "forced"})),
        "io.force is answered by the shared dispatcher"
    );
    assert!(
        host.events.borrow().is_empty(),
        "io.force is not debug surface"
    );
    assert!(
        host.extension_calls.borrow().is_empty(),
        "io.force no longer falls through to the platform"
    );
}

// The Live Values operations the shared dispatcher answers from the host's ports: none of them
// reaches the platform's extension dispatch, and each lands in the state the host owns.

#[test]
fn io_list_and_io_read_answer_from_the_hosts_snapshot_slot() {
    let host = FakeHost::running();
    let listed = dispatch_value(json!({"id": 1, "type": "io.list"}), &host, UNIX);
    assert_eq!(listed.error.as_deref(), Some("no snapshot available"));
    let read = dispatch_value(json!({"id": 2, "type": "io.read"}), &host, UNIX);
    assert_eq!(read.result, Some(json!({"snapshot": null})));

    *host.io_snapshot.borrow_mut() = IoSnapshotRead::Published(IoSnapshot {
        scan: Some(9),
        ..IoSnapshot::default()
    });
    let listed = dispatch_value(json!({"id": 3, "type": "io.list"}), &host, UNIX);
    assert_eq!(
        listed.result,
        Some(json!({"scan": 9, "inputs": [], "outputs": [], "memory": []})),
        "io.list answers the host's published snapshot"
    );

    assert!(
        host.extension_calls.borrow().is_empty(),
        "io.list and io.read are the shared dispatcher's"
    );
}

#[test]
fn io_write_and_io_force_queue_in_the_hosts_io_queues() {
    let host = FakeHost::running();
    let written = dispatch_value(
        json!({"id": 1, "type": "io.write", "params": {"address": "%QX0.0", "value": "TRUE"}}),
        &host,
        UNIX,
    );
    assert_eq!(written.result, Some(json!({"status": "queued"})));
    let forced = dispatch_value(
        json!({"id": 2, "type": "io.force", "params": {"address": "%QW0", "value": "-3"}}),
        &host,
        UNIX,
    );
    assert_eq!(forced.result, Some(json!({"status": "forced"})));
    assert!(
        host.extension_calls.borrow().is_empty(),
        "io.write and io.force are the shared dispatcher's"
    );

    let queues = host.live_values.borrow();
    assert_eq!(
        queues.io_writes,
        vec![(
            IoAddress::parse("%QX0.0").expect("test address"),
            RuntimeValue::Bool(true)
        )]
    );
    assert_eq!(
        queues.forced_io,
        vec![(
            IoAddress::parse("%QW0").expect("test address"),
            RuntimeValue::LInt(-3)
        )]
    );
}

#[test]
fn set_and_var_force_queue_in_the_hosts_variable_queues() {
    let host = FakeHost::running();
    let accepted = dispatch_value(
        json!({"id": 1, "type": "set", "params": {"target": "global:kept", "value": "7"}}),
        &host,
        UNIX,
    );
    assert_eq!(accepted.result, Some(json!({"status": "queued"})));
    let forced = dispatch_value(
        json!({"id": 2, "type": "var.force", "params": {"target": "instance:7:c", "value": "TRUE"}}),
        &host,
        UNIX,
    );
    assert_eq!(forced.result, Some(json!({"status": "forced"})));
    assert!(
        host.extension_calls.borrow().is_empty(),
        "set and var.force are the shared dispatcher's"
    );

    let queues = host.live_values.borrow();
    assert_eq!(queues.pending_var_writes.len(), 1);
    assert_eq!(
        queues.pending_var_writes[0].target,
        PendingVarTarget::Global(SmolStr::new("kept"))
    );
    assert_eq!(queues.pending_var_writes[0].value, RuntimeValue::LInt(7));
    assert_eq!(queues.forced_vars.len(), 1);
    assert_eq!(
        queues.forced_vars[0].target,
        ForcedVarTarget::Instance(Generation::FIRST, InstanceId(7), SmolStr::new("c")),
        "a bare instance identity belongs to the generation the host reports"
    );
    assert_eq!(queues.forced_vars[0].value, RuntimeValue::Bool(true));
}

#[test]
fn var_forced_projects_and_var_unforce_releases_the_hosts_forces() {
    let host = FakeHost::running();
    let forced = dispatch_value(
        json!({"id": 1, "type": "var.force", "params": {"target": "retain:kept", "value": "-2"}}),
        &host,
        UNIX,
    );
    assert_eq!(forced.result, Some(json!({"status": "forced"})));

    let projected = dispatch_value(json!({"id": 2, "type": "var.forced"}), &host, UNIX);
    assert_eq!(
        projected.result,
        Some(json!({"vars": [{"target": "retain:kept", "value": "-2"}]}))
    );

    let released = dispatch_value(
        json!({"id": 3, "type": "var.unforce", "params": {"target": "retain:kept"}}),
        &host,
        UNIX,
    );
    assert_eq!(released.result, Some(json!({"status": "released"})));
    let projected = dispatch_value(json!({"id": 4, "type": "var.forced"}), &host, UNIX);
    assert_eq!(projected.result, Some(json!({"vars": []})));
    assert!(
        host.extension_calls.borrow().is_empty(),
        "var.forced and var.unforce are the shared dispatcher's"
    );
}

#[test]
fn a_disabled_feature_event_has_no_time_while_the_wall_clock_is_not_set() {
    // Spec 34 TARGET-06: before its clock is set a device reports `null`, never a made-up 0.
    let mut host = FakeHost::running();
    host.debug_enabled = false;
    host.utc_ms = None;
    let response = dispatch_value(json!({"id": 7703, "type": "debug.state"}), &host, UNIX);
    assert_eq!(response.error_code.as_deref(), Some("feature_disabled"));
    let events = Vec::from(host.events.borrow().clone());
    assert_eq!(
        events,
        vec![RuntimeEvent::FeatureDisabled {
            feature: "debug".into(),
            request_type: Some("debug.state".into()),
            time: None,
        }]
    );
    let projected = crate::events::runtime_event_to_json(events[0].clone());
    assert_eq!(projected["time_ns"], Value::Null);

    // A set clock keeps its exact millisecond, as trust-runtime reports it today.
    let mut host = FakeHost::running();
    host.debug_enabled = false;
    let _ = dispatch_value(json!({"id": 7704, "type": "debug.state"}), &host, UNIX);
    let projected = crate::events::runtime_event_to_json(host.events.borrow()[0].clone());
    assert_eq!(
        projected["time_ns"],
        json!(i64::try_from(WALL_TIME_MS).expect("the fake wall time fits") * 1_000_000)
    );
}

#[test]
fn platform_operations_fall_through_to_the_extension() {
    let host = FakeHost::running();
    let response = dispatch_value(json!({"id": 9, "type": "config.get"}), &host, UNIX);
    assert_eq!(
        wire(&response),
        r#"{"id":9,"ok":true,"result":{"answered_by":"platform"},"audit_id":"audit-1"}"#
    );
    let audit = only_audit(&host);
    assert!(audit.ok);
    assert_eq!(audit.error, None);

    let shared = FakeHost::running();
    let _ = dispatch_value(json!({"id": 10, "type": "health"}), &shared, UNIX);
    assert!(
        shared.extension_calls.borrow().is_empty(),
        "shared operations are answered without the platform"
    );
}

#[test]
fn audit_record_carries_the_request_identity() {
    let host = FakeHost::running();
    let response = dispatch_value(
        json!({
            "id": 5,
            "type": "comm.test",
            "auth": "token",
            "correlation_id": "c-1",
            "params": {"protocol": "mqtt"},
        }),
        &host,
        REMOTE,
    );
    assert_eq!(
        response.error.as_deref(),
        Some("forbidden: requires role engineer")
    );
    let audit = only_audit(&host);
    assert_eq!(audit.request_id, 5);
    assert_eq!(audit.request_type, "comm.test");
    assert_eq!(audit.correlation_id.as_deref(), Some("c-1"));
    assert!(!audit.ok);
    assert_eq!(
        audit.error.as_deref(),
        Some("forbidden: requires role engineer")
    );
    assert!(audit.auth_present);
    assert_eq!(audit.client.as_deref(), Some("192.0.2.1:9000"));
    assert_eq!(
        audit.details,
        Some(json!({"params": {
            "protocol": "mqtt",
            "credential_channel": "untrusted_remote_plain_tcp",
        }})),
        "the credential channel is stamped before the role and the audit"
    );
}

#[test]
fn invalid_request_value_gets_id_zero_and_an_invalid_audit_record() {
    for value in [
        Value::Null,
        json!([]),
        json!({}),
        json!({"id": "one", "type": "status"}),
        json!({"id": 1, "type": null}),
        json!({"id": 1, "type": "status", "auth": 7}),
    ] {
        let host = FakeHost::running();
        let response = dispatch_value(value.clone(), &host, UNIX);
        assert!(!response.ok, "value={value}");
        let error = response.error.clone().expect("error text");
        assert!(error.starts_with("invalid request:"), "value={value}");
        assert_eq!(response.error_code, None);
        let encoded = serde_json::to_value(&response).expect("encode");
        assert_eq!(encoded["id"], json!(0));
        assert_eq!(encoded["audit_id"], json!("audit-1"));
        let audit = only_audit(&host);
        assert_eq!(audit.request_id, 0);
        assert_eq!(audit.request_type, "invalid");
        assert_eq!(audit.error.as_deref(), Some(error.as_str()));
        assert!(!audit.auth_present);
        assert_eq!(audit.client.as_deref(), Some("unix"));
        assert_eq!(audit.details, None);
    }
}

#[test]
fn request_lines_always_return_one_serialized_response() {
    let host = FakeHost::running();
    for line in ["", "{", "not json", "[] trailing"] {
        let prepared = prepare_line(line, &host, UNIX);
        let value: Value = serde_json::from_str(&prepared.response).expect("response JSON");
        assert_eq!(value["id"], json!(0), "line={line:?}");
        assert_eq!(value["ok"], json!(false), "line={line:?}");
        assert!(
            value["error"]
                .as_str()
                .is_some_and(|error| error.starts_with("invalid request:")),
            "line={line:?}"
        );
        assert_eq!(prepared.after_response, None);
    }
    assert!(
        host.audits().is_empty(),
        "unreadable lines write no audit record"
    );

    let prepared = prepare_line(r#"{"id":4,"type":"resume"}"#, &host, UNIX);
    assert_eq!(
        prepared.response,
        r#"{"id":4,"ok":true,"result":{"status":"running"},"audit_id":"audit-1"}"#
    );
}

#[test]
fn a_reply_longer_than_the_link_moves_is_replaced_by_an_error_and_never_cut() {
    let host = FakeHost::running();
    let line = r#"{"id":4,"type":"resume"}"#;
    let reply = r#"{"id":4,"ok":true,"result":{"status":"running"},"audit_id":"audit-1"}"#;
    let fits = prepare_line_within(line, &host, UNIX, reply.len());
    assert_eq!(
        fits.response, reply,
        "a reply of exactly the bound is sent as it is"
    );

    let host = FakeHost::running();
    let bound = reply.len() - 1;
    let replaced = prepare_line_within(line, &host, UNIX, bound);
    assert_eq!(
        replaced.response,
        format!(
            r#"{{"id":4,"ok":false,"error":"reply longer than {bound} bytes","audit_id":"audit-1"}}"#
        ),
        "the reply keeps its request's id and audit record"
    );
    assert!(
        only_audit(&host).ok,
        "the request was served; only its reply could not be sent"
    );

    let host = FakeHost::running();
    let stop = prepare_line_within(r#"{"id":41,"type":"shutdown"}"#, &host, UNIX, 10);
    assert_eq!(
        stop.after_response,
        Some(ControlAfterResponse::StopResource),
        "the work after the reply stays"
    );
}

#[test]
fn shutdown_replies_first_and_defers_the_stop() {
    let host = FakeHost::running();
    let prepared = prepare_line(r#"{"id":41,"type":"shutdown"}"#, &host, UNIX);
    assert_eq!(
        prepared.response,
        r#"{"id":41,"ok":true,"result":{"status":"stopping"},"audit_id":"audit-1"}"#
    );
    assert_eq!(
        prepared.after_response,
        Some(ControlAfterResponse::StopResource)
    );
    let response = dispatch_value(json!({"id": 42, "type": "shutdown"}), &host, UNIX);
    assert_eq!(
        response.after_response,
        Some(ControlAfterResponse::StopResource)
    );
}

#[test]
fn status_carries_the_access_capabilities_of_the_request_role() {
    let host = FakeHost::running();
    let viewer = dispatch_value(json!({"id": 1, "type": "status"}), &host, NETWORK);
    let viewer = viewer.result.expect("status result");
    let access = &viewer["access"];
    assert_eq!(access["role"], json!("viewer"));
    assert_eq!(
        access["reason"],
        json!("Viewer role — connect with an engineer token to write or force.")
    );

    let admin = dispatch_value(json!({"id": 2, "type": "status"}), &host, UNIX);
    let admin = admin.result.expect("status result");
    let access = &admin["access"];
    assert_eq!(access["role"], json!("admin"));
    assert_eq!(access["io"]["force"], json!(true));

    let mut unreadable = FakeHost::running();
    unreadable.unreadable = vec!["settings"];
    let failed = dispatch_value(json!({"id": 3, "type": "status"}), &unreadable, UNIX);
    assert_eq!(
        wire(&failed),
        r#"{"id":3,"ok":false,"error":"runtime settings unavailable","audit_id":"audit-1"}"#,
        "a failed status carries no access block"
    );
}

#[test]
fn shared_resource_operations_are_routed_with_their_parameters() {
    let host = FakeHost::running();
    let restart = dispatch_value(
        json!({"id": 1, "type": "restart", "params": {"mode": "COLD"}}),
        &host,
        UNIX,
    );
    assert_eq!(restart.result, Some(json!({"status": "restart queued"})));
    assert_eq!(
        *host.pending_restart.borrow(),
        Some(trust_runtime_core::retain::RestartMode::Cold)
    );

    host.events.borrow_mut().extend([
        RuntimeEvent::fault(
            &RuntimeError::SimulationFault("first".into()),
            Duration::ZERO,
        ),
        RuntimeEvent::fault(
            &RuntimeError::SimulationFault("second".into()),
            Duration::ZERO,
        ),
    ]);
    let faults = dispatch_value(
        json!({"id": 2, "type": "faults", "params": {"limit": 1}}),
        &host,
        UNIX,
    );
    assert_eq!(
        faults.result,
        Some(
            json!({"faults": [{"type": "fault", "error": "simulation fault 'second'", "time_ns": 0}]})
        )
    );
    let events = dispatch_value(
        json!({"id": 3, "type": "events", "params": {"limit": 0}}),
        &host,
        UNIX,
    );
    assert_eq!(
        events.error.as_deref(),
        Some("invalid params: limit is outside the supported range")
    );
}
