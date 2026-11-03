// Catches a canvas topology the stopped-project session cannot answer, answers for another project
// or takes from a status that is not current: the extension asks the session with the owning
// runtime's `plant.status` answer (specification 47 §20.6, §20.7).
#[test]
fn stopped_project_session_projects_the_saved_plants_for_the_canvas() {
    let root = temp_dir("plant-topology-session");
    write_file(
        &root.join("simulation.toml"),
        r#"[simulation]
enabled = true

[[plants]]
id = "tank"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#,
    );
    let session = crate::control::OfflinePlantSession::new(root.clone());
    let read = session.request_json(json!({
        "id": 6900,
        "type": "plant.read",
        "params": { "contract": "trust.plant", "schema_version": 1 }
    }));
    let project_id = read
        .pointer("/result/project_id")
        .and_then(serde_json::Value::as_str)
        .expect("offline project identity")
        .to_string();
    let revision = read
        .pointer("/result/configuration_revision")
        .and_then(serde_json::Value::as_str)
        .expect("saved revision")
        .to_string();
    let topology = |id: u64, status: serde_json::Value| {
        session.request_json(json!({
            "id": id,
            "type": "plant.topology",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": project_id,
                "status": status
            }
        }))
    };

    // Without a project ID, as for `plant.read`: the answer names the project.
    let first = session.request_json(json!({
        "id": 6906,
        "type": "plant.topology",
        "params": { "contract": "trust.plant", "schema_version": 1 }
    }));
    assert_eq!(first.pointer("/result/project_id"), Some(&json!(project_id)));
    assert_eq!(first.pointer("/result/plants/0/id"), Some(&json!("tank")));

    // The session's own status: no runtime runs, so the plant's live status is unavailable.
    let status = session.request_json(json!({
        "id": 6901,
        "type": "plant.status",
        "params": { "contract": "trust.plant", "schema_version": 1, "project_id": project_id }
    }));
    assert_eq!(status.get("error_code"), Some(&json!("status_unavailable")));
    let stopped = topology(6902, status["result"].clone());
    assert_eq!(stopped.get("ok"), Some(&json!(true)), "{stopped}");
    assert_eq!(
        stopped.pointer("/result/operation"),
        Some(&json!("plant.topology"))
    );
    assert_eq!(
        stopped.pointer("/result/project_id"),
        Some(&json!(project_id))
    );
    assert_eq!(
        stopped.pointer("/result/configuration_revision"),
        Some(&json!(revision))
    );
    assert_eq!(stopped.pointer("/result/plants/0/id"), Some(&json!("tank")));
    assert_eq!(
        stopped.pointer("/result/plants/0/statusAvailability"),
        Some(&json!("unavailable"))
    );
    assert_eq!(
        stopped.pointer("/result/plants/0/states"),
        Some(&json!(["Status unavailable"]))
    );

    // A current status of the running runtime joins the saved plant it names.
    let running = topology(
        6903,
        json!({
            "contract": "trust.plant",
            "schema_version": 1,
            "operation": "plant.status",
            "project_id": project_id,
            "available": true,
            "active_configuration_revision": revision,
            "plants": [
                { "id": "tank", "operating_state": "running", "simulation_time_ns": "1000000", "command": [0.5], "published_feedback": [0.25] }
            ]
        }),
    );
    assert_eq!(
        running.pointer("/result/plants/0/states"),
        Some(&json!(["Running"]))
    );
    assert_eq!(
        running.pointer("/result/plants/0/liveSample"),
        Some(&json!({ "command": [0.5], "feedback": [0.25] }))
    );

    let foreign = session.request_json(json!({
        "id": 6904,
        "type": "plant.topology",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": "project:sha256:another"
        }
    }));
    assert_eq!(foreign.get("error_code"), Some(&json!("malformed_request")));
    let unknown_field = session.request_json(json!({
        "id": 6905,
        "type": "plant.topology",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": project_id,
            "plants": []
        }
    }));
    assert_eq!(
        unknown_field.get("error_code"),
        Some(&json!("malformed_request"))
    );
    fs::remove_dir_all(root).expect("remove plant topology fixture");
}

// Catches the runtime control surface projecting a status that a caller supplies instead of its
// own: on the runtime, `plant.topology` takes the runtime's `plant.status` (specification 47 §20.6).
#[test]
fn runtime_projects_its_own_plant_status() {
    let root = temp_dir("plant-topology-runtime");
    write_file(
        &root.join("simulation.toml"),
        r#"[simulation]
enabled = true
seed = 42

[[plants]]
id = "tank"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#,
    );
    let mut state = control_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6910,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("read result");
    let revision = read["configuration_revision"]
        .as_str()
        .expect("saved revision")
        .to_string();
    let config = crate::simulation::SimulationConfig::load(root.join("simulation.toml"))
        .expect("active simulation config");
    let snapshot = crate::simulation::SimulationController::new(config)
        .with_active_configuration_revision(revision)
        .runtime_snapshot();
    let (resource, commands) = ResourceControl::stub(StdClock::new());
    state.resource = resource;
    std::thread::spawn(move || {
        while let Ok(command) = commands.recv() {
            if let ResourceCommand::SimulationSnapshot { respond_to } = command {
                let _ = respond_to.send(Some(snapshot.clone()));
            }
        }
    });

    // A fresh simulation has not stepped: its own status says the tank is waiting for samples,
    // where no status would say it is unknown.
    let own = handle_request_value(
        json!({
            "id": 6911,
            "type": "plant.topology",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    );
    assert!(own.ok, "plant.topology failed: {:?}", own.error);
    let own = own.result.expect("topology result");
    assert_eq!(own.pointer("/plants/0/statusAvailability"), Some(&json!("available")));
    assert_eq!(own.pointer("/plants/0/states"), Some(&json!(["Waiting for samples"])));

    let supplied = handle_request_value(
        json!({
            "id": 6912,
            "type": "plant.topology",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "status": { "operation": "plant.status", "available": true }
            }
        }),
        &state,
        None,
    );
    assert_eq!(supplied.error_code.as_deref(), Some("malformed_request"));
    fs::remove_dir_all(root).expect("remove plant topology runtime fixture");
}

// Catches the data loss of checklist G33: opening a saved plant whose actuator is on-off in the
// editor and saving it unchanged deleted its actuator (specification 47 §13.2). The path is the
// extension's: the canvas's projection, the editor's draft and `open`, its candidate, `plant.save`.
#[test]
fn a_reopened_on_off_actuator_survives_saving() {
    let root = temp_dir("plant-topology-on-off");
    write_file(
        &root.join("simulation.toml"),
        r#"[simulation]
enabled = true

[[plants]]
id = "heater"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0

[plants.actuator]
kind = "on-off"
minimum = 0.0
maximum = 100.0
minimum_on_s = 20.0
minimum_off_s = 10.0
"#,
    );
    let session = crate::control::OfflinePlantSession::new(root.clone());
    let topology = session.request_json(json!({
        "id": 6920,
        "type": "plant.topology",
        "params": { "contract": "trust.plant", "schema_version": 1 }
    }));
    let result = &topology["result"];
    let project_id = result["project_id"].clone();
    let revision = result["configuration_revision"].clone();
    let plant = &result["plants"][0];
    // The editor's draft as the extension builds it from the projection (`createPlantEditDraft`).
    let mut draft = plant["configuration"]
        .as_object()
        .expect("the heater opens in the editor")
        .clone();
    for (key, value) in [
        ("id", json!("plant:heater")),
        ("projectId", project_id.clone()),
        ("originalPlantId", plant["id"].clone()),
        ("plantId", plant["id"].clone()),
        ("displayName", plant["displayName"].clone()),
        ("runtimeId", json!("simulator")),
        ("runtimeName", json!("Simulator")),
        ("enabled", plant["enabled"].clone()),
        ("startingPoint", json!("")),
        ("model", plant["model"].clone()),
        ("modelValuesTouched", json!(false)),
        ("commandSources", plant["commandSources"].clone()),
        ("feedbackTargets", plant["feedbackTargets"].clone()),
        ("savedConfigurationRevision", revision.clone()),
    ] {
        draft.insert(key.into(), value);
    }
    let opened = session.request_json(json!({
        "id": 6921,
        "type": "plant.edit",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": project_id,
            "draft": draft,
            "action": { "kind": "open" }
        }
    }));
    assert_eq!(opened.get("ok"), Some(&json!(true)), "{opened}");
    let saved = session.request_json(json!({
        "id": 6922,
        "type": "plant.save",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": project_id,
            "operation_id": "reopen-on-off",
            "expected_revision": revision,
            "plant": opened["result"]["candidate"]
        }
    }));
    assert_eq!(saved.get("ok"), Some(&json!(true)), "{saved}");

    let file = fs::read_to_string(root.join("simulation.toml")).expect("saved simulation.toml");
    let document: toml::Value = toml::from_str(&file).expect("saved TOML");
    let actuator = &document["plants"][0]["actuator"];
    assert_eq!(actuator.get("kind").and_then(toml::Value::as_str), Some("on-off"));
    for (key, expected) in [
        ("minimum", 0.0),
        ("maximum", 100.0),
        ("minimum_on_s", 20.0),
        ("minimum_off_s", 10.0),
    ] {
        assert_eq!(
            actuator.get(key).and_then(toml::Value::as_float),
            Some(expected),
            "{key}"
        );
    }
    fs::remove_dir_all(root).expect("remove on-off actuator fixture");
}
