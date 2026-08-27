#[test]
fn plant_contract_enforces_viewer_operator_and_engineer_roles_before_dispatch() {
    let root = temp_dir("plant-auth-boundary");
    write_file(&root.join("simulation.toml"), "[simulation]\nenabled = true\n");
    let mut state = hmi_test_state("PROGRAM Main\nEND_PROGRAM");
    state.project_root = Some(root.clone());
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    let pairing_path = pairing_file("plant-auth-boundary");
    let store = Arc::new(PairingStore::load(pairing_path.clone()));
    state.pairing = Some(store.clone());
    let token = |role| {
        let code = store.start_pairing();
        store.claim(&code.code, Some(role)).expect("paired role token")
    };
    let viewer = token(AccessRole::Viewer);
    let operator = token(AccessRole::Operator);
    let engineer = token(AccessRole::Engineer);

    let viewer_read = handle_request_value(
        json!({
            "id": 6600,
            "type": "plant.read",
            "auth": viewer,
            "params": { "contract": "trust.plant", "schema_version": 1 }
        }),
        &state,
        Some("tcp:viewer"),
    );
    assert!(viewer_read.ok, "viewer read failed: {:?}", viewer_read.error);
    let read = viewer_read.result.expect("read result");

    for (id, auth, role) in [
        (6601, viewer.as_str(), "viewer"),
        (6602, operator.as_str(), "operator"),
    ] {
        let denied = handle_request_value(
            json!({
                "id": id,
                "type": "plant.save",
                "auth": auth,
                "params": {
                    "contract": "trust.plant",
                    "schema_version": 1,
                    "project_id": read.get("project_id").unwrap(),
                    "operation_id": "unauthorized-save",
                    "expected_revision": read.get("configuration_revision").unwrap(),
                    "plant": { "id": "invalid-on-purpose" }
                }
            }),
            &state,
            Some("tcp:restricted"),
        );
        assert_eq!(denied.error_code.as_deref(), Some("insufficient_role"), "{role}");
        let result = denied.result.expect("structured plant authorization result");
        assert_eq!(result.get("contract"), Some(&json!("trust.plant")));
        assert_eq!(result.get("schema_version"), Some(&json!(1)));
        assert_eq!(result.get("operation"), Some(&json!("plant.save")));
        assert_eq!(result.get("project_id"), read.get("project_id"));
    }
    assert_eq!(
        fs::read_to_string(root.join("simulation.toml")).unwrap(),
        "[simulation]\nenabled = true\n"
    );

    let viewer_clear = handle_request_value(
        json!({
            "id": 6603,
            "type": "plant.trace.clear",
            "auth": viewer,
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "viewer-clear",
                "plant_id": "tank",
                "active_configuration_revision": "sha256:none"
            }
        }),
        &state,
        Some("tcp:viewer"),
    );
    assert_eq!(viewer_clear.error_code.as_deref(), Some("insufficient_role"));

    let operator_clear = handle_request_value(
        json!({
            "id": 6604,
            "type": "plant.trace.clear",
            "auth": operator,
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "operation_id": "operator-clear",
                "plant_id": "tank",
                "active_configuration_revision": "sha256:none"
            }
        }),
        &state,
        Some("tcp:operator"),
    );
    assert_eq!(
        operator_clear.error_code.as_deref(),
        Some("status_unavailable"),
        "operator must pass authorization before runtime evidence is checked"
    );

    let engineer_validate = handle_request_value(
        json!({
            "id": 6605,
            "type": "plant.validate",
            "auth": engineer,
            "params": {
                "contract": "trust.plant",
                "schema_version": 1,
                "project_id": read.get("project_id").unwrap(),
                "plant": { "id": "invalid-on-purpose" }
            }
        }),
        &state,
        Some("tcp:engineer"),
    );
    assert_ne!(engineer_validate.error_code.as_deref(), Some("insufficient_role"));

    let _ = fs::remove_file(pairing_path);
    fs::remove_dir_all(root).expect("remove plant auth fixture");
}
