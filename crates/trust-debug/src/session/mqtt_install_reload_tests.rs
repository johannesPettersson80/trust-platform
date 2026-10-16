use super::*;
use trust_runtime::memory::IoArea;
use trust_runtime::value::Duration;

fn mqtt_install_project_root(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "trust-debug-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create project root");
    dir
}

/// Spec 32 §5: the local trust-debug simulator resolves a symbolic output mapping before it builds the
/// MQTT driver, so the output binding is allocated at its deterministic image bit and carries the
/// program variable into the output image after a scan.
#[test]
fn mqtt_install_debug_reload_binds_symbolic_output_at_its_image_bit() {
    let project_root = mqtt_install_project_root("reload_mqtt_output_image");
    let src = project_root.join("src");
    std::fs::create_dir_all(&src).expect("create project src");
    let main_path = src.join("main.st");
    let config_path = src.join("config.st");
    std::fs::write(
        &main_path,
        r#"PROGRAM TrafficLight
VAR
    Green : BOOL := FALSE;
END_VAR
Green := TRUE;
END_PROGRAM
"#,
    )
    .expect("write program");
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
    .expect("write configuration");
    std::fs::write(
        project_root.join("io.toml"),
        r#"[io]
driver = "mqtt"

[io.params]
broker = "127.0.0.1:1883"
reconnect_ms = 500
keep_alive_s = 5
allow_insecure_remote = false
on_error = "warn"

[[io.params.mappings]]
tag = "MainInstance.Green"
topic = "traffic/north/green"
direction = "write"
"#,
    )
    .expect("write io.toml");

    let mut session = DebugSession::new(trust_runtime::RuntimeBuilder::new().build().unwrap());
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
    let mut runtime = handle.lock().unwrap();
    let snapshot = runtime.io().snapshot();
    assert_eq!(
        snapshot.outputs.len(),
        1,
        "the symbolic output mapping must allocate one output binding"
    );
    let entry = &snapshot.outputs[0];
    assert_eq!(entry.name.as_deref(), Some("MainInstance.Green"));
    assert!(
        matches!(entry.address.area, IoArea::Output),
        "the symbolic output must be bound in the output area"
    );
    assert_eq!(entry.address.byte, 0);
    assert_eq!(entry.address.bit, 0);
    assert_eq!(
        runtime.io().outputs(),
        &[0u8],
        "no scan has copied Green into the output image yet"
    );

    runtime.set_current_time(Duration::from_millis(100));
    runtime
        .execute_cycle()
        .expect("scan the reloaded program through the MQTT output mapping");
    assert_eq!(
        runtime.io().outputs(),
        &[1u8],
        "the scanned Green value must drive bit 0 of the output image"
    );
    drop(runtime);
    let _ = std::fs::remove_dir_all(project_root);
}
