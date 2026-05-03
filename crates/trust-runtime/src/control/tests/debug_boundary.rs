#[test]
fn debug_boundary_requests_fail_closed_for_stale_or_missing_names() {
    let source = r#"
PROGRAM Main
VAR
    drive : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    assert!(state.debug.snapshot().is_some(), "debug snapshot");

    let stale_frame = handle_request_value(
        json!({
            "id": 10,
            "type": "debug.scopes",
            "params": { "frame_id": 9999 }
        }),
        &state,
        None,
    );
    assert!(!stale_frame.ok, "stale frame must fail closed");
    assert!(
        stale_frame
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unknown frame id")),
        "unexpected stale-frame error: {:?}",
        stale_frame.error
    );

    let unknown_reference = handle_request_value(
        json!({
            "id": 11,
            "type": "debug.variables",
            "params": { "variables_reference": 4242 }
        }),
        &state,
        None,
    );
    assert!(
        !unknown_reference.ok,
        "unknown variable handle must not return an empty successful list"
    );
    assert!(
        unknown_reference
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unknown variables reference")),
        "unexpected variable-reference error: {:?}",
        unknown_reference.error
    );

    let unknown_eval = handle_request_value(
        json!({
            "id": 12,
            "type": "debug.evaluate",
            "params": { "expression": "not_declared" }
        }),
        &state,
        None,
    );
    assert!(!unknown_eval.ok, "unknown debug eval name must fail closed");
}

#[test]
fn debug_boundary_io_snapshot_poison_is_an_error() {
    let source = r#"
PROGRAM Main
VAR
    drive : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    assert!(state.debug.snapshot().is_some(), "debug snapshot");

    let poisoned = state.io_snapshot.clone();
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(move || {
        let _guard = poisoned.lock().expect("lock before poison");
        panic!("poison debug I/O snapshot");
    });
    std::panic::set_hook(original_hook);

    let response = handle_request_value(
        json!({
            "id": 13,
            "type": "debug.variables",
            "params": { "variables_reference": 4242 }
        }),
        &state,
        None,
    );
    assert!(!response.ok, "poisoned I/O snapshot must fail closed");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("I/O snapshot unavailable")),
        "unexpected poisoned-lock error: {:?}",
        response.error
    );
}
