// Included into control/tests.rs with the sibling test files, so the helpers and imports they use are in scope.

/// Spec 34 §4.3.1: a checked write refused by the caller's access role is rejected at the role gate, before the forcing path, and changes no forced destination.
#[test]
fn operator_variable_force_is_refused_at_the_role_gate_and_forces_nothing() {
    let source = r#"
VAR_GLOBAL
    counter : INT := 0;
END_VAR
PROGRAM Main
counter := counter + 1;
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    // Debug is enabled, so only the caller's role decides this request.
    state.debug_enabled.store(true, Ordering::Relaxed);

    let pairing_path = pairing_file("checked-admission-access-policy");
    let store = Arc::new(PairingStore::load(pairing_path.clone()));
    state.pairing = Some(store.clone());
    let operator_code = store.start_pairing();
    let operator_token = store
        .claim(&operator_code.code, Some(AccessRole::Operator))
        .expect("operator token");
    assert!(state.debug.forced_snapshot().vars.is_empty());

    let response = handle_request_value(
        json!({
            "id": 1,
            "type": "var.force",
            "auth": operator_token,
            "params": { "target": "global:counter", "value": 9 }
        }),
        &state,
        None,
    );

    assert!(!response.ok, "an operator must not force a variable");
    assert_eq!(response.error_code.as_deref(), Some("insufficient_role"));
    assert_eq!(
        response.error.as_deref(),
        Some("forbidden: requires role engineer")
    );
    assert!(state.debug.forced_snapshot().vars.is_empty());
    let _ = fs::remove_file(pairing_path);
}
