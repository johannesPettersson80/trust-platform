#[test]
fn plant_remove_is_revision_guarded_atomic_and_preserves_other_configuration() {
    let root = temp_dir("plant-remove-atomic");
    let original = r#"# retained heading
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

# keep this plant and its event
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
            "id": 6600,
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
            "id": 6601,
            "type": "plant.remove",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "remove-tank-1",
                "expected_revision": read.get("configuration_revision").unwrap(),
                "plant_id": "tank"
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "plant.remove failed: {:?}", response.error);
    let result = response.result.expect("remove result");
    assert_eq!(result.get("operation"), Some(&json!("plant.remove")));
    assert_eq!(result.get("affected_plant_ids"), Some(&json!(["tank"])));
    assert_eq!(result.get("plant_count"), Some(&json!(1)));

    let saved = fs::read_to_string(root.join("simulation.toml")).expect("saved simulation file");
    assert!(saved.contains("# retained heading"));
    assert!(saved.contains("# keep this plant and its event"));
    let document: toml::Value = toml::from_str(&saved).expect("valid saved TOML");
    let plants = document.get("plants").and_then(toml::Value::as_array).unwrap();
    assert_eq!(plants.len(), 1);
    assert_eq!(plants[0].get("id").and_then(toml::Value::as_str), Some("oven"));
    let events = document
        .get("plant_events")
        .and_then(toml::Value::as_array)
        .expect("remaining plant events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].get("plant").and_then(toml::Value::as_str), Some("oven"));
    assert_eq!(
        fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count(),
        0,
        "successful atomic removal left a temporary file"
    );
    fs::remove_dir_all(root).expect("remove plant-remove fixture");
}

#[test]
fn plant_remove_failures_do_not_change_simulation_toml() {
    let root = temp_dir("plant-remove-failures");
    let original = r#"[simulation]
enabled = true

[[plants]]
id = "tank"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#;
    write_file(&root.join("simulation.toml"), original);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6610,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("read result");

    for (request_id, operation_id, revision, plant_id, expected_code) in [
        (6611, "stale-remove", json!("sha256:stale"), "tank", "revision_conflict"),
        (
            6612,
            "missing-remove",
            read.get("configuration_revision").unwrap().clone(),
            "missing",
            "plant_not_found",
        ),
    ] {
        let response = handle_request_value(
            json!({
                "id": request_id,
                "type": "plant.remove",
                "params": {
                    "contract": "trust.plant",
                    "schema_version": 1,
                    "project_id": read.get("project_id").unwrap(),
                    "operation_id": operation_id,
                    "expected_revision": revision,
                    "plant_id": plant_id
                }
            }),
            &state,
            None,
        );
        assert!(!response.ok, "{operation_id} must fail");
        assert_eq!(response.error_code.as_deref(), Some(expected_code));
        assert_eq!(fs::read_to_string(root.join("simulation.toml")).unwrap(), original);
    }

    fs::remove_dir_all(root).expect("remove plant-remove failures fixture");
}
