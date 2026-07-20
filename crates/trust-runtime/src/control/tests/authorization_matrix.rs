#[test]
fn debug_activation_and_mutation_follow_the_reviewed_role_boundary() {
    let source = r#"
PROGRAM Main
VAR
    output_bit AT %QX0.0 : BOOL := FALSE;
END_VAR
output_bit := FALSE;
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    state.debug_enabled.store(false, Ordering::Relaxed);

    let pairing_path = pairing_file("debug-authorization-matrix");
    let store = Arc::new(PairingStore::load(pairing_path.clone()));
    state.pairing = Some(store.clone());

    let viewer_code = store.start_pairing();
    let viewer_token = store
        .claim(&viewer_code.code, Some(AccessRole::Viewer))
        .expect("viewer token");
    let operator_code = store.start_pairing();
    let operator_token = store
        .claim(&operator_code.code, Some(AccessRole::Operator))
        .expect("operator token");
    let engineer_code = store.start_pairing();
    let engineer_token = store
        .claim(&engineer_code.code, Some(AccessRole::Engineer))
        .expect("engineer token");

    for (id, role, token) in [
        (1, "viewer", viewer_token.as_str()),
        (2, "operator", operator_token.as_str()),
    ] {
        let denied = handle_request_value(
            json!({
                "id": id,
                "type": "io.force",
                "auth": token,
                "params": { "address": "%QX0.0", "value": "TRUE" }
            }),
            &state,
            None,
        );
        assert!(!denied.ok, "{role} must not force I/O");
        assert!(denied
            .error
            .as_deref()
            .is_some_and(|error| error.contains("requires role engineer")));
        assert!(state.debug.forced_snapshot().io.is_empty());
    }

    let engineer_enable = handle_request_value(
        json!({
            "id": 3,
            "type": "config.set",
            "auth": engineer_token,
            "params": { "control.debug_enabled": true }
        }),
        &state,
        None,
    );
    assert!(!engineer_enable.ok, "Engineer must not activate debug");
    assert!(engineer_enable
        .error
        .as_deref()
        .is_some_and(|error| error.contains("requires role admin")));
    assert!(!state.debug_enabled.load(Ordering::Relaxed));

    let admin_enable = handle_request_value(
        json!({
            "id": 4,
            "type": "config.set",
            "auth": "admin-token",
            "params": { "control.debug_enabled": true }
        }),
        &state,
        None,
    );
    assert!(admin_enable.ok, "Admin should activate debug: {admin_enable:?}");
    assert!(state.debug_enabled.load(Ordering::Relaxed));

    let engineer_force = handle_request_value(
        json!({
            "id": 5,
            "type": "io.force",
            "auth": engineer_token,
            "params": { "address": "%QX0.0", "value": "TRUE" }
        }),
        &state,
        None,
    );
    assert!(engineer_force.ok, "Engineer should force enabled debug I/O");
    assert_eq!(state.debug.forced_snapshot().io.len(), 1);

    let viewer_release = handle_request_value(
        json!({
            "id": 6,
            "type": "io.unforce",
            "auth": viewer_token,
            "params": { "address": "%QX0.0" }
        }),
        &state,
        None,
    );
    assert!(!viewer_release.ok, "Viewer must not release a force");
    assert_eq!(state.debug.forced_snapshot().io.len(), 1);

    let engineer_release = handle_request_value(
        json!({
            "id": 7,
            "type": "io.unforce",
            "auth": engineer_token,
            "params": { "address": "%QX0.0" }
        }),
        &state,
        None,
    );
    assert!(engineer_release.ok, "Engineer should release a force");
    assert!(state.debug.forced_snapshot().io.is_empty());

    let _ = fs::remove_file(pairing_path);
}
