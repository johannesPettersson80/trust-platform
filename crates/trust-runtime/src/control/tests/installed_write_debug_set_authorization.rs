/// Spec 34 §4.3.1 live value writes: a debugger `set` into installed PLC storage needs the engineer role, and a refused request queues no write.
#[test]
fn installed_write_debugger_set_below_engineer_is_refused_before_any_write_is_queued() {
    let source = r#"
PROGRAM Main
VAR
    counter : DINT := DINT#1;
END_VAR
counter := counter + DINT#1;
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    state.debug_enabled.store(true, Ordering::Relaxed);

    let pairing_path = pairing_file("installed-write-debug-set");
    let store = Arc::new(PairingStore::load(pairing_path.clone()));
    state.pairing = Some(store.clone());
    let operator_code = store.start_pairing();
    let operator_token = store
        .claim(&operator_code.code, Some(AccessRole::Operator))
        .expect("operator token");

    let denied = handle_request_value(
        json!({
            "id": 1,
            "type": "set",
            "auth": operator_token,
            "params": { "target": "counter", "value": "5" }
        }),
        &state,
        None,
    );
    assert!(
        !denied.ok,
        "an operator must not write installed PLC storage"
    );
    assert_eq!(
        denied.error.as_deref(),
        Some("forbidden: requires role engineer")
    );
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "a refused debugger write must not reach the installed write queue"
    );

    let _ = fs::remove_file(pairing_path);
}
