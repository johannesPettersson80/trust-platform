#[test]
fn session_reload_resolves_mqtt_symbolic_output_mappings() {
    let project_root = temp_project_root("reload_mqtt_mapping");
    let src = project_root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let main_path = src.join("main.st");
    let config_path = src.join("config.st");
    std::fs::write(
        &main_path,
        r#"PROGRAM TrafficLight
VAR
    Green : BOOL := FALSE;
END_VAR
END_PROGRAM
"#,
    )
    .unwrap();
    std::fs::write(
        &config_path,
        r#"CONFIGURATION Config
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : TrafficLight;
END_RESOURCE
END_CONFIGURATION
"#,
    )
    .unwrap();
    std::fs::write(
        project_root.join("io.toml"),
        r#"[io]
driver = "mqtt"

[io.params]
broker = "127.0.0.1:1883"
reconnect_ms = 500
keep_alive_s = 5
allow_insecure_remote = false

[[io.params.mappings]]
tag = "MainInstance.Green"
topic = "traffic/north/green"
direction = "write"
"#,
    )
    .unwrap();

    let mut session = DebugSession::new(Runtime::new());
    session.update_source_options(SourceOptionsUpdate {
        root: Some(project_root.to_string_lossy().to_string()),
        include_globs: None,
        exclude_globs: None,
        ignore_pragmas: None,
    });
    session
        .reload_program(Some(config_path.to_string_lossy().as_ref()))
        .unwrap();

    let handle = session.runtime_handle();
    let runtime = handle.lock().unwrap();
    let snapshot = runtime.io().snapshot();
    assert_eq!(
        snapshot.outputs.len(),
        1,
        "debug-session MQTT mappings must allocate their symbolic output binding"
    );
    assert_eq!(
        snapshot.outputs[0].name.as_deref(),
        Some("MainInstance.Green"),
        "debug-session MQTT binding must retain the resolved symbolic tag name"
    );
}
