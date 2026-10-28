#[test]
fn plant_save_round_trips_optional_name_and_description() {
    let root = temp_dir("plant-metadata-save-v1");
    let mut state = control_test_state(
        "PROGRAM Main\nVAR\n  command AT %QW0 : UINT;\n  process AT %IW0 : UINT;\nEND_VAR\nEND_PROGRAM",
    );
    state.project_root = Some(root.clone());

    let read = handle_request_value(
        json!({
            "id": 6180,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("initial plant.read result");
    let response = handle_request_value(
        json!({
            "id": 6181,
            "type": "plant.save",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "metadata-save-1",
                "expected_revision": read.get("configuration_revision").unwrap(),
                "plant": {
                    "id": "tank_level",
                    "name": "Training tank",
                    "description": "Slow level process for controller training",
                    "model": "first_order",
                    "command_sources": ["%QW0"],
                    "feedback_targets": ["%IW0"],
                    "parameters": { "gain": 1.0, "time_constant_s": 5.0 }
                }
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "plant.save failed: {:?}", response.error);
    let result = response.result.expect("plant.save result");
    let descriptor = result
        .get("plants")
        .and_then(serde_json::Value::as_array)
        .and_then(|plants| plants.first())
        .expect("saved plant descriptor");
    assert_eq!(
        descriptor
            .get("display_name")
            .and_then(serde_json::Value::as_str),
        Some("Training tank")
    );
    assert_eq!(
        descriptor
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Slow level process for controller training")
    );
    let saved = fs::read_to_string(root.join("simulation.toml")).expect("saved simulation.toml");
    assert!(saved.contains("name = \"Training tank\""));
    assert!(saved.contains("description = \"Slow level process for controller training\""));

    fs::remove_dir_all(root).expect("remove plant metadata fixture");
}
