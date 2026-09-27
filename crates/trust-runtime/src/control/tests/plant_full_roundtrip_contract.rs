#[test]
fn plant_save_round_trips_full_candidate_without_dropping_runtime_fields() {
    let root = temp_dir("plant-save-full-roundtrip");
    write_file(
        &root.join("simulation.toml"),
        r#"[simulation]
enabled = true
seed = 42
plant_step_ms = 10
time_scale = 2

[[couplings]]
source = "%QX4.0"
target = "%IX4.0"

[[plants]]
id = "oven"
model = "static_gain"
command_sources = ["%QW2"]
feedback_targets = ["%IW2"]
[plants.parameters]
gain = 2.0

[[plants]]
id = "tank"
model = "first_order_dead_time"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]
step_ms = 20
initial_state = [1.5]
initial_input = [2.5]
input_scale = [0.5]
input_offset = [1.0]
output_scale = [2.0]
output_offset = [-3.0]
[plants.parameters]
gain = 1.2
time_constant_s = 5.0
dead_time_s = 0.5
initial_delayed_input = 2.0
[plants.actuator]
minimum = 0.0
maximum = 10.0
rate_up_per_s = 3.0
rate_down_per_s = 4.0
deadband = 0.1
lag_s = 0.2
initial_applied = 2.5
[plants.validity]
input_minimum = [0.0]
input_maximum = [10.0]
state_minimum = [0.0]
state_maximum = [20.0]
output_minimum = [0.0]
output_maximum = [20.0]

[[plants.uncertainties]]
id = "gain-mismatch"
target = "parameter"
parameter = "gain"
mode = "multiply"
generator = "constant"
value = 1.1

[[plant_events]]
at_ms = 100
plant = "tank"
kind = "add_state"
channel = 0
value = 0.25
"#,
    );
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({"id": 6800, "type": "plant.read", "params": {"contract": "trust.plant", "schema_version": 1}}),
        &state,
        None,
    )
    .result
    .expect("plant.read result");
    let response = handle_request_value(
        json!({
            "id": 6801,
            "type": "plant.save",
            "params": {
                "contract": "trust.plant", "schema_version": 1,
                "project_id": read["project_id"], "operation_id": "full-roundtrip-1",
                "expected_revision": read["configuration_revision"],
                "plant": {
                    "id": "tank", "model": "first_order_dead_time",
                    "command_sources": ["%QW0"], "feedback_targets": ["%IW0"],
                    "step_ms": 20, "initial_state": [1.5], "initial_input": [2.5],
                    "input_scale": [0.5], "input_offset": [1.0],
                    "output_scale": [2.0], "output_offset": [-3.0],
                    "parameters": {"gain": 1.2, "time_constant_s": 5.0, "dead_time_s": 0.5, "initial_delayed_input": 2.0},
                    "actuator": {"minimum": 0.0, "maximum": 10.0, "rate_up_per_s": 3.0, "rate_down_per_s": 4.0, "deadband": 0.1, "lag_s": 0.2, "initial_applied": 2.5},
                    "validity": {"input_minimum": [0.0], "input_maximum": [10.0], "state_minimum": [0.0], "state_maximum": [20.0], "output_minimum": [0.0], "output_maximum": [20.0]},
                    "uncertainties": [{"id": "gain-mismatch", "target": "parameter", "parameter": "gain", "mode": "multiply", "generator": "constant", "value": 1.1}],
                    "scenario_events": [{"at_ms": 100, "kind": "add_state", "channel": 0, "value": 0.25}]
                }
            }
        }),
        &state,
        None,
    );
    assert!(response.ok, "full plant.save failed: {:?}", response.error);
    let saved = fs::read_to_string(root.join("simulation.toml")).expect("saved simulation");
    for fragment in [
        "seed = 42",
        "time_scale = 2",
        "id = \"oven\"",
        "step_ms = 20",
        "initial_input = [2.5]",
        "input_scale = [0.5]",
        "output_offset = [-3.0]",
        "validity = {",
        "input_maximum = [10.0]",
        "uncertainties = [",
        "[[plant_events]]",
        "at_ms = 100",
        "[[couplings]]",
    ] {
        assert!(
            saved.contains(fragment),
            "saved candidate dropped {fragment:?}: {saved}"
        );
    }
    crate::simulation::SimulationConfig::load(root.join("simulation.toml"))
        .expect("reloaded full candidate");
    fs::remove_dir_all(root).expect("remove full roundtrip fixture");
}

#[test]
fn plant_save_rejects_unknown_fields_without_changing_bytes() {
    let root = temp_dir("plant-save-unknown-fields");
    let baseline = r#"[simulation]
enabled = true

[[plants]]
id = "tank"
model = "first_order"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#;
    write_file(&root.join("simulation.toml"), baseline);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({"id": 6900, "type": "plant.read", "params": {"contract": "trust.plant", "schema_version": 1}}),
        &state,
        None,
    )
    .result
    .expect("baseline plant.read result");
    assert!(
        read.get("saved_configuration").is_some(),
        "baseline plant.read must return saved configuration"
    );
    let candidate = json!({
        "id": "tank", "model": "first_order", "command_sources": ["%QL0"], "feedback_targets": ["%IL0"],
        "initial_state": [0.0], "parameters": {"gain": 1.0, "time_constant_s": 5.0},
    });
    for (label, key, extra) in [
        ("top-level", "future_field", json!({"future_field": true})),
        ("parameter", "future_gain", json!({"future_gain": 2.0})),
        ("actuator", "future_mode", json!({"future_mode": "hold"})),
    ] {
        write_file(&root.join("simulation.toml"), baseline);
        let before = std::fs::read(root.join("simulation.toml")).expect("baseline bytes");
        let mut candidate = candidate.clone();
        match label {
            "top-level" => candidate
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone()),
            "parameter" => candidate["parameters"]
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone()),
            "actuator" => {
                candidate
                    .as_object_mut()
                    .unwrap()
                    .insert("actuator".into(), extra);
            }
            _ => unreachable!(),
        }
        let response = handle_request_value(
            json!({
                "id": 6901, "type": "plant.save", "params": {
                    "contract": "trust.plant", "schema_version": 1, "project_id": read["project_id"],
                    "operation_id": format!("unknown-{label}"), "expected_revision": read["configuration_revision"],
                    "plant": candidate,
                }
            }),
            &state,
            None,
        );
        assert!(
            !response.ok,
            "unknown {label} field must reject the candidate"
        );
        assert!(
            format!("{:?}", response.error).contains(key),
            "unknown {label} rejection should identify {key}: {:?}",
            response.error
        );
        assert_eq!(
            std::fs::read(root.join("simulation.toml")).expect("unchanged bytes"),
            before,
            "unknown {label} field rejection must not write simulation.toml"
        );
    }
    std::fs::remove_dir_all(root).expect("remove unknown-field fixture");
}
