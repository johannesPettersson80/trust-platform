/// `plant.save` validates the candidate itself before it writes (specification 47 §20: it replaces
/// the configuration "after validation and revision checks"), so the editor sends no separate
/// `plant.validate` first. A candidate the simulation refuses is `validation_failed` with the
/// simulation's reason, `simulation.toml` stays as it was, and the refusal does not use up the
/// operation ID: the corrected plant saves under the same ID. Catches a save that writes a plant
/// the runtime cannot load, and a refused save remembered as the operation's result.
#[test]
fn plant_save_refuses_a_candidate_the_simulation_refuses_without_writing() {
    let root = temp_dir("plant-save-validation");
    let original = "# retained heading\n[simulation]\nenabled = true\n";
    write_file(&root.join("simulation.toml"), original);
    let mut state = control_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6700,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("read result");
    let save = |id: u64, time_constant_s: f64| {
        handle_request_value(
            json!({
                "id": id,
                "type": "plant.save",
                "params": {
                    "contract": "trust.plant",
                    "schema_version": 1,
                    "project_id": read.get("project_id").unwrap(),
                    "operation_id": "validated-save-1",
                    "expected_revision": read.get("configuration_revision").unwrap(),
                    "plant": {
                        "id": "tank",
                        "model": "first_order",
                        "command_sources": ["%QW0"],
                        "feedback_targets": ["%IW0"],
                        "parameters": { "gain": 1.0, "time_constant_s": time_constant_s }
                    }
                }
            }),
            &state,
            None,
        )
    };

    let refused = save(6701, 0.0);
    assert!(!refused.ok, "a zero time constant must be refused");
    assert_eq!(refused.error_code.as_deref(), Some("validation_failed"));
    assert!(
        refused
            .error
            .as_deref()
            .is_some_and(|error| error.contains("time_constant_s must be > 0")),
        "{:?}",
        refused.error
    );
    assert_eq!(
        fs::read_to_string(root.join("simulation.toml")).unwrap(),
        original
    );

    let saved = save(6702, 5.0);
    assert!(saved.ok, "the corrected plant.save failed: {:?}", saved.error);
    assert_eq!(
        saved
            .result
            .as_ref()
            .and_then(|result| result.get("affected_plant_ids")),
        Some(&json!(["tank"]))
    );
    assert!(fs::read_to_string(root.join("simulation.toml"))
        .unwrap()
        .contains("[[plants]]"));
    fs::remove_dir_all(root).expect("remove plant-save validation fixture");
}
