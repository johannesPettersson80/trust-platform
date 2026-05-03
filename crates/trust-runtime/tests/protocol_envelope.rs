use trust_runtime::harness::HarnessAutomation;

#[test]
fn watch_snapshot_uses_per_entry_error_for_unknown_paths() {
    let mut automation = HarnessAutomation::new();
    automation
        .load_sources(&[r#"
PROGRAM Main
VAR
    drive : BOOL;
END_VAR
END_PROGRAM
"#
        .to_string()])
        .expect("load harness");

    let snapshot = automation
        .snapshot(&["drive".to_string(), "drrive".to_string()])
        .expect("snapshot");

    assert!(snapshot.values["drive"].is_ok());
    let error = snapshot.values["drrive"]
        .error
        .as_ref()
        .expect("missing watch entry should carry error");
    assert_eq!(error.code(), "unresolved_name");
}
