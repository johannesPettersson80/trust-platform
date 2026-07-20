#[test]
fn auth_token_change_preserves_force_until_authorized_release() {
    let source = r#"
PROGRAM Main
VAR
    output_bit AT %QX0.0 : BOOL := FALSE;
END_VAR
output_bit := FALSE;
END_PROGRAM
"#;
    let mut state = hmi_test_state(source);
    state.auth_token = std::sync::Arc::new(std::sync::Mutex::new(Some(
        smol_str::SmolStr::new("old-admin-token"),
    )));
    state.control_requires_auth = true;

    let force = handle_request_value(
        serde_json::json!({
            "id": 1,
            "type": "io.force",
            "auth": "old-admin-token",
            "params": { "address": "%QX0.0", "value": "TRUE" }
        }),
        &state,
        None,
    );
    assert!(force.ok, "initial authorized force failed: {force:?}");

    let update = handle_request_value(
        serde_json::json!({
            "id": 2,
            "type": "config.set",
            "auth": "old-admin-token",
            "params": { "control.auth_token": "new-admin-token" }
        }),
        &state,
        None,
    );
    assert!(update.ok, "authorized token update failed: {update:?}");
    assert_eq!(
        state.debug.forced_snapshot().io.len(),
        1,
        "changing authorization must not silently clear an active force"
    );

    let stale_release = handle_request_value(
        serde_json::json!({
            "id": 3,
            "type": "io.unforce",
            "auth": "old-admin-token",
            "params": { "address": "%QX0.0" }
        }),
        &state,
        None,
    );
    assert!(!stale_release.ok, "the old token must stop authorizing commands");
    assert_eq!(
        state.debug.forced_snapshot().io.len(),
        1,
        "a denied command must not clear an existing force"
    );

    let release = handle_request_value(
        serde_json::json!({
            "id": 4,
            "type": "io.unforce",
            "auth": "new-admin-token",
            "params": { "address": "%QX0.0" }
        }),
        &state,
        None,
    );
    assert!(release.ok, "the new token should authorize release: {release:?}");
    assert!(
        state.debug.forced_snapshot().io.is_empty(),
        "authorized release must remove the force"
    );
}
