//! Port of `control-studio-experiment.test.ts`: the client-request assertions of tests 1, 5 and 12, and the client's
//! tests 21 and 22. The reviewed plans are written out with the values those tests assert (the plan builder is
//! trust-control's, tested there). The fake runtime answers as annex E5 names each answer.

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentPlan;
use trust_control::failure::NextAction;

use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};
use crate::runtime_client::RuntimeClient;

/// A reviewed plan (`ControlExperimentPlan`) from its extension-shaped JSON, with `overrides` applied.
fn plan(overrides: Value) -> ControlExperimentPlan {
    let mut value = json!({
        "id": "experiment:loop:1:project_sequence",
        "source": "simulation",
        "kind": "project_sequence",
        "affectedSignal": "P1.Run",
        "address": "%MX64.0",
        "valueType": "BOOL",
        "from": false,
        "to": true,
        "unit": "authority",
        "minimum": 0.0,
        "maximum": 1.0,
        "durationMs": 35100.0,
        "expectedScan": 1,
        "task": "$background",
        "stopConditions": ["Abort"],
        "restorationValue": false,
        "requiresPhysicalApproval": false,
        "trajectory": [{ "atMs": 0.0, "value": 0.0 }, { "atMs": 5000.0, "value": 1.0 }]
    });
    if let Value::Object(fields) = overrides {
        for (key, field) in fields {
            value[key.as_str()] = field;
        }
    }
    serde_json::from_value(value).expect("the plan decodes")
}

/// The runtime's start answer for the experiment the request names (E5.3): running, restoration pending.
#[allow(clippy::unnecessary_wraps)] // It is a handler of the fake runtime, which answers `Option<Value>`.
fn started(request: &Value) -> Option<Value> {
    let id = request["params"]["experiment_id"].clone();
    Some(answer(
        request,
        json!({ "experiment_id": id, "status": "running", "restoration": "pending" }),
    ))
}

/// The experiment's record as the runtime answers it (E5.6), for the experiment the request names.
#[allow(clippy::unnecessary_wraps)] // It is a handler of the fake runtime, which answers `Option<Value>`.
fn record(request: &Value, status: &str, stop_reason: Option<&str>) -> Option<Value> {
    Some(answer(
        request,
        json!({
            "experiment_id": request["params"]["experiment_id"].clone(),
            "source": "simulation",
            "kind": "setpoint_step",
            "task": "MainTask",
            "address": "%ML0",
            "status": status,
            "stop_reason": stop_reason,
            "restoration": "pending",
            "restoration_reason": null,
            "elapsed_ms": 1000,
            "duration_ms": 30000,
            "reviewed_scan": 41,
            "started_scan": 42,
            "last_applied": null,
            "relay_feedback": null,
            "abort_binding": null
        }),
    ))
}

/// Starts `plan` on a fake runtime that answers the start, and returns the start request's parameters.
fn start_params(plan: &ControlExperimentPlan, physical_approved: bool) -> Value {
    let fake = FakeRuntime::start(started);
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("connected");
    client
        .start_experiment(plan, physical_approved)
        .expect("the experiment starts");
    fake.requests_of("control.experiment.start")[0]["params"].clone()
}

#[test]
fn test_1_the_baseline_and_the_authority_are_sent_as_booleans_at_their_times() {
    let request = start_params(&plan(json!({})), false);
    assert_eq!(
        request["trajectory"],
        json!([{ "at_ms": 0, "value": false }, { "at_ms": 5000, "value": true }])
    );
    assert_eq!(request["restoration_value"], json!(false));
    assert_eq!(
        request["duration_ms"],
        json!(35100),
        "the reviewed maximum includes baseline and active intervals"
    );
}

#[test]
fn test_5_a_latched_abort_assertion_is_sent_with_its_addressed_plan() {
    let reviewed = plan(json!({
        "abortBinding": { "symbol": "P1.Abort", "address": "%MX64.1", "valueType": "BOOL", "assertedValue": true }
    }));
    let request = start_params(&reviewed, false);
    assert_eq!(
        request["abort_binding"],
        json!({ "address": "%MX64.1", "value_type": "BOOL", "asserted_value": true })
    );
    assert_eq!(request["restoration_value"], json!(false));
}

#[test]
fn test_12_an_mv_pulse_sends_its_rise_and_return_as_its_trajectory() {
    let pulse = plan(json!({
        "id": "experiment:loop:3:mv_pulse",
        "kind": "mv_pulse",
        "affectedSignal": "ValveMV",
        "address": "%QL0",
        "valueType": "LREAL",
        "from": 32.0,
        "to": 37.0,
        "unit": "%",
        "maximum": 100.0,
        "durationMs": 30000.0,
        "expectedScan": 3,
        "task": "MainTask",
        "restorationValue": 32.0,
        "trajectory": [{ "atMs": 0.0, "value": 37.0 }, { "atMs": 15000.0, "value": 32.0 }]
    }));
    let request = start_params(&pulse, false);
    assert_eq!(
        request["trajectory"],
        json!([{ "at_ms": 0, "value": 37.0 }, { "at_ms": 15000, "value": 32.0 }])
    );
}

#[test]
fn test_21_start_status_and_abort_map_to_the_runtime_control_requests() {
    let fake = FakeRuntime::start(|request| {
        if request["type"] == "control.experiment.abort" {
            record(request, "aborted", Some("user_abort"))
        } else if request["type"] == "control.experiment.status" {
            record(request, "running", None)
        } else {
            started(request)
        }
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("connected");
    let reviewed = plan(json!({
        "id": "experiment-1",
        "kind": "setpoint_step",
        "affectedSignal": "LevelSP",
        "address": "%ML0",
        "valueType": "LREAL",
        "from": 50.0,
        "to": 55.0,
        "maximum": 100.0,
        "durationMs": 30000.0,
        "expectedScan": 41,
        "task": "MainTask",
        "modeBinding": { "symbol": "P1.Run", "manualValue": false },
        "restorationValue": 50.0,
        "trajectory": [{ "atMs": 0.0, "value": 55.0 }]
    }));
    client.start_experiment(&reviewed, false).expect("started");
    client.experiment_status("experiment-1").expect("status");
    client.abort_experiment("experiment-1").expect("aborted");
    let operations: Vec<Value> = fake
        .requests()
        .iter()
        .map(|request| request["type"].clone())
        .collect();
    assert_eq!(
        operations,
        vec![
            json!("control.experiment.start"),
            json!("control.experiment.status"),
            json!("control.experiment.abort"),
        ]
    );
    assert_eq!(
        fake.requests()[0]["params"],
        json!({
            "experiment_id": "experiment-1",
            "source": "simulation",
            "kind": "setpoint_step",
            "address": "%ML0",
            "value": 55.0,
            "restoration_value": 50.0,
            "value_type": "LREAL",
            "minimum": 0.0,
            "maximum": 100.0,
            "duration_ms": 30000,
            "expected_scan": 41,
            "task": "MainTask",
            "mode_binding": { "symbol": "P1.Run", "manual_value": false },
            "trajectory": [{ "at_ms": 0, "value": 55.0 }]
        }),
        "no other field is sent: no stop_on_stale_ms, no controller_mode, no physical_approval"
    );
    assert_eq!(
        fake.requests()[2]["params"],
        json!({ "experiment_id": "experiment-1" })
    );
}

#[test]
fn test_22_finish_requests_a_normal_completion_of_the_named_experiment() {
    let fake = FakeRuntime::start(|request| record(request, "completed", Some("finished_early")));
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("connected");
    let finished = client.finish_experiment("experiment-1").expect("finished");
    assert_eq!(finished.experiment_id, "experiment-1");
    let requests = fake.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["type"], "control.experiment.finish");
    assert_eq!(
        requests[0]["params"],
        json!({ "experiment_id": "experiment-1" })
    );
}

#[test]
fn a_physical_plan_carries_the_engineers_approval_and_the_topology_review() {
    let physical = plan(json!({
        "source": "physical",
        "requiresPhysicalApproval": true
    }));
    let request = start_params(&physical, true);
    assert_eq!(request["source"], "physical");
    assert_eq!(
        request["physical_approval"],
        json!({ "user_approved": true, "topology_reviewed": true })
    );
}

#[test]
fn a_plan_without_an_address_names_its_variable_and_sends_no_address() {
    let by_name = plan(json!({ "address": null }));
    let request = start_params(&by_name, false);
    assert_eq!(request["symbol"], "P1.Run");
    assert!(
        request.get("address").is_none(),
        "exactly one target (E2.4)"
    );
}

#[test]
fn a_refusal_keeps_the_runtimes_text_and_its_next_action() {
    let fake = FakeRuntime::start(|request| {
        Some(refusal(
            request,
            "outside_limits",
            "Value 120 is outside its limits.",
        ))
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("connected");
    let failure = client
        .start_experiment(&plan(json!({})), false)
        .expect_err("refused");
    assert_eq!(failure.message, "Value 120 is outside its limits.");
    assert_eq!(failure.next_action, NextAction::OpenLimits);
}

#[test]
fn a_start_answer_for_another_experiment_is_not_accepted() {
    let fake = FakeRuntime::start(|request| {
        Some(answer(
            request,
            json!({ "experiment_id": "someone-else", "status": "running", "restoration": "pending" }),
        ))
    });
    let mut client = RuntimeClient::connect(&fake.connection(None)).expect("connected");
    let failure = client
        .start_experiment(&plan(json!({})), false)
        .expect_err("another experiment");
    assert!(
        failure.message.contains("does not match the protocol"),
        "{failure:?}"
    );
}

#[test]
fn a_status_record_of_another_experiment_or_outside_the_vocabulary_is_refused() {
    let other = FakeRuntime::start(|request| {
        Some(answer(
            request,
            json!({ "experiment_id": "someone-else", "status": "running", "restoration": "pending",
                "source": "simulation", "kind": "mv_step", "task": "MainTask", "address": "%QL0",
                "stop_reason": null, "restoration_reason": null, "elapsed_ms": 0, "duration_ms": 1000,
                "reviewed_scan": 1, "started_scan": 1, "last_applied": null, "relay_feedback": null,
                "abort_binding": null }),
        ))
    });
    let mut client = RuntimeClient::connect(&other.connection(None)).expect("connected");
    assert!(client.experiment_status("experiment-1").is_err());

    let unknown = FakeRuntime::start(|request| {
        Some(answer(
            request,
            json!({ "experiment_id": "experiment-1", "status": "paused", "restoration": "pending" }),
        ))
    });
    let mut client = RuntimeClient::connect(&unknown.connection(None)).expect("connected");
    assert!(client.experiment_status("experiment-1").is_err());
}
