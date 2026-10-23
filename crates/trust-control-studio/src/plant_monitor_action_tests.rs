//! The plant summary against a fake runtime (specification 47, §20.4; specification 35, §0.1a): the one `plant.status` read the engine
//! makes, the reasons of a read that fails, and the runtime that is never read when its state forbids it.

use std::net::TcpListener;

use serde_json::{json, Value};
use trust_control::plant_monitor::{
    PlantAvailability, PlantMonitorRequest, AUTHORIZATION_REJECTED, MALFORMED, STOPPED, UNREACHABLE,
};
use trust_control::runtime::RuntimeConnection;

use super::answer;
use crate::runtime_client::fake_runtime::{answer as reply, refusal, FakeRuntime};

const PROJECT: &str = "project:sha256:project";
const REVISION: &str = "sha256:revision";

/// A `plant.status` answer with one plant, the `heater`.
fn status() -> Value {
    json!({
        "contract": "trust.plant", "schema_version": 1, "operation": "plant.status", "project_id": PROJECT,
        "available": true, "active_configuration_revision": REVISION, "simulation_time_ns": "5000000",
        "effective_seed": "42", "random_algorithm": "xoshiro256", "random_algorithm_version": 1,
        "runtime_version": "0.24.75", "time_scale": 1.0, "restart_policy": "never",
        "plants": [{
            "id": "heater", "model": "first_order", "operating_state": "running", "enabled": true,
            "step_ns": "1000000", "simulation_time_ns": "5000000", "state": [0.5], "command": [1.0],
            "applied_input": [1.0], "ideal_output": [20.5], "measured_feedback": [20.25],
            "published_feedback": [20.0], "actuators": [], "active_uncertainties": [], "active_dropouts": [],
            "pending_event_count": 0, "delay_buffer_depth": 0, "accumulated_remainder_ns": "0",
            "last_substeps": 1, "current_fault": null
        }]
    })
}

fn request(connection: RuntimeConnection) -> PlantMonitorRequest {
    PlantMonitorRequest {
        project_id: PROJECT.to_string(),
        plant_id: "heater".to_string(),
        active_configuration_digest: Some(REVISION.to_string()),
        runtime_stopped: false,
        endpoint_reachable: true,
        connection,
    }
}

/// A fake runtime that answers `plant.status` with `value`.
fn runtime_with(value: Value) -> FakeRuntime {
    FakeRuntime::start(move |request| {
        (request["type"] == "plant.status").then(|| reply(request, value.clone()))
    })
}

#[test]
fn the_runtime_is_read_once_for_the_project_and_answers_the_plant_summary() {
    let runtime = runtime_with(status());
    let result = answer(&request(runtime.connection(Some("token"))));
    assert_eq!(result.availability, PlantAvailability::Available);
    assert_eq!(result.plant_id, "heater");
    let reads = runtime.requests_of("plant.status");
    assert_eq!(reads.len(), 1, "one read of plant.status");
    assert_eq!(reads[0]["params"]["project_id"], PROJECT);
    assert_eq!(reads[0]["params"]["contract"], "trust.plant");
    assert_eq!(reads[0]["params"]["schema_version"], 1);
}

#[test]
fn a_stopped_runtime_is_not_read_at_all() {
    let runtime = runtime_with(status());
    let mut stopped = request(runtime.connection(None));
    stopped.runtime_stopped = true;
    stopped.endpoint_reachable = false;
    let result = answer(&stopped);
    assert_eq!(result.reason.as_deref(), Some(STOPPED));
    assert!(
        runtime.requests().is_empty(),
        "a stopped runtime receives no request"
    );
}

#[test]
fn an_endpoint_nobody_listens_on_is_unreachable() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("the port").port();
    drop(listener);
    let connection = RuntimeConnection {
        endpoint: format!("tcp://127.0.0.1:{port}"),
        token: None,
        runtime_id: None,
    };
    let result = answer(&request(connection));
    assert_eq!(result.availability, PlantAvailability::Unavailable);
    assert_eq!(result.reason.as_deref(), Some(UNREACHABLE));
}

#[test]
fn a_refused_token_is_an_authorization_rejection() {
    let runtime = FakeRuntime::start(|request| {
        (request["type"] == "plant.status").then(|| {
            refusal(
                request,
                "invalid_auth_token",
                "The control token is not valid.",
            )
        })
    });
    let result = answer(&request(runtime.connection(Some("wrong"))));
    assert_eq!(result.reason.as_deref(), Some(AUTHORIZATION_REJECTED));
}

#[test]
fn an_answer_that_is_not_a_plant_status_is_malformed() {
    let runtime = runtime_with(json!({ "contract": "trust.plant" }));
    let result = answer(&request(runtime.connection(None)));
    assert_eq!(result.reason.as_deref(), Some(MALFORMED));
}
