//! Specification 34, §20.4 and specification 35, §0.1a: the plant summary the engine answers from the runtime's `plant.status`. The
//! tests pin which reads are made, the decode of the runtime's answer, the refusals of a stale revision and a missing plant, and
//! the reason of a failed read.

use serde_json::{json, Value};

use super::*;

const PROJECT: &str = "project:sha256:project";
const REVISION: &str = "sha256:revision";

fn plant() -> Value {
    json!({
        "id": "heater", "model": "first_order", "operating_state": "running", "enabled": true,
        "step_ns": "1000000", "simulation_time_ns": "5000000", "state": [0.5], "command": [1.0],
        "applied_input": [1.0], "ideal_output": [20.5], "measured_feedback": [20.25],
        "published_feedback": [20.0],
        "actuators": [{
            "position": 0.5, "applied": 0.5, "saturation_active": false, "slew_limit_active": false,
            "deadband_active": false, "stiction_active": false, "stuck": false
        }],
        "active_uncertainties": [], "active_dropouts": [], "pending_event_count": 0,
        "delay_buffer_depth": 0, "accumulated_remainder_ns": "0", "last_substeps": 1, "current_fault": null
    })
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn status(plants: Value) -> Value {
    json!({
        "contract": "trust.plant", "schema_version": 1, "operation": "plant.status", "project_id": PROJECT,
        "available": true, "active_configuration_revision": REVISION, "simulation_time_ns": "5000000",
        "effective_seed": "42", "random_algorithm": "xoshiro256", "random_algorithm_version": 1,
        "runtime_version": "0.24.75", "time_scale": 1.0, "restart_policy": "never", "plants": plants
    })
}

fn request() -> PlantMonitorRequest {
    PlantMonitorRequest {
        project_id: PROJECT.to_string(),
        plant_id: "heater".to_string(),
        active_configuration_digest: Some(REVISION.to_string()),
        runtime_stopped: false,
        endpoint_reachable: true,
        connection: RuntimeConnection {
            endpoint: "tcp://127.0.0.1:9000".to_string(),
            token: None,
            runtime_id: None,
        },
    }
}

/// The status with one change made to its only plant.
fn changed_plant(change: impl FnOnce(&mut Value)) -> Value {
    let mut plant = plant();
    change(&mut plant);
    status(json!([plant]))
}

fn reason_of(result: &PlantMonitorResult) -> Option<&str> {
    result.reason.as_deref()
}

#[test]
fn a_plant_the_runtime_answers_is_available_with_its_status() {
    let result = answer(&request(), |_| Ok(status(json!([plant()]))));
    assert_eq!(result.availability, PlantAvailability::Available);
    assert_eq!(result.source, MonitorSource::Simulation);
    assert_eq!(result.reason, None);
    let view = result
        .status
        .as_ref()
        .expect("an available plant has its status");
    assert_eq!(view.model, "first_order");
    assert_eq!(view.operating_state, OperatingState::Running);
    assert_eq!(view.simulation_time_ns, "5000000");
    assert_eq!(view.effective_seed, "42");
    assert_eq!(view.random_algorithm_version, 1);
    assert_eq!(json!(view.measured_feedback), json!([20.25]));
    assert_eq!(json!(view.actuators[0].position), json!(0.5));
    assert_eq!(view.current_fault, None);
    let json = serde_json::to_value(&result).expect("the answer serializes");
    assert_eq!(json["availability"], "available");
    assert_eq!(json["source"], "Simulation");
    assert_eq!(json["plantId"], "heater");
    assert_eq!(json["status"]["timeScale"], json!(1.0));
    assert!(
        json.get("reason").is_none(),
        "an available answer names no reason"
    );
}

#[test]
fn the_read_is_made_on_the_endpoint_trimmed() {
    let mut request = request();
    request.connection.endpoint = "  tcp://127.0.0.1:9000 \n".to_string();
    let result = answer(&request, |connection| {
        assert_eq!(connection.endpoint, "tcp://127.0.0.1:9000");
        Ok(status(json!([plant()])))
    });
    assert_eq!(result.availability, PlantAvailability::Available);
}

#[test]
fn a_stopped_runtime_that_does_not_answer_its_probe_is_not_read() {
    let mut request = request();
    request.runtime_stopped = true;
    request.endpoint_reachable = false;
    let result = answer(&request, |_| panic!("a stopped runtime is not read"));
    assert_eq!(result.availability, PlantAvailability::Unavailable);
    assert_eq!(reason_of(&result), Some(STOPPED));
    assert!(result.status.is_none());
}

#[test]
fn a_running_runtime_is_read_even_when_its_probe_did_not_answer() {
    let mut request = request();
    request.runtime_stopped = false;
    request.endpoint_reachable = false;
    let result = answer(&request, |_| Ok(status(json!([plant()]))));
    assert_eq!(result.availability, PlantAvailability::Available);
}

#[test]
fn a_stopped_runtime_that_answers_its_probe_is_read() {
    let mut request = request();
    request.runtime_stopped = true;
    let result = answer(&request, |_| Ok(status(json!([plant()]))));
    assert_eq!(result.availability, PlantAvailability::Available);
}

#[test]
fn a_blank_endpoint_is_unreachable_and_is_not_read() {
    let mut request = request();
    request.connection.endpoint = "   ".to_string();
    let result = answer(&request, |_| panic!("a blank endpoint is not read"));
    assert_eq!(reason_of(&result), Some(UNREACHABLE));
}

#[test]
fn a_refused_token_is_an_authorization_rejection() {
    let result = answer(&request(), |_| {
        Err("invalid_auth_token The control token is not valid.".to_string())
    });
    assert_eq!(reason_of(&result), Some(AUTHORIZATION_REJECTED));
}

#[test]
fn a_read_that_names_an_unsupported_operation_is_unsupported() {
    // The runtime's own refusal of a plant contract it does not serve (`plant_handlers.rs`, code `unsupported_schema`).
    let result = answer(&request(), |_| {
        Err("unsupported_schema unsupported plant contract schema".to_string())
    });
    assert_eq!(reason_of(&result), Some(UNSUPPORTED));
}

#[test]
fn a_read_that_cannot_connect_is_unreachable() {
    let result = answer(&request(), |_| {
        Err("Control Studio cannot reach the runtime. connection refused".to_string())
    });
    assert_eq!(reason_of(&result), Some(UNREACHABLE));
}

#[test]
fn the_reason_of_a_failed_read_ignores_case() {
    assert_eq!(read_reason("UNAUTHORIZED"), AUTHORIZATION_REJECTED);
    assert_eq!(read_reason("Unsupported"), UNSUPPORTED);
    assert_eq!(read_reason("timed out"), UNREACHABLE);
}

#[test]
fn a_status_for_another_project_is_malformed() {
    let mut value = status(json!([plant()]));
    value["project_id"] = json!("project:sha256:other");
    let result = answer(&request(), |_| Ok(value));
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn an_answer_of_another_operation_is_malformed() {
    let mut value = status(json!([plant()]));
    value["operation"] = json!("plant.read");
    let result = answer(&request(), |_| Ok(value));
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn an_unavailable_runtime_answer_is_malformed() {
    let mut value = status(json!([plant()]));
    value["available"] = json!(false);
    let result = answer(&request(), |_| Ok(value));
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn an_answer_with_a_field_the_protocol_does_not_name_is_malformed() {
    let mut value = status(json!([plant()]));
    value["extra"] = json!(1);
    let result = answer(&request(), |_| Ok(value));
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn a_plant_without_one_of_its_fields_is_malformed() {
    let result = answer(&request(), |_| {
        Ok(changed_plant(|plant| {
            plant
                .as_object_mut()
                .expect("a plant")
                .remove("last_substeps");
        }))
    });
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn an_actuator_without_its_flags_is_malformed() {
    let result = answer(&request(), |_| {
        Ok(changed_plant(|plant| {
            plant["actuators"][0]
                .as_object_mut()
                .expect("an actuator")
                .remove("stuck");
        }))
    });
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn a_flag_of_the_wrong_type_is_malformed() {
    let result = answer(&request(), |_| {
        Ok(changed_plant(|plant| {
            plant["actuators"][0]["stuck"] = json!("no");
        }))
    });
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn a_time_that_is_not_a_canonical_decimal_is_malformed() {
    let result = answer(&request(), |_| {
        Ok(changed_plant(|plant| {
            plant["simulation_time_ns"] = json!("05");
        }))
    });
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn a_time_beyond_128_bits_is_malformed() {
    let too_large = "340282366920938463463374607431768211456";
    let result = answer(&request(), |_| {
        Ok(changed_plant(|plant| plant["step_ns"] = json!(too_large)))
    });
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn an_operating_state_the_protocol_does_not_name_is_malformed() {
    let result = answer(&request(), |_| {
        Ok(changed_plant(|plant| {
            plant["operating_state"] = json!("stopped");
        }))
    });
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn a_zero_time_scale_is_malformed() {
    let mut value = status(json!([plant()]));
    value["time_scale"] = json!(0);
    let result = answer(&request(), |_| Ok(value));
    assert_eq!(reason_of(&result), Some(MALFORMED));
}

#[test]
fn a_revision_other_than_the_handoff_names_is_stale() {
    let mut request = request();
    request.active_configuration_digest = Some("sha256:other".to_string());
    let result = answer(&request, |_| Ok(status(json!([plant()]))));
    assert_eq!(reason_of(&result), Some(STALE));
    assert!(result.status.is_none());
}

#[test]
fn the_revision_is_checked_before_the_plant_is_looked_up() {
    let mut request = request();
    request.active_configuration_digest = Some("sha256:other".to_string());
    let result = answer(&request, |_| Ok(status(json!([]))));
    assert_eq!(reason_of(&result), Some(STALE));
}

#[test]
fn a_handoff_that_names_no_revision_does_not_check_it() {
    let mut request = request();
    request.active_configuration_digest = None;
    let result = answer(&request, |_| Ok(status(json!([plant()]))));
    assert_eq!(result.availability, PlantAvailability::Available);
}

#[test]
fn a_plant_the_project_does_not_run_no_longer_exists() {
    let result = answer(&request(), |_| Ok(status(json!([]))));
    assert_eq!(reason_of(&result), Some(NO_PLANT));
}

#[test]
fn a_faulted_plant_shows_its_fault() {
    let result = answer(&request(), |_| {
        Ok(changed_plant(|plant| {
            plant["operating_state"] = json!("faulted");
            plant["current_fault"] = json!("overheat");
        }))
    });
    let view = result.status.expect("a faulted plant still has its status");
    assert_eq!(view.operating_state, OperatingState::Faulted);
    assert_eq!(view.current_fault.as_deref(), Some("overheat"));
}

#[test]
fn an_unavailable_answer_names_its_plant_and_its_reason_only() {
    let result = answer(&request(), |_| Err("refused".to_string()));
    let json = serde_json::to_value(&result).expect("the answer serializes");
    assert_eq!(json["availability"], "unavailable");
    assert_eq!(json["plantId"], "heater");
    assert_eq!(json["source"], "Simulation");
    assert!(json.get("status").is_none());
    assert!(json["reason"].is_string());
}
