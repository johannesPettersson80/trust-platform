#[test]
fn plant_contract_read_returns_the_versioned_saved_configuration_envelope() {
    let root = temp_dir("plant-contract-read-v1");
    write_file(
        &root.join("simulation.toml"),
        r#"
[simulation]
enabled = true
plant_step_ms = 10

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
    let mut state = hmi_test_state(
        "PROGRAM Main\nVAR\n  command AT %QW0 : UINT;\n  process AT %IW0 : UINT;\nEND_VAR\nEND_PROGRAM",
    );
    state.project_root = Some(root.clone());

    let response = handle_request_value(
        json!({
            "id": 6101,
            "type": "plant.read",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "plant.read failed: {:?}", response.error);
    let result = response.result.expect("plant.read result");
    assert_eq!(result.get("contract").and_then(serde_json::Value::as_str), Some("trust.plant"));
    assert_eq!(result.get("schema_version").and_then(serde_json::Value::as_u64), Some(1));
    assert_eq!(result.get("operation").and_then(serde_json::Value::as_str), Some("plant.read"));
    let project_id = result
        .get("project_id")
        .and_then(serde_json::Value::as_str)
        .expect("opaque project identity");
    assert!(project_id.starts_with("project:sha256:"));
    assert!(!project_id.contains(root.to_string_lossy().as_ref()));
    let revision = result
        .get("configuration_revision")
        .and_then(serde_json::Value::as_str)
        .expect("configuration revision");
    assert!(revision.starts_with("sha256:"), "unexpected revision: {revision}");
    let plants = result
        .get("plants")
        .and_then(serde_json::Value::as_array)
        .expect("saved plant descriptors");
    assert_eq!(plants.len(), 1);
    assert_eq!(plants[0].get("id").and_then(serde_json::Value::as_str), Some("tank"));

    fs::remove_dir_all(root).expect("remove plant contract fixture");
}

#[test]
fn plant_capabilities_reports_the_closed_runtime_surface() {
    let root = temp_dir("plant-capabilities-v1");
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());

    let response = handle_request_value(
        json!({
            "id": 6100,
            "type": "plant.capabilities",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    );

    assert!(response.ok, "plant.capabilities failed: {:?}", response.error);
    let result = response.result.expect("capabilities result");
    assert_eq!(result.get("contract").and_then(serde_json::Value::as_str), Some("trust.plant"));
    assert_eq!(result.get("schema_version").and_then(serde_json::Value::as_u64), Some(1));
    assert!(
        result
            .get("project_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("project:sha256:"))
    );
    assert_eq!(
        result.get("model_kinds"),
        Some(&json!([
            "static_gain",
            "first_order",
            "first_order_dead_time",
            "integrator",
            "piecewise_linear",
            "second_order",
            "state_space"
        ]))
    );
    assert_eq!(
        result.pointer("/limits/max_trace_rows_per_plant").and_then(serde_json::Value::as_u64),
        Some(100_000)
    );
    assert_eq!(
        result.get("operations"),
        Some(&json!([
            "plant.capabilities",
            "plant.read",
            "plant.validate",
            "plant.save",
            "plant.remove",
            "plant.status",
            "plant.trace.read",
            "plant.trace.clear"
        ]))
    );
    fs::remove_dir_all(root).expect("remove capabilities fixture");
}

#[test]
fn plant_status_unavailable_contains_no_fabricated_runtime_values() {
    let root = temp_dir("plant-status-unavailable");
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let project_id = handle_request_value(
        json!({
            "id": 6150,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .and_then(|result| result.get("project_id").cloned())
    .expect("project identity");

    let response = handle_request_value(
        json!({
            "id": 6151,
            "type": "plant.status",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": project_id
            }
        }),
        &state,
        None,
    );

    assert!(!response.ok, "missing runtime evidence must not report success");
    assert_eq!(response.error_code.as_deref(), Some("status_unavailable"));
    let unavailable = response.result.expect("structured unavailable result");
    assert_eq!(unavailable.get("contract").and_then(serde_json::Value::as_str), Some("trust.plant"));
    assert_eq!(unavailable.get("operation").and_then(serde_json::Value::as_str), Some("plant.status"));
    assert_eq!(unavailable.get("available").and_then(serde_json::Value::as_bool), Some(false));
    assert_eq!(
        unavailable.get("reason_code").and_then(serde_json::Value::as_str),
        Some("runtime_evidence_unavailable")
    );
    for forbidden in ["simulation_time", "plants", "samples", "healthy", "running"] {
        assert!(unavailable.get(forbidden).is_none(), "fabricated field: {forbidden}");
    }
    fs::remove_dir_all(root).expect("remove unavailable-status fixture");
}

#[test]
fn plant_status_returns_exact_active_runtime_evidence() {
    let root = temp_dir("plant-status-active");
    let simulation = r#"[simulation]
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
"#;
    write_file(&root.join("simulation.toml"), simulation);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6160,
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
        .expect("active simulation config");
    let snapshot = crate::simulation::SimulationController::new(config)
        .with_active_configuration_revision(revision.clone())
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

    let response = handle_request_value(
        json!({
            "id": 6161,
            "type": "plant.status",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "plant_id": "tank"
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "active plant.status failed: {:?}", response.error);
    let result = response.result.expect("active status result");
    assert_eq!(result.get("available").and_then(serde_json::Value::as_bool), Some(true));
    assert_eq!(
        result.get("active_configuration_revision").and_then(serde_json::Value::as_str),
        Some(revision.as_str())
    );
    assert_eq!(result.get("effective_seed").and_then(serde_json::Value::as_str), Some("42"));
    assert_eq!(
        result
            .get("random_algorithm_version")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(result.get("simulation_time_ns").and_then(serde_json::Value::as_str), Some("0"));
    let plants = result.get("plants").and_then(serde_json::Value::as_array).unwrap();
    assert_eq!(plants.len(), 1);
    assert_eq!(plants[0].get("id").and_then(serde_json::Value::as_str), Some("tank"));
    assert_eq!(plants[0].get("model").and_then(serde_json::Value::as_str), Some("first_order"));
    assert_eq!(plants[0].get("operating_state").and_then(serde_json::Value::as_str), Some("running"));
    assert_eq!(plants[0].get("state"), Some(&json!([0.0])));
    assert_eq!(plants[0].get("command"), Some(&json!([0.0])));
    assert_eq!(plants[0].get("measured_feedback"), Some(&json!([0.0])));
    assert_eq!(plants[0].get("step_ns").and_then(serde_json::Value::as_str), Some("10000000"));
    assert_eq!(
        plants[0]
            .get("accumulated_remainder_ns")
            .and_then(serde_json::Value::as_str),
        Some("0")
    );
    fs::remove_dir_all(root).expect("remove active-status fixture");
}

#[test]
fn plant_trace_read_reports_disabled_without_fabricating_rows() {
    let root = temp_dir("plant-trace-disabled");
    let simulation = r#"[simulation]
enabled = true
plant_trace_capacity = 0

[[plants]]
id = "tank"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#;
    write_file(&root.join("simulation.toml"), simulation);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6170,
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

    let response = handle_request_value(
        json!({
            "id": 6171,
            "type": "plant.trace.read",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "plant_id": "tank"
            }
        }),
        &state,
        None,
    );

    assert!(!response.ok, "disabled trace must not report an empty success");
    assert_eq!(response.error_code.as_deref(), Some("trace_disabled"));
    let result = response.result.expect("structured trace-disabled result");
    assert_eq!(result.get("operation").and_then(serde_json::Value::as_str), Some("plant.trace.read"));
    assert_eq!(result.get("plant_id").and_then(serde_json::Value::as_str), Some("tank"));
    assert!(result.get("rows").is_none(), "disabled trace fabricated rows");
    fs::remove_dir_all(root).expect("remove trace-disabled fixture");
}

#[test]
fn plant_trace_read_returns_an_immutable_plant_scoped_runtime_snapshot() {
    let root = temp_dir("plant-trace-snapshot");
    let simulation = r#"[simulation]
enabled = true
seed = 42
plant_step_ms = 10
plant_trace_capacity = 4

[[plants]]
id = "tank"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#;
    write_file(&root.join("simulation.toml"), simulation);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
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
    let command = crate::io::IoAddress::parse("%QW0").unwrap();
    let feedback = crate::io::IoAddress::parse("%IW0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), trust_hir::TypeId::WORD);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback, trust_hir::TypeId::WORD);
    runtime
        .io_mut()
        .write(&command, crate::value::Value::Word(100))
        .unwrap();
    controller
        .apply_post_cycle(crate::value::Duration::from_millis(10), &runtime)
        .expect("advance active plant");
    let snapshot = controller.runtime_snapshot();
    let (resource, commands) = ResourceControl::stub(StdClock::new());
    state.resource = resource;
    std::thread::spawn(move || {
        while let Ok(command) = commands.recv() {
            if let ResourceCommand::SimulationSnapshot { respond_to } = command {
                let _ = respond_to.send(Some(snapshot.clone()));
            }
        }
    });
    let request = json!({
        "id": 6181,
        "type": "plant.trace.read",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": read.get("project_id").unwrap(),
            "plant_id": "tank"
        }
    });

    let first = handle_request_value(request.clone(), &state, None);
    let second = handle_request_value(request, &state, None);

    assert!(first.ok, "plant.trace.read failed: {:?}", first.error);
    assert_eq!(second.result, first.result, "reading a trace changed its snapshot");
    let result = first.result.expect("trace result");
    assert_eq!(result.get("active_configuration_revision").and_then(serde_json::Value::as_str), Some(revision.as_str()));
    assert_eq!(result.get("capacity").and_then(serde_json::Value::as_u64), Some(4));
    assert_eq!(result.get("retained_row_count").and_then(serde_json::Value::as_u64), Some(1));
    assert_eq!(result.get("dropped_row_count").and_then(serde_json::Value::as_u64), Some(0));
    assert_eq!(result.get("effective_seed").and_then(serde_json::Value::as_str), Some("42"));
    assert!(result.get("configuration_digest").and_then(serde_json::Value::as_str).is_some());
    let rows = result.get("rows").and_then(serde_json::Value::as_array).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("sequence").and_then(serde_json::Value::as_str), Some("0"));
    assert_eq!(rows[0].get("plant_id").and_then(serde_json::Value::as_str), Some("tank"));
    assert_eq!(
        rows[0]
            .get("simulation_time_ns")
            .and_then(serde_json::Value::as_str),
        Some("10000000")
    );
    assert_eq!(rows[0].get("command"), Some(&json!([100.0])));
    assert_eq!(rows[0].get("applied_input"), Some(&json!([100.0])));
    fs::remove_dir_all(root).expect("remove trace-snapshot fixture");
}

#[test]
fn plant_contract_mutations_reject_unknown_fields_without_writing() {
    let root = temp_dir("plant-contract-closed-mutation");
    let original = "[simulation]\nenabled = true\n";
    write_file(&root.join("simulation.toml"), original);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());

    let response = handle_request_value(
        json!({
            "id": 6102,
            "type": "plant.save",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": "fixture-project",
                "operation_id": "save-1",
                "expected_revision": "sha256:stale",
                "plant": { "id": "tank" },
                "unexpected": true
            }
        }),
        &state,
        None,
    );

    assert!(!response.ok, "unknown mutating field must be rejected");
    assert_eq!(response.error_code.as_deref(), Some("malformed_request"));
    let error_result = response.result.expect("structured plant error result");
    assert_eq!(error_result.get("contract"), Some(&json!("trust.plant")));
    assert_eq!(error_result.get("operation"), Some(&json!("plant.save")));
    assert!(
        error_result
            .get("project_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("project:sha256:"))
    );
    assert_eq!(fs::read_to_string(root.join("simulation.toml")).unwrap(), original);
    fs::remove_dir_all(root).expect("remove closed-mutation fixture");
}

#[test]
fn plant_contract_operations_use_the_specified_role_boundaries() {
    for operation in [
        "plant.capabilities",
        "plant.read",
        "plant.status",
        "plant.trace.read",
    ] {
        assert_eq!(
            required_role_for_control_request(operation, None),
            AccessRole::Viewer,
            "{operation} must be readable by a viewer"
        );
    }
    for operation in ["plant.validate", "plant.save", "plant.remove"] {
        assert_eq!(
            required_role_for_control_request(operation, None),
            AccessRole::Engineer,
            "{operation} must require an engineer"
        );
    }
    assert_eq!(
        required_role_for_control_request("plant.trace.clear", None),
        AccessRole::Operator,
        "trace clearing must require an operator"
    );
}

#[test]
fn plant_contract_rejects_unknown_operations_with_the_stable_code() {
    let root = temp_dir("plant-unsupported-operation");
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let response = handle_request_value(
        json!({
            "id": 6103,
            "type": "plant.unknown",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    );
    assert_eq!(response.error_code.as_deref(), Some("unsupported_operation"));
    assert_eq!(
        response
            .result
            .as_ref()
            .and_then(|result| result.get("operation"))
            .and_then(serde_json::Value::as_str),
        Some("plant.unknown")
    );
    fs::remove_dir_all(root).expect("remove unsupported-operation fixture");
}

#[test]
fn plant_validate_is_side_effect_free_for_a_valid_complete_candidate() {
    let root = temp_dir("plant-validate-side-effect-free");
    let original = "# retained heading\n[simulation]\nenabled = true\n";
    write_file(&root.join("simulation.toml"), original);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let project_id = handle_request_value(
        json!({
            "id": 6200,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .and_then(|value| value.get("project_id").cloned())
    .expect("project identity");

    let response = handle_request_value(
        json!({
            "id": 6201,
            "type": "plant.validate",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": project_id,
                "plant": {
                    "id": "tank",
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

    assert!(response.ok, "plant.validate failed: {:?}", response.error);
    let result = response.result.expect("validation result");
    assert_eq!(result.get("valid").and_then(serde_json::Value::as_bool), Some(true));
    assert_eq!(result.get("issues").and_then(serde_json::Value::as_array).map(Vec::len), Some(0));
    assert_eq!(fs::read_to_string(root.join("simulation.toml")).unwrap(), original);
    fs::remove_dir_all(root).expect("remove validation fixture");
}

#[test]
fn plant_save_is_revision_guarded_atomic_and_preserves_unrelated_text() {
    let root = temp_dir("plant-save-atomic");
    let original = "# retained heading\n[simulation]\nenabled = true\n\n# unrelated rule\n[[couplings]]\nsource = \"%QX2.0\"\ntarget = \"%IX2.0\"\n";
    write_file(&root.join("simulation.toml"), original);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6300,
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
            "id": 6301,
            "type": "plant.save",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "save-tank-1",
                "expected_revision": read.get("configuration_revision").unwrap(),
                "plant": {
                    "id": "tank",
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
    let result = response.result.expect("save result");
    assert_eq!(result.get("restart_required").and_then(serde_json::Value::as_bool), Some(false));
    assert_ne!(
        result.get("configuration_revision"),
        read.get("configuration_revision"),
        "save must return the new exact-file revision"
    );
    let saved = fs::read_to_string(root.join("simulation.toml")).expect("saved simulation file");
    assert!(saved.contains("# retained heading"));
    assert!(saved.contains("# unrelated rule"));
    assert!(saved.contains("[[couplings]]"));
    assert!(saved.contains("[[plants]]"));
    crate::simulation::SimulationConfig::load(root.join("simulation.toml"))
        .expect("atomically written complete simulation config");
    let temporary_files = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .count();
    assert_eq!(temporary_files, 0, "successful atomic save left a temporary file");
    fs::remove_dir_all(root).expect("remove save fixture");
}

#[test]
fn plant_save_reports_the_unchanged_active_revision_and_restart_requirement() {
    let root = temp_dir("plant-save-active-restart");
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
            "id": 6350,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("read result");
    let active_revision = read
        .get("configuration_revision")
        .and_then(serde_json::Value::as_str)
        .expect("active revision")
        .to_string();
    let config = crate::simulation::SimulationConfig::load(root.join("simulation.toml"))
        .expect("active config");
    let snapshot = crate::simulation::SimulationController::new(config)
        .with_active_configuration_revision(active_revision.clone())
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

    let response = handle_request_value(
        json!({
            "id": 6351,
            "type": "plant.save",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "active-save-1",
                "expected_revision": read.get("configuration_revision").unwrap(),
                "plant": {
                    "id": "tank",
                    "model": "first_order",
                    "command_sources": ["%QW0"],
                    "feedback_targets": ["%IW0"],
                    "parameters": { "gain": 2.0, "time_constant_s": 5.0 }
                }
            }
        }),
        &state,
        None,
    );

    assert!(response.ok, "active plant.save failed: {:?}", response.error);
    let result = response.result.expect("active save result");
    assert_eq!(result.get("restart_required").and_then(serde_json::Value::as_bool), Some(true));
    assert_eq!(
        result.get("active_configuration_revision").and_then(serde_json::Value::as_str),
        Some(active_revision.as_str())
    );
    assert_ne!(
        result.get("configuration_revision").and_then(serde_json::Value::as_str),
        Some(active_revision.as_str())
    );
    assert_eq!(result.get("affected_plant_ids"), Some(&json!(["tank"])));
    fs::remove_dir_all(root).expect("remove active-save fixture");
}

#[test]
fn plant_save_rejects_a_stale_revision_and_returns_current_reload_data() {
    let root = temp_dir("plant-save-revision-conflict");
    let current = r#"[simulation]
enabled = true

[[plants]]
id = "current-tank"
model = "first_order"
command_sources = ["%QW0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0
"#;
    write_file(&root.join("simulation.toml"), current);
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6400,
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
            "id": 6401,
            "type": "plant.save",
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "stale-save-1",
                "expected_revision": "sha256:stale",
                "plant": {
                    "id": "current-tank",
                    "model": "first_order",
                    "command_sources": ["%QW2"],
                    "feedback_targets": ["%IW2"],
                    "parameters": { "gain": 2.0, "time_constant_s": 7.0 }
                }
            }
        }),
        &state,
        None,
    );

    assert!(!response.ok, "stale save must be rejected");
    assert_eq!(response.error_code.as_deref(), Some("revision_conflict"));
    let conflict = response.result.expect("current reload data");
    assert_eq!(
        conflict.get("configuration_revision"),
        read.get("configuration_revision")
    );
    let descriptor = conflict
        .get("current_plant")
        .and_then(serde_json::Value::as_object)
        .expect("current saved plant descriptor");
    assert_eq!(
        descriptor.get("id").and_then(serde_json::Value::as_str),
        Some("current-tank")
    );
    assert_eq!(fs::read_to_string(root.join("simulation.toml")).unwrap(), current);
    fs::remove_dir_all(root).expect("remove revision-conflict fixture");
}

#[test]
fn plant_save_retry_with_the_same_operation_returns_the_original_result() {
    let root = temp_dir("plant-save-idempotent-retry");
    write_file(&root.join("simulation.toml"), "[simulation]\nenabled = true\n");
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    let read = handle_request_value(
        json!({
            "id": 6500,
            "type": "plant.read",
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        None,
    )
    .result
    .expect("read result");
    let request = json!({
        "id": 6501,
        "type": "plant.save",
        "params": {
            "contract": "trust.plant",
            "schema_version": 1,
            "project_id": read.get("project_id").unwrap(),
            "operation_id": "idempotent-save-1",
            "expected_revision": read.get("configuration_revision").unwrap(),
            "plant": {
                "id": "tank",
                "model": "first_order",
                "command_sources": ["%QW0"],
                "feedback_targets": ["%IW0"],
                "parameters": { "gain": 1.0, "time_constant_s": 5.0 }
            }
        }
    });

    let first = handle_request_value(request.clone(), &state, None);
    assert!(first.ok, "initial plant.save failed: {:?}", first.error);
    let saved_after_first = fs::read(root.join("simulation.toml")).unwrap();
    let second = handle_request_value(request, &state, None);

    assert!(second.ok, "identical retry failed: {:?}", second.error);
    assert_eq!(second.result, first.result, "retry must return the original result");
    assert_eq!(
        fs::read(root.join("simulation.toml")).unwrap(),
        saved_after_first,
        "retry must not rewrite the saved configuration"
    );
    fs::remove_dir_all(root).expect("remove idempotent retry fixture");
}
