//! Tests of the model route (`model_route`, `selected_model_binding`): the selected model's inline plant for a `Control.PID` loop, and
//! the controller's initial values at the model's operating point (specification 35, B5.3, B6.2 and C1; `engineerVerificationPlan.ts`,
//! its model route, and the `control-pid-v1` initialization of C1).

#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{model_route, selected_model_binding};
use crate::commissioning::fixture_loops::{first_order_model, library_pid_loop};
use crate::failure::NextAction;
use crate::model::general::decode_general_model;
use crate::model::{ActuatorCapabilities, ActuatorKind, GeneralModel};

/// A transfer function with one input and one output that operates at `operating_input` and `operating_output`, with the given delay in
/// seconds.
fn transfer_model(
    numerator: &Value,
    denominator: &Value,
    delay_seconds: f64,
    operating_input: f64,
    operating_output: f64,
) -> GeneralModel {
    let file = json!({
        "kind": "transfer-function", "revision": "model:fixture", "source": "provided",
        "validation": "validated", "evidence": ["bench"], "delaySeconds": delay_seconds,
        "numerator": numerator, "denominator": denominator,
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "operatingPoint": { "inputs": [operating_input], "outputs": [operating_output] }
    });
    decode_general_model(&file).expect("the fixture model decodes")
}

#[test]
fn the_selected_model_binds_a_control_pid_loop_to_its_own_measurement_and_actuator() {
    // B5.3: the loop's plant input is the model's command; a library PID's own measurement is fed to that PID (C1); the actuator's
    // applied position is the model's actuator feedback.
    let binding = selected_model_binding(&library_pid_loop(), &first_order_model())
        .expect("the binding of the fixture loop");
    assert_eq!(binding.plant_id, "selected-model");
    assert_eq!(binding.command_sources, vec!["%QL0".to_owned()]);
    assert_eq!(
        binding.feedback_targets,
        vec!["pid:Main.TankController".to_owned()]
    );
    assert_eq!(
        binding.actuator_feedback_targets,
        Some(vec![Some("%IL8".to_owned())])
    );
    assert_eq!(binding.initial_state, vec![0.0]);
    assert_eq!(binding.initial_input, vec![0.0]);
    assert_eq!(binding.initial_delayed_input, Some(0.0));
    assert_eq!(binding.step_ms, 10.0);
    assert_eq!(
        binding.actuators,
        Some(vec![ActuatorCapabilities {
            kind: ActuatorKind::Continuous,
            minimum: 0.0,
            maximum: 100.0,
            rate_up_per_s: None,
            rate_down_per_s: None,
            minimum_on_seconds: None,
            minimum_off_seconds: None,
        }])
    );
}

#[test]
fn the_controller_states_start_at_the_model_operating_point() {
    // C1, the `control-pid-v1` initialization at a stationary operating point (input 3, output 4): the setpoint and the process value
    // at the model's output, the nominal and saturated outputs at its input, and the rates and the feedforward at zero. These are the
    // values that `control-studio-library-pid.test.ts` asserts for the same model (its `expected` map).
    let model = transfer_model(&json!([1]), &json!([1, 1]), 0.0, 3.0, 4.0);
    let route = model_route(&library_pid_loop(), &model, &BTreeMap::new())
        .expect("the route of the fixture loop");
    let state =
        |member: &str| route.initial_values[&format!("Main.TankController.StateValue.{member}")];
    assert_eq!(state("Setpoint"), 4.0);
    assert_eq!(state("SetpointRate"), 0.0);
    assert_eq!(state("ProcessValue"), 4.0);
    assert_eq!(state("ProcessValueRate"), 0.0);
    assert_eq!(state("FeedForward"), 0.0);
    assert_eq!(state("NominalOutput"), 3.0);
    assert_eq!(state("SaturatedOutput"), 3.0);
    assert_eq!(
        route.initial_values.len(),
        7,
        "exactly the seven declared states"
    );
}

#[test]
fn a_two_state_transfer_function_starts_both_states_at_zero_deviation() {
    let model = transfer_model(&json!([1]), &json!([1, 3, 2]), 0.0, 0.0, 2.0);
    let binding = selected_model_binding(&library_pid_loop(), &model).expect("the binding");
    assert_eq!(binding.initial_state, vec![0.0, 0.0]);
    let route = model_route(&library_pid_loop(), &model, &BTreeMap::new()).expect("the route");
    assert_eq!(route.simulation.model_revision, "model:fixture");
    assert!(
        route
            .simulation
            .model_provenance
            .starts_with("declared-provided-"),
        "{}",
        route.simulation.model_provenance
    );
    assert!(
        route
            .simulation
            .simulation_toml
            .contains("id = \"selected-model\""),
        "{}",
        route.simulation.simulation_toml
    );
}

#[test]
fn a_first_order_model_with_delay_materializes_its_delayed_plant() {
    let model = transfer_model(&json!([1]), &json!([1, 1]), 2.0, 0.0, 2.0);
    let route = model_route(&library_pid_loop(), &model, &BTreeMap::new())
        .expect("the route of a delayed model");
    assert!(
        route
            .simulation
            .simulation_toml
            .contains("id = \"selected-model\""),
        "{}",
        route.simulation.simulation_toml
    );
}

#[test]
fn a_loop_without_an_output_limit_has_no_model_binding() {
    // The actuator's range of the model's plant comes from the output limits (B3.9): without the minimum there is no binding.
    let mut loop_definition = library_pid_loop();
    loop_definition.limits.output_minimum = None;
    let refusal = selected_model_binding(&loop_definition, &first_order_model())
        .expect_err("no actuator range");
    assert_eq!(refusal.next_action, NextAction::OpenLimits);
}
