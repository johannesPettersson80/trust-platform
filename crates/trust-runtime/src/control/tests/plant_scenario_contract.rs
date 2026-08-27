#[test]
fn plant_save_replaces_only_the_owning_plants_ordered_scenario_events() {
    let root = temp_dir("plant-save-scenario-events");
    let original = r#"
[simulation]
enabled = true

[[plants]]
id = "tank"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0

[[plants]]
id = "oven"
model = "first_order"
command_sources = ["%QW2"]
feedback_targets = ["%IW2"]

[plants.parameters]
gain = 2.0
time_constant_s = 20.0

[[plant_events]]
at_ms = 100
plant = "tank"
kind = "set_state"
channel = 0
value = 3.0

[[plant_events]]
at_ms = 250
plant = "oven"
kind = "reset_plant"
"#;
    write_file(&root.join("simulation.toml"), original);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6380,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("read result");

    let response = handle_request_value(
        json!({
            "id": 6381,
            "type": "plant.save",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "replace-tank-events",
                "expected_revision": read.get("configuration_revision").unwrap(),
                "plant": {
                    "id": "tank",
                    "model": "first_order",
                    "command_sources": ["%QW0"],
                    "feedback_targets": ["%IW0"],
                    "parameters": { "gain": 1.0, "time_constant_s": 5.0 },
                    "scenario_events": [
                        { "at_ms": 500, "kind": "add_state", "channel": 0, "value": 1.5 },
                        { "at_ms": 500, "kind": "set_parameter", "parameter": "gain", "value": 0.8 }
                    ]
                }
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "plant.save failed: {:?}", response.error);
    let saved = fs::read_to_string(root.join("simulation.toml")).expect("saved simulation file");
    assert!(!saved.contains("scenario_events"), "request-only field leaked into [[plants]]");
    let document: toml::Value = toml::from_str(&saved).expect("saved TOML");
    let events = document
        .get("plant_events")
        .and_then(toml::Value::as_array)
        .expect("global plant events");
    assert_eq!(events.len(), 3, "unrelated event plus two replacement events");
    assert_eq!(events[0].get("plant").and_then(toml::Value::as_str), Some("oven"));
    assert_eq!(events[0].get("kind").and_then(toml::Value::as_str), Some("reset_plant"));
    assert_eq!(events[1].get("plant").and_then(toml::Value::as_str), Some("tank"));
    assert_eq!(events[1].get("kind").and_then(toml::Value::as_str), Some("add_state"));
    assert_eq!(events[2].get("plant").and_then(toml::Value::as_str), Some("tank"));
    assert_eq!(events[2].get("kind").and_then(toml::Value::as_str), Some("set_parameter"));

    fs::remove_dir_all(root).expect("remove scenario-event fixture");
}
