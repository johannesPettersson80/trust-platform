use serde_json::{json, Value};

use super::*;

fn wide(id: &str, unit: &str) -> Value {
    json!({"id": id, "unit": unit, "operatingRange": {"minimum": -100.0, "maximum": 100.0}})
}

/// The `TypeScript` `binding(initialState)`: one command, one feedback, one initial input, 20 ms.
fn binding(initial_state: &[f64]) -> SelectedModelSimulationBinding {
    SelectedModelSimulationBinding {
        plant_id: "selected-plant".to_owned(),
        command_sources: vec!["%QL0".to_owned()],
        feedback_targets: vec!["%IL0".to_owned()],
        actuator_feedback_targets: None,
        initial_state: initial_state.to_vec(),
        initial_input: vec![0.25],
        initial_delayed_input: None,
        step_ms: 20.0,
        actuators: None,
    }
}

fn toml_of(result: &SelectedModelSimulation) -> &str {
    &result.simulation_override.simulation_toml
}

#[test]
fn a_delayed_integrator_materializes_with_exact_rate_input_history_and_offsets() {
    // model-simulation.test.ts, lines 37 to 54.
    let plant = json!({
        "kind": "transfer-function", "numerator": [4.0], "denominator": [2.0, 0.0], "delaySeconds": 0.13,
        "revision": "delayed-integrator", "source": "provided", "validation": "validated",
        "evidence": ["record"], "inputs": [wide("u", "V")], "outputs": [wide("y", "m")],
        "operatingPoint": {"inputs": [2.0], "outputs": [10.0]}
    });
    let mut bound = binding(&[0.5]);
    bound.initial_delayed_input = Some(0.25);
    let result = materialize_selected_model_simulation(&plant, &bound).expect("it materializes");
    let toml = toml_of(&result);
    for expected in [
        "model = \"integrator_dead_time\"",
        "gain = 2",
        "dead_time_s = 0.13",
        "initial_delayed_input = 0.25",
        "initial_state = [1]",
        "input_offset = [-2]",
        "output_offset = [10]",
    ] {
        assert!(toml.contains(expected), "missing {expected}");
    }
    assert!(!toml.contains("time_constant_s"));
    let refused = materialize_selected_model_simulation(&plant, &binding(&[0.0]))
        .expect_err("the delayed adapter needs the declared input history");
    assert_eq!(refused, ModelRefusal::ExactDelayAdapterDomain);
}

#[test]
fn a_plant_is_driven_from_a_program_variable_in_memory() {
    // model-simulation.test.ts, lines 56 to 66.
    let closed_inner = json!({
        "kind": "transfer-function", "numerator": [0.8337], "denominator": [5.21, 4.57, 1.0], "delaySeconds": 0.0,
        "revision": "vessel", "source": "fitted", "validation": "validated", "evidence": ["record"],
        "inputs": [wide("u", "degC")], "outputs": [wide("y", "degC")],
        "operatingPoint": {"inputs": [38.0], "outputs": [35.0]}
    });
    let mut bound = binding(&[0.0, 0.0]);
    bound.command_sources = vec!["%ML8".to_owned()];
    bound.feedback_targets = vec!["%IL8".to_owned()];
    let result =
        materialize_selected_model_simulation(&closed_inner, &bound).expect("materializes");
    assert!(toml_of(&result).contains("command_sources = [\"%ML8\"]"));

    let mut input_command = binding(&[0.0, 0.0]);
    input_command.command_sources = vec!["%IL0".to_owned()];
    let refused = materialize_selected_model_simulation(&closed_inner, &input_command)
        .expect_err("an input cannot command a plant");
    assert!(refused.to_string().contains("%Q output or %M memory"));
}

#[test]
fn a_delay_free_transfer_function_keeps_its_zero_in_controllable_companion_coordinates() {
    // model-simulation.test.ts, lines 68 to 100.
    let model = json!({
        "revision": "sha256:tf-with-zero", "source": "provided", "validation": "unvalidated", "evidence": [],
        "delaySeconds": 0.0, "kind": "transfer-function",
        "inputs": [wide("command", "V")], "outputs": [wide("position", "m")],
        "numerator": [1.0, 1.0], "denominator": [1.0, 3.0, 2.0]
    });
    let result = materialize_selected_model_simulation(&model, &binding(&[0.25, -0.5]))
        .expect("it materializes");
    assert_eq!(
        result.simulation_override.model_revision,
        "sha256:tf-with-zero"
    );
    assert_eq!(
        result.simulation_override.model_provenance,
        "declared-provided-unvalidated"
    );
    let description = result.state_coordinate_description.to_lowercase();
    assert!(description.contains("controllable companion"));
    assert!(description.contains("not physical measured states"));
    let toml = toml_of(&result);
    for expected in [
        "model = \"state_space\"",
        "command_sources = [\"%QL0\"]",
        "feedback_targets = [\"%IL0\"]",
        "initial_state = [0.25, -0.5]",
        "initial_input = [0.25]",
        "a = [[-3, -2], [1, 0]]",
        "b = [[1], [0]]",
        "c = [[1, 1]]",
        "d = [[0]]",
    ] {
        assert!(toml.contains(expected), "missing {expected}");
    }
}

#[test]
fn supplied_continuous_state_coordinates_and_mimo_channels_stay_intact() {
    // model-simulation.test.ts, lines 102 to 137.
    let model = json!({
        "revision": "sha256:supplied-state-space", "source": "fitted", "validation": "validated",
        "evidence": ["independent-record"], "delaySeconds": 0.0, "kind": "continuous-state-space",
        "states": [wide("stored-energy", "J"), wide("flow-mode", "m3/s")],
        "inputs": [wide("heater", "W"), wide("valve", "%")],
        "outputs": [wide("temperature", "degC"), wide("level", "m")],
        "a": [[-2.0, 1.0], [0.0, -3.0]], "b": [[1.0, 2.0], [3.0, 4.0]],
        "c": [[5.0, 6.0], [7.0, 8.0]], "d": [[0.0, 9.0], [10.0, 0.0]]
    });
    let bound = SelectedModelSimulationBinding {
        plant_id: "mimo-plant".to_owned(),
        command_sources: vec!["%QL0".to_owned(), "%QL8".to_owned()],
        feedback_targets: vec!["%IL0".to_owned(), "%IL8".to_owned()],
        actuator_feedback_targets: None,
        initial_state: vec![11.0, 12.0],
        initial_input: vec![13.0, 14.0],
        initial_delayed_input: None,
        step_ms: 50.0,
        actuators: None,
    };
    let result = materialize_selected_model_simulation(&model, &bound).expect("it materializes");
    assert!(result
        .state_coordinate_description
        .contains("stored-energy, flow-mode"));
    let toml = toml_of(&result);
    for expected in [
        "command_sources = [\"%QL0\", \"%QL8\"]",
        "feedback_targets = [\"%IL0\", \"%IL8\"]",
        "initial_state = [11, 12]",
        "initial_input = [13, 14]",
        "a = [[-2, 1], [0, -3]]",
        "b = [[1, 2], [3, 4]]",
        "c = [[5, 6], [7, 8]]",
        "d = [[0, 9], [10, 0]]",
    ] {
        assert!(toml.contains(expected), "missing {expected}");
    }
}

#[test]
fn exact_delay_adapters_are_used_and_mismatched_discrete_timing_is_refused() {
    // model-simulation.test.ts, lines 139 to 184.
    let delayed = json!({
        "revision": "sha256:fopdt", "source": "fitted", "validation": "validated",
        "evidence": ["independent-record"], "delaySeconds": 1.5, "kind": "transfer-function",
        "inputs": [wide("command", "V")], "outputs": [wide("temperature", "degC")],
        "numerator": [4.0], "denominator": [2.0, 1.0]
    });
    let mut bound = binding(&[18.0]);
    bound.initial_delayed_input = Some(0.125);
    let result = materialize_selected_model_simulation(&delayed, &bound).expect("it materializes");
    let toml = toml_of(&result);
    assert!(toml.contains("model = \"first_order_dead_time\""));
    assert!(toml.contains("dead_time_s = 1.5"));
    assert!(toml.contains("initial_delayed_input = 0.125"));

    let discrete = json!({
        "revision": "sha256:discrete", "source": "provided", "validation": "unvalidated", "evidence": [],
        "delaySeconds": 0.0, "kind": "discrete-state-space", "sampleTimeSeconds": 0.1,
        "states": [wide("x", "1")], "inputs": [wide("command", "V")], "outputs": [wide("output", "V")],
        "a": [[0.5]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]]
    });
    let refused = materialize_selected_model_simulation(&discrete, &binding(&[0.0]))
        .expect_err("the step differs from the sample time");
    let text = refused.to_string().to_lowercase();
    assert!(text.contains("discrete") && text.contains("sample time"));

    let delayed_state = json!({
        "revision": "sha256:delayed-state-space", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.1, "kind": "continuous-state-space",
        "states": [wide("x", "1")], "inputs": [wide("command", "V")], "outputs": [wide("output", "V")],
        "a": [[-1.0]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]]
    });
    let result = materialize_selected_model_simulation(&delayed_state, &binding(&[0.0]))
        .expect("the delayed state-space model materializes");
    assert!(toml_of(&result).contains("model = \"discrete_state_space\""));
    let description = &result.state_coordinate_description;
    assert!(description.contains("input-history slots"));
    assert!(description.contains("Exact held-input delay"));
}

/// The `TypeScript` operating-point model: a first-order transfer function with an operating point.
fn offset_model(operating_point: &Value) -> Value {
    json!({
        "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 1.0], "delaySeconds": 0.0,
        "revision": "offset-model", "source": "provided", "validation": "validated", "evidence": ["bench"],
        "inputs": [{"id": "u", "unit": "V", "operatingRange": {"minimum": -20.0, "maximum": 20.0}}],
        "outputs": [{"id": "y", "unit": "m", "operatingRange": {"minimum": -20.0, "maximum": 20.0}}],
        "operatingPoint": operating_point
    })
}

#[test]
fn an_operating_point_must_match_the_channel_dimensions_and_ranges() {
    // operating-point.test.ts, lines 20 to 27: Infinity is written as null, as JSON transports it.
    assert!(
        decode_general_model(&offset_model(&json!({"inputs": [2.0], "outputs": [10.0]}))).is_some()
    );
    for operating_point in [
        json!({"inputs": [], "outputs": [10.0]}),
        json!({"inputs": [2.0], "outputs": [Value::Null]}),
        json!({"inputs": [2.0], "outputs": [21.0]}),
        json!({"inputs": [2.0]}),
    ] {
        assert!(
            decode_general_model(&offset_model(&operating_point)).is_none(),
            "{operating_point}"
        );
    }
}

#[test]
fn the_materialized_plant_uses_the_physical_io_offsets() {
    // operating-point.test.ts, lines 29 to 33.
    let mut bound = binding(&[0.0]);
    bound.plant_id = "offset".to_owned();
    bound.initial_input = vec![0.0];
    let result = materialize_selected_model_simulation(
        &offset_model(&json!({"inputs": [2.0], "outputs": [10.0]})),
        &bound,
    )
    .expect("it materializes");
    let toml = toml_of(&result);
    assert!(toml.contains("input_offset = [-2]"));
    assert!(toml.contains("output_offset = [10]"));
}

#[test]
fn a_pid_measurement_is_accepted_only_as_a_plant_feedback() {
    // generated-loop.test.ts, lines 220 to 225: the tank fixture and its base binding.
    let tank = json!({
        "kind": "transfer-function", "numerator": [0.2], "denominator": [300.0, 1.0], "delaySeconds": 0.0,
        "revision": "tank", "source": "provided", "validation": "validated", "evidence": ["fixture"],
        "inputs": [{"id": "u", "unit": "%", "operatingRange": {"minimum": 0.0, "maximum": 100.0}}],
        "outputs": [{"id": "y", "unit": "degC", "operatingRange": {"minimum": 0.0, "maximum": 100.0}}],
        "operatingPoint": {"inputs": [30.0], "outputs": [45.0]}
    });
    let mut base = binding(&[0.0]);
    base.plant_id = "selected".to_owned();
    base.step_ms = 100.0;
    base.initial_delayed_input = Some(0.0);
    base.command_sources = vec!["Main.TIC101_Output".to_owned()];
    base.feedback_targets = vec!["pid:Main.TIC101".to_owned()];
    assert!(materialize_selected_model_simulation(&tank, &base).is_ok());

    let mut pid_command = base.clone();
    pid_command.command_sources = vec!["pid:Main.TIC101".to_owned()];
    pid_command.feedback_targets = vec!["%IL8".to_owned()];
    let refused = materialize_selected_model_simulation(&tank, &pid_command)
        .expect_err("a PID measurement cannot command a plant");
    assert!(refused.to_string().contains("command bindings"));
}
