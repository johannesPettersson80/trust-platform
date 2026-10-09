#[test]
fn hmi_trends_and_alarm_contracts_support_ack_flow() {
    let source = r#"
PROGRAM Main
VAR
    // @hmi(min=0, max=100)
    speed : REAL := 120.0;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);

    let trends = handle_request_value(
        json!({
            "id": 10,
            "type": "hmi.trends.get",
            "params": { "duration_ms": 60_000, "buckets": 24 }
        }),
        &state,
        None,
    );
    assert!(trends.ok, "hmi.trends.get failed: {:?}", trends.error);
    let trend_series = trends
        .result
        .as_ref()
        .and_then(|value| value.get("series"))
        .and_then(serde_json::Value::as_array)
        .expect("trend series");
    assert!(!trend_series.is_empty(), "expected trend series");

    let alarms = handle_request_value(
        json!({
            "id": 11,
            "type": "hmi.alarms.get",
            "params": { "limit": 10 }
        }),
        &state,
        None,
    );
    assert!(alarms.ok, "hmi.alarms.get failed: {:?}", alarms.error);
    let active = alarms
        .result
        .as_ref()
        .and_then(|value| value.get("active"))
        .and_then(serde_json::Value::as_array)
        .expect("active alarms");
    assert_eq!(active.len(), 1, "expected one raised alarm");
    let alarm_id = active[0]
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("alarm id");

    let ack = handle_request_value(
        json!({
            "id": 12,
            "type": "hmi.alarm.ack",
            "params": { "id": alarm_id }
        }),
        &state,
        None,
    );
    assert!(ack.ok, "hmi.alarm.ack failed: {:?}", ack.error);
    let ack_active = ack
        .result
        .as_ref()
        .and_then(|value| value.get("active"))
        .and_then(serde_json::Value::as_array)
        .expect("ack active alarms");
    assert_eq!(ack_active.len(), 1);
    assert_eq!(
        ack_active[0]
            .get("state")
            .and_then(serde_json::Value::as_str),
        Some("acknowledged")
    );
}

#[test]
fn hmi_descriptor_watcher_updates_schema_without_runtime_restart() {
    let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-live-refresh");
    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed A"
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);
    let state = Arc::new(state);
    spawn_hmi_descriptor_watcher(state.clone());

    let (initial_revision, initial_label) = hmi_schema_revision_and_speed_label(state.as_ref());
    assert_eq!(initial_revision, 0);
    assert_eq!(initial_label, "Speed A");

    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed B"
"#,
    );

    let (revision, label) = wait_for_schema_revision(state.as_ref(), 1, Duration::from_secs(10));
    assert_eq!(revision, 1);
    assert_eq!(label, "Speed B");
    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_descriptor_watcher_retains_last_good_schema_on_invalid_toml() {
    let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-live-invalid");
    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed A"
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);
    let state = Arc::new(state);
    spawn_hmi_descriptor_watcher(state.clone());

    let (initial_revision, initial_label) = hmi_schema_revision_and_speed_label(state.as_ref());
    assert_eq!(initial_revision, 0);
    assert_eq!(initial_label, "Speed A");

    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Drive"
span = "wide"
"#,
    );

    let invalid_schema = wait_for_descriptor_error(state.as_ref(), Duration::from_secs(10));
    let (revision_after_invalid, label_after_invalid) =
        hmi_schema_revision_and_speed_label(state.as_ref());
    assert_eq!(revision_after_invalid, 0);
    assert_eq!(label_after_invalid, "Speed A");
    assert!(invalid_schema
        .get("descriptor_error")
        .and_then(serde_json::Value::as_str)
        .is_some());

    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed C"
"#,
    );

    let (revision_after_fix, label_after_fix) =
        wait_for_schema_revision(state.as_ref(), 1, Duration::from_secs(10));
    assert_eq!(revision_after_fix, 1);
    assert_eq!(label_after_fix, "Speed C");
    let fixed_schema = wait_for_descriptor_error_clear(state.as_ref(), Duration::from_secs(10));
    assert!(
        fixed_schema.get("descriptor_error").is_none(),
        "descriptor_error should clear after descriptor recovers"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_descriptor_watcher_handles_rapid_file_changes_without_deadlock() {
    let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-live-rapid");
    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed A"
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);
    let state = Arc::new(state);
    spawn_hmi_descriptor_watcher(state.clone());

    let (initial_revision, initial_label) = hmi_schema_revision_and_speed_label(state.as_ref());
    assert_eq!(initial_revision, 0);
    assert_eq!(initial_label, "Speed A");

    for index in 0..24_u32 {
        if index % 5 == 0 {
            write_file(
                &root.join("hmi/overview.toml"),
                r#"
title = "Overview"

[[section]]
title = "Drive"
span = "wide"
"#,
            );
        } else {
            write_file(
                &root.join("hmi/overview.toml"),
                format!(
                    r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed {index}"
"#
                )
                .as_str(),
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed Final"
"#,
    );

    // Intermediate valid/fallback descriptors may already have advanced revision 1.
    // Wait for the final requested content, not merely any earlier watcher activity.
    let deadline = Instant::now() + Duration::from_secs(10);
    let (revision_after_churn, label_after_churn) = loop {
        let current = hmi_schema_revision_and_speed_label(state.as_ref());
        if current.0 >= 1 && current.1 == "Speed Final" {
            break current;
        }
        assert!(
            Instant::now() < deadline,
            "final descriptor not observed: {current:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(revision_after_churn >= 1);
    assert_eq!(label_after_churn, "Speed Final");

    for id in 0..40_u64 {
        let response = handle_request_value(
            json!({"id": 9_000_u64 + id, "type": "hmi.schema.get"}),
            &state,
            None,
        );
        assert!(
            response.ok,
            "schema request failed during churn: {:?}",
            response.error
        );
    }

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_descriptor_get_returns_inferred_layout_when_files_missing() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let response = handle_request_value(
        json!({"id": 700, "type": "hmi.descriptor.get"}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "hmi.descriptor.get failed: {:?}",
        response.error
    );
    let pages = response
        .result
        .as_ref()
        .and_then(|value| value.get("pages"))
        .and_then(serde_json::Value::as_array)
        .expect("descriptor pages");
    assert!(
        !pages.is_empty(),
        "inferred descriptor should include at least one page"
    );
}
