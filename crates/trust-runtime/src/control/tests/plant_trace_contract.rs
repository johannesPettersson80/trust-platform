#[test]
fn plant_trace_clear_is_revision_guarded_idempotent_and_plant_scoped() {
    let root = temp_dir("plant-trace-clear");
    let simulation = r#"[simulation]
enabled = true
plant_step_ms = 10
plant_trace_capacity = 4

[[plants]]
id = "tank-a"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]
[plants.parameters]
gain = 1.0
time_constant_s = 5.0

[[plants]]
id = "tank-b"
model = "first_order"
command_sources = ["%QW2"]
feedback_targets = ["%IW2"]
[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#;
    write_file(&root.join("simulation.toml"), simulation);
    let saved_before = fs::read(root.join("simulation.toml")).unwrap();
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6190,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("read result");
    let revision = read
        .get("configuration_revision")
        .and_then(serde_json::Value::as_str)
        .expect("saved revision")
        .to_string();
    let config = crate::simulation::SimulationConfig::load(root.join("simulation.toml"))
        .expect("active config");
    let mut controller = crate::simulation::SimulationController::new(config)
        .with_active_configuration_revision(revision.clone());
    let mut runtime = crate::Runtime::new();
    for (name, address) in [
        ("command-a", "%QW0"),
        ("feedback-a", "%IW0"),
        ("command-b", "%QW2"),
        ("feedback-b", "%IW2"),
    ] {
        runtime.io_mut().bind_typed(
            name,
            crate::io::IoAddress::parse(address).unwrap(),
            trust_hir::TypeId::WORD,
        );
    }
    runtime
        .io_mut()
        .write(
            &crate::io::IoAddress::parse("%QW0").unwrap(),
            crate::value::Value::Word(100),
        )
        .unwrap();
    runtime
        .io_mut()
        .write(
            &crate::io::IoAddress::parse("%QW2").unwrap(),
            crate::value::Value::Word(200),
        )
        .unwrap();
    controller
        .apply_post_cycle(crate::value::Duration::from_millis(10), &runtime)
        .expect("advance both plants");
    let (resource, commands) = ResourceControl::stub(StdClock::new());
    state.resource = resource;
    std::thread::spawn(move || {
        while let Ok(command) = commands.recv() {
            match command {
                ResourceCommand::SimulationSnapshot { respond_to } => {
                    let _ = respond_to.send(Some(controller.runtime_snapshot()));
                }
                ResourceCommand::SimulationTraceClear {
                    plant_id,
                    active_configuration_revision,
                    respond_to,
                } => {
                    let _ = respond_to.send(controller.clear_plant_trace(
                        &plant_id,
                        &active_configuration_revision,
                    ));
                }
                _ => {}
            }
        }
    });
    let status_request = json!({
        "id": 6191,
        "type": "plant.status",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": read.get("project_id").unwrap()
        }
    });
    let status_before = handle_request_value(status_request.clone(), &state, None)
        .result
        .expect("status before clear");
    let clear_request = json!({
        "id": 6192,
        "type": "plant.trace.clear",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": read.get("project_id").unwrap(),
            "operation_id": "clear-tank-a-1",
            "plant_id": "tank-a",
            "active_configuration_revision": revision
        }
    });

    let first = handle_request_value(clear_request.clone(), &state, None);
    let retry = handle_request_value(clear_request, &state, None);

    assert!(first.ok, "plant.trace.clear failed: {:?}", first.error);
    assert_eq!(retry.result, first.result, "clear retry must replay the first result");
    let result = first.result.expect("clear result");
    assert_eq!(result.get("plant_id").and_then(serde_json::Value::as_str), Some("tank-a"));
    assert_eq!(result.get("prior_row_count").and_then(serde_json::Value::as_u64), Some(1));
    assert_eq!(result.get("prior_dropped_row_count").and_then(serde_json::Value::as_u64), Some(0));
    let trace = |plant_id: &str, request_id: u64| {
        handle_request_value(
            json!({
                "id": request_id,
                "type": "plant.trace.read",
                "params": {
                    "contract": "trust.plant",
                    "schema_version": 1,
                    "project_id": read.get("project_id").unwrap(),
                    "plant_id": plant_id
                }
            }),
            &state,
            None,
        )
        .result
        .expect("trace after clear")
    };
    assert_eq!(trace("tank-a", 6193).get("retained_row_count").and_then(serde_json::Value::as_u64), Some(0));
    assert_eq!(trace("tank-b", 6194).get("retained_row_count").and_then(serde_json::Value::as_u64), Some(1));
    assert_eq!(
        handle_request_value(status_request, &state, None).result,
        Some(status_before),
        "trace clear changed plant or simulation state"
    );
    assert_eq!(fs::read(root.join("simulation.toml")).unwrap(), saved_before);
    fs::remove_dir_all(root).expect("remove trace-clear fixture");
}
