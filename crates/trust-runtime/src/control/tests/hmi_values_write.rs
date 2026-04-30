#[test]
fn hmi_schema_contract_includes_required_mapping() {
    let source = r#"
TYPE MODE : (OFF, AUTO); END_TYPE
TYPE POINT :
STRUCT
    X : INT;
    Y : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    run : BOOL := TRUE;
    speed : REAL := 42.5;
    mode : MODE := MODE#AUTO;
    name : STRING := 'pump';
    nums : ARRAY[1..3] OF INT;
    point : POINT;
END_VAR
nums[1] := 1;
nums[2] := 2;
nums[3] := 3;
point.X := 11;
point.Y := 12;
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let response = handle_request_value(json!({"id": 1, "type": "hmi.schema.get"}), &state, None);
    assert!(
        response.ok,
        "schema response should be ok: {:?}",
        response.error
    );
    let result = response.result.expect("schema result");
    assert_eq!(
        result.get("mode").and_then(serde_json::Value::as_str),
        Some("read_only")
    );
    assert_eq!(
        result
            .get("schema_revision")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        result.get("read_only").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        result
            .get("theme")
            .and_then(|theme| theme.get("style"))
            .and_then(serde_json::Value::as_str),
        Some("classic")
    );
    assert!(result
        .get("pages")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|pages| !pages.is_empty()));

    let widgets = result
        .get("widgets")
        .and_then(serde_json::Value::as_array)
        .expect("widgets array");
    let mut by_path = IndexMap::new();
    for widget in widgets {
        let path = widget
            .get("path")
            .and_then(serde_json::Value::as_str)
            .expect("widget path");
        let kind = widget
            .get("widget")
            .and_then(serde_json::Value::as_str)
            .expect("widget kind");
        by_path.insert(path.to_string(), kind.to_string());
    }

    assert_eq!(
        by_path.get("Main.run").map(String::as_str),
        Some("indicator")
    );
    assert_eq!(by_path.get("Main.speed").map(String::as_str), Some("value"));
    assert_eq!(
        by_path.get("Main.mode").map(String::as_str),
        Some("readout")
    );
    assert_eq!(by_path.get("Main.name").map(String::as_str), Some("text"));
    assert_eq!(by_path.get("Main.nums").map(String::as_str), Some("table"));
    assert_eq!(by_path.get("Main.point").map(String::as_str), Some("tree"));
    let run_widget = widgets
        .iter()
        .find(|widget| {
            widget
                .get("path")
                .and_then(serde_json::Value::as_str)
                .map(|path| path == "Main.run")
                .unwrap_or(false)
        })
        .expect("run widget");
    assert_eq!(
        run_widget.get("id").and_then(serde_json::Value::as_str),
        Some("resource/RESOURCE/program/Main/field/run")
    );
}

#[test]
fn hmi_values_contract_returns_timestamp_quality_and_typed_values() {
    let source = r#"
TYPE POINT :
STRUCT
    X : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    run : BOOL := TRUE;
    speed : REAL := 42.5;
    name : STRING := 'pump';
    nums : ARRAY[1..3] OF INT;
    point : POINT;
END_VAR
nums[1] := 1;
nums[2] := 2;
nums[3] := 3;
point.X := 11;
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let ids = vec![
        "resource/RESOURCE/program/Main/field/run",
        "resource/RESOURCE/program/Main/field/speed",
        "resource/RESOURCE/program/Main/field/name",
        "resource/RESOURCE/program/Main/field/nums",
        "resource/RESOURCE/program/Main/field/point",
    ];
    let response = handle_request_value(
        json!({
            "id": 2,
            "type": "hmi.values.get",
            "params": { "ids": ids }
        }),
        &state,
        None,
    );
    assert!(
        response.ok,
        "values response should be ok: {:?}",
        response.error
    );
    let result = response.result.expect("values result");
    assert_eq!(
        result.get("connected").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(result
        .get("timestamp_ms")
        .and_then(serde_json::Value::as_u64)
        .is_some());

    let values = result
        .get("values")
        .and_then(serde_json::Value::as_object)
        .expect("values object");
    let run = values
        .get("resource/RESOURCE/program/Main/field/run")
        .expect("run value");
    assert_eq!(
        run.get("q").and_then(serde_json::Value::as_str),
        Some("good")
    );
    assert_eq!(
        run.get("v").and_then(serde_json::Value::as_bool),
        Some(true)
    );

    let speed = values
        .get("resource/RESOURCE/program/Main/field/speed")
        .expect("speed value");
    assert!(speed.get("v").and_then(serde_json::Value::as_f64).is_some());

    let name = values
        .get("resource/RESOURCE/program/Main/field/name")
        .expect("name value");
    assert_eq!(
        name.get("v").and_then(serde_json::Value::as_str),
        Some("pump")
    );

    let nums = values
        .get("resource/RESOURCE/program/Main/field/nums")
        .expect("nums value");
    assert_eq!(
        nums.get("v")
            .and_then(serde_json::Value::as_array)
            .map(|values| values.len()),
        Some(3)
    );

    let point = values
        .get("resource/RESOURCE/program/Main/field/point")
        .expect("point value");
    assert!(point
        .get("v")
        .and_then(serde_json::Value::as_object)
        .is_some());
}

#[test]
fn hmi_runtime_read_port_is_code_backed_without_json_transport() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let ids = vec!["resource/RESOURCE/program/Main/field/run".to_string()];
    let read = hmi_handlers::read_hmi_values_port(&state, Some(ids.as_slice()))
        .expect("read hmi values through control port");

    assert_eq!(read.schema.schema_revision, 0);
    assert!(read.values.connected);
    assert_eq!(
        read.values
            .values
            .get("resource/RESOURCE/program/Main/field/run")
            .map(|record| &record.v),
        Some(&json!(true))
    );
}

#[test]
fn hmi_write_is_disabled_in_read_only_mode() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let state = hmi_test_state(source);
    let response = handle_request_value(
        json!({
            "id": 3,
            "type": "hmi.write",
            "params": { "id": "resource/RESOURCE/program/Main/field/run", "value": false }
        }),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some("hmi.write disabled in read-only mode")
    );
}

#[test]
fn hmi_runtime_write_port_queues_allowlisted_write_without_json_transport() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-port-write-program");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = ["Main.run"]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);

    let queued = hmi_handlers::queue_hmi_runtime_write_port(&state, "Main.run", &json!(false))
        .expect("queue hmi write through control port");
    assert_eq!(queued.path, "Main.run");

    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::Bool(false));

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_write_queues_allowlisted_program_variable_write() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-write-program");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/run"]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);

    let response = handle_request_value(
        json!({
            "id": 4,
            "type": "hmi.write",
            "params": {
                "id": "resource/RESOURCE/program/Main/field/run",
                "value": false
            }
        }),
        &state,
        None,
    );
    assert!(response.ok, "hmi.write failed: {:?}", response.error);
    let result = response.result.expect("hmi.write result");
    assert_eq!(
        result.get("status").and_then(serde_json::Value::as_str),
        Some("queued")
    );
    assert_eq!(
        result.get("id").and_then(serde_json::Value::as_str),
        Some("resource/RESOURCE/program/Main/field/run")
    );

    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::Bool(false));
    match &writes[0].target {
        PendingVarTarget::Instance(_, name) => assert_eq!(name.as_str(), "run"),
        other => panic!("expected instance write, got {other:?}"),
    }

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_write_supports_path_allowlist_and_alias_param() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-write-path");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = ["Main.run"]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);

    let response = handle_request_value(
        json!({
            "id": 5,
            "type": "hmi.write",
            "params": {
                "path": "Main.run",
                "value": "FALSE"
            }
        }),
        &state,
        None,
    );
    assert!(response.ok, "hmi.write failed: {:?}", response.error);
    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::Bool(false));

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_write_rejects_non_allowlisted_target() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-write-denied");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/other"]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);
    let response = handle_request_value(
        json!({
            "id": 6,
            "type": "hmi.write",
            "params": {
                "id": "resource/RESOURCE/program/Main/field/run",
                "value": true
            }
        }),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some("hmi.write target is not in allowlist")
    );
    assert!(state.debug.drain_var_writes().is_empty());
    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_write_rejects_type_mismatch() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-write-type");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/run"]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);
    let response = handle_request_value(
        json!({
            "id": 7,
            "type": "hmi.write",
            "params": {
                "id": "resource/RESOURCE/program/Main/field/run",
                "value": 1
            }
        }),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some("invalid hmi.write value for target 'resource/RESOURCE/program/Main/field/run'")
    );
    assert!(state.debug.drain_var_writes().is_empty());
    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_write_processing_stays_under_cycle_budget() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-write-budget");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/run"]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);

    let writes: u32 = 300;
    let mut max = Duration::ZERO;
    let mut total = Duration::ZERO;
    for index in 0..writes {
        let started = Instant::now();
        let response = handle_request_value(
            json!({
                "id": 70_u64 + u64::from(index),
                "type": "hmi.write",
                "params": {
                    "id": "resource/RESOURCE/program/Main/field/run",
                    "value": index % 2 == 0
                }
            }),
            &state,
            None,
        );
        let elapsed = started.elapsed();
        assert!(response.ok, "hmi.write failed: {:?}", response.error);
        max = max.max(elapsed);
        total += elapsed;
    }

    let avg = total / writes;
    assert!(
        max < Duration::from_millis(100),
        "max hmi.write latency {:?} exceeded write cycle budget",
        max
    );
    assert!(
        avg < Duration::from_millis(25),
        "avg hmi.write latency {:?} exceeded expected write overhead",
        avg
    );

    let drained = state.debug.drain_var_writes();
    assert!(
        !drained.is_empty(),
        "expected queued writes after budget benchmark loop"
    );
    fs::remove_dir_all(root).ok();
}
