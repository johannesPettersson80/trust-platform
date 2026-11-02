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
                { "id": "tank", "operating_state": "running", "command": [0.5], "published_feedback": [0.25] }
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
