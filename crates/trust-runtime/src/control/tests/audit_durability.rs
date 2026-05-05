#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 8"]
fn control_audit_send_failure_records_audit_dropped_event() {
    let mut state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    let (audit_tx, audit_rx) = std::sync::mpsc::channel();
    drop(audit_rx);
    state.audit_tx = Some(audit_tx);

    let response = handle_request_value(json!({ "id": 7701, "type": "status" }), &state, None);
    assert!(response.ok, "status request should still return");
    let encoded = serde_json::to_value(&response).expect("serialize response");
    assert!(
        encoded.get("audit_id").and_then(serde_json::Value::as_str).is_some(),
        "audit id should still be returned even when the audit sink is closed: {encoded}"
    );

    let events = state.events.lock().expect("events lock");
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("AuditDropped")),
        "closed audit sink must be observable as AuditDropped, got {events:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 8"]
fn debug_feature_disabled_returns_structured_feature_disabled_response() {
    let state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    state
        .debug_enabled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let response = handle_request_value(json!({ "id": 7702, "type": "debug.state" }), &state, None);
    assert!(!response.ok, "debug request should be rejected");

    let encoded = serde_json::to_value(&response).expect("serialize response");
    assert_eq!(
        encoded.get("error_code").and_then(serde_json::Value::as_str),
        Some("feature_disabled"),
        "debug-disabled response must expose a structured feature_disabled code: {encoded}"
    );

    let events = state.events.lock().expect("events lock");
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("FeatureDisabled")),
        "feature-disabled request must be observable as FeatureDisabled, got {events:?}"
    );
}
