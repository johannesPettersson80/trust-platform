// Included into the tests of `commands.rs` (path_ranges_and_tests.rs), beside the other
// `trust-lsp.hmiBindings` tests.

/// Specification 39, §14.5 items 4 and 8: the messages the program gives its alarms are texts of
/// the project, computed by PLC execution remain unavailable during declaration discovery.
#[test]
fn hmi_authoring_does_not_execute_a_program_to_discover_alarm_messages() {
    let root = temp_dir("trustlsp-alarm-messages");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("src").join("alarm.st"),
        include_str!("../../../../../libraries/alarm/src/alarm.st"),
    )
    .expect("write the alarm library");
    std::fs::write(
        root.join("src").join("main.st"),
        "USING Alarm;\nPROGRAM Main\nVAR Level : LREAL; High : LimitAlarm; END_VAR\n    High(Value := Level, Limit := 90.0, Message := 'T-101 level high', Priority := AlarmPriority#Medium);\nEND_PROGRAM\n",
    )
    .expect("write source");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(
            root_uri.clone(),
            test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
        )],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    let result = hmi_bindings_value_with_context(&context, vec![json!({"authoring":true})]).expect("a response");
    assert_eq!(result["ok"], json!(true), "{result}");
    assert_eq!(result["alarms"],json!([]),"dynamic text is not evaluated in the editor");
    assert_eq!(result["inspectionMode"],"declarations");
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// Configured projects are inspected without advancing their first periodic scan.
#[test]
fn hmi_authoring_does_not_advance_a_configured_task_for_alarm_text() {
    let root = temp_dir("trustlsp-scheduled-alarm-messages");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/alarm.st"), include_str!("../../../../../libraries/alarm/src/alarm.st")).unwrap();
    std::fs::write(root.join("src/main.st"),
        "USING Alarm;\nPROGRAM Process\nVAR High : LimitAlarm; END_VAR\nHigh(Value := 42.0, Limit := 90.0, Message := 'Scheduled vessel high', Priority := AlarmPriority#Medium);\nEND_PROGRAM\nCONFIGURATION Config\nRESOURCE Cpu ON PLC\nTASK Cycle(INTERVAL := T#100ms, PRIORITY := 1);\nPROGRAM Main WITH Cycle : Process;\nEND_RESOURCE\nEND_CONFIGURATION\n").unwrap();
    let root_uri = Url::from_directory_path(&root).unwrap();
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri], ..MockContext::default()
    };
    let result = hmi_bindings_value_with_context(&context, vec![json!({"authoring":true})]).unwrap();
    assert_eq!(result["ok"], json!(true), "{result}");
    assert_eq!(result["alarms"],json!([]));
    assert_eq!(result["inspectionMode"],"declarations");
    std::fs::remove_dir_all(root).unwrap();
}
