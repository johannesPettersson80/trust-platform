#[test]
fn stopped_project_session_reads_configuration_and_reports_live_data_unavailable() {
    let root = temp_dir("plant-offline-session");
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
    let capabilities = session.request_json(json!({
        "id": 6700,
        "type": "plant.capabilities",
        "params": { "contract": "trust.plant", "schema_version": 1 }
    }));
    assert_eq!(capabilities.get("ok"), Some(&json!(true)));
    let project_id = capabilities
        .pointer("/result/project_id")
        .and_then(serde_json::Value::as_str)
        .expect("offline project identity")
        .to_string();

    let read = session.request_json(json!({
        "id": 6701,
        "type": "plant.read",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": project_id
        }
    }));
    assert_eq!(read.get("ok"), Some(&json!(true)));
    assert_eq!(read.pointer("/result/plants/0/id"), Some(&json!("tank")));

    let status = session.request_json(json!({
        "id": 6702,
        "type": "plant.status",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": project_id
        }
    }));
    assert_eq!(status.get("error_code"), Some(&json!("status_unavailable")));
    assert_eq!(status.pointer("/result/available"), Some(&json!(false)));
    assert!(status.pointer("/result/plants").is_none());
    fs::remove_dir_all(root).expect("remove offline plant fixture");
}

#[test]
fn stopped_project_session_reuses_runtime_validation_revision_and_replay_semantics() {
    let root = temp_dir("plant-offline-save");
    write_file(&root.join("simulation.toml"), "[simulation]\nenabled = true\n");
    let session = crate::control::OfflinePlantSession::new(root.clone());
    let read = session.request_json(json!({
        "id": 6710,
        "type": "plant.read",
        "params": { "contract": "trust.plant", "schema_version": 1 }
    }));
    let project_id = read.pointer("/result/project_id").unwrap().clone();
    let revision = read.pointer("/result/configuration_revision").unwrap().clone();
    let request = json!({
        "id": 6711,
        "type": "plant.save",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": project_id,
            "operation_id": "offline-save-1",
            "expected_revision": revision,
            "plant": {
                "id": "tank",
                "model": "first_order",
                "command_sources": ["%QW0"],
                "feedback_targets": ["%IW0"],
                "parameters": { "gain": 1.0, "time_constant_s": 5.0 }
            }
        }
    });
    let first = session.request_json(request.clone());
    let replay = session.request_json(request);
    assert_eq!(first.get("ok"), Some(&json!(true)));
    assert_eq!(replay.pointer("/result"), first.pointer("/result"));

    let conflicting = session.request_json(json!({
        "id": 6712,
        "type": "plant.save",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": project_id,
            "operation_id": "offline-save-1",
            "expected_revision": revision,
            "plant": {
                "id": "different",
                "model": "static_gain",
                "command_sources": ["%QW2"],
                "feedback_targets": ["%IW2"],
                "parameters": { "gain": 2.0 }
            }
        }
    }));
    assert_eq!(
        conflicting.get("error_code"),
        Some(&json!("idempotency_conflict"))
    );
    fs::remove_dir_all(root).expect("remove offline save fixture");
}
